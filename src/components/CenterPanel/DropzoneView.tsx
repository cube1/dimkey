import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Upload, FileText, Clock, Eye, RotateCcw, Trash2, ClipboardPaste, MessageSquareText } from "lucide-react";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useAutoDesensitize } from "../../hooks/useAutoDesensitize";
import { MAX_QUEUE_SIZE } from "../../types";
import type { QueueFile, RestoreResult, ProcessingRecord } from "../../types";

/** 支持的文件扩展名 */
const SUPPORTED_EXTENSIONS = [".xlsx", ".xls", ".csv", ".tsv", ".docx", ".txt", ".pdf"];

function getExtension(path: string): string {
  const dot = path.lastIndexOf(".");
  return dot >= 0 ? path.slice(dot).toLowerCase() : "";
}

function validateFile(filePath: string): string | null {
  const ext = getExtension(filePath);
  if (!SUPPORTED_EXTENSIONS.includes(ext)) {
    return "unsupported";
  }
  return null;
}

export function DropzoneView() {
  const { t } = useTranslation();
  const [isDragOver, setIsDragOver] = useState(false);
  const [workspaceRestoring, setWorkspaceRestoring] = useState(false);
  const [aiRestoring, setAiRestoring] = useState(false);
  const wsData = useWorkspaceStore((s) => s.activeWorkspaceData);
  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId);
  const refreshActiveWorkspace = useWorkspaceStore((s) => s.refreshActiveWorkspace);
  const setRestoreResult = useWorkspaceStore((s) => s.setRestoreResult);
  const setCenterView = useWorkspaceStore((s) => s.setCenterView);
  const initFileQueue = useWorkspaceStore((s) => s.initFileQueue);
  const isBatchMode = useWorkspaceStore((s) => s.isBatchMode);
  const hasUnfinishedFiles = useWorkspaceStore((s) => s.hasUnfinishedFiles);
  const { processFile, processClipboardText } = useAutoDesensitize();

  const handleWorkspaceRestore = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "AI处理后的文件",
          extensions: ["xlsx", "xls", "csv", "tsv", "docx", "txt", "pdf"],
        },
      ],
    });
    if (!selected || !activeWorkspaceId) return;
    setWorkspaceRestoring(true);
    try {
      const result = await invoke<RestoreResult>("restore_from_workspace", {
        workspaceId: activeWorkspaceId,
        filePath: selected,
      });
      setRestoreResult(result);
      setCenterView("restore");
    } catch (err) {
      toast.error(typeof err === "string" ? err : t("restorePage.restoreFailed"));
    }
    setWorkspaceRestoring(false);
  };

  const handleAiRestore = async () => {
    if (!activeWorkspaceId) return;
    setAiRestoring(true);
    try {
      const text = await navigator.clipboard.readText();
      if (!text.trim()) {
        toast.error(t("dict.clipboardEmpty"));
        setAiRestoring(false);
        return;
      }
      const result = await invoke<RestoreResult>("restore_ai_response", {
        workspaceId: activeWorkspaceId,
        aiText: text,
      });
      if (result.matched_count === 0) {
        toast(t("restorePage.noMatch"), { icon: "ℹ️" });
      } else {
        toast.success(t("restorePage.restored", { count: result.matched_count }));
      }
      setRestoreResult(result);
      setCenterView("restore");
    } catch (err) {
      toast.error(typeof err === "string" ? err : t("restorePage.restoreFailed"));
    }
    setAiRestoring(false);
  };

  const handlePasteText = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (!text.trim()) {
        toast.error(t("dict.clipboardEmpty"));
        return;
      }
      await processClipboardText(text);
    } catch {
      toast.error(t("dropzone.clipboardFailed"));
    }
  };

  const handleImportFile = useCallback(
    async (filePath: string) => {
      const error = validateFile(filePath);
      if (error) {
        toast.error(t("home.supportedFormatsLong"));
        return;
      }
      await processFile(filePath);
    },
    [processFile]
  );

  const handleImportFiles = useCallback(
    async (paths: string[]) => {
      // 全自动进行中：拒绝新文件
      const bsNow = useWorkspaceStore.getState().batchSession;
      if (bsNow?.phase === "running") {
        toast.error(t("fileQueue.batchMode.dropRejected"));
        return;
      }
      // 老的 fileQueue 仍有未完成 → 拒绝
      if (isBatchMode() && hasUnfinishedFiles() && !bsNow) {
        toast.error(t("dropzone.waitQueue"));
        return;
      }

      const validPaths: string[] = [];
      const invalidNames: string[] = [];
      for (const p of paths) {
        const error = validateFile(p);
        if (error) {
          invalidNames.push(p.split(/[/\\]/).pop() || p);
        } else {
          validPaths.push(p);
        }
      }

      if (invalidNames.length > 0) {
        toast.error(t("dropzone.formatNotSupported", { count: invalidNames.length }));
      }

      if (validPaths.length === 0) return;

      if (validPaths.length === 1) {
        await handleImportFile(validPaths[0]);
        return;
      }

      const filesToProcess = validPaths.slice(0, MAX_QUEUE_SIZE);
      if (validPaths.length > MAX_QUEUE_SIZE) {
        toast(t("dropzone.maxQueue", { max: MAX_QUEUE_SIZE }), { icon: "ℹ️" });
      }

      // 全部 pending，等模式选择器决定后续流程
      const queue: QueueFile[] = filesToProcess.map((p) => ({
        id: crypto.randomUUID(),
        filePath: p,
        fileName: p.split(/[/\\]/).pop() || p,
        status: "pending",
      }));

      initFileQueue(queue);
      // 此处不再直接调 processFile；由 CenterPanel 根据 fileQueue.length > 1 展示 BatchModeSelector
    },
    [handleImportFile, isBatchMode, hasUnfinishedFiles, initFileQueue, t],
  );

  // 监听 Tauri 拖放事件
  useEffect(() => {
    const webview = getCurrentWebview();
    const unlisten = webview.onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        setIsDragOver(true);
      } else if (event.payload.type === "leave") {
        setIsDragOver(false);
      } else if (event.payload.type === "drop") {
        setIsDragOver(false);
        const paths = event.payload.paths;
        if (paths.length > 0) {
          handleImportFiles(paths);
        }
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [handleImportFiles]);

  // 监听键盘粘贴事件（Ctrl/Cmd+V）
  useEffect(() => {
    const handlePaste = (e: ClipboardEvent) => {
      // 只处理纯文本粘贴（不含文件）
      if (e.clipboardData?.files?.length) return;
      const text = e.clipboardData?.getData("text/plain");
      if (text?.trim()) {
        e.preventDefault();
        processClipboardText(text);
      }
    };
    window.addEventListener("paste", handlePaste);
    return () => window.removeEventListener("paste", handlePaste);
  }, [processClipboardText]);

  const handleClickSelect = async () => {
    if (isBatchMode() && hasUnfinishedFiles()) {
      toast.error("请先完成当前队列中的文件处理");
      return;
    }

    const selected = await open({
      multiple: true,
      filters: [
        {
          name: "支持的文件",
          extensions: ["xlsx", "xls", "csv", "tsv", "docx", "txt", "pdf"],
        },
      ],
    });
    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      if (paths.length > 0) {
        await handleImportFiles(paths);
      }
    }
  };

  const history = wsData?.history || [];

  return (
    <div className="flex-1 flex flex-col min-h-0" data-testid="view-dropzone">
      {/* 拖放区域 */}
      <div className="p-6 shrink-0">
        <div
          onClick={handleClickSelect}
          className={`
            flex flex-col items-center justify-center
            w-full max-w-lg mx-auto h-52 border-2 border-dashed rounded-2xl
            transition-all cursor-pointer
            ${
              isDragOver
                ? "border-primary-500 bg-primary-50 ring-4 ring-primary-500/10 shadow-elevated scale-[1.01]"
                : "border-slate-300 bg-gradient-to-b from-white to-slate-50/50 hover:border-slate-400 hover:bg-white"
            }
          `}
        >
          <div className={`w-12 h-12 rounded-xl flex items-center justify-center mb-3 ${
            isDragOver
              ? "bg-primary-100"
              : "bg-gradient-to-br from-primary-50 to-slate-100"
          }`}>
            <Upload
              className={`w-6 h-6 transition-colors ${
                isDragOver ? "text-primary-500" : "text-slate-400"
              }`}
            />
          </div>
          <p className="text-base text-slate-600 font-medium">
            {isDragOver ? t("home.dropRelease") : t("home.dropHint")}
          </p>
          <p className="text-sm text-slate-400 mt-1">
            {t("home.selectFile")}
          </p>
          <p className="text-xs text-slate-400 mt-2">
            {t("home.supportedFormats")}
          </p>
          <button
            onClick={(e) => { e.stopPropagation(); handlePasteText(); }}
            className="mt-3 inline-flex items-center gap-1.5 px-3 py-1.5
                       bg-white border border-slate-200 rounded-lg
                       text-xs text-slate-500 hover:text-primary-600 hover:border-primary-300
                       transition-colors"
          >
            <ClipboardPaste className="w-3.5 h-3.5" />
            {t("home.pasteText")}
          </button>
        </div>
      </div>

      {/* 还原操作 */}
      {!(isBatchMode() && hasUnfinishedFiles()) && (
      <div className="px-6 pb-3 shrink-0">
        <div className="flex items-center gap-2 mb-2">
          <RotateCcw className="w-4 h-4 text-slate-400" />
          <h3 className="text-xs font-bold uppercase tracking-wider text-slate-500">{t("dropzone.restoreTitle")}</h3>
        </div>
        <div className="flex gap-2">
          <button
            onClick={handleAiRestore}
            disabled={aiRestoring}
            data-testid="btn-restore-ai"
            className="flex-1 py-2 px-3 bg-emerald-50 hover:bg-emerald-100
                       border border-emerald-300 rounded-lg
                       text-xs text-emerald-700 transition-colors disabled:opacity-50
                       flex items-center justify-center gap-1.5"
            title="将 AI 回复中的脱敏占位符还原为真实数据（推荐使用替换策略）"
          >
            <MessageSquareText className="w-3.5 h-3.5" />
            {aiRestoring ? t("home.restoring") : t("restorePage.pasteAiReply")}
          </button>
          <button
            onClick={handleWorkspaceRestore}
            disabled={workspaceRestoring}
            data-testid="btn-restore-workspace"
            className="flex-1 py-2 px-3 bg-emerald-50 hover:bg-emerald-100
                       border border-emerald-300 rounded-lg
                       text-xs text-emerald-700 transition-colors disabled:opacity-50
                       flex items-center justify-center gap-1.5"
            title="将 AI 处理后的文件还原为真实数据（合并本工作区所有映射）"
          >
            <FileText className="w-3.5 h-3.5" />
            {workspaceRestoring ? t("home.restoring") : t("restorePage.importRestore")}
          </button>
        </div>
      </div>
      )}

      {/* 处理历史 */}
      <div className="flex-1 overflow-auto px-6 pb-6">
        {history.length > 0 && (
          <>
            <div className="flex items-center gap-2 mb-3">
              <Clock className="w-4 h-4 text-slate-400" />
              <h3 className="text-xs font-bold uppercase tracking-wider text-slate-500">{t("dropzone.historyTitle")}</h3>
            </div>
            <div className="space-y-2">
              {history.map((record) => (
                <HistoryItem
                  key={record.id}
                  record={record}
                  workspaceId={activeWorkspaceId!}
                  onRefresh={refreshActiveWorkspace}
                />
              ))}
            </div>
          </>
        )}
      </div>

    </div>
  );
}

