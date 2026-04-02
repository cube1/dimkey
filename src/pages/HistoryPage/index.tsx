import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Dialog, DialogPanel, DialogTitle, Transition, TransitionChild } from "@headlessui/react";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useRestore } from "../../hooks/useRestore";
import type { TaskRecord } from "../../types";
import { SENSITIVE_TYPE_CONFIG, getSensitiveTypeKey } from "../../types";

export function HistoryPage() {
  const { t } = useTranslation();
  const { restoring, handleRestore } = useRestore();

  const [tasks, setTasks] = useState<TaskRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<TaskRecord | null>(null);

  // 加载任务列表
  useEffect(() => {
    const load = async () => {
      try {
        const list = await invoke<TaskRecord[]>("list_tasks");
        setTasks(list);
      } catch (e) {
        toast.error(typeof e === "string" ? e : t("history.loadFailed"));
      } finally {
        setLoading(false);
      }
    };
    load();
  }, []);

  // 删除任务
  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await invoke("delete_task", { taskId: deleteTarget.id });
      setTasks((prev) => prev.filter((t) => t.id !== deleteTarget.id));
      toast.success(t("history.deleted"));
    } catch (e) {
      toast.error(typeof e === "string" ? e : t("history.deleteFailed"));
    } finally {
      setDeleteTarget(null);
    }
  };

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center text-gray-400">
        {t("common.loading")}
      </div>
    );
  }

  if (tasks.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-gray-400 gap-2">
        <svg className="w-16 h-16 text-gray-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
        </svg>
        <p>{t("history.empty")}</p>
        <p className="text-sm">{t("history.emptyHint")}</p>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-3xl mx-auto space-y-3">
          {tasks.map((task) => (
            <TaskCard
              key={task.id}
              task={task}
              expanded={expandedId === task.id}
              restoring={restoring === task.id}
              onToggleExpand={() =>
                setExpandedId(expandedId === task.id ? null : task.id)
              }
              onDelete={() => setDeleteTarget(task)}
              onRestore={() => handleRestore(task)}
            />
          ))}
        </div>
      </div>

      {/* 删除确认弹窗 */}
      <Transition appear show={deleteTarget !== null}>
        <Dialog
          as="div"
          className="relative z-50"
          onClose={() => setDeleteTarget(null)}
        >
          <TransitionChild
            enter="ease-out duration-200"
            enterFrom="opacity-0"
            enterTo="opacity-100"
            leave="ease-in duration-150"
            leaveFrom="opacity-100"
            leaveTo="opacity-0"
          >
            <div className="fixed inset-0 bg-black/25" />
          </TransitionChild>

          <div className="fixed inset-0 flex items-center justify-center p-4">
            <TransitionChild
              enter="ease-out duration-200"
              enterFrom="opacity-0 scale-95"
              enterTo="opacity-100 scale-100"
              leave="ease-in duration-150"
              leaveFrom="opacity-100 scale-100"
              leaveTo="opacity-0 scale-95"
            >
              <DialogPanel className="bg-white rounded-xl shadow-xl p-6 w-full max-w-sm">
                <DialogTitle className="text-lg font-semibold text-gray-800">
                  {t("common.confirm")}
                </DialogTitle>
                <p className="mt-2 text-sm text-gray-600">
                  {t("history.deleteRecord")}?
                </p>
                <div className="mt-4 flex justify-end gap-3">
                  <button
                    onClick={() => setDeleteTarget(null)}
                    className="px-4 py-2 text-sm text-gray-600 hover:bg-gray-100 rounded-lg"
                  >
                    {t("common.cancel")}
                  </button>
                  <button
                    onClick={handleDelete}
                    className="px-4 py-2 text-sm text-white bg-red-600 hover:bg-red-700 rounded-lg"
                  >
                    {t("common.delete")}
                  </button>
                </div>
              </DialogPanel>
            </TransitionChild>
          </div>
        </Dialog>
      </Transition>
    </div>
  );
}

/** 单个任务卡片 */
function TaskCard({
  task,
  expanded,
  restoring,
  onToggleExpand,
  onDelete,
  onRestore,
}: {
  task: TaskRecord;
  expanded: boolean;
  restoring: boolean;
  onToggleExpand: () => void;
  onDelete: () => void;
  onRestore: () => void;
}) {
  const { t } = useTranslation();
  const date = new Date(task.created_at);
  const dateStr = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")} ${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;

  return (
    <div className="bg-white rounded-lg border border-gray-200 overflow-hidden">
      {/* 卡片主体 */}
      <div className="px-5 py-4">
        <div className="flex items-start justify-between">
          <div className="flex-1 min-w-0">
            <h3 className="text-sm font-medium text-gray-800 truncate">
              {task.original_file_name}
            </h3>
            <p className="text-xs text-gray-400 mt-1">{dateStr}</p>
          </div>
          <div className="flex items-center gap-2 ml-4 shrink-0">
            <span className="text-xs text-gray-500">
              {t("history.sensitiveCount", { count: task.sensitive_count })} · {task.replaced_count}
            </span>
          </div>
        </div>

        {/* 操作按钮 */}
        <div className="flex items-center gap-2 mt-3">
          <button
            onClick={onRestore}
            disabled={restoring}
            className="px-3 py-1.5 text-xs font-medium text-blue-600 bg-blue-50 hover:bg-blue-100 rounded-md transition-colors disabled:opacity-50"
          >
            {restoring ? t("home.restoring") : t("home.restore")}
          </button>
          <button
            onClick={onToggleExpand}
            className="px-3 py-1.5 text-xs text-gray-500 hover:bg-gray-100 rounded-md transition-colors"
          >
            {expanded ? t("common.close") : t("history.viewComparison")}
          </button>
          <button
            onClick={onDelete}
            className="px-3 py-1.5 text-xs text-red-500 hover:bg-red-50 rounded-md transition-colors ml-auto"
          >
            {t("common.delete")}
          </button>
        </div>
      </div>

      {/* 展开的映射摘要 */}
      {expanded && (
        <div className="border-t border-gray-100 px-5 py-3 bg-gray-50">
          <div className="space-y-1.5 max-h-60 overflow-auto">
            {task.mappings.map((m, i) => {
              const typeKey = getSensitiveTypeKey(m.sensitive_type);
              const info = SENSITIVE_TYPE_CONFIG[typeKey];
              return (
                <div
                  key={i}
                  className="flex items-center gap-3 text-xs"
                >
                  <span
                    className={`shrink-0 px-1.5 py-0.5 rounded ${info?.bgClass ?? "bg-gray-100"} ${info?.textClass ?? "text-gray-600"}`}
                  >
                    {info?.label ?? typeKey}
                  </span>
                  <span className="text-gray-800 truncate">{m.original_text}</span>
                  <span className="text-gray-400">→</span>
                  <span className="text-gray-600 truncate">{m.replaced_text}</span>
                  {m.occurrences > 1 && (
                    <span className="text-gray-400">×{m.occurrences}</span>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
