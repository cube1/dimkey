import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { SENSITIVE_TYPE_CONFIG } from "../../types";

const TYPE_KEYS = Object.keys(SENSITIVE_TYPE_CONFIG).filter((k) => k !== "Custom");

export function TypeSelector() {
  const { t } = useTranslation();
  const wsData = useWorkspaceStore((s) => s.activeWorkspaceData);
  const updateEnabledTypes = useWorkspaceStore((s) => s.updateEnabledTypes);

  if (!wsData) return null;

  const enabledTypes = wsData.workspace.enabled_types;

  const toggle = (typeKey: string) => {
    const next = enabledTypes.includes(typeKey)
      ? enabledTypes.filter((t) => t !== typeKey)
      : [...enabledTypes, typeKey];
    updateEnabledTypes(next);
  };

  return (
    <div className="flex flex-wrap items-center gap-1.5 py-1">
      <span className="text-xs font-medium text-slate-500 shrink-0 mr-1">{t("typeSelector.label")}</span>
      {TYPE_KEYS.map((typeKey) => {
        const info = SENSITIVE_TYPE_CONFIG[typeKey];
        const enabled = enabledTypes.includes(typeKey);
        return (
          <button
            key={typeKey}
            onClick={() => toggle(typeKey)}
            className={`
              shrink-0 whitespace-nowrap px-2.5 py-1 rounded-md text-xs font-medium transition-all cursor-pointer select-none
              ${enabled
                ? `${info.bgClass} ${info.textClass} ring-1 ring-inset ring-current/10`
                : "bg-slate-50 text-slate-400 opacity-60 line-through"
              }
            `}
          >
            {info.label}
          </button>
        );
      })}
    </div>
  );
}
