// src/components/LicenseStatusCard/index.tsx
//
// 左栏底部 license 状态卡 — 把激活/试用/过期/宽限/撤销等状态露出到 UI 外层。
// 解决"用户不知道左下角图标可以点"的发现性问题。

import { useEffect, useRef, useState } from "react";
import {
  AlertTriangle,
  Clock,
  HelpCircle,
  Info,
  KeyRound,
  Mail,
  MoreVertical,
  ShieldAlert,
  ShieldCheck,
  Smartphone,
  Wifi,
} from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useTranslation } from "react-i18next";
import toast from "react-hot-toast";
import { useLicenseStore, type DeviceInfo } from "../../stores/licenseStore";
import { ActivationDialog } from "../license/ActivationDialog";
import { DeviceListDialog } from "../license/DeviceListDialog";
import { RecoverDialog } from "../license/RecoverDialog";
import { AboutModal } from "../AboutModal";

const FEEDBACK_EMAIL = "support@dimkey.com";
const MENU_ID = "license-status-card-menu";

/** 从 Tauri invoke reject 的 payload 里提取人类可读消息。
 *  LicenseError 序列化为 {code, data?}；其它后端可能 reject 字符串；fallback 走 String(). */
function extractErrMsg(e: unknown): string {
  if (e && typeof e === "object") {
    const obj = e as { code?: string; data?: { message?: string } };
    if (obj.data?.message) return obj.data.message;
    if (obj.code) return obj.code;
  }
  if (typeof e === "string") return e;
  return String(e);
}

type Tone = "neutral" | "success" | "warn" | "danger";

interface CardVisual {
  Icon: typeof Clock;
  tone: Tone;
  title: string;
  cta?: string;
}

const TONE_CLASS: Record<Tone, { wrap: string; icon: string; title: string; cta: string }> = {
  neutral: {
    wrap: "bg-slate-50 hover:bg-slate-100 border-slate-200",
    icon: "text-slate-400",
    title: "text-slate-600",
    cta: "text-slate-500",
  },
  success: {
    wrap: "bg-white hover:bg-slate-50 border-slate-200",
    icon: "text-emerald-500",
    title: "text-slate-700",
    cta: "text-slate-400",
  },
  warn: {
    wrap: "bg-amber-50 hover:bg-amber-100 border-amber-200",
    icon: "text-amber-600",
    title: "text-amber-900",
    cta: "text-amber-700 font-medium",
  },
  danger: {
    wrap: "bg-red-50 hover:bg-red-100 border-red-200",
    icon: "text-red-600",
    title: "text-red-900",
    cta: "text-red-700 font-medium",
  },
};