function HistoryItem({
  record,
  workspaceId,
  onRefresh,
}: {
  record: ProcessingRecord;
  workspaceId: string;
  onRefresh: () => Promise<void>;
}) {
  const { t, i18n } = useTranslation();
  const [deleting, setDeleting] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const viewRecord = useWorkspaceStore((s) => s.viewRecord);
  const setRestoreResult = useWorkspaceStore((s) => s.setRestoreResult);
  const setCenterView = useWorkspaceStore((s) => s.setCenterView);

  const handleDelete = async () => {
    setDeleting(true);
    try {
      await invoke("delete_processing_record", {
        workspaceId,
        recordId: record.id,
      });
      await onRefresh();
    } catch {
      toast.error(t("history.deleteFailed"));
    }
    setDeleting(false);
  };

  const handleRestore = async () => {
    // 选择要还原的文件
    const selected = await open({
      multiple: false,
      filters: [{ name: "脱敏后文件", extensions: ["xlsx", "xls", "csv", "tsv", "docx", "txt", "pdf"] }],
    });
    if (!selected) return;

    setRestoring(true);
    try {
      const result = await invoke<RestoreResult>("restore_processing", {
        workspaceId,
        recordId: record.id,
        filePath: selected,
      });
      setRestoreResult(result);
      setCenterView("restore");
    } catch (err) {
      toast.error(typeof err === "string" ? err : t("restorePage.restoreFailed"));
    }
    setRestoring(false);
  };

  const formatTime = (iso: string) => {
    try {
      const d = new Date(iso);
      const locale = i18n.language.startsWith("en") ? "en-US" : "zh-CN";
      return d.toLocaleString(locale, {
        month: "numeric",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
    } catch {
      return "";
    }
  };

  return (
    <div className="flex items-center gap-3 px-3 py-2.5 bg-white rounded-lg border border-slate-100 hover:border-slate-200 hover:shadow-xs transition-all group">
      <FileText className="w-4 h-4 text-slate-400 shrink-0" />
      <div className="flex-1 min-w-0">
        <div className="text-sm text-slate-700 truncate">{record.file_name}</div>
        <div className="text-xs text-slate-400">
          {formatTime(record.processed_at)} · {t("history.sensitiveCount", { count: record.sensitive_count })}
          {record.status === "Restored" && (
            <span className="ml-1 text-emerald-500">{t("history.restored")}</span>
          )}
        </div>
      </div>
      <div className="flex items-center gap-1 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
        <button
          onClick={() => viewRecord(record.id)}
          className="p-1 text-slate-400 hover:text-primary-500 rounded transition-colors"
          title={t("history.viewComparison")}
        >
          <Eye className="w-3.5 h-3.5" />
        </button>
        <button
          onClick={handleRestore}
          disabled={restoring}
          className="p-1 text-slate-400 hover:text-emerald-500 rounded transition-colors disabled:opacity-50"
          title={t("home.restore")}
        >
          <RotateCcw className="w-3.5 h-3.5" />
        </button>
        <button
          onClick={handleDelete}
          disabled={deleting}
          className="p-1 text-slate-400 hover:text-rose-500 rounded transition-colors disabled:opacity-50"
          title={t("history.deleteRecord")}
        >
          <Trash2 className="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  );
}
