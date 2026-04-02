import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useAppStore } from "../stores/appStore";
import type { TaskRecord, RestoreResult } from "../types";

/** 还原流程 hook：选文件 → restore_file → 跳转还原页 */
export function useRestore() {
  const { t } = useTranslation();
  const setView = useAppStore((s) => s.setView);
  const setRestoreResult = useAppStore((s) => s.setRestoreResult);
  const [restoring, setRestoring] = useState<string | null>(null);

  const handleRestore = useCallback(async (task: TaskRecord) => {
    try {
      const filePath = await open({
        multiple: false,
        filters: [
          { name: t("restore.supportedFiles"), extensions: ["xlsx", "xls", "csv", "tsv", "docx", "txt", "pdf"] },
        ],
      });
      if (!filePath) return;

      setRestoring(task.id);
      const result = await invoke<RestoreResult>("restore_file", {
        taskId: task.id,
        filePath,
      });

      setRestoreResult(result);
      setView("restore");
    } catch (e) {
      toast.error(typeof e === "string" ? e : t("restore.restoreFailed"));
    } finally {
      setRestoring(null);
    }
  }, [setRestoreResult, setView, t]);

  return { restoring, handleRestore };
}
