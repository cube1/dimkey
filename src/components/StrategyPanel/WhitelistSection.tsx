import { useState } from "react";
import { ChevronDown, ChevronRight, ShieldCheck, X } from "lucide-react";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useReDetectDict } from "../../hooks/useReDetectDict";
import { useAutoDesensitize } from "../../hooks/useAutoDesensitize";

export function WhitelistSection() {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState(true);
  const wsData = useWorkspaceStore((s) => s.activeWorkspaceData);
  const removeWhitelistEntry = useWorkspaceStore((s) => s.removeWhitelistEntry);
  const reDetectDict = useReDetectDict();
  const { reDesensitizeWithFilteredItems } = useAutoDesensitize();

  const whitelist = wsData?.workspace.whitelist || [];
  const workspaceMode = wsData?.workspace.mode || "Desensitize";
  const isTemplateMode = workspaceMode === "TemplateReplace";

  if (whitelist.length === 0) return null;

  const handleRemove = async (index: number) => {
    try {
      await removeWhitelistEntry(index);
      toast.success(t("strategyPanel.removeFromWhitelist"));
      // 重新检测词典（其中包含白名单过滤逻辑）
      await reDetectDict();
      if (!isTemplateMode) {
        await reDesensitizeWithFilteredItems();
      }
    } catch {
      toast.error(t("strategyPanel.removeFailed", { error: "" }));
    }
  };

  return (
    <div className="border-b border-slate-100">
      <button
        onClick={() => setCollapsed(!collapsed)}
        className="w-full flex items-center gap-2 px-4 py-2.5 text-xs font-semibold text-slate-600 hover:bg-slate-50 transition-colors"
      >
        {collapsed ? (
          <ChevronRight className="w-3.5 h-3.5 text-slate-400" />
        ) : (
          <ChevronDown className="w-3.5 h-3.5 text-slate-400" />
        )}
        <ShieldCheck className="w-3.5 h-3.5 text-amber-500" />
        {t("strategyPanel.whitelist")}
        <span className="text-[11px] font-normal text-slate-400">({whitelist.length})</span>
      </button>

      {!collapsed && (
        <div className="px-4 pb-3">
          <div className="space-y-1.5 max-h-32 overflow-auto">
            {whitelist.map((entry, index) => (
              <div
                key={`${entry.text}-${index}`}
                className="flex items-center gap-2 px-2.5 py-1.5 bg-amber-50 rounded group"
              >
                <span className="text-xs text-slate-800 truncate flex-1 min-w-0">
                  {entry.text}
                </span>
                <span className="shrink-0 text-xs text-slate-400">
                  {entry.match_mode === "Exact" ? t("common.exact") : t("common.fuzzy")}
                </span>
                <button
                  onClick={() => handleRemove(index)}
                  className="shrink-0 p-0.5 text-slate-300 hover:text-rose-500 rounded transition-colors opacity-0 group-hover:opacity-100"
                  title={t("strategyPanel.removeFromWhitelist")}
                >
                  <X className="w-3 h-3" />
                </button>
              </div>
            ))}
          </div>
          <p className="text-[11px] text-slate-400 mt-2 px-1">
            {t("strategyPanel.whitelistHint")}
          </p>
        </div>
      )}
    </div>
  );
}
