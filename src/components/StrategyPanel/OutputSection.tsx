import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { ChevronDown, ChevronRight, FolderOpen, FolderOutput, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/workspaceStore";

export function OutputSection() {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState(false);
  const wsData = useWorkspaceStore((s) => s.activeWorkspaceData);
  const updateOutputDir = useWorkspaceStore((s) => s.updateOutputDir);

  const outputDir = wsData?.workspace.output_dir || null;

  const handleSelectDir = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
    });
    if (selected) {
      updateOutputDir(selected);
    }
  };

  const handleClear = () => {
    updateOutputDir(null);
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
        <FolderOutput className="w-3.5 h-3.5 text-emerald-400" />
        {t("strategyPanel.output")}
      </button>

      {!collapsed && (
        <div className="px-4 pb-4">
          <label className="text-xs text-slate-500 mb-2 block">{t("strategyPanel.defaultOutputDir")}</label>
          {outputDir ? (
            <div className="flex items-center gap-2 p-2 bg-slate-50 rounded-lg">
              <FolderOpen className="w-4 h-4 text-slate-400 shrink-0" />
              <span className="text-xs text-slate-700 truncate flex-1 min-w-0">
                {outputDir}
              </span>
              <button
                onClick={handleClear}
                className="shrink-0 p-0.5 text-slate-300 hover:text-rose-500 rounded transition-colors"
                title={t("strategyPanel.clearDir")}
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
          ) : (
            <button
              onClick={handleSelectDir}
              className="w-full py-2 text-xs text-slate-500 border border-dashed border-slate-300 rounded-lg hover:border-primary-400 hover:bg-primary-50/50 hover:text-slate-600 transition-colors"
            >
              {t("strategyPanel.selectDir")}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
