import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { Strategy, DictEntry, ReplaceStyle } from "../types";

/** 默认策略配置 */
const DEFAULT_STRATEGIES: Record<string, Strategy> = {
  Phone:         { Mask: { keep_prefix: 3, keep_suffix: 4 } },
  IdCard:        { Mask: { keep_prefix: 4, keep_suffix: 4 } },
  BankCard:      { Mask: { keep_prefix: 4, keep_suffix: 4 } },
  Email:         { Mask: { keep_prefix: 1, keep_suffix: 0 } },
  IpAddress:     { Mask: { keep_prefix: 8, keep_suffix: 0 } },
  LandlinePhone: { Mask: { keep_prefix: 4, keep_suffix: 4 } },
  LicensePlate:  { Mask: { keep_prefix: 2, keep_suffix: 2 } },
  CreditCode:    { Mask: { keep_prefix: 4, keep_suffix: 3 } },
  PersonName:    { Replace: { style: "Fake" } },
  OrgName:       { Replace: { style: "Fake" } },
  Address:       { Replace: { style: "Fake" } },
  Title:         "Generalize",
  Custom:        { Mask: { keep_prefix: 1, keep_suffix: 1 } },
};

interface ConfigState {
  /** 各类型默认策略 */
  strategies: Record<string, Strategy>;
  /** 全局替换风格 */
  replaceStyle: ReplaceStyle;
  /** 自定义词典条目 */
  dictEntries: DictEntry[];
  /** 是否已加载 */
  loaded: boolean;

  // --- 策略操作 ---
  loadConfig: () => Promise<void>;
  saveConfig: () => Promise<void>;
  updateStrategy: (typeKey: string, strategy: Strategy) => void;
  updateReplaceStyle: (style: ReplaceStyle) => void;
  resetToDefault: () => void;

  // --- 词典操作 ---
  loadDict: () => Promise<void>;
  saveDict: () => Promise<void>;
  addDictEntry: (entry: DictEntry) => void;
  updateDictEntry: (index: number, entry: DictEntry) => void;
  removeDictEntry: (index: number) => void;
}

export const useConfigStore = create<ConfigState>((set, get) => ({
  strategies: { ...DEFAULT_STRATEGIES },
  replaceStyle: "Fake" as ReplaceStyle,
  dictEntries: [],
  loaded: false,

  loadConfig: async () => {
    try {
      const result = await invoke<{ strategies: Record<string, Strategy>; replace_style?: ReplaceStyle }>("load_config");
      // 将已保存的策略合并到默认值上（保留未保存类型的默认值）
      if (result.strategies && Object.keys(result.strategies).length > 0) {
        set({
          strategies: { ...DEFAULT_STRATEGIES, ...result.strategies },
          replaceStyle: result.replace_style || "Fake",
          loaded: true,
        });
      } else {
        set({ loaded: true });
      }
    } catch {
      // 首次使用或文件不存在，用默认值
      set({ loaded: true });
    }
  },

  saveConfig: async () => {
    const { strategies, replaceStyle } = get();
    try {
      await invoke("save_config", { config: { strategies, replace_style: replaceStyle } });
    } catch (e) {
      console.error("保存配置失败:", e);
    }
  },

  updateStrategy: (typeKey, strategy) =>
    set((state) => ({
      strategies: { ...state.strategies, [typeKey]: strategy },
    })),

  updateReplaceStyle: (style: ReplaceStyle) => {
    set((state) => {
      const newStrategies = { ...state.strategies };
      for (const key in newStrategies) {
        const s = newStrategies[key];
        if (typeof s === "object" && "Replace" in s) {
          newStrategies[key] = { Replace: { style } };
        }
      }
      return { replaceStyle: style, strategies: newStrategies };
    });
    get().saveConfig();
  },

  resetToDefault: () => set({ strategies: { ...DEFAULT_STRATEGIES }, replaceStyle: "Fake" }),

  loadDict: async () => {
    try {
      const entries = await invoke<DictEntry[]>("load_dict");
      set({ dictEntries: entries });
    } catch {
      set({ dictEntries: [] });
    }
  },

  saveDict: async () => {
    const { dictEntries } = get();
    try {
      await invoke("save_dict", { entries: dictEntries });
    } catch (e) {
      console.error("保存词典失败:", e);
    }
  },

  addDictEntry: (entry) =>
    set((state) => ({ dictEntries: [...state.dictEntries, entry] })),

  updateDictEntry: (index, entry) =>
    set((state) => ({
      dictEntries: state.dictEntries.map((e, i) => (i === index ? entry : e)),
    })),

  removeDictEntry: (index) =>
    set((state) => ({
      dictEntries: state.dictEntries.filter((_, i) => i !== index),
    })),
}));
