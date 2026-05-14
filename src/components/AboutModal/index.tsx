import { useState, useEffect } from "react";
import { Mail, Copy, X } from "lucide-react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useLicenseStore, type DeviceInfo } from "../../stores/licenseStore";
import { ActivationDialog } from "../license/ActivationDialog";
import { DeviceListDialog } from "../license/DeviceListDialog";
import { RecoverDialog } from "../license/RecoverDialog";

const FEEDBACK_EMAIL = "cube1@live.cn";

interface AboutModalProps {
  visible: boolean;
  onClose: () => void;
}

export function AboutModal({ visible, onClose }: AboutModalProps) {
  const { t } = useTranslation();
  const [version, setVersion] = useState("");
  const [copied, setCopied] = useState(false);
  const [fpCopied, setFpCopied] = useState(false);

  const state = useLicenseStore((s) => s.state);
  const fingerprint = useLicenseStore((s) => s.fingerprint);
  const fpMismatchHint = useLicenseStore((s) => s.fingerprintMismatchHint);
  const deactivateCurrent = useLicenseStore((s) => s.deactivateCurrent);
  const openPurchase = useLicenseStore((s) => s.openPurchase);

  const [actOpen, setActOpen] = useState(false);
  const [devOpen, setDevOpen] = useState(false);
  const [recOpen, setRecOpen] = useState(false);
  const [devicesPreload, setDevicesPreload] = useState<DeviceInfo[] | undefined>(undefined);
  const [maxPreload, setMaxPreload] = useState<number | undefined>(undefined);

  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
  }, []);

  if (!visible) return null;

  const handleFeedback = async () => {
    try {
      await openUrl(`mailto:${FEEDBACK_EMAIL}?subject=${encodeURIComponent("Dimkey反馈")}`);
    } catch {
      toast.error("无法打开邮件客户端");
    }
  };

  const handleCopyEmail = async () => {
    try {
      await navigator.clipboard.writeText(FEEDBACK_EMAIL);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      toast.error("复制失败");
    }
  };

  const handleCopyFingerprint = async () => {
    try {
      await navigator.clipboard.writeText(fingerprint);
      setFpCopied(true);
      toast.success(t("license.about.fingerprint_copy"));
      setTimeout(() => setFpCopied(false), 2000);
    } catch {
      // ignore
    }
  };

  const fpShort = fingerprint
    ? `${fingerprint.slice(0, 8)}...${fingerprint.slice(-4)}`
    : "";

  return (
    <>
      <div
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-sm"
        onClick={onClose}
      >
        <div
          className="bg-white rounded-xl shadow-xl w-[420px] mx-4 p-6 relative"
          onClick={(e) => e.stopPropagation()}
        >
          <button
            onClick={onClose}
            className="absolute top-3 right-3 p-1 text-slate-300 hover:text-slate-500 rounded transition-colors"
          >
            <X className="w-4 h-4" />
          </button>

          {/* 应用信息 */}
          <div className="text-center mb-4">
            <h3 className="text-lg font-semibold text-slate-800">{t("about.name")}</h3>
            <p className="text-xs text-slate-400 mt-0.5">
              Dimkey{version && ` · v${version}`}
            </p>
          </div>

          <p className="text-sm text-slate-600 leading-relaxed text-center mb-5">
            {t("about.description")}
          </p>

          {/* 反馈区域 */}
          <div className="flex gap-2 mb-5">
            <button
              onClick={handleFeedback}
              className="flex-1 flex items-center justify-center gap-1.5 px-4 py-1.5 text-sm text-primary-600 hover:bg-primary-50 rounded-lg transition-colors"
            >
              <Mail className="w-3.5 h-3.5" />
              {t("about.feedback")}
            </button>
            <button
              onClick={handleCopyEmail}
              className="flex-1 flex items-center justify-center gap-1.5 px-4 py-1.5 text-sm text-slate-500 hover:text-slate-700 hover:bg-slate-100 rounded-lg transition-colors"
            >
              <Copy className="w-3.5 h-3.5" />
              {copied ? t("common.copied") : t("about.copyEmail")}
            </button>
          </div>

          {/* === License 区块 === */}
          <div className="border-t border-slate-200 pt-4 mb-2">
            {fpMismatchHint && (
              <div className="mb-3 px-3 py-2 bg-amber-50 border border-amber-200 rounded-lg text-xs text-amber-900">
                <button onClick={() => setActOpen(true)} className="text-left underline">
                  {t("license.about.fingerprint_mismatch_hint", { fp: fpMismatchHint })}
                </button>
              </div>
            )}

            {state.kind === "Activated" && (
              <>
                <div className="mb-3">
                  <p className="text-sm text-slate-700">
                    ✓ {t("license.about.activated_to", { email: state.email })}
                  </p>
                  <p className="text-xs text-slate-500 mt-1">
                    {t("license.about.plan_perpetual")} ·{" "}
                    {t("license.about.devices", {
                      active: state.active_devices,
                      max: state.max_devices,
                    })}
                  </p>
                </div>
                <div className="flex gap-2">
                  <button
                    onClick={() => {
                      setDevicesPreload(undefined);
                      setMaxPreload(undefined);
                      setDevOpen(true);
                    }}
                    className="flex-1 px-3 py-1.5 text-xs border border-slate-300 rounded-lg hover:bg-slate-50"
                  >
                    {t("license.about.manage_devices")}
                  </button>
                  <button
                    onClick={() => {
                      void deactivateCurrent();
                    }}
                    className="flex-1 px-3 py-1.5 text-xs border border-red-300 text-red-600 rounded-lg hover:bg-red-50"
                  >
                    {t("license.about.deactivate_current")}
                  </button>
                </div>
              </>
            )}

            {state.kind === "Trial" && (
              <div className="flex items-center justify-between gap-2">
                <p className="text-sm text-slate-700">
                  {t("license.trial.remaining", { days: state.days_remaining })}
                </p>
                <div className="flex gap-2">
                  <button
                    onClick={() => setActOpen(true)}
                    className="px-3 py-1.5 text-xs bg-gray-900 text-white rounded-lg hover:bg-gray-800"
                  >
                    {t("license.purchase.enter_key")}
                  </button>
                  <button
                    onClick={() => {
                      void openPurchase();
                    }}
                    className="px-3 py-1.5 text-xs border border-slate-300 rounded-lg hover:bg-slate-50"
                  >
                    {t("license.purchase.button")}
                  </button>
                </div>
              </div>
            )}

            {state.kind === "TrialExpired" && (
              <div className="flex items-center justify-between gap-2">
                <p className="text-sm text-amber-700">⚠ {t("license.trial.expired_banner")}</p>
                <div className="flex gap-2">
                  <button
                    onClick={() => setActOpen(true)}
                    className="px-3 py-1.5 text-xs bg-gray-900 text-white rounded-lg hover:bg-gray-800"
                  >
                    {t("license.purchase.enter_key")}
                  </button>
                  <button
                    onClick={() => {
                      void openPurchase();
                    }}
                    className="px-3 py-1.5 text-xs border border-slate-300 rounded-lg hover:bg-slate-50"
                  >
                    {t("license.purchase.button")}
                  </button>
                </div>
              </div>
            )}

            {state.kind === "GraceMode" && (
              <p className="text-sm text-slate-700">{t("license.error.network")}</p>
            )}

            {state.kind === "Revoked" && (
              <div className="flex items-center justify-between gap-2">
                <p className="text-sm text-red-700">⚠ {t("license.error.revoked")}</p>
                <button
                  onClick={() => setActOpen(true)}
                  className="px-3 py-1.5 text-xs bg-gray-900 text-white rounded-lg hover:bg-gray-800"
                >
                  {t("license.purchase.enter_key")}
                </button>
              </div>
            )}

            {fingerprint && (
              <div className="mt-4 flex items-center gap-2 text-[11px] text-slate-400">
                <span>{t("license.about.fingerprint_label")}</span>
                <code className="bg-slate-100 px-1.5 py-0.5 rounded font-mono">{fpShort}</code>
                <button
                  onClick={handleCopyFingerprint}
                  className="hover:bg-slate-100 px-1 rounded text-slate-500"
                  title={t("license.about.fingerprint_copy")}
                >
                  {fpCopied ? "✓" : <Copy className="w-3 h-3 inline" />}
                </button>
              </div>
            )}
          </div>

          <p className="text-[11px] text-slate-300 text-center mt-3">© 2025 Dimkey</p>
        </div>
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
    </>
  );
}
