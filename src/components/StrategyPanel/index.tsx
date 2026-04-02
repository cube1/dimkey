import { useCallback } from "react";
import { Settings2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useAutoDesensitize } from "../../hooks/useAutoDesensitize";
import { TypeSelector } from "../TypeSelector";
import { RulesSection } from "./RulesSection";
import { DictSection } from "./DictSection";
import { OutputSection } from "./OutputSection";
import { WhitelistSection } from "./WhitelistSection";
import { AliasGroupSection } from "./AliasGroupSection";

export function StrategyPanel() {
  const { t } = useTranslation();
  const wsData = useWorkspaceStore((s) => s.activeWorkspaceData);
  const updateWorkspaceMode = useWorkspaceStore((s) => s.updateWorkspaceMode);
  const { processFile } = useAutoDesensitize();
  const workspaceMode = wsData?.workspace.mode || "Desensitize";
  const isTemplateMode = workspaceMode === "TemplateReplace";

  const handleModeSwitch = useCallback(async (mode: "Desensitize" | "TemplateReplace") => {
    if (mode === workspaceMode) return;
    const filePath = await updateWorkspaceMode(mode);
    if (filePath) {
      await processFile(filePath);
    }
  }, [updateWorkspaceMode, processFile, workspaceMode]);

  if (!wsData) {
    return (
      <div className="flex-1 flex items-center justify-center text-sm text-slate-400">
        {t("strategyPanel.selectWorkspace")}
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col min-h-0">
      {/* 标题 */}
      <div className="px-4 py-3 border-b border-slate-200 shrink-0">
        <div className="flex items-center gap-2">
          <div className="p-1 rounded-md bg-primary-50">
            <Settings2 className="w-3.5 h-3.5 text-primary-500" />
          </div>
          <div>
            <h2 className="text-sm font-bold text-slate-700">{t("strategyPanel.title")}</h2>
            <p className="text-[11px] text-slate-400 leading-tight">{wsData.workspace.name}</p>
          </div>
        </div>
      </div>

      {/* 模式切换 */}
      <div className="px-4 py-2 border-b border-slate-200 shrink-0">
        <div className="flex gap-1 rounded-lg bg-slate-100 p-0.5">
          <button
            onClick={() => handleModeSwitch("Desensitize")}
            className={`flex-1 px-2 py-1 text-xs rounded-md transition-colors ${
              !isTemplateMode
                ? "bg-white text-slate-800 shadow-sm font-medium"
                : "text-slate-500 hover:text-slate-700"
            }`}
          >
            {t("strategyPanel.desensitizeMode")}
          </button>
          <button
            onClick={() => handleModeSwitch("TemplateReplace")}
            className={`flex-1 px-2 py-1 text-xs rounded-md transition-colors ${
              isTemplateMode
                ? "bg-white text-slate-800 shadow-sm font-medium"
                : "text-slate-500 hover:text-slate-700"
            }`}
          >
            {t("strategyPanel.templateMode")}
          </button>
        </div>
      </div>

      {/* 类型选择器 - 仅脱敏模式 */}
      {!isTemplateMode && (
        <div className="px-4 py-2 border-b border-slate-200 shrink-0">
          <TypeSelector />
        </div>
      )}

      {/* 可滚动的配置面板 */}
      <div className="flex-1 overflow-auto">
        {!isTemplateMode && <RulesSection />}
        <DictSection />
        <WhitelistSection />
        {!isTemplateMode && <AliasGroupSection />}
        <OutputSection />
      </div>
    </div>
  );
}
