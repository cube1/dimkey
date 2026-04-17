import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { StopCircle } from "lucide-react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useBatchAutoProcess } from "../../hooks/useBatchAutoProcess";

export function BatchProgressBar() {
  const { t } = useTranslation();
  const fileQueue = useWorkspaceStore((s) => s.fileQueue);
  const batchSession = useWorkspaceStore((s) => s.batchSession);
  const { abortAll } = useBatchAutoProcess();

  const { done, total, remainingSeconds } = useMemo(() => {
    const total = fileQueue.length;
    const completed = fileQueue.filter(
      (f) => f.status === "confirmed" || f.status === "failed" || f.status === "aborted",
    );
    const done = completed.length;
    // 平均耗时 × 剩余数 / 并发数
    const timedFiles = completed.filter((f) => typeof f.durationMs === "number");
    const avg = timedFiles.length > 0
      ? timedFiles.reduce((s, f) => s + (f.durationMs || 0), 0) / timedFiles.length
      : 0;
    const remaining = total - done;
    const remainingMs = remaining > 0 && avg > 0 ? (remaining * avg) / 3 : 0; // MAX_CONCURRENCY = 3
    return { done, total, remainingSeconds: Math.round(remainingMs / 1000) };
  }, [fileQueue]);

  if (!batchSession || batchSession.phase !== "running") return null;

  const pct = total > 0 ? Math.round((done / total) * 100) : 0;

  const handleAbort = () => {
    if (window.confirm(t("fileQueue.batchMode.abortConfirm"))) {
      abortAll();
    }
  };

  return (
    <div className="bg-white border-b border-slate-200 px-4 py-2 shrink-0" data-testid="batch-progress">
      <div className="flex items-center gap-3">
        <button
          onClick={handleAbort}
          data-testid="btn-abort-batch"
          disabled={batchSession.aborted}
          className="inline-flex items-center gap-1 px-2 py-1 text-xs text-rose-600 border border-rose-300 rounded hover:bg-rose-50 disabled:opacity-50"
        >
          <StopCircle className="w-3.5 h-3.5" />
          {t("fileQueue.batchMode.abort")}
        </button>
        <div className="flex-1">
          <div className="flex items-center justify-between text-xs text-slate-600 mb-1">
            <span>{t("fileQueue.batchMode.inProgress")} · {done}/{total}</span>
            {remainingSeconds > 0 && (
              <span>{t("fileQueue.batchMode.remaining", { seconds: remainingSeconds })}</span>
            )}
          </div>
          <div className="h-1.5 bg-slate-100 rounded-full overflow-hidden">
            <div
              className="h-full bg-primary-500 transition-all"
              style={{ width: `${pct}%` }}
              data-testid="batch-progress-fill"
            />
          </div>
        </div>
      </div>
    </div>
  );
}
