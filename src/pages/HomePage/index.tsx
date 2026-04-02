import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { FileDropZone } from "../../components/FileDropZone";
import { useAppStore } from "../../stores/appStore";
import { useRestore } from "../../hooks/useRestore";
import type { TaskRecord } from "../../types";

export function HomePage() {
  const { t } = useTranslation();
  const setView = useAppStore((s) => s.setView);
  const { restoring, handleRestore } = useRestore();

  const [recentTasks, setRecentTasks] = useState<TaskRecord[]>([]);

  useEffect(() => {
    const load = async () => {
      try {
        const tasks = await invoke<TaskRecord[]>("list_tasks");
        setRecentTasks(tasks.slice(0, 3));
      } catch {
        // 静默处理，首页不影响主流程
      }
    };
    load();
  }, []);

  return (
    <div className="max-w-4xl mx-auto px-6 py-8">
      <div className="text-center mb-8">
        <p className="text-sm text-gray-500">
          {t("app.tagline")}
        </p>
      </div>

      <FileDropZone />

      {/* 最近脱敏任务 */}
      {recentTasks.length > 0 && (
        <div className="mt-8">
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-sm font-medium text-gray-600">{t("home.recentTasks")}</h2>
            <button
              onClick={() => setView("history")}
              className="text-xs text-blue-600 hover:text-blue-700"
            >
              {t("home.viewAll")}
            </button>
          </div>
          <div className="space-y-2">
            {recentTasks.map((task) => {
              const date = new Date(task.created_at);
              const dateStr = `${date.getMonth() + 1}/${date.getDate()} ${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
              return (
                <div
                  key={task.id}
                  className="flex items-center justify-between bg-white border border-gray-200 rounded-lg px-4 py-3"
                >
                  <div className="flex items-center gap-3 min-w-0">
                    <span className="text-sm text-gray-800 truncate">
                      {task.original_file_name}
                    </span>
                    <span className="text-xs text-gray-400 shrink-0">
                      {dateStr}
                    </span>
                    <span className="text-xs text-gray-400 shrink-0">
                      {t("home.items", { count: task.sensitive_count })}
                    </span>
                  </div>
                  <button
                    onClick={() => handleRestore(task)}
                    disabled={restoring === task.id}
                    className="text-xs text-blue-600 hover:text-blue-700 hover:bg-blue-50 px-2 py-1 rounded shrink-0 disabled:opacity-50"
                  >
                    {restoring === task.id ? t("home.restoring") : t("home.restore")}
                  </button>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
