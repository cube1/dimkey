import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { FolderOpen, Zap, CheckCircle2 } from "lucide-react";
import toast from "react-hot-toast";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useBatchAutoProcess } from "../../hooks/useBatchAutoProcess";
import { useAutoDesensitize } from "../../hooks/useAutoDesensitize";
import type { BatchMode } from "../../types";

export function BatchModeSelector() {
  const { t } = useTranslation();
  const [mode, setMode] = useState<BatchMode>("auto");
  const fileQueue = useWorkspaceStore((s) => s.fileQueue);
  const wsData = useWorkspaceStore((s) => s.activeWorkspaceData);
  const [outputDir, setOutputDir] = useState<string | null>(
    wsData?.workspace.output_dir ?? null,
  );
  const [starting, setStarting] = useState(false);
  const { startAutoProcess } = useBatchAutoProcess();
  const { processFile } = useAutoDesensitize();

  const handleChooseDir = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") setOutputDir(selected);
  };

  const handleStart = async () => {
    if (mode === "auto" && !outputDir) {
      toast.error(t("fileQueue.batchMode.outputDirRequired"));
      return;
    }
    setStarting(true);
    if (mode === "sequential") {
      // 沿用现有逐个确认流程：第一个文件 status=processing，调 processFile
      const first = fileQueue[0];
      if (first) {
        useWorkspaceStore.getState().updateQueueFileStatus(first.id, "processing");
        await processFile(first.filePath);
      }
    } else {
      await startAutoProcess(outputDir!);
    }
    setStarting(false);
  };

  return (
    <div className="flex-1 flex items-center justify-center p-8" data-testid="batch-mode-selector">
      <div className="bg-white rounded-xl border border-slate-200 shadow-sm p-6 w-full max-w-xl">
        <h2 className="text-lg font-semibold text-slate-800 mb-1">
          {t("fileQueue.batchMode.selectorTitle")}
        </h2>
        <p className="text-sm text-slate-500 mb-5">
          {t("fileQueue.batchMode.filesCount", { count: fileQueue.length })}
        </p>

        {/* Segmented Control */}
        <div className="grid grid-cols-2 gap-2 mb-5">
          <button
            onClick={() => setMode("sequential")}
            data-testid="mode-sequential"
            className={`p-4 rounded-lg border-2 text-left transition-all ${
              mode === "sequential"
                ? "border-primary-500 bg-primary-50 ring-2 ring-primary-500/10"
                : "border-slate-200 hover:border-slate-300"
            }`}
          >
            <div className="flex items-center gap-2 mb-1">
              <CheckCircle2 className="w-4 h-4 text-primary-600" />
              <span className="font-medium text-slate-800">
                {t("fileQueue.batchMode.sequential")}
              </span>
            </div>
            <p className="text-xs text-slate-500">
              {t("fileQueue.batchMode.sequentialHint")}
            </p>
          </button>
          <button
            onClick={() => setMode("auto")}
            data-testid="mode-auto"
            className={`p-4 rounded-lg border-2 text-left transition-all ${
              mode === "auto"
                ? "border-primary-500 bg-primary-50 ring-2 ring-primary-500/10"
                : "border-slate-200 hover:border-slate-300"
            }`}
          >
            <div className="flex items-center gap-2 mb-1">
              <Zap className="w-4 h-4 text-amber-500" />
              <span className="font-medium text-slate-800">
                {t("fileQueue.batchMode.auto")}
              </span>
            </div>
            <p className="text-xs text-slate-500">
              {t("fileQueue.batchMode.autoHint")}
            </p>
          </button>
        </div>

        {/* 输出目录（仅 auto 模式） */}
        {mode === "auto" && (
          <div className="mb-5" data-testid="output-dir-section">
            <label className="block text-xs font-medium text-slate-600 mb-1.5">
              {t("fileQueue.batchMode.outputDir")}
            </label>
            <div className="flex gap-2">
              <input
                readOnly
                value={outputDir ?? ""}
                placeholder={t("fileQueue.batchMode.outputDirRequired")}
                className="flex-1 px-3 py-2 border border-slate-200 rounded-md bg-slate-50 text-sm text-slate-700"
              />
              <button
                onClick={handleChooseDir}
                data-testid="btn-choose-dir"
                className="inline-flex items-center gap-1.5 px-3 py-2 bg-white border border-slate-300 rounded-md text-sm text-slate-700 hover:bg-slate-50"
              >
                <FolderOpen className="w-4 h-4" />
                {t("fileQueue.batchMode.chooseDir")}
              </button>
            </div>
          </div>
        )}

        <button
          onClick={handleStart}
          disabled={starting || (mode === "auto" && !outputDir)}
          data-testid="btn-start-batch"
          className="w-full py-2.5 bg-primary-600 text-white font-medium rounded-md hover:bg-primary-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {t("fileQueue.batchMode.startProcessing")}
        </button>
      </div>
    </div>
  );
}
