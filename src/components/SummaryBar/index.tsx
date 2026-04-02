import { Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useDetectStore, useSummaryByType, useAllActiveItems } from "../../stores/detectStore";
import { SENSITIVE_TYPE_CONFIG } from "../../types";

export function SummaryBar() {
  const { t } = useTranslation();
  const nerStatus = useDetectStore((s) => s.nerStatus);
  const hiddenTypes = useDetectStore((s) => s.hiddenTypes);
  const toggleType = useDetectStore((s) => s.toggleType);
  const showAllTypes = useDetectStore((s) => s.showAllTypes);
  const hideAllTypes = useDetectStore((s) => s.hideAllTypes);
  const allActiveItems = useAllActiveItems();
  const summaryByType = useSummaryByType();

  const total = allActiveItems.length;
  const hasHidden = hiddenTypes.size > 0;

  return (
    <div className="bg-white border-b border-gray-200 px-6 py-3 flex items-center gap-4 text-sm">
      {/* 总数 */}
      <span className="font-medium text-gray-700 whitespace-nowrap">
        {t("summaryBar.totalDetected", { count: total })}
      </span>

      {/* 分隔线 */}
      <div className="h-4 w-px bg-gray-300" />

      {/* 全选/取消全选 */}
      <button
        onClick={hasHidden ? showAllTypes : hideAllTypes}
        className="text-xs text-gray-500 hover:text-gray-700 whitespace-nowrap"
      >
        {hasHidden ? t("summaryBar.showAll") : t("summaryBar.hideAll")}
      </button>

      {/* 各类型标签（可点击筛选） */}
      <div className="flex items-center gap-2 flex-wrap flex-1 min-w-0">
        {Object.entries(summaryByType).map(([typeKey, count]) => {
          const config = SENSITIVE_TYPE_CONFIG[typeKey];
          if (!config) return null;
          const isHidden = hiddenTypes.has(typeKey);
          return (
            <button
              key={typeKey}
              onClick={() => toggleType(typeKey)}
              className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium transition-opacity cursor-pointer ${config.bgClass} ${config.textClass} ${isHidden ? "opacity-30" : "opacity-100"}`}
              title={isHidden ? `点击显示 ${config.label}` : `点击隐藏 ${config.label}`}
            >
              {config.label}
              <span className="opacity-70">({count})</span>
            </button>
          );
        })}
      </div>

      {/* NER 状态 */}
      {nerStatus === "running" && (
        <div className="flex items-center gap-1.5 text-gray-500 whitespace-nowrap">
          <Loader2 className="w-3.5 h-3.5 animate-spin" />
          <span className="text-xs">{t("preview.nerDetecting")}</span>
        </div>
      )}
    </div>
  );
}
