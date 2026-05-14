// src/components/license/ActivationDialog.tsx
import { useState } from "react";
import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useLicenseStore, type DeviceInfo } from "../../stores/licenseStore";

interface ActivationDialogProps {
  visible: boolean;
  onClose: () => void;
  /** DEVICE_LIMIT_REACHED 时回调，把后端返回的设备列表交给上层切到 DeviceListDialog */
  onShowDevices?: (devices: DeviceInfo[], max: number) => void;
  onShowRecover?: () => void;
}

const ALPHA = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";

function formatLicenseKey(raw: string): string {
  const upper = raw.toUpperCase().replace(/[^A-Z0-9]/g, "");
  const stripped = upper.startsWith("DK") ? upper.slice(2) : upper;
  const valid = stripped
    .split("")
    .filter((c) => ALPHA.includes(c))
    .slice(0, 25)
    .join("");
  if (valid.length === 0) return "";
  const segs = (valid.match(/.{1,5}/g) || []).slice(0, 5);
  return "DK-" + segs.join("-");
}

export function ActivationDialog({
  visible,
  onClose,
  onShowDevices,
  onShowRecover,
}: ActivationDialogProps) {
  const { t } = useTranslation();
  const activate = useLicenseStore((s) => s.activate);
  const [email, setEmail] = useState("");
  const [keyInput, setKeyInput] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!visible) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await activate(keyInput, email);
      // 成功：关闭对话框（success toast 由 App 层监听 license:state-changed 后触发；本组件不弹 toast）
      onClose();
      // reset for next open
      setEmail("");
      setKeyInput("");
    } catch (err: unknown) {
      // err 形如 { code: "INVALID_LICENSE", data: ... } 或 { code: "DEVICE_LIMIT_REACHED", data: { devices, max } }
      const e = err as { code?: string; data?: { devices?: DeviceInfo[]; max?: number } } | null;
      const code: string = e?.code ?? "server";
      const data = e?.data;
      switch (code) {
        case "INVALID_LICENSE":
          setError(t("license.error.invalid"));
          break;
        case "DEVICE_LIMIT_REACHED": {
          const max = data?.max ?? 3;
          setError(
            `${t("license.error.device_limit", { max })} · ${t("license.error.device_limit_action")}`,
          );
          if (onShowDevices && Array.isArray(data?.devices)) {
            onShowDevices(data.devices, max);
          }
          break;
        }
        case "LICENSE_REVOKED":
          setError(t("license.error.revoked"));
          break;
        case "FINGERPRINT_MISMATCH":
          setError(t("license.error.fingerprint_mismatch"));
          break;
        case "RATE_LIMITED":
          setError(t("license.error.rate_limited"));
          break;
        case "NETWORK_UNAVAILABLE":
          setError(t("license.error.network"));
          break;
        case "SIGNATURE_INVALID":
          setError(t("license.error.signature_invalid"));
          break;
        default:
          setError(t("license.error.server"));
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="bg-white rounded-xl shadow-xl w-[460px] mx-4 p-6 relative"
        onClick={(e) => e.stopPropagation()}
      >
        <button
          onClick={onClose}
          className="absolute top-4 right-4 text-gray-400 hover:text-gray-600"
          aria-label="close"
        >
          <X size={20} />
        </button>
        <h2 className="text-lg font-semibold mb-5">{t("license.activate.title")}</h2>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm text-gray-600 mb-1">
              {t("license.activate.email_label")}
            </label>
            <input
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:border-gray-700 text-sm"
            />
          </div>
          <div>
            <label className="block text-sm text-gray-600 mb-1">
              {t("license.activate.key_label")}
            </label>
            <input
              type="text"
              required
              value={keyInput}
              onChange={(e) => setKeyInput(formatLicenseKey(e.target.value))}
              placeholder={t("license.activate.key_placeholder")}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg font-mono text-sm focus:outline-none focus:border-gray-700"
            />
            {onShowRecover && (
              <button
                type="button"
                onClick={onShowRecover}
                className="text-sm text-blue-600 hover:underline mt-2"
              >
                {t("license.activate.recover_link")}
              </button>
            )}
          </div>
          {error && <p className="text-sm text-red-600">{error}</p>}
          <div className="flex justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-gray-600 hover:bg-gray-100 rounded-lg text-sm"
            >
              {t("license.activate.cancel")}
            </button>
            <button
              type="submit"
              disabled={submitting}
              className="px-4 py-2 bg-gray-900 text-white rounded-lg hover:bg-gray-800 disabled:opacity-50 text-sm"
            >
              {submitting ? t("license.activate.button_loading") : t("license.activate.button")}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
