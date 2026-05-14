// src/components/license/TrialCountdownBadge.tsx
import { useTranslation } from "react-i18next";
import { useLicenseStore } from "../../stores/licenseStore";

interface TrialCountdownBadgeProps {
  onActivate: () => void;
}

/** 试用倒计时小角标。仅在剩余 ≤ 7 天时出现：≤7 橙色，≤3 红色。 */
export function TrialCountdownBadge({ onActivate }: TrialCountdownBadgeProps) {
  const { t } = useTranslation();
  const state = useLicenseStore((s) => s.state);

  if (state.kind !== "Trial" || state.days_remaining > 7) return null;

  const color =
    state.days_remaining <= 3
      ? "bg-red-100 text-red-700 border-red-300"
      : "bg-orange-100 text-orange-700 border-orange-300";

  return (
    <button
      onClick={onActivate}
      className={`text-xs px-2 py-0.5 border rounded-full ${color} hover:opacity-80`}
    >
      {t("license.trial.remaining", { days: state.days_remaining })}
    </button>
  );
}
