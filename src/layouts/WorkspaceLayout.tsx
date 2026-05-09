import { useState, useMemo, useCallback } from "react";
import { PanelLeft, PanelRight, ArrowLeft, Download, ClipboardCopy, Check, SkipForward } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../stores/workspaceStore";
import { useAutoDesensitize } from "../hooks/useAutoDesensitize";
import { WorkspaceList } from "../components/WorkspaceList";
import { CenterPanel } from "../components/CenterPanel";
import { StrategyPanel } from "../components/StrategyPanel";

/** 导出成功 Toast（含打开目录 + 复制到剪贴板） */
function ExportToast({ toastId, outputPath, label }: { toastId: string; outputPath: string; label: string }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const handleCopy = async () => {
    try {
      await invoke("copy_file_to_clipboard", { filePath: outputPath });
      setCopied(true);
    } catch {
      toast.error(t("common.copyFailed"));
    }
  };
  return (
    <div className="flex items-center gap-3">
      <span>{label}</span>
      <button
        onClick={() => {
          revealItemInDir(outputPath).catch(() => {});
          toast.dismiss(toastId);
        }}
        className="text-primary-600 hover:text-primary-700 text-sm font-medium whitespace-nowrap underline underline-offset-2"
      >
        {t("result.openDir")}
      </button>
      <button
        onClick={handleCopy}
        disabled={copied}
        className={`inline-flex items-center gap-1 text-sm font-medium whitespace-nowrap ${
          copied
            ? "text-emerald-600"
            : "text-slate-500 hover:text-slate-700 underline underline-offset-2"
        }`}
      >
        {copied ? <Check className="w-3.5 h-3.5" /> : <ClipboardCopy className="w-3.5 h-3.5" />}
        {copied ? t("common.copied") : t("result.copyFile")}
      </button>
    </div>
  );
}

/** processing 步骤 i18n key 映射 */
const STEP_KEYS: Record<string, string> = {
  parsing: "steps.parsing",
  detecting: "steps.detecting",
  desensitizing: "steps.desensitizing",
  saving: "steps.saving",
  done: "steps.done",
};