export function LicenseStatusCard() {
  const { t } = useTranslation();
  const state = useLicenseStore((s) => s.state);
  const initialized = useLicenseStore((s) => s.initialized);
  const fpMismatchHint = useLicenseStore((s) => s.fingerprintMismatchHint);
  const deactivateCurrent = useLicenseStore((s) => s.deactivateCurrent);
  const forceHeartbeat = useLicenseStore((s) => s.forceHeartbeat);

  const [menuOpen, setMenuOpen] = useState(false);
  const [actOpen, setActOpen] = useState(false);
  const [devOpen, setDevOpen] = useState(false);
  const [recOpen, setRecOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [reconnecting, setReconnecting] = useState(false);
  const [devicesPreload, setDevicesPreload] = useState<DeviceInfo[] | undefined>(undefined);
  const [maxPreload, setMaxPreload] = useState<number | undefined>(undefined);

  const menuRef = useRef<HTMLDivElement>(null);
  const cardRef = useRef<HTMLDivElement>(null);

  // 点击外部关闭菜单
  useEffect(() => {
    if (!menuOpen) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as Node;
      if (!menuRef.current?.contains(target) && !cardRef.current?.contains(target)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [menuOpen]);

  // Escape 关菜单（键盘可达性）
  useEffect(() => {
    if (!menuOpen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        setMenuOpen(false);
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [menuOpen]);

  // Activated 态下的指纹漂移 — 单独标记，仅用于在 Activated 视觉上叠加 warn 副标
  // （但不抢占 Revoked / TrialExpired 等更严重的状态判定）
  const activatedFpDrift =
    state.kind === "Activated" && state.fingerprint_mismatch;
  // 未激活态的本地 .lic 指纹冲突：boot 会回退到 Trial/TrialExpired，
  // 但左下角主卡片仍应优先提示用户重新激活本机。
  const orphanFpDrift = !!fpMismatchHint && state.kind !== "Activated";

  const visual: CardVisual = (() => {
    if (!initialized) {
      return { Icon: HelpCircle, tone: "neutral", title: t("license.statusCard.checking") };
    }
    // 优先级：Revoked > orphanFpDrift > TrialExpired > 其它正常态
    // —— revoked 不可被指纹漂移这种 warn 级提示掩盖；试用态则应让位给本地证书冲突。
    switch (state.kind) {
      case "Revoked":
        return {
          Icon: ShieldAlert,
          tone: "danger",
          title: t("license.statusCard.revoked"),
          cta: t("license.statusCard.contactSupport"),
        };
      case "Trial":
      case "TrialExpired":
      case "Unknown":
        if (orphanFpDrift) {
          return {
            Icon: ShieldAlert,
            tone: "warn",
            title: t("license.statusCard.fpDrift"),
            cta: fpMismatchHint
              ? t("license.statusCard.fpDriftDetail", { fp: fpMismatchHint })
              : t("license.statusCard.viewDetail"),
          };
        }
        if (state.kind === "Trial") {
          const days = Math.max(0, state.days_remaining);
          return {
            Icon: Clock,
            tone: days <= 3 ? "warn" : "neutral",
            title: t("license.statusCard.trial"),
            cta: t("license.statusCard.trialDaysLeftCta", { days }),
          };
        }
        if (state.kind === "TrialExpired") {
          return {
            Icon: AlertTriangle,
            tone: "danger",
            title: t("license.statusCard.trialExpired"),
            cta: t("license.statusCard.activateNow"),
          };
        }
        return { Icon: HelpCircle, tone: "neutral", title: t("license.statusCard.checking") };
      case "Activated":
        // Activated + 指纹漂移：变 warn + cta 显示 fp 短码，但不变 danger
        if (activatedFpDrift) {
          return {
            Icon: ShieldAlert,
            tone: "warn",
            title: t("license.statusCard.fpDrift"),
            cta: fpMismatchHint
              ? t("license.statusCard.fpDriftDetail", { fp: fpMismatchHint })
              : t("license.statusCard.viewDetail"),
          };
        }
        return {
          Icon: ShieldCheck,
          tone: "success",
          title: t("license.statusCard.activated"),
          cta: state.email || undefined,
        };
      case "GraceMode":
        return {
          Icon: Wifi,
          tone: "warn",
          title: t("license.statusCard.graceMode"),
          cta: t("license.statusCard.graceDaysLeft", { days: state.days_until_block }),
        };
      default:
        return { Icon: HelpCircle, tone: "neutral", title: t("license.statusCard.checking") };
    }
  })();

  const tone = TONE_CLASS[visual.tone];

  // 真正触发 force heartbeat — 用 toast.promise 把 pending/success/error 串起来
  // 复用于 GraceMode 主点击、菜单"立即联网恢复"、Activated+fpDrift "重新验证"
  const handleReconnect = () => {
    setMenuOpen(false);
    if (reconnecting) return;
    setReconnecting(true);
    toast
      .promise(forceHeartbeat(), {
        loading: t("license.statusCard.reconnecting"),
        success: t("license.statusCard.reconnectOk"),
        error: (e: unknown) =>
          t("license.statusCard.reconnectFailed", { msg: extractErrMsg(e) }),
      })
      .finally(() => setReconnecting(false));
  };

  // 主点击：根据状态走不同入口
  const handlePrimaryAction = () => {
    // 未初始化态不响应点击 — 视觉是 "正在检查授权…"，承诺无操作
    if (!initialized) return;
    setMenuOpen(false);
    // Activated + fpDrift：触发真实联网验证（让后端重新确认本机指纹是否匹配）
    if (activatedFpDrift) {
      handleReconnect();
      return;
    }
    switch (state.kind) {
      case "Trial":
      case "TrialExpired":
        setActOpen(true);
        break;
      case "Unknown":
        // Unknown + 有孤儿 .lic：进激活；否则也开激活（兜底）
        setActOpen(true);
        break;
      case "GraceMode":
        handleReconnect();
        break;
      case "Revoked":
        void openUrl(
          `mailto:${FEEDBACK_EMAIL}?subject=${encodeURIComponent(t("license.statusCard.revokedSubject"))}`,
        );
        break;
      case "Activated":
        // 已激活：默认开菜单（与点 ⋮ 一致）
        setMenuOpen(true);
        break;
    }
  };

  const handleFeedback = async () => {
    setMenuOpen(false);
    try {
      await openUrl(
        `mailto:${FEEDBACK_EMAIL}?subject=${encodeURIComponent(t("license.statusCard.feedbackSubject"))}`,
      );
    } catch {
      toast.error(t("license.statusCard.mailFailed"));
    }
  };

  const handleAbout = () => {
    setMenuOpen(false);
    setAboutOpen(true);
  };

  const handleActivate = () => {
    setMenuOpen(false);
    setActOpen(true);
  };

  const handleDevices = () => {
    setMenuOpen(false);
    setDevicesPreload(undefined);
    setMaxPreload(undefined);
    setDevOpen(true);
  };

  const handleDeactivate = async () => {
    setMenuOpen(false);
    if (!window.confirm(t("license.statusCard.deactivateConfirm"))) return;
    try {
      await deactivateCurrent();
      toast.success(t("license.statusCard.deactivated"));
    } catch (e) {
      // LicenseError 序列化为 {code, data} 对象 — 必须按 code 取本地化文案，
      // 不能 String(e) 否则得到 "[object Object]"
      const code = (e as { code?: string })?.code;
      const errKey = `license.statusCard.deactivateError.${code}`;
      // 如果 i18n 里没这个 code，i18next 返回 key 本身 → 走 fallback 文案
      const msg = code && t(errKey) !== errKey
        ? t(errKey)
        : t("license.statusCard.deactivateError.fallback", { msg: extractErrMsg(e) });
      toast.error(msg);
    }
  };

  const { Icon } = visual;

  return (
    <>
      <div className="px-3 pt-2 pb-1 shrink-0 relative">
        {/* 卡片 = 两个并排真实 button，避免 role=button 嵌套真 button 的 ARIA 违规。
            外层 div 只做布局，hover 视觉用 group/group-hover 同步两个 button。 */}
        <div
          ref={cardRef}
          className={`group flex items-center gap-1 rounded-lg border transition-colors ${tone.wrap}`}
        >
          <button
            type="button"
            onClick={handlePrimaryAction}
            disabled={!initialized}
            className={`flex-1 min-w-0 flex items-center gap-2 px-2.5 py-1.5 rounded-l-lg text-left disabled:cursor-default ${
              initialized ? "cursor-pointer" : ""
            }`}
          >
            <Icon aria-hidden="true" className={`w-4 h-4 shrink-0 ${tone.icon}`} />
            <div className="flex-1 min-w-0">
              <div className={`text-xs leading-tight truncate ${tone.title}`}>{visual.title}</div>
              {visual.cta && (
                <div className={`text-[11px] leading-tight truncate ${tone.cta}`}>
                  {visual.cta}
                </div>
              )}
            </div>
          </button>
          <button
            type="button"
            onClick={() => setMenuOpen((v) => !v)}
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            aria-controls={MENU_ID}
            aria-label={t("license.statusCard.menu.title")}
            title={t("license.statusCard.menu.title")}
            className="p-1.5 mr-1 rounded hover:bg-black/5 text-slate-400 hover:text-slate-600"
          >
            <MoreVertical aria-hidden="true" className="w-3.5 h-3.5" />
          </button>
        </div>

        {menuOpen && (() => {
          // 首项焦点 key — 保证多条件渲染时只有一个 MenuItem 拿到焦点
          // 优先级：reverify (Activated+fpDrift/Grace) > activate (未激活/孤儿等) > devices (Activated)
          const firstKey: "reverify" | "activate" | "devices" | null =
            activatedFpDrift || state.kind === "GraceMode"
            ? "reverify"
            : state.kind === "Trial" ||
                state.kind === "TrialExpired" ||
                state.kind === "Revoked" ||
                orphanFpDrift ||
                (state.kind === "Unknown" && initialized)
              ? "activate"
              : state.kind === "Activated"
                ? "devices"
                : null;
          return (
          <div
            ref={menuRef}
            id={MENU_ID}
            role="menu"
            aria-label={t("license.statusCard.menu.title")}
            className="absolute left-3 right-3 bottom-full mb-1 bg-white border border-slate-200 rounded-lg shadow-lg py-1 z-30"
          >
            {/* 未激活 / 试用 / 过期 / 已撤销 / Unknown+孤儿lic：激活入口 */}
            {/* —— 关键：Activated 即使有 fpDrift 也不显示"激活"（它已激活，应"重新验证"） */}
            {(state.kind === "Trial" ||
              state.kind === "TrialExpired" ||
              state.kind === "Revoked" ||
              orphanFpDrift ||
              (state.kind === "Unknown" && initialized)) && (
              <MenuItem
                icon={KeyRound}
                label={t("license.statusCard.menu.activate")}
                onClick={handleActivate}
                autoFocus={firstKey === "activate"}
              />
            )}

            {/* Activated + 指纹漂移：重新验证此设备（联网） */}
            {activatedFpDrift && (
              <MenuItem
                icon={Wifi}
                label={t("license.statusCard.menu.reverify")}
                onClick={handleReconnect}
                autoFocus={firstKey === "reverify"}
              />
            )}

            {/* 已激活：设备管理。GraceMode 下设备列表需要联网，避免离线时显示误导性空列表。 */}
            {state.kind === "Activated" && (
              <MenuItem
                icon={Smartphone}
                label={t("license.statusCard.menu.devices")}
                onClick={handleDevices}
                autoFocus={firstKey === "devices"}
              />
            )}

            {/* 宽限期：联网恢复 */}
            {state.kind === "GraceMode" && (
              <MenuItem
                icon={Wifi}
                label={t("license.statusCard.menu.recoverOnline")}
                onClick={handleReconnect}
                autoFocus={firstKey === "reverify"}
              />
            )}

            <MenuItem icon={Mail} label={t("license.statusCard.menu.feedback")} onClick={handleFeedback} />
            <MenuItem icon={Info} label={t("license.statusCard.menu.about")} onClick={handleAbout} />

            {state.kind === "Activated" && (
              <>
                <div className="my-1 border-t border-slate-100" role="separator" />
                <MenuItem
                  icon={AlertTriangle}
                  label={t("license.statusCard.menu.deactivate")}
                  onClick={() => void handleDeactivate()}
                  danger
                />
              </>
            )}
          </div>
          );
        })()}
      </div>

      <ActivationDialog
        visible={actOpen}
        onClose={() => setActOpen(false)}
        onShowDevices={(devs, m) => {
          setDevicesPreload(devs);
          setMaxPreload(m);
          setActOpen(false);
          setDevOpen(true);
        }}
        onShowRecover={() => {
          setActOpen(false);
          setRecOpen(true);
        }}
      />
      <DeviceListDialog
        visible={devOpen}
        onClose={() => setDevOpen(false)}
        initialDevices={devicesPreload}
        initialMax={maxPreload}
      />
      <RecoverDialog visible={recOpen} onClose={() => setRecOpen(false)} />
      <AboutModal visible={aboutOpen} onClose={() => setAboutOpen(false)} />
    </>
  );
}

interface MenuItemProps {
  icon: typeof Clock;
  label: string;
  onClick: () => void;
  danger?: boolean;
  /** 菜单打开时第一项自动获得焦点（键盘可达性） */
  autoFocus?: boolean;
}

function MenuItem({ icon: Icon, label, onClick, danger, autoFocus }: MenuItemProps) {
  const ref = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (autoFocus) ref.current?.focus();
  }, [autoFocus]);
  return (
    <button
      ref={ref}
      type="button"
      role="menuitem"
      onClick={onClick}
      className={`w-full flex items-center gap-2 px-3 py-1.5 text-xs text-left transition-colors focus:outline-none focus:bg-slate-100 ${
        danger
          ? "text-red-600 hover:bg-red-50 focus:bg-red-50"
          : "text-slate-700 hover:bg-slate-50"
      }`}
    >
      <Icon aria-hidden="true" className="w-3.5 h-3.5 shrink-0" />
      <span className="flex-1 truncate">{label}</span>
    </button>
  );
}
