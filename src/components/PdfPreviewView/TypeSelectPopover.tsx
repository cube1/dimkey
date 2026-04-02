import { useTranslation } from "react-i18next";
import { SENSITIVE_TYPE_CONFIG } from "../../types";
import type { SensitiveType } from "../../types";

const COMMON_TYPES = ["Phone", "IdCard", "PersonName", "Email", "Address", "BankCard"];

interface TypeSelectPopoverProps {
  position: { x: number; y: number };
  onSelect: (type: SensitiveType) => void;
  onCancel: () => void;
}

export function TypeSelectPopover({ position, onSelect, onCancel }: TypeSelectPopoverProps) {
  const { t } = useTranslation();

  return (
    <div
      className="fixed z-50 bg-white rounded-xl shadow-float border border-slate-200/80 overflow-hidden animate-slide-up"
      style={{ top: position.y, left: position.x }}
    >
      <div className="px-3 pt-2.5 pb-2">
        <div className="text-[11px] text-slate-400 mb-1.5 tracking-wider">{t("textToolbar.selectType")}</div>
        <div className="flex flex-wrap gap-1">
          {COMMON_TYPES.map((type) => {
            const config = SENSITIVE_TYPE_CONFIG[type];
            return (
              <button
                key={type}
                onClick={() => onSelect(type as SensitiveType)}
                className={`text-xs px-2 py-1 rounded-md ${config?.bgClass} ${config?.textClass} hover:opacity-75 transition-opacity`}
              >
                {config?.label || type}
              </button>
            );
          })}
          <button
            onClick={onCancel}
            className="text-xs px-2 py-1 rounded-md text-slate-400 hover:text-slate-600 hover:bg-slate-100 transition-colors"
          >
            {t("common.cancel")}
          </button>
        </div>
      </div>
    </div>
  );
}