export function WorkspaceLayout() {
  const { t } = useTranslation();
  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId);
  const leftOpen = useWorkspaceStore((s) => s.leftSidebarOpen);
  const rightOpen = useWorkspaceStore((s) => s.rightSidebarOpen);
  const toggleLeft = useWorkspaceStore((s) => s.toggleLeftSidebar);
  const toggleRight = useWorkspaceStore((s) => s.toggleRightSidebar);
  const centerView = useWorkspaceStore((s) => s.centerView);
  const wsData = useWorkspaceStore((s) => s.activeWorkspaceData);
  const processingStep = useWorkspaceStore((s) => s.processingStep);
  const processingFileName = useWorkspaceStore((s) => s.processingFileName);
  const currentFileContent = useWorkspaceStore((s) => s.currentFileContent);
  const currentFilePath = useWorkspaceStore((s) => s.currentFilePath);
  const currentResult = useWorkspaceStore((s) => s.currentResult);
  const setCurrentResult = useWorkspaceStore((s) => s.setCurrentResult);
  const rawSensitiveItems = useWorkspaceStore((s) => s.currentSensitiveItems);
  const restoreResult = useWorkspaceStore((s) => s.restoreResult);
  const setRestoreResult = useWorkspaceStore((s) => s.setRestoreResult);
  const setCenterView = useWorkspaceStore((s) => s.setCenterView);
  const fileQueue = useWorkspaceStore((s) => s.fileQueue);
  const activeQueueIndex = useWorkspaceStore((s) => s.activeQueueIndex);
  const isBatchMode = useWorkspaceStore((s) => s.isBatchMode);
  const batchSession = useWorkspaceStore((s) => s.batchSession);

  const { processFile } = useAutoDesensitize();

  const [exporting, setExporting] = useState(false);

  // 按 enabledTypes 过滤后的敏感项数量
  const enabledTypes = wsData?.workspace.enabled_types || [];
  const filteredCount = useMemo(() => {
    return rawSensitiveItems.filter((item) => {
      const key = typeof item.sensitive_type === "string" ? item.sensitive_type : "Custom";
      return enabledTypes.includes(key);
    }).length;
  }, [rawSensitiveItems, enabledTypes]);

  const isTemplateMode = wsData?.workspace.mode === "TemplateReplace";

  // comparison 导出，返回 true 表示导出成功
  const handleComparisonExport = useCallback(async (): Promise<boolean> => {
    if (!currentFileContent) return false;
    // 脱敏模式需要 currentResult；模版模式导出时实时调后端
    if (!isTemplateMode && !currentResult) return false;
    setExporting(true);
    try {
      const fileName = currentFileContent.file_name;
      const ext = fileName.split(".").pop()?.toLowerCase() || "csv";
      const suffix = isTemplateMode ? t("fileSuffix.template") : t("fileSuffix.desensitized");
      const defaultName = fileName.replace(/\.[^.]+$/, `${suffix}.${ext}`);

      const outputPath = await save({
        defaultPath: defaultName,
        filters: [{ name: ext, extensions: [ext] }],
      });

      if (!outputPath) {
        setExporting(false);
        return false;
      }

      let exportContent: import("../types").FileContent;

      if (isTemplateMode) {
        // 模版模式：导出时调用后端 apply_desensitize 生成替换结果
        const items = useWorkspaceStore.getState().currentSensitiveItems;
        const ws = wsData!.workspace;
        // 筛选有替换值的词典匹配项
        const dictTexts = new Set(
          ws.dict_entries.filter((e) => e.replacement).map((e) => e.text)
        );
        const templateItems = items.filter((item) => dictTexts.has(item.text));

        if (templateItems.length === 0) {
          toast.error(t("strategyPanel.noDictReplacements"));
          setExporting(false);
          return false;
        }

        // 构建策略（模版模式使用 Replace 策略作为占位，后端根据词典替换）
        const typeKeys = [...new Set(templateItems.map((item) => {
          return typeof item.sensitive_type === "string" ? item.sensitive_type : "Custom";
        }))];
        const strategies = typeKeys.map((key) => ({
          sensitive_type: key === "Custom" ? { Custom: key } : key,
          strategy: { Replace: { style: "Fake" as const } },
          consistent: false,
        }));

        const result = await invoke<import("../types").DesensitizeResult>("apply_desensitize", {
          content: currentFileContent,
          items: templateItems,
          strategies,
          workspaceId: ws.id,
        });
        exportContent = result.content;
      } else {
        exportContent = currentResult!.content;
      }

      const isPdf = currentFileContent.file_type === "Pdf";

      if (isPdf) {
        // PDF 使用专用涂黑导出命令，后端内部重新解析坐标
        const items = useWorkspaceStore.getState().currentSensitiveItems;
        await invoke("export_pdf_redacted_cmd", {
          originalPath: currentFilePath,
          sensitiveItems: items,
          outputPath,
        });
      } else {
        await invoke("export_file", {
          content: exportContent,
          outputPath,
          originalPath: currentFilePath,
        });
      }

      toast.success(
        (ti) => <ExportToast toastId={ti.id} outputPath={outputPath} label={t("result.exportSuccess")} />,
        { duration: 8000 }
      );
      return true;
    } catch (err) {
      const message = typeof err === "string" ? err : t("result.exportFailed");
      toast.error(message);
      return false;
    } finally {
      setExporting(false);
    }
  }, [currentResult, currentFileContent, currentFilePath, isTemplateMode, wsData, t]);

  const handleExportAndNext = useCallback(async () => {
    const success = await handleComparisonExport();
    if (!success) return;

    const store = useWorkspaceStore.getState();
    const currentFile = store.fileQueue[store.activeQueueIndex];
    if (currentFile) {
      store.updateQueueFileStatus(currentFile.id, "confirmed");
    }

    const nextFile = store.advanceToNextFile();
    if (nextFile) {
      await processFile(nextFile.filePath);
    } else {
      toast.success(t("fileQueue.allDone"));
      store.setCenterView("dropzone");
    }
  }, [handleComparisonExport, processFile]);

  const handleExportOnly = useCallback(async () => {
    const success = await handleComparisonExport();
    if (!success) return;

    const store = useWorkspaceStore.getState();
    const currentFile = store.fileQueue[store.activeQueueIndex];
    if (currentFile) {
      store.updateQueueFileStatus(currentFile.id, "confirmed");
    }
  }, [handleComparisonExport]);

  // restore 导出
  const handleRestoreExport = useCallback(async () => {
    if (!restoreResult) return;
    setExporting(true);
    try {
      const fileName = restoreResult.restored_content.file_name;
      const ext = fileName.split(".").pop()?.toLowerCase() || "csv";
      const defaultName = fileName.replace(/\.[^.]+$/, `${t("fileSuffix.restored")}.${ext}`);

      const outputPath = await save({
        defaultPath: defaultName,
        filters: [{ name: ext, extensions: [ext] }],
      });

      if (!outputPath) {
        setExporting(false);
        return;
      }

      await invoke("export_file", {
        content: restoreResult.restored_content,
        outputPath,
        originalPath: restoreResult.file_path || null,
      });

      toast.success(
        (ti) => <ExportToast toastId={ti.id} outputPath={outputPath} label={t("result.restoreExportSuccess")} />,
        { duration: 8000 }
      );
    } catch (e) {
      toast.error(typeof e === "string" ? e : t("result.exportFailed"));
    } finally {
      setExporting(false);
    }
  }, [restoreResult]);

  // 返回
  const handleGoBack = useCallback(async () => {
    const store = useWorkspaceStore.getState();

    if (centerView === "restore") {
      setRestoreResult(null);
      setCenterView("dropzone");
      return;
    }

    // 批量模式：返回 = 跳过当前文件
    if (store.isBatchMode() && store.hasUnfinishedFiles()) {
      const currentFile = store.fileQueue[store.activeQueueIndex];
      if (currentFile && currentFile.status === "processing") {
        store.updateQueueFileStatus(currentFile.id, "failed", "skipped");
      }
      setCurrentResult(null);

      const nextFile = store.advanceToNextFile();
      if (nextFile) {
        await processFile(nextFile.filePath);
        return;
      } else {
        toast.success(t("fileQueue.allDone"));
      }
    }

    setCurrentResult(null);
    setCenterView("dropzone");
  }, [centerView, setRestoreResult, setCurrentResult, setCenterView, processFile]);

  /** 工具栏中间区域 */
  const renderToolbarCenter = () => {
    switch (centerView) {
      case "empty":
        return null;

      case "dropzone":
        return (
          <div className="flex-1 min-w-0 flex items-center justify-center gap-2">
            <span className="text-sm font-medium text-slate-600 truncate">
              {wsData?.workspace.name}
            </span>
          </div>
        );

      case "processing":
        return (
          <div className="flex-1 min-w-0 flex items-center justify-center gap-2">
            <span className="text-sm text-slate-500 truncate">
              {processingFileName && (
                <span className="font-medium text-slate-600 mr-2">{processingFileName}</span>
              )}
              {STEP_KEYS[processingStep] ? t(STEP_KEYS[processingStep]) : t("preview.processing")}
            </span>
          </div>
        );

      case "comparison": {
        const activeRecordId = useWorkspaceStore.getState().activeRecordId;
        const record = activeRecordId && wsData
          ? wsData.history.find((r) => r.id === activeRecordId)
          : null;
        const isHistoryMode = !!record && !currentResult;

        const displayName = isHistoryMode ? record.file_name : currentFileContent?.file_name;
        const displayCount = isHistoryMode ? record.sensitive_count : filteredCount;

        return (
          <>
            <div className="flex items-center gap-2 min-w-0">
              <button
                onClick={handleGoBack}
                data-testid="btn-back"
                className="p-1.5 rounded-md text-slate-500 hover:text-slate-700 hover:bg-slate-100 transition-colors"
                title={t("common.back")}
              >
                <ArrowLeft className="w-4 h-4" />
              </button>
              <span className="text-sm text-slate-700 truncate">
                <span className="font-medium">{displayName}</span>
                <span className="text-slate-400 mx-1.5">·</span>
                <span className="text-primary-600">{t("result.totalItems", { count: displayCount })}</span>
              </span>
            </div>
            <div className="flex-1" />
          </>
        );
      }

      case "restore":
        return (
          <>
            <div className="flex items-center gap-2 min-w-0">
              <button
                onClick={handleGoBack}
                data-testid="btn-back"
                className="p-1.5 rounded-md text-slate-500 hover:text-slate-700 hover:bg-slate-100 transition-colors"
                title={t("common.back")}
              >
                <ArrowLeft className="w-4 h-4" />
              </button>
              <span className="text-sm text-slate-700 truncate">
                <span className="font-medium">{restoreResult?.restored_content.file_name}</span>
                <span className="text-slate-400 mx-1.5">·</span>
                <span className="text-emerald-600">{t("restorePage.totalRestored", { count: restoreResult?.matched_count ?? 0 })}</span>
              </span>
            </div>
            <div className="flex-1" />
          </>
        );

      default:
        return null;
    }
  };

  /** 工具栏右侧操作按钮 */
  const renderToolbarActions = () => {
    // 全自动模式处理中：隐藏导出按钮（进度条自己管理中止）
    if (batchSession?.mode === "auto") {
      if (batchSession.phase === "running") return null;
      // 已完成 + 进入 comparison（抽查）：只读徽标，无导出
      if (batchSession.phase === "finished" && centerView === "comparison") {
        return (
          <span className="text-xs text-slate-400 px-2" data-testid="viewonly-badge">
            {t("fileQueue.batchMode.viewOnly")}
          </span>
        );
      }
    }
    if (centerView === "comparison" && (currentResult || (isTemplateMode && currentFileContent))) {
      const batchMode = isBatchMode();
      const isLastFile = batchMode && !fileQueue.some((f, i) => i > activeQueueIndex && f.status === "pending");

      if (batchMode && !isLastFile) {
        return (
          <div className="flex items-center gap-2">
            <button
              onClick={handleExportOnly}
              disabled={exporting}
              data-testid="btn-export-only"
              className="inline-flex items-center gap-1.5 px-3 py-1.5 bg-white text-slate-600 text-xs font-medium rounded-md border border-slate-300 hover:bg-slate-50 shadow-sm transition-colors disabled:opacity-50"
            >
              <Download className="w-3.5 h-3.5" />
              {exporting ? t("common.loading") : t("common.export")}
            </button>
            <button
              onClick={handleExportAndNext}
              disabled={exporting}
              data-testid="btn-export-next"
              className="inline-flex items-center gap-1.5 px-3 py-1.5 bg-primary-600 text-white text-xs font-medium rounded-md hover:bg-primary-700 shadow-sm transition-colors disabled:opacity-50"
            >
              <SkipForward className="w-3.5 h-3.5" />
              {exporting ? t("common.loading") : t("common.export")}
            </button>
          </div>
        );
      }

      return (
        <button
          onClick={batchMode ? handleExportOnly : handleComparisonExport}
          disabled={exporting}
          data-testid="btn-export"
          className="inline-flex items-center gap-1.5 px-3 py-1.5 bg-primary-600 text-white text-xs font-medium rounded-md hover:bg-primary-700 shadow-sm transition-colors disabled:opacity-50"
        >
          <Download className="w-3.5 h-3.5" />
          {exporting ? t("common.loading") : t("common.export")}
        </button>
      );
    }
    if (centerView === "restore") {
      return (
        <button
          onClick={handleRestoreExport}
          disabled={exporting}
          data-testid="btn-export-restore"
          className="inline-flex items-center gap-1.5 px-3 py-1.5 bg-emerald-600 text-white text-xs font-medium rounded-md hover:bg-emerald-700 shadow-sm transition-colors disabled:opacity-50"
        >
          <Download className="w-3.5 h-3.5" />
          {exporting ? t("common.loading") : t("common.export")}
        </button>
      );
    }
    return null;
  };

  const showRightControls = activeWorkspaceId && centerView !== "empty";

  return (
    <div className="h-screen flex bg-slate-50">
      {/* 左栏：通顶，可收缩 */}
      <aside
        className={`border-r border-slate-200 bg-white flex flex-col shrink-0 overflow-hidden transition-[width] duration-300 ease-in-out ${
          leftOpen ? "w-60" : "w-0 border-r-0"
        }`}
      >
        <div className="w-60 h-full flex flex-col">
          <WorkspaceList />
        </div>
      </aside>

      {/* 中间区域 */}
      <main className="flex-1 flex flex-col min-w-0">
        {/* 中间顶部工具栏 */}
        <div className="h-12 border-b border-slate-200 bg-white/80 backdrop-blur-sm flex items-center px-3 shrink-0 gap-2">
          {/* 左栏切换按钮 */}
          <button
            onClick={toggleLeft}
            className={`p-1.5 rounded-md transition-colors shrink-0 ${
              leftOpen
                ? "text-slate-500 hover:text-slate-700 hover:bg-slate-100"
                : "text-primary-500 hover:text-primary-600 hover:bg-primary-50"
            }`}
            title={`${t("sidebar.toggleLeft")} (⌘⇧L)`}
          >
            <PanelLeft className="w-4 h-4" />
          </button>

          {/* 中间：按 centerView 渲染不同内容 */}
          {renderToolbarCenter()}

          {/* 右侧：操作按钮 + 右栏切换 */}
          <div className="flex items-center gap-2 shrink-0">
            {showRightControls && (
              <>
                {renderToolbarActions()}
                <button
                  onClick={toggleRight}
                  className={`p-1.5 rounded-md transition-colors ${
                    rightOpen
                      ? "text-slate-500 hover:text-slate-700 hover:bg-slate-100"
                      : "text-primary-500 hover:text-primary-600 hover:bg-primary-50"
                  }`}
                  title={`${t("sidebar.toggleRight")} (⌘⇧R)`}
                >
                  <PanelRight className="w-4 h-4" />
                </button>
              </>
            )}
          </div>
        </div>

        {/* 中栏：主操作区 */}
        <div className="flex-1 flex flex-col min-h-0">
          <CenterPanel />
        </div>
      </main>

      {/* 右栏：通顶，可收缩 */}
      {activeWorkspaceId && (
        <aside
          className={`border-l border-slate-200 bg-white flex flex-col shrink-0 overflow-hidden transition-[width] duration-300 ease-in-out ${
            rightOpen ? "w-80" : "w-0 border-l-0"
          }`}
        >
          <div className="w-80 h-full flex flex-col">
            <StrategyPanel />
          </div>
        </aside>
      )}
    </div>
  );
}
