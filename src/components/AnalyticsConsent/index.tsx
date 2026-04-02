import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

const CONSENT_SHOWN_KEY = "analytics_consent_shown";

export function AnalyticsConsent() {
  const { t } = useTranslation();
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    // 仅首次启动时显示
    if (!localStorage.getItem(CONSENT_SHOWN_KEY)) {
      setVisible(true);
    }
  }, []);

  const handleAccept = () => {
    localStorage.setItem(CONSENT_SHOWN_KEY, "1");
    setVisible(false);
  };

  const handleDecline = async () => {
    try {
      await invoke("set_analytics_enabled", { enabled: false });
    } catch {
      // 静默忽略
    }
    localStorage.setItem(CONSENT_SHOWN_KEY, "1");
    setVisible(false);
  };

  if (!visible) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-sm">
      <div className="bg-white rounded-xl shadow-xl max-w-sm mx-4 p-6">
        <h3 className="text-base font-semibold text-slate-800 mb-2">
          {t("analytics.title")}
        </h3>
        <p className="text-sm text-slate-600 leading-relaxed mb-4">
          {t("analytics.description")}
        </p>
        <div className="flex gap-2 justify-end">
          <button
            onClick={handleDecline}
            className="px-4 py-1.5 text-sm text-slate-500 hover:text-slate-700 hover:bg-slate-100 rounded-lg transition-colors"
          >
            {t("analytics.decline")}
          </button>
          <button
            onClick={handleAccept}
            className="px-4 py-1.5 text-sm font-medium text-white bg-primary-600 hover:bg-primary-700 rounded-lg transition-colors"
          >
            {t("analytics.accept")}
          </button>
        </div>
      </div>
    </div>
  );
}
