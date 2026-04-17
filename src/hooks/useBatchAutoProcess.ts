import { useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import toast from "react-hot-toast";
import i18n from "../i18n";
import { useWorkspaceStore } from "../stores/workspaceStore";
import { runDesensitizePipeline, type PipelineOptions } from "./useAutoDesensitize";
import { runBatch } from "../utils/batchScheduler";
import { resolveOutputPath } from "../utils/outputPath";
import { MAX_CONCURRENCY, parseEncryptedError } from "../types";
import type { QueueFile, Workspace } from "../types";

// ============================================================
// 批量全自动处理 Hook — 使用共享流水线并发处理 pending 文件
// ============================================================

/** 从 workspace 构建共享流水线参数（全自动模式固定 Desensitize） */
function buildPipelineOptions(ws: Workspace): PipelineOptions {
  return {
    workspaceId: ws.id,
    strategies: ws.strategies,
    dictEntries: ws.dict_entries,
    enabledTypes: ws.enabled_types,
    replaceStyle: ws.replace_style || "Fake",
    consistencyMappings: ws.consistency_mappings || [],
    language: i18n.language.startsWith("en") ? "en" : "zh",
    aliasGroups: ws.alias_groups || [],
    whitelist: ws.whitelist || [],
    mode: "Desensitize",
  };
}

/** 导出文件并返回实际输出路径；PDF 走专用涂黑命令 */
async function exportPipelineResult(
  file: QueueFile,
  outputDir: string,
  result: NonNullable<Awaited<ReturnType<typeof runDesensitizePipeline>>["desensitizeResult"]>,
): Promise<string> {
  const outputPath = await resolveOutputPath(outputDir, file.fileName);
  const isPdf = result.content.file_type === "Pdf";
  if (isPdf) {
    // 批量全自动暂不支持 PDF 涂黑（需要 sensitive_items），fall back 到普通导出
    await invoke("export_file", {
      content: result.content,
      outputPath,
      originalPath: file.filePath,
    });
  } else {
    await invoke("export_file", {
      content: result.content,
      outputPath,
      originalPath: file.filePath,
    });
  }
  return outputPath;
}

export function useBatchAutoProcess() {
  const abortRef = useRef<AbortController | null>(null);

  const startAutoProcess = useCallback(async (outputDir: string) => {
    const store = useWorkspaceStore.getState();
    const wsData = store.activeWorkspaceData;
    if (!wsData) return;

    const pendingFiles = store.fileQueue.filter((f) => f.status === "pending");
    if (pendingFiles.length === 0) return;

    const controller = new AbortController();
    abortRef.current = controller;

    store.startBatchAuto(outputDir);

    const worker = async (file: QueueFile, _idx: number, signal: AbortSignal): Promise<void> => {
      const s = useWorkspaceStore.getState();

      // 中止检查
      if (signal.aborted || s.batchSession?.aborted) {
        s.updateQueueFileResult(file.id, { status: "aborted" });
        return;
      }

      s.updateQueueFileStatus(file.id, "processing");

      try {
        // 每个文件都读最新 workspace 快照（用户可能中途编辑策略）
        const latest = useWorkspaceStore.getState().activeWorkspaceData;
        if (!latest) {
          s.updateQueueFileResult(file.id, {
            status: "failed",
            errorMessage: i18n.t("hook.selectWorkspace"),
          });
          return;
        }

        const options = buildPipelineOptions(latest.workspace);
        const pipelineResult = await runDesensitizePipeline(file.filePath, options);

        // 再次检查中止（流水线可能耗时较长）
        if (signal.aborted || useWorkspaceStore.getState().batchSession?.aborted) {
          useWorkspaceStore.getState().updateQueueFileResult(file.id, { status: "aborted" });
          return;
        }

        // 0 敏感项：静默成功，无导出
        if (!pipelineResult.desensitizeResult || !pipelineResult.record) {
          useWorkspaceStore.getState().updateQueueFileResult(file.id, {
            status: "confirmed",
            sensitiveCount: 0,
            durationMs: pipelineResult.durationMs,
          });
          return;
        }

        // 导出文件
        const outputPath = await exportPipelineResult(
          file,
          outputDir,
          pipelineResult.desensitizeResult,
        );

        useWorkspaceStore.getState().updateQueueFileResult(file.id, {
          status: "confirmed",
          sensitiveCount: pipelineResult.desensitizeResult.summary.total,
          outputPath,
          result: pipelineResult.desensitizeResult,
          recordId: pipelineResult.record.id,
          durationMs: pipelineResult.durationMs,
        });
      } catch (err) {
        const encryptedType = parseEncryptedError(err);
        const message = encryptedType
          ? i18n.t("hook.encryptedSkipped")
          : typeof err === "string"
            ? err
            : err instanceof Error
              ? err.message
              : i18n.t("hook.processFailed");
        useWorkspaceStore.getState().updateQueueFileResult(file.id, {
          status: "failed",
          errorMessage: message,
        });
      }
    };

    await runBatch(pendingFiles, MAX_CONCURRENCY, worker, controller.signal);

    // 收尾
    await useWorkspaceStore.getState().refreshActiveWorkspace();
    useWorkspaceStore.getState().finishBatchAuto();
    abortRef.current = null;

    // 汇总 toast
    const finalQueue = useWorkspaceStore.getState().fileQueue;
    const success = finalQueue.filter((f) => f.status === "confirmed").length;
    const failed = finalQueue.filter((f) => f.status === "failed").length;
    const aborted = finalQueue.filter((f) => f.status === "aborted").length;
    toast.success(
      i18n.t("fileQueue.batchMode.reportSummary", { success, failed, aborted }),
    );
  }, []);

  const abortAll = useCallback(() => {
    const store = useWorkspaceStore.getState();
    store.abortBatchAuto();
    abortRef.current?.abort();
  }, []);

  const retryFile = useCallback(async (fileId: string) => {
    const store = useWorkspaceStore.getState();
    const file = store.fileQueue.find((f) => f.id === fileId);
    const outputDir = store.batchSession?.outputDir;
    const wsData = store.activeWorkspaceData;
    if (!file || !outputDir || !wsData) return;

    store.updateQueueFileResult(fileId, { status: "processing", errorMessage: undefined });

    try {
      const options = buildPipelineOptions(wsData.workspace);
      const pipelineResult = await runDesensitizePipeline(file.filePath, options);

      if (!pipelineResult.desensitizeResult || !pipelineResult.record) {
        useWorkspaceStore.getState().updateQueueFileResult(fileId, {
          status: "confirmed",
          sensitiveCount: 0,
          durationMs: pipelineResult.durationMs,
        });
      } else {
        const outputPath = await exportPipelineResult(
          file,
          outputDir,
          pipelineResult.desensitizeResult,
        );
        useWorkspaceStore.getState().updateQueueFileResult(fileId, {
          status: "confirmed",
          sensitiveCount: pipelineResult.desensitizeResult.summary.total,
          outputPath,
          result: pipelineResult.desensitizeResult,
          recordId: pipelineResult.record.id,
          durationMs: pipelineResult.durationMs,
        });
      }

      await useWorkspaceStore.getState().refreshActiveWorkspace();
    } catch (err) {
      const encryptedType = parseEncryptedError(err);
      const message = encryptedType
        ? i18n.t("hook.encryptedSkipped")
        : typeof err === "string"
          ? err
          : err instanceof Error
            ? err.message
            : i18n.t("hook.processFailed");
      useWorkspaceStore.getState().updateQueueFileResult(fileId, {
        status: "failed",
        errorMessage: message,
      });
    }
  }, []);

  return { startAutoProcess, abortAll, retryFile };
}
