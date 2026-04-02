import { useState, useEffect } from "react";
import { Mail, Copy, X } from "lucide-react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";

const FEEDBACK_EMAIL = "cube1@live.cn";

interface AboutModalProps {
  visible: boolean;
  onClose: () => void;
}

export function AboutModal({ visible, onClose }: AboutModalProps) {
  const { t } = useTranslation();
  const [version, setVersion] = useState("");
  const [copied, setCopied] = useState(false);

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

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="bg-white rounded-xl shadow-xl max-w-sm mx-4 p-6 relative"
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
        <div className="flex gap-2">
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

        <p className="text-[11px] text-slate-300 text-center mt-4">
          © 2025 Dimkey
        </p>
      </div>
    </div>
  );
}
