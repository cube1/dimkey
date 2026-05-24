// src/stores/licenseStore.ts
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export type LicenseState =
  | { kind: "Trial"; days_remaining: number }
  | { kind: "TrialExpired" }
  | {
      kind: "Activated";
      email: string;
      plan: string;
      max_devices: number;
      active_devices: number;
      device_id: string;
      license_id: string;
      fingerprint_mismatch: boolean;
    }
  | { kind: "GraceMode"; email: string; days_until_block: number }
  | { kind: "Revoked"; reason: string }
  | { kind: "Unknown" };

export interface DeviceInfo {
  device_id: string;
  machine_label: string | null;
  os: string;
  flavor: string;
  first_activated: number;
  last_seen: number;
  is_current: boolean;
}

export interface LicenseError {
  code: string;
  data?: unknown;
}

interface LicenseStoreState {
  state: LicenseState;
  fingerprint: string;
  fingerprintMismatchHint: string | null;
  initialized: boolean;
  init: () => Promise<UnlistenFn | undefined>;
  refresh: () => Promise<void>;
  activate: (license_key: string, email: string) => Promise<void>;
  deactivateCurrent: () => Promise<void>;
  listDevices: () => Promise<DeviceInfo[]>;
  deactivateDevice: (device_id: string) => Promise<void>;
  recover: (email: string) => Promise<void>;
  openPurchase: () => Promise<void>;
  /** 用户主动触发的强制 heartbeat — GraceMode "立即联网恢复" / Activated "重新验证"。
   * 错误（NETWORK_UNAVAILABLE / SERVER_ERROR 等）冒泡给调用方。 */
  forceHeartbeat: () => Promise<void>;
}

export const useLicenseStore = create<LicenseStoreState>((set, get) => ({
  state: { kind: "Unknown" },
  fingerprint: "",
  fingerprintMismatchHint: null,
  initialized: false,

  init: async () => {
    const [state, fp, hint] = await Promise.all([
      invoke<LicenseState>("license_get_state"),
      invoke<string>("license_get_fingerprint"),
      invoke<string | null>("license_get_fingerprint_mismatch_hint"),
    ]);
    set({
      state,
      fingerprint: fp,
      fingerprintMismatchHint: hint,
      initialized: true,
    });
    const un = await listen<LicenseState>("license:state-changed", (e) =>
      set({ state: e.payload }),
    );
    return un;
  },

  refresh: async () => {
    const [state, hint] = await Promise.all([
      invoke<LicenseState>("license_get_state"),
      invoke<string | null>("license_get_fingerprint_mismatch_hint"),
    ]);
    set({ state, fingerprintMismatchHint: hint });
  },

  activate: async (license_key, email) => {
    await invoke("license_activate", { licenseKey: license_key, email });
    await get().refresh();
  },

  deactivateCurrent: async () => {
    await invoke("license_deactivate_current");
    await get().refresh();
  },

  listDevices: async () => invoke<DeviceInfo[]>("license_list_devices"),

  deactivateDevice: async (device_id) => {
    await invoke("license_deactivate_device", { deviceId: device_id });
  },

  recover: async (email) => {
    await invoke("license_recover_email", { email });
  },

  openPurchase: async () => {
    await invoke("license_open_purchase_page");
  },

  forceHeartbeat: async () => {
    // 后端会在 heartbeat 成功且当前 GraceMode 时自行 boot 并 emit license:state-changed
    // 这里只补一次本地 refresh，覆盖事件竞速场景（监听器异步注册前后端已变状态）
    await invoke("license_force_heartbeat");
    await get().refresh();
  },
}));

// dev/E2E 调试用：暴露 store 到 window，便于测试驱动 refresh / 检查状态
// 生产构建中 import.meta.env.DEV 为 false，整段会被 tree-shake 掉
if (import.meta.env.DEV && typeof window !== "undefined") {
  (window as unknown as { __DIMKEY_LICENSE_STORE__: typeof useLicenseStore }).__DIMKEY_LICENSE_STORE__ =
    useLicenseStore;
}
