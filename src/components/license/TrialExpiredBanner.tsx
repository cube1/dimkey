// src/components/license/TrialExpiredBanner.tsx
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useLicenseStore } from "../../stores/licenseStore";

interface TrialExpiredBannerProps {
  onActivate: () => void;
}

/** 试用过期顶部横幅。仅在 LicenseState.kind === 'TrialExpired' 时显示，可被用户关闭（下次启动重现）。 */
export function TrialExpiredBanner({ onActivate }: TrialExpiredBannerProps) {
  const { t } = useTranslation();
  const state = useLicenseStore((s) => s.state);
  const openPurchase = useLicenseStore((s) => s.openPurchase);
  const [dismissed, setDismissed] = useState(false);

  if (state.kind !== "TrialExpired" || dismissed) return null;

  return (
    <div className="bg-amber-50 border-b border-amber-200 px-4 py-2 flex items-center justify-between text-sm">
      <span className="text-amber-900">⚠ {t("license.trial.expired_banner")}</span>
      <div className="flex gap-2">
        <button
          onClick={onActivate}
          className="px-3 py-1 bg-gray-900 text-white rounded hover:bg-gray-800 text-xs"
        >
          {t("license.purchase.enter_key")}
        </button>
        <button
          onClick={() => {
            void openPurchase();
          }}
          className="px-3 py-1 border border-gray-300 rounded hover:bg-white text-xs"
        >
          {t("license.purchase.button")}
        </button>
        <button
          onClick={() => setDismissed(true)}
          className="px-3 py-1 text-gray-600 hover:bg-amber-100 rounded text-xs"
        >
          {t("license.trial.banner_dismiss")}
        </button>
      </div>
    </div>
  );
}
