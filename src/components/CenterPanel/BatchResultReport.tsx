import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { CheckCircle2, XCircle, Ban, FolderOpen, FileText, RotateCcw, Eye } from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import toast from "react-hot-toast";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useBatchAutoProcess } from "../../hooks/useBatchAutoProcess";
import type { QueueFile } from "../../types";

const STATUS_ICON: Record<QueueFile["status"], { Icon: typeof CheckCircle2; className: string }> = {
  pending:    { Icon: FileText,     className: "text-slate-400" },
  processing: { Icon: FileText,     className: "text-primary-500" },
  confirmed:  { Icon: CheckCircle2, className: "text-emerald-500" },
  failed:     { Icon: XCircle,      className: "text-rose-500" },
  aborted:    { Icon: Ban,          className: "text-slate-400" },
};

export function BatchResultReport() {
  const { t } = useTranslation();
  const fileQueue = useWorkspaceStore((s) => s.fileQueue);
  const batchSession = useWorkspaceStore((s) => s.batchSession);
  const clearBatchSession = useWorkspaceStore((s) => s.clearBatchSession);
  const setCurrentResult = useWorkspaceStore((s) => s.setCurrentResult);
  const setCurrentFileContent = useWorkspaceStore((s) => s.setCurrentFileContent);
  const setCurrentRecordId = useWorkspaceStore((s) => s.setCurrentRecordId);
  const setCenterView = useWorkspaceStore((s) => s.setCenterView);
  const { retryFile } = useBatchAutoProcess();

  const stats = useMemo(() => ({
    success: fileQueue.filter((f) => f.status === "confirmed").length,
    failed:  fileQueue.filter((f) => f.status === "failed").length,
    aborted: fileQueue.filter((f) => f.status === "aborted").length,
  }), [fileQueue]);

  if (!batchSession || batchSession.phase !== "finished") return null;

  const handleOpenDir = async () => {
    if (batchSession.outputDir) {
      await openPath(batchSession.outputDir);
    }
  };

  const handleView = (file: QueueFile) => {
    if (!file.result || !file.recordId) return;
    setCurrentFileContent(file.result.content, file.filePath);
    setCurrentResult(file.result);
    setCurrentRecordId(file.recordId);
    setCenterView("comparison");
    toast(t("fileQueue.batchMode.viewOnly"), { icon: "👁" });
  };

  return (
    <div className="flex-1 overflow-auto p-6" data-testid="batch-result-report">
      <div className="max-w-2xl mx-auto">
        <h2 className="text-lg font-semibold text-slate-800 mb-2">
          {t("fileQueue.batchMode.reportTitle")}
        </h2>
        <p className="text-sm text-slate-600 mb-1">
          {t("fileQueue.batchMode.reportSummary", stats)}
        </p>
        {batchSession.outputDir && (
          <div className="flex items-center gap-2 mb-5">
            <p className="text-xs text-slate-500">
              {t("fileQueue.batchMode.reportOutputDir", { dir: batchSession.outputDir })}
            </p>
            <button
              onClick={handleOpenDir}
              data-testid="btn-open-output-dir"
              className="inline-flex items-center gap-1 px-2 py-0.5 text-xs text-primary-600 border border-primary-300 rounded hover:bg-primary-50"
            >
              <FolderOpen className="w-3 h-3" />
              {t("fileQueue.batchMode.openDir")}
            </button>
          </div>
        )}

        <div className="space-y-1.5">
          {fileQueue.map((file) => {
            const { Icon, className } = STATUS_ICON[file.status];
            return (
              <div
                key={file.id}
                data-testid={`result-row-${file.status}`}
                className="flex items-center gap-3 p-3 bg-white border border-slate-200 rounded-lg"
              >
                <Icon className={`w-4 h-4 shrink-0 ${className}`} />
                <div className="flex-1 min-w-0">
                  <div className="text-sm text-slate-700 truncate">{file.fileName}</div>
                  <div className="text-xs text-slate-500 truncate">
                    {file.status === "confirmed" && (
                      <>敏感项 {file.sensitiveCount ?? 0} · {file.outputPath ?? ""}</>
                    )}
                    {file.status === "failed" && (file.errorMessage || "处理失败")}
                    {file.status === "aborted" && t("fileQueue.batchMode.abort")}
                  </div>
                </div>
                {file.status === "confirmed" && file.result && file.recordId && (
                  <button
                    onClick={() => handleView(file)}
                    data-testid="btn-view-result"
                    className="p-1.5 text-slate-400 hover:text-primary-500 rounded"
                    title={t("fileQueue.batchMode.viewOnly")}
                  >
                    <Eye className="w-4 h-4" />
                  </button>
                )}
                {file.status === "failed" && (
                  <button
                    onClick={() => retryFile(file.id)}
                    data-testid="btn-retry-file"
                    className="inline-flex items-center gap-1 px-2 py-1 text-xs text-primary-600 border border-primary-300 rounded hover:bg-primary-50"
                  >
                    <RotateCcw className="w-3 h-3" />
                    {t("fileQueue.batchMode.retry")}
                  </button>
                )}
              </div>
            );
          })}
        </div>

        <div className="mt-6 flex justify-end">
          <button
            onClick={() => { clearBatchSession(); setCenterView("dropzone"); }}
            data-testid="btn-close-report"
            className="px-4 py-2 bg-white border border-slate-300 rounded-md text-sm text-slate-700 hover:bg-slate-50"
          >
            {t("fileQueue.batchMode.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
