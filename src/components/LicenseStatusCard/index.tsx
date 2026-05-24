// src/components/LicenseStatusCard/index.tsx
//
// 左栏底部 license 状态卡 — 把激活/试用/过期/宽限/撤销等状态露出到 UI 外层。
// 解决"用户不知道左下角图标可以点"的发现性问题。

import { useEffect, useRef, useState } from "react";
import {
  AlertTriangle,
  ChevronRight,
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
  const refresh = useLicenseStore((s) => s.refresh);

  const [menuOpen, setMenuOpen] = useState(false);
  const [actOpen, setActOpen] = useState(false);
  const [devOpen, setDevOpen] = useState(false);
  const [recOpen, setRecOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
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

  // 指纹漂移优先级最高 — 即使已激活也提示
  const hasFpDrift =
    !!fpMismatchHint ||
    (state.kind === "Activated" && state.fingerprint_mismatch);

  const visual: CardVisual = (() => {
    if (!initialized) {
      return { Icon: HelpCircle, tone: "neutral", title: t("license.statusCard.checking") };
    }
    if (hasFpDrift) {
      return {
        Icon: ShieldAlert,
        tone: "warn",
        title: t("license.statusCard.fpDrift"),
        cta: t("license.statusCard.viewDetail"),
      };
    }
    switch (state.kind) {
      case "Trial":
        return {
          Icon: Clock,
          tone: state.days_remaining <= 3 ? "warn" : "neutral",
          title: t("license.statusCard.trial"),
          cta: t("license.statusCard.trialDaysLeftCta", { days: state.days_remaining }),
        };
      case "TrialExpired":
        return {
          Icon: AlertTriangle,
          tone: "danger",
          title: t("license.statusCard.trialExpired"),
          cta: t("license.statusCard.activateNow"),
        };
      case "Activated":
        return {
          Icon: ShieldCheck,
          tone: "success",
          title: t("license.statusCard.activated"),
          cta: state.email,
        };
      case "GraceMode":
        return {
          Icon: Wifi,
          tone: "warn",
          title: t("license.statusCard.graceMode"),
          cta: t("license.statusCard.graceDaysLeft", { days: state.days_until_block }),
        };
      case "Revoked":
        return {
          Icon: ShieldAlert,
          tone: "danger",
          title: t("license.statusCard.revoked"),
          cta: t("license.statusCard.contactSupport"),
        };
      case "Unknown":
      default:
        return { Icon: HelpCircle, tone: "neutral", title: t("license.statusCard.checking") };
    }
  })();

  const tone = TONE_CLASS[visual.tone];

  // 主点击：根据状态走不同入口
  const handlePrimaryAction = () => {
    setMenuOpen(false);
    if (hasFpDrift) {
      setActOpen(true);
      return;
    }
    switch (state.kind) {
      case "Trial":
      case "TrialExpired":
      case "Unknown":
        setActOpen(true);
        break;
      case "GraceMode":
        // 联网恢复 — 触发 refresh 拉一次
        void refresh();
        toast.success(t("license.statusCard.refreshing"));
        break;
      case "Revoked":
        void openUrl(`mailto:${FEEDBACK_EMAIL}?subject=${encodeURIComponent("Dimkey - License Revoked")}`);
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
      await openUrl(`mailto:${FEEDBACK_EMAIL}?subject=${encodeURIComponent("Dimkey反馈")}`);
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
      toast.error(String(e));
    }
  };

  const { Icon } = visual;

  return (
    <>
      <div className="px-3 pt-2 pb-1 shrink-0 relative">
        <div
          ref={cardRef}
          className={`flex items-center gap-2 px-2.5 py-1.5 rounded-lg border transition-colors cursor-pointer ${tone.wrap}`}
          onClick={handlePrimaryAction}
          role="button"
          tabIndex={0}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              handlePrimaryAction();
            }
          }}
        >
          <Icon className={`w-4 h-4 shrink-0 ${tone.icon}`} />
          <div className="flex-1 min-w-0">
            <div className={`text-xs leading-tight truncate ${tone.title}`}>{visual.title}</div>
            {visual.cta && (
              <div className={`text-[11px] leading-tight truncate ${tone.cta}`}>
                {visual.cta}
              </div>
            )}
          </div>
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              setMenuOpen((v) => !v);
            }}
            className="p-1 -mr-1 rounded hover:bg-black/5 text-slate-400 hover:text-slate-600"
            title={t("license.statusCard.menu.title")}
          >
            <MoreVertical className="w-3.5 h-3.5" />
          </button>
        </div>

        {menuOpen && (
          <div
            ref={menuRef}
            className="absolute left-3 right-3 bottom-full mb-1 bg-white border border-slate-200 rounded-lg shadow-lg py-1 z-30"
          >
            {/* 未激活 / 试用 / 过期：激活入口 */}
            {(state.kind === "Trial" ||
              state.kind === "TrialExpired" ||
              state.kind === "Unknown" ||
              hasFpDrift) && (
              <MenuItem icon={KeyRound} label={t("license.statusCard.menu.activate")} onClick={handleActivate} />
            )}

            {/* 已激活：设备管理 + 取消激活 */}
            {state.kind === "Activated" && (
              <MenuItem icon={Smartphone} label={t("license.statusCard.menu.devices")} onClick={handleDevices} />
            )}

            {/* 宽限期：联网恢复 */}
            {state.kind === "GraceMode" && (
              <MenuItem
                icon={Wifi}
                label={t("license.statusCard.menu.recoverOnline")}
                onClick={() => {
                  setMenuOpen(false);
                  void refresh();
                  toast.success(t("license.statusCard.refreshing"));
                }}
              />
            )}

            <MenuItem icon={Mail} label={t("license.statusCard.menu.feedback")} onClick={handleFeedback} />
            <MenuItem icon={Info} label={t("license.statusCard.menu.about")} onClick={handleAbout} />

            {state.kind === "Activated" && (
              <>
                <div className="my-1 border-t border-slate-100" />
                <MenuItem
                  icon={AlertTriangle}
                  label={t("license.statusCard.menu.deactivate")}
                  onClick={() => void handleDeactivate()}
                  danger
                />
              </>
            )}
          </div>
        )}
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
}

function MenuItem({ icon: Icon, label, onClick, danger }: MenuItemProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`w-full flex items-center gap-2 px-3 py-1.5 text-xs text-left transition-colors ${
        danger
          ? "text-red-600 hover:bg-red-50"
          : "text-slate-700 hover:bg-slate-50"
      }`}
    >
      <Icon className="w-3.5 h-3.5 shrink-0" />
      <span className="flex-1 truncate">{label}</span>
      <ChevronRight className="w-3 h-3 text-slate-300 shrink-0" />
    </button>
  );
}
