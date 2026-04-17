import { useEffect, useRef } from "react";
import { CheckCircle2, XCircle, Circle, Loader2 } from "lucide-react";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import type { QueueFile } from "../../types";

const STATUS_CONFIG: Record<QueueFile["status"], {
  icon: typeof Circle;
  colorClass: string;
  bgClass: string;
  borderClass: string;
}> = {
  pending: {
    icon: Circle,
    colorClass: "text-slate-400",
    bgClass: "bg-white",
    borderClass: "border-slate-200",
  },
  processing: {
    icon: Loader2,
    colorClass: "text-primary-600",
    bgClass: "bg-primary-50",
    borderClass: "border-primary-300",
  },
  confirmed: {
    icon: CheckCircle2,
    colorClass: "text-emerald-600",
    bgClass: "bg-emerald-50",
    borderClass: "border-emerald-200",
  },
  failed: {
    icon: XCircle,
    colorClass: "text-rose-500",
    bgClass: "bg-rose-50",
    borderClass: "border-rose-200",
  },
  aborted: {
    icon: Circle,
    colorClass: "text-slate-400",
    bgClass: "bg-slate-100",
    borderClass: "border-slate-200",
  },
};

export function FileQueueTabs() {
  const { t } = useTranslation();
  const fileQueue = useWorkspaceStore((s) => s.fileQueue);
  const activeQueueIndex = useWorkspaceStore((s) => s.activeQueueIndex);
  const batchSession = useWorkspaceStore((s) => s.batchSession);
  const setCurrentResult = useWorkspaceStore((s) => s.setCurrentResult);
  const setCurrentFileContent = useWorkspaceStore((s) => s.setCurrentFileContent);
  const setCurrentRecordId = useWorkspaceStore((s) => s.setCurrentRecordId);
  const setCenterView = useWorkspaceStore((s) => s.setCenterView);
  const scrollRef = useRef<HTMLDivElement>(null);
  const activeTabRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (activeTabRef.current) {
      activeTabRef.current.scrollIntoView({ behavior: "smooth", block: "nearest", inline: "center" });
    }
  }, [activeQueueIndex]);

  if (fileQueue.length <= 1) return null;

  const handleTabClick = (file: QueueFile) => {
    const isAutoMode = batchSession?.mode === "auto";
    switch (file.status) {
      case "confirmed":
        if (isAutoMode && file.result && file.recordId) {
          setCurrentFileContent(file.result.content, file.filePath);
          setCurrentResult(file.result);
          setCurrentRecordId(file.recordId);
          setCenterView("comparison");
          toast(t("fileQueue.batchMode.viewOnly"), { icon: "👁" });
        } else {
          toast(t("fileQueue.exported"), { icon: "✓" });
        }
        break;
      case "failed":
        toast.error(file.errorMessage || t("fileQueue.failed"));
        break;
      case "aborted":
        toast(t("fileQueue.batchMode.abort"), { icon: "ℹ️" });
        break;
      case "pending":
        toast(t("fileQueue.waitOrder"), { icon: "ℹ️" });
        break;
      case "processing":
        break;
    }
  };

  const doneCount = fileQueue.filter((f) => f.status === "confirmed" || f.status === "failed").length;

  return (
    <div className="bg-white border-b border-slate-200 px-3 py-1.5 shrink-0" data-testid="file-queue">
      <div className="flex items-center gap-2">
        <span className="text-xs text-slate-400 shrink-0">
          {doneCount}/{fileQueue.length}
        </span>
        <div ref={scrollRef} className="flex items-center gap-1 overflow-x-auto flex-1 min-w-0">
          {fileQueue.map((file, idx) => {
            const config = STATUS_CONFIG[file.status];
            const Icon = config.icon;
            const isActive = idx === activeQueueIndex;

            return (
              <button
                key={file.id}
                ref={isActive ? activeTabRef : undefined}
                onClick={() => handleTabClick(file)}
                className={`
                  inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs
                  border transition-all whitespace-nowrap shrink-0
                  ${config.bgClass} ${config.borderClass}
                  ${isActive ? "ring-1 ring-primary-300 shadow-sm" : ""}
                  ${file.status === "pending" || file.status === "aborted" ? "opacity-60" : "cursor-pointer hover:shadow-xs"}
                `}
              >
                <Icon className={`w-3.5 h-3.5 ${config.colorClass} ${file.status === "processing" ? "animate-spin" : ""}`} />
                <span className={`truncate max-w-[120px] ${isActive ? "font-medium text-slate-700" : "text-slate-500"}`}>
                  {file.fileName}
                </span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
