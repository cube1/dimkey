// src/components/license/RecoverDialog.tsx
import { useState } from "react";
import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useLicenseStore } from "../../stores/licenseStore";

interface RecoverDialogProps {
  visible: boolean;
  onClose: () => void;
}

export function RecoverDialog({ visible, onClose }: RecoverDialogProps) {
  const { t } = useTranslation();
  const recover = useLicenseStore((s) => s.recover);
  const [email, setEmail] = useState("");
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  if (!visible) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    try {
      await recover(email);
    } catch {
      // 防扫号：无论失败成功都显示同样消息
    }
    setSubmitted(true);
    setSubmitting(false);
  };

  const handleClose = () => {
    onClose();
    // 重置以便下次打开是干净状态
    setEmail("");
    setSubmitted(false);
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-sm"
      onClick={handleClose}
    >
      <div
        className="bg-white rounded-xl shadow-xl w-[420px] mx-4 p-6 relative"
        onClick={(e) => e.stopPropagation()}
      >
        <button
          onClick={handleClose}
          className="absolute top-4 right-4 text-gray-400 hover:text-gray-600"
          aria-label="close"
        >
          <X size={20} />
        </button>
        <h2 className="text-lg font-semibold mb-3">{t("license.recover.title")}</h2>
        <p className="text-sm text-gray-600 mb-4">{t("license.recover.hint")}</p>

        {!submitted ? (
          <form onSubmit={handleSubmit} className="space-y-3">
            <input
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:border-gray-700 text-sm"
            />
            <button
              type="submit"
              disabled={submitting}
              className="w-full px-4 py-2 bg-gray-900 text-white rounded-lg hover:bg-gray-800 disabled:opacity-50 text-sm"
            >
              {t("license.recover.send")}
            </button>
          </form>
        ) : (
          <p className="text-sm text-green-700">{t("license.recover.success_msg")}</p>
        )}
      </div>
    </div>
  );
}
