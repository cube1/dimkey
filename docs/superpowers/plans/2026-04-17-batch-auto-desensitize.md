# 批量文件脱敏优化 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在现有逐个确认队列模式基础上新增"全自动模式"，支持文件级并发处理（默认 3 个）+ 统一目录导出 + 结果报告视图。

**Architecture:** 前端纯 TS 改造。新增 Promise 池调度器（`batchScheduler`）驱动并发；抽取 `runDesensitizePipeline` 共享函数同时服务逐个确认和全自动两种模式；NER 通过现有 `Arc<Mutex<NerEngine>>` 自动串行，无需后端改动。结果存储在 `QueueFile.result` 中避免并发污染单文件视图状态。

**Tech Stack:** React 19 + TypeScript + Zustand + TailwindCSS + Tauri v2 IPC；E2E: Playwright + pytest + IPC Mock。

**Spec:** `docs/superpowers/specs/2026-04-17-batch-auto-desensitize-design.md`

---

## 文件结构

| 文件 | 类型 | 职责 |
|---|---|---|
| `src/types/index.ts` | 修改 | 扩展 `QueueFile`；新增 `BatchMode`/`BatchSession`/`MAX_CONCURRENCY` |
| `src/locales/zh.json`, `en.json` | 修改 | 批量模式文案 |
| `src/utils/batchScheduler.ts` | 新增 | 通用 Promise 池，与业务解耦 |
| `src/utils/outputPath.ts` | 新增 | 解析输出文件名（重名追加 `_N`） |
| `src/stores/workspaceStore.ts` | 修改 | 新增 `batchSession` 状态 + actions |
| `src/hooks/useAutoDesensitize.ts` | 修改 | 抽取 `runDesensitizePipeline` 共享函数 |
| `src/hooks/useBatchAutoProcess.ts` | 新增 | 全自动批量处理 Hook |
| `src/components/CenterPanel/BatchModeSelector.tsx` | 新增 | 模式选择器 UI |
| `src/components/CenterPanel/BatchProgressBar.tsx` | 新增 | 进度条 + 中止按钮 |
| `src/components/CenterPanel/BatchResultReport.tsx` | 新增 | 结果报告视图 |
| `src/components/CenterPanel/FileQueueTabs.tsx` | 修改 | 全自动模式下 Tab 点击行为 |
| `src/components/CenterPanel/DropzoneView.tsx` | 修改 | 多文件拖入后展示模式选择器 |
| `src/components/CenterPanel/index.tsx` | 修改 | 按 `batchSession.phase` 切换子视图 |
| `src/layouts/WorkspaceLayout.tsx` | 修改 | 全自动模式下工具栏行为 |
| `e2e/tests/test_batch_auto.py` | 新增 | 全自动批量处理 E2E 测试 |

---

## Task 1: 类型定义扩展

**Files:**
- Modify: `src/types/index.ts`

- [ ] **Step 1: 扩展 QueueFile 接口并新增批量类型**

在 `src/types/index.ts` 找到现有 `QueueFile` 定义（约第 474 行），替换为：

```typescript
/** 批量导入队列中的单个文件 */
export interface QueueFile {
  id: string;
  filePath: string;
  fileName: string;
  status: "pending" | "processing" | "confirmed" | "failed" | "aborted";
  errorMessage?: string;
  // --- 全自动模式结果字段（处理完成后回写） ---
  /** 识别到的敏感项数量 */
  sensitiveCount?: number;
  /** 全自动模式下的实际导出路径 */
  outputPath?: string;
  /** 处理结果（用于抽查时重载到对比视图） */
  result?: DesensitizeResult;
  /** 处理记录 ID */
  recordId?: string;
  /** 单文件耗时（毫秒，用于剩余时间预估） */
  durationMs?: number;
}

/** 批量处理模式 */
export type BatchMode = "sequential" | "auto";

/** 批量处理会话（仅全自动模式使用） */
export interface BatchSession {
  mode: BatchMode;
  /** 全自动模式的统一输出目录；sequential 模式为 null */
  outputDir: string | null;
  startedAt: number;
  aborted: boolean;
  /** idle=未启动 / running=处理中 / finished=全部完成或被中止 */
  phase: "idle" | "running" | "finished";
}

/** 批量导入最大文件数 */
export const MAX_QUEUE_SIZE = 20;
/** 文件级并发数 */
export const MAX_CONCURRENCY = 3;
```

- [ ] **Step 2: TypeScript 编译验证**

Run: `npx tsc --noEmit`
Expected: 无新增错误（可能有现有 QueueFile 使用者因新增可选字段仍兼容）

- [ ] **Step 3: 提交**

```bash
git add src/types/index.ts
git commit -m "feat(batch): 扩展 QueueFile 类型，新增 BatchMode/BatchSession"
```

---

## Task 2: i18n 文案

**Files:**
- Modify: `src/locales/zh.json`
- Modify: `src/locales/en.json`

- [ ] **Step 1: 添加中文文案**

在 `src/locales/zh.json` 的 `fileQueue` 对象（约第 191 行）内追加：

```json
"batchMode": {
  "selectorTitle": "批量处理模式",
  "sequential": "逐个确认",
  "sequentialHint": "逐个文件进入对比视图，手动调整后导出",
  "auto": "全自动",
  "autoHint": "使用当前策略自动处理所有文件并导出到指定目录",
  "filesCount": "已选 {{count}} 个文件",
  "outputDir": "输出目录",
  "chooseDir": "选择目录",
  "startProcessing": "开始批量处理",
  "outputDirRequired": "请先选择输出目录",
  "inProgress": "批量处理进行中",
  "remaining": "预计剩余 {{seconds}}s",
  "abort": "中止全部",
  "abortConfirm": "中止后未开始的文件将被标记为已取消，已完成的保留。确定中止？",
  "reportTitle": "批量处理完成",
  "reportSummary": "成功 {{success}} · 失败 {{failed}} · 取消 {{aborted}}",
  "reportOutputDir": "输出到 {{dir}}",
  "openDir": "打开目录",
  "retry": "用当前策略重试",
  "retrying": "重试中…",
  "close": "关闭结果报告",
  "viewOnly": "只读抽查（全自动结果）",
  "switchConfirm": "批量处理进行中，切换工作区将中止并保留已完成结果，是否继续？",
  "deleteConfirm": "批量处理进行中，删除工作区将中止并放弃所有结果，是否继续？",
  "dropRejected": "批量处理进行中，请先完成或中止"
}
```

- [ ] **Step 2: 添加英文文案**

在 `src/locales/en.json` 对应位置追加：

```json
"batchMode": {
  "selectorTitle": "Batch Processing Mode",
  "sequential": "Review Each",
  "sequentialHint": "Enter comparison view per file, adjust and export manually",
  "auto": "Auto",
  "autoHint": "Process all files with current strategy and export to target directory",
  "filesCount": "{{count}} files selected",
  "outputDir": "Output Directory",
  "chooseDir": "Choose",
  "startProcessing": "Start Batch",
  "outputDirRequired": "Please select an output directory first",
  "inProgress": "Batch processing in progress",
  "remaining": "~{{seconds}}s left",
  "abort": "Abort All",
  "abortConfirm": "Unstarted files will be marked as cancelled; completed ones are kept. Abort?",
  "reportTitle": "Batch Processing Complete",
  "reportSummary": "{{success}} success · {{failed}} failed · {{aborted}} cancelled",
  "reportOutputDir": "Output to {{dir}}",
  "openDir": "Open Folder",
  "retry": "Retry with current strategy",
  "retrying": "Retrying…",
  "close": "Close Report",
  "viewOnly": "Read-only review (auto result)",
  "switchConfirm": "Batch is running. Switching workspace will abort and keep completed results. Continue?",
  "deleteConfirm": "Batch is running. Deleting will abort and discard all results. Continue?",
  "dropRejected": "Batch is in progress; please finish or abort first"
}
```

- [ ] **Step 3: 提交**

```bash
git add src/locales/zh.json src/locales/en.json
git commit -m "feat(batch): 新增批量模式 i18n 文案"
```

---

## Task 3: outputPath 工具函数

**Files:**
- Create: `src/utils/outputPath.ts`

- [ ] **Step 1: 创建 utils 目录并实现 resolveOutputPath**

```bash
mkdir -p src/utils
```

Create `src/utils/outputPath.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";

/** 拆分文件名为 base + ext（保留点号） */
export function splitExt(fileName: string): { base: string; ext: string } {
  const dot = fileName.lastIndexOf(".");
  if (dot <= 0) return { base: fileName, ext: "" };
  return { base: fileName.slice(0, dot), ext: fileName.slice(dot) };
}

/** 拼接路径（兼容 Unix/Windows） */
export function joinPath(dir: string, name: string): string {
  const sep = dir.includes("\\") ? "\\" : "/";
  const trimmed = dir.endsWith("/") || dir.endsWith("\\") ? dir.slice(0, -1) : dir;
  return `${trimmed}${sep}${name}`;
}

/**
 * 解析输出文件路径：`{base}_脱敏{ext}`；重名时追加 `_1`、`_2`...
 * 并发下存在理论竞态（两个 worker 同时探测到同名不存在），但批次内文件基本不重名，可接受。
 */
export async function resolveOutputPath(outputDir: string, originalName: string): Promise<string> {
  const { base, ext } = splitExt(originalName);
  let candidate = `${base}_脱敏${ext}`;
  let n = 1;
  // check_file_exists 是项目已有 Tauri 命令，返回 Result<bool, String>
  // eslint-disable-next-line no-constant-condition
  while (true) {
    const full = joinPath(outputDir, candidate);
    const exists = await invoke<boolean>("check_file_exists", { filePath: full }).catch(() => false);
    if (!exists) return full;
    candidate = `${base}_脱敏_${n}${ext}`;
    n++;
    if (n > 999) return full; // 兜底，防死循环
  }
}
```

- [ ] **Step 2: 类型检查**

Run: `npx tsc --noEmit`
Expected: PASS

- [ ] **Step 3: 提交**

```bash
git add src/utils/outputPath.ts
git commit -m "feat(batch): 新增 outputPath 工具，解析脱敏导出文件名"
```

---

## Task 4: batchScheduler Promise 池

**Files:**
- Create: `src/utils/batchScheduler.ts`

- [ ] **Step 1: 实现滚动窗口 Promise 池**

Create `src/utils/batchScheduler.ts`:

```typescript
/**
 * 通用 Promise 池：对 items 以固定并发数 concurrency 执行 worker。
 * - 任一 worker 完成后立刻启动下一个 pending 任务（滚动窗口）
 * - signal.aborted 为 true 时不再启动新任务；已启动的任务由 worker 自行决定是否短路
 * - worker 内部需 try/catch，不应 reject 到调度器（否则会中断整体）
 * - 全部任务 settle 后 resolve
 */
export async function runBatch<T>(
  items: T[],
  concurrency: number,
  worker: (item: T, index: number, signal: AbortSignal) => Promise<void>,
  signal: AbortSignal,
): Promise<void> {
  if (items.length === 0) return;
  const effectiveConcurrency = Math.max(1, Math.min(concurrency, items.length));
  let nextIndex = 0;

  const runOne = async (): Promise<void> => {
    while (true) {
      if (signal.aborted) return;
      const idx = nextIndex++;
      if (idx >= items.length) return;
      try {
        await worker(items[idx], idx, signal);
      } catch (err) {
        // 防御性：worker 理应自己 catch；若抛出也不中断整体
        console.error("[batchScheduler] worker threw:", err);
      }
    }
  };

  const runners: Promise<void>[] = [];
  for (let i = 0; i < effectiveConcurrency; i++) {
    runners.push(runOne());
  }
  await Promise.all(runners);
}
```

- [ ] **Step 2: 类型检查**

Run: `npx tsc --noEmit`
Expected: PASS

- [ ] **Step 3: 提交**

```bash
git add src/utils/batchScheduler.ts
git commit -m "feat(batch): 新增 batchScheduler Promise 池调度器"
```

---

## Task 5: workspaceStore 扩展 batchSession

**Files:**
- Modify: `src/stores/workspaceStore.ts`

- [ ] **Step 1: 导入 BatchSession 类型并添加状态与 action 声明**

在 `src/stores/workspaceStore.ts` 顶部 import 段加入 `BatchSession`：

```typescript
import type {
  // ... 现有导入
  QueueFile,
  BatchSession,
  AliasGroup,
} from "../types";
```

在 `WorkspaceState` 接口中找到 `// --- 批量文件队列 ---` 段（约第 69 行），追加字段：

```typescript
  // --- 批量文件队列 ---
  fileQueue: QueueFile[];
  activeQueueIndex: number;
  /** 批量处理会话（null 表示无批量或逐个确认模式未启动） */
  batchSession: BatchSession | null;
```

在 actions 接口段（约 `// --- 批量队列操作 ---` 下）追加：

```typescript
  // --- 批量自动模式 ---
  startBatchAuto: (outputDir: string) => void;
  abortBatchAuto: () => void;
  finishBatchAuto: () => void;
  /** 更新队列文件的部分字段（用于回写 result/outputPath 等） */
  updateQueueFileResult: (id: string, patch: Partial<QueueFile>) => void;
  /** 清除批量会话（回到 dropzone） */
  clearBatchSession: () => void;
```

- [ ] **Step 2: 初始化 batchSession 字段**

在 store 默认值段（约第 179-180 行 `fileQueue: []` 附近）追加：

```typescript
  fileQueue: [],
  activeQueueIndex: -1,
  batchSession: null,
```

- [ ] **Step 3: 在 selectWorkspace / deleteWorkspace 的重置段中清除 batchSession**

找到 `selectWorkspace` 的 `set({...})` 块（约第 205 行），在 `activeQueueIndex: -1,` 后加：

```typescript
        batchSession: null,
```

同样在 `deleteWorkspace` 的 `set({...})` 块（约第 260 行）追加 `batchSession: null,`。

- [ ] **Step 4: 实现 actions**

在 `advanceToNextFile` action 之后（约第 563 行）追加：

```typescript
  startBatchAuto: (outputDir) => set({
    batchSession: {
      mode: "auto",
      outputDir,
      startedAt: Date.now(),
      aborted: false,
      phase: "running",
    },
  }),

  abortBatchAuto: () => set((s) => ({
    batchSession: s.batchSession ? { ...s.batchSession, aborted: true } : null,
  })),

  finishBatchAuto: () => set((s) => ({
    batchSession: s.batchSession ? { ...s.batchSession, phase: "finished" } : null,
  })),

  updateQueueFileResult: (id, patch) => set((s) => ({
    fileQueue: s.fileQueue.map((f) => (f.id === id ? { ...f, ...patch } : f)),
  })),

  clearBatchSession: () => set({
    fileQueue: [],
    activeQueueIndex: -1,
    batchSession: null,
  }),
```

- [ ] **Step 5: 也更新 clearFileQueue 让它清除 batchSession**

找到 `clearFileQueue: () => set({ fileQueue: [], activeQueueIndex: -1 }),`，改为：

```typescript
  clearFileQueue: () => set({ fileQueue: [], activeQueueIndex: -1, batchSession: null }),
```

- [ ] **Step 6: 类型检查**

Run: `npx tsc --noEmit`
Expected: PASS

- [ ] **Step 7: 提交**

```bash
git add src/stores/workspaceStore.ts
git commit -m "feat(batch): workspaceStore 新增 batchSession 状态与 actions"
```

---

## Task 6: 抽取 runDesensitizePipeline 共享函数

**Files:**
- Modify: `src/hooks/useAutoDesensitize.ts`

读取当前 `useAutoDesensitize.ts`（约 500 行）理解 `processFile` 完整流程：导入 → 正则/NER/词典 → 合并 → 一致性脱敏 → 保存记录。

- [ ] **Step 1: 在文件顶部（export function useAutoDesensitize 之前）新增纯函数 runDesensitizePipeline**

这是一个**不依赖 React/store** 的纯函数，接受所需参数返回结果；批量模式可复用。

```typescript
/** 批量与单文件共享的脱敏流水线结果 */
export interface PipelineResult {
  content: FileContent;
  sensitiveItems: SensitiveItem[];
  rawSensitiveItems: SensitiveItem[];
  desensitizeResult: DesensitizeResult;
  record: ProcessingRecord;
  durationMs: number;
}

/** 流水线参数（由调用者从 workspace 中提取） */
export interface PipelineOptions {
  workspaceId: string;
  strategies: Record<string, StrategyConfig>;
  dictEntries: DictEntry[];
  enabledTypes: string[];
  replaceStyle: string;
  consistencyMappings: ConsistencyMapping[];
  language: "zh" | "en";
  aliasGroups: AliasGroup[];
  whitelist: WhitelistEntry[];
  password?: string;
}

/**
 * 不依赖 React 的纯脱敏流水线：导入 → 三层识别 → 合并 → 一致性脱敏 → 保存记录。
 * 抛出的错误由调用方处理（加密/密码/通用）。
 */
export async function runDesensitizePipeline(
  filePath: string,
  options: PipelineOptions,
): Promise<PipelineResult> {
  const started = Date.now();

  // 1. 导入
  const content: FileContent = options.password
    ? await invoke("import_file_with_password", { filePath, password: options.password })
    : await invoke("import_file", { filePath });

  // 2. 三层识别（regex/ner/dict）
  const [regexItems, nerItems, dictItems] = await Promise.all([
    invoke<SensitiveItem[]>("detect_by_regex", {
      content, enabledTypes: options.enabledTypes, language: options.language,
    }),
    invoke<SensitiveItem[]>("detect_by_ner", { content }),
    invoke<SensitiveItem[]>("detect_by_dict", {
      content, dictEntries: options.dictEntries, language: options.language,
    }),
  ]);

  // 3. 合并去重（复用现有合并函数；如现有逻辑内联则此处内联同样实现）
  const rawSensitiveItems = mergeAndDedupeItems(regexItems, nerItems, dictItems);

  // 4. 白名单过滤 + enabledTypes 过滤
  const sensitiveItems = filterByEnabledAndWhitelist(
    rawSensitiveItems, options.enabledTypes, options.whitelist,
  );

  // 5. 一致性脱敏
  const desensitizeResult = await invoke<DesensitizeResult>("apply_desensitize", {
    content,
    sensitiveItems,
    strategies: options.strategies,
    replaceStyle: options.replaceStyle,
    consistencyMappings: options.consistencyMappings,
    aliasGroups: options.aliasGroups,
    workspaceId: options.workspaceId,
  });

  // 6. 构造 ProcessingRecord（调用方负责 invoke add_processing_record）
  const record: ProcessingRecord = {
    id: generateRecordId(),
    file_name: filePath.split(/[/\\]/).pop() || filePath,
    file_path: filePath,
    file_type: content.file_type,
    processed_at: new Date().toISOString(),
    mappings: desensitizeResult.mappings,
    sensitive_count: desensitizeResult.summary.total,
    status: "Completed",
  };

  return {
    content,
    sensitiveItems,
    rawSensitiveItems,
    desensitizeResult,
    record,
    durationMs: Date.now() - started,
  };
}
```

**注意**：`mergeAndDedupeItems`、`filterByEnabledAndWhitelist`、`generateRecordId` 如在当前 hook 内为局部函数，将其提升到文件顶层 export（修改 `function` 声明为 `export function`）；如现有实现是内联在 `processFile` 里，需先抽取到独立函数。

- [ ] **Step 2: 修改 useAutoDesensitize 中的 processFile 调用新函数**

找到 `processFile` 实现内从"1. 导入"到"4. 脱敏结果保存前"的段落，替换为：

```typescript
      const options: PipelineOptions = {
        workspaceId: ws.id,
        strategies: ws.strategies,
        dictEntries: ws.dict_entries,
        enabledTypes: ws.enabled_types,
        replaceStyle: ws.replace_style,
        consistencyMappings: ws.consistency_mappings,
        language: langState,
        aliasGroups: ws.alias_groups ?? [],
        whitelist: ws.whitelist ?? [],
        password,
      };
      const pipeline = await runDesensitizePipeline(filePath, options);
      const { content, rawSensitiveItems, sensitiveItems, desensitizeResult, record } = pipeline;

      // 回写到 store（单文件模式专属）
      store.setCurrentFileContent(content, filePath);
      store.setRawSensitiveItems(rawSensitiveItems);
      store.setCurrentSensitiveItems(sensitiveItems);
      store.setCurrentResult(desensitizeResult);
      store.setCurrentRecordId(record.id);

      store.setProcessingStep("saving");
      await invoke("add_processing_record", { workspaceId: ws.id, record });
      await store.refreshActiveWorkspace();
      store.setCenterView("comparison");
      store.setProcessingStep("done");
```

- [ ] **Step 3: 类型检查 + 前端构建**

Run: `npx tsc --noEmit && npm run build`
Expected: PASS

- [ ] **Step 4: 手动冒烟（可选，若有本地 dev 环境）**

Run: `cargo tauri dev`, 拖入单个 xlsx 验证逐个确认模式未回归。

- [ ] **Step 5: 提交**

```bash
git add src/hooks/useAutoDesensitize.ts
git commit -m "refactor(batch): 抽取 runDesensitizePipeline 共享脱敏流水线函数"
```

---

## Task 7: useBatchAutoProcess Hook

**Files:**
- Create: `src/hooks/useBatchAutoProcess.ts`

- [ ] **Step 1: 实现 Hook**

```typescript
import { useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import toast from "react-hot-toast";
import i18n from "../i18n";
import { useWorkspaceStore } from "../stores/workspaceStore";
import { runDesensitizePipeline, PipelineOptions } from "./useAutoDesensitize";
import { runBatch } from "../utils/batchScheduler";
import { resolveOutputPath } from "../utils/outputPath";
import { MAX_CONCURRENCY } from "../types";
import type { QueueFile } from "../types";
import { parseEncryptedError } from "../types";

export function useBatchAutoProcess() {
  const abortRef = useRef<AbortController | null>(null);

  /** 启动全自动批量处理 */
  const startAutoProcess = useCallback(async (outputDir: string) => {
    const wsStore = useWorkspaceStore.getState();
    const ws = wsStore.activeWorkspaceData?.workspace;
    if (!ws) return;

    const queue = wsStore.fileQueue.filter((f) => f.status === "pending");
    if (queue.length === 0) return;

    const controller = new AbortController();
    abortRef.current = controller;
    wsStore.startBatchAuto(outputDir);

    const langState = (useWorkspaceStore.getState() as any).language ?? "zh";

    await runBatch(
      queue,
      MAX_CONCURRENCY,
      async (file: QueueFile, _idx, signal) => {
        const latest = useWorkspaceStore.getState();
        // 已取消：标记 aborted 不再跑
        if (signal.aborted || latest.batchSession?.aborted) {
          latest.updateQueueFileResult(file.id, { status: "aborted" });
          return;
        }
        latest.updateQueueFileStatus(file.id, "processing");

        try {
          // 读取最新工作区配置（用户可能在中途改过）
          const wsNow = useWorkspaceStore.getState().activeWorkspaceData?.workspace;
          if (!wsNow) throw new Error("工作区已失效");

          const options: PipelineOptions = {
            workspaceId: wsNow.id,
            strategies: wsNow.strategies,
            dictEntries: wsNow.dict_entries,
            enabledTypes: wsNow.enabled_types,
            replaceStyle: wsNow.replace_style,
            consistencyMappings: wsNow.consistency_mappings,
            language: langState,
            aliasGroups: wsNow.alias_groups ?? [],
            whitelist: wsNow.whitelist ?? [],
          };

          const pipeline = await runDesensitizePipeline(file.filePath, options);

          // 导出到统一目录
          const outputPath = await resolveOutputPath(outputDir, file.fileName);
          await invoke("export_file", {
            result: pipeline.desensitizeResult,
            filePath: outputPath,
          });

          // 保存处理记录
          await invoke("add_processing_record", {
            workspaceId: wsNow.id,
            record: pipeline.record,
          });

          useWorkspaceStore.getState().updateQueueFileResult(file.id, {
            status: "confirmed",
            sensitiveCount: pipeline.desensitizeResult.summary.total,
            outputPath,
            result: pipeline.desensitizeResult,
            recordId: pipeline.record.id,
            durationMs: pipeline.durationMs,
          });
        } catch (err) {
          const encrypted = parseEncryptedError(err);
          const message = encrypted
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
      },
      controller.signal,
    );

    // 批量结束：刷新工作区（新增的 processing_records 写入）
    await useWorkspaceStore.getState().refreshActiveWorkspace();
    useWorkspaceStore.getState().finishBatchAuto();
    abortRef.current = null;

    const final = useWorkspaceStore.getState().fileQueue;
    const success = final.filter((f) => f.status === "confirmed").length;
    const failed = final.filter((f) => f.status === "failed").length;
    const aborted = final.filter((f) => f.status === "aborted").length;
    toast.success(
      i18n.t("fileQueue.batchMode.reportSummary", { success, failed, aborted }),
    );
  }, []);

  /** 中止 */
  const abortAll = useCallback(() => {
    const wsStore = useWorkspaceStore.getState();
    wsStore.abortBatchAuto();
    abortRef.current?.abort();
  }, []);

  /** 重试单个失败文件（使用当前最新策略） */
  const retryFile = useCallback(async (fileId: string) => {
    const wsStore = useWorkspaceStore.getState();
    const file = wsStore.fileQueue.find((f) => f.id === fileId);
    const outputDir = wsStore.batchSession?.outputDir;
    const ws = wsStore.activeWorkspaceData?.workspace;
    if (!file || !outputDir || !ws) return;

    wsStore.updateQueueFileResult(fileId, { status: "processing", errorMessage: undefined });
    try {
      const options: PipelineOptions = {
        workspaceId: ws.id,
        strategies: ws.strategies,
        dictEntries: ws.dict_entries,
        enabledTypes: ws.enabled_types,
        replaceStyle: ws.replace_style,
        consistencyMappings: ws.consistency_mappings,
        language: (useWorkspaceStore.getState() as any).language ?? "zh",
        aliasGroups: ws.alias_groups ?? [],
        whitelist: ws.whitelist ?? [],
      };
      const pipeline = await runDesensitizePipeline(file.filePath, options);
      const outputPath = await resolveOutputPath(outputDir, file.fileName);
      await invoke("export_file", { result: pipeline.desensitizeResult, filePath: outputPath });
      await invoke("add_processing_record", { workspaceId: ws.id, record: pipeline.record });
      useWorkspaceStore.getState().updateQueueFileResult(fileId, {
        status: "confirmed",
        sensitiveCount: pipeline.desensitizeResult.summary.total,
        outputPath,
        result: pipeline.desensitizeResult,
        recordId: pipeline.record.id,
        durationMs: pipeline.durationMs,
      });
      await useWorkspaceStore.getState().refreshActiveWorkspace();
    } catch (err) {
      const message = typeof err === "string" ? err : err instanceof Error ? err.message : "处理失败";
      useWorkspaceStore.getState().updateQueueFileResult(fileId, {
        status: "failed",
        errorMessage: message,
      });
    }
  }, []);

  return { startAutoProcess, abortAll, retryFile };
}
```

- [ ] **Step 2: 类型检查**

Run: `npx tsc --noEmit`
Expected: PASS（如报错，通常是 `parseEncryptedError` 或 `ConsistencyMapping` 等类型名称与现有不完全一致 —— 按 `src/types/index.ts` 真实定义调整）

- [ ] **Step 3: 提交**

```bash
git add src/hooks/useBatchAutoProcess.ts
git commit -m "feat(batch): 新增 useBatchAutoProcess Hook"
```

---

## Task 8: BatchModeSelector 组件

**Files:**
- Create: `src/components/CenterPanel/BatchModeSelector.tsx`

- [ ] **Step 1: 实现选择器**

```typescript
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
      // 沿用现有流程：第一个文件 status=processing（已由 initFileQueue 设置第 0 个为 pending，需改为 processing）
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
```

- [ ] **Step 2: 类型检查 + 构建**

Run: `npx tsc --noEmit && npm run build`
Expected: PASS

- [ ] **Step 3: 提交**

```bash
git add src/components/CenterPanel/BatchModeSelector.tsx
git commit -m "feat(batch): 新增 BatchModeSelector 组件"
```

---

## Task 9: BatchProgressBar 组件

**Files:**
- Create: `src/components/CenterPanel/BatchProgressBar.tsx`

- [ ] **Step 1: 实现进度条 + 中止按钮**

```typescript
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
```

- [ ] **Step 2: 类型检查 + 构建**

Run: `npx tsc --noEmit && npm run build`
Expected: PASS

- [ ] **Step 3: 提交**

```bash
git add src/components/CenterPanel/BatchProgressBar.tsx
git commit -m "feat(batch): 新增 BatchProgressBar 进度条组件"
```

---

## Task 10: BatchResultReport 组件

**Files:**
- Create: `src/components/CenterPanel/BatchResultReport.tsx`

- [ ] **Step 1: 实现结果报告视图**

```typescript
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
                      <>敏感项 {file.sensitiveCount ?? 0} · {file.outputPath}</>
                    )}
                    {file.status === "failed" && (file.errorMessage || "处理失败")}
                    {file.status === "aborted" && t("fileQueue.batchMode.abort")}
                  </div>
                </div>
                {file.status === "confirmed" && (
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
```

- [ ] **Step 2: 类型检查 + 构建**

Run: `npx tsc --noEmit && npm run build`
Expected: PASS

- [ ] **Step 3: 提交**

```bash
git add src/components/CenterPanel/BatchResultReport.tsx
git commit -m "feat(batch): 新增 BatchResultReport 结果报告视图"
```

---

## Task 11: FileQueueTabs 扩展全自动点击行为

**Files:**
- Modify: `src/components/CenterPanel/FileQueueTabs.tsx`

- [ ] **Step 1: 修改 handleTabClick，auto 模式下 confirmed Tab 可进入抽查**

找到 `handleTabClick`（约第 55 行），替换为：

```typescript
  const batchSession = useWorkspaceStore((s) => s.batchSession);
  const setCurrentResult = useWorkspaceStore((s) => s.setCurrentResult);
  const setCurrentFileContent = useWorkspaceStore((s) => s.setCurrentFileContent);
  const setCurrentRecordId = useWorkspaceStore((s) => s.setCurrentRecordId);
  const setCenterView = useWorkspaceStore((s) => s.setCenterView);

  const handleTabClick = (file: QueueFile) => {
    const isAutoMode = batchSession?.mode === "auto";
    switch (file.status) {
      case "confirmed":
        if (isAutoMode && file.result && file.recordId) {
          // 只读抽查
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
```

- [ ] **Step 2: 补充 aborted 状态的 STATUS_CONFIG**

找到 `STATUS_CONFIG`（约第 8 行），新增：

```typescript
  aborted: {
    icon: Circle,
    colorClass: "text-slate-400",
    bgClass: "bg-slate-100",
    borderClass: "border-slate-200",
  },
```

并在顶部 import 中确保已引入所有需要的类型/组件（`Ban` 图标如果更合适可以替换）。

- [ ] **Step 3: 类型检查 + 构建**

Run: `npx tsc --noEmit && npm run build`
Expected: PASS

- [ ] **Step 4: 提交**

```bash
git add src/components/CenterPanel/FileQueueTabs.tsx
git commit -m "feat(batch): FileQueueTabs 支持 aborted 状态和全自动模式抽查"
```

---

## Task 12: DropzoneView 多文件走模式选择器

**Files:**
- Modify: `src/components/CenterPanel/DropzoneView.tsx`

- [ ] **Step 1: 修改 handleImportFiles — 多文件不再自动触发 processFile**

找到 `handleImportFiles`（约第 121 行）。关键变更：构建 queue 后**不**自动调用 `processFile`，改为所有文件初始 status 为 `pending`，交由模式选择器决定。

替换整个 `handleImportFiles`：

```typescript
  const handleImportFiles = useCallback(
    async (paths: string[]) => {
      if (isBatchMode() && hasUnfinishedFiles()) {
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
```

注意：`initFileQueue` 当前实现（Task 5 之前的老代码）将 `activeQueueIndex` 设为 0；保留该行为无副作用。

- [ ] **Step 2: 类型检查 + 构建**

Run: `npx tsc --noEmit && npm run build`
Expected: PASS

- [ ] **Step 3: 提交**

```bash
git add src/components/CenterPanel/DropzoneView.tsx
git commit -m "feat(batch): 多文件拖入后交由模式选择器处理"
```

---

## Task 13: CenterPanel 按 phase 切换子视图

**Files:**
- Modify: `src/components/CenterPanel/index.tsx`

- [ ] **Step 1: 读取现有 CenterPanel 结构**

Run: `cat src/components/CenterPanel/index.tsx`
观察其如何根据 `centerView` 切换视图。

- [ ] **Step 2: 引入新组件并加入条件渲染**

在 import 段补充：

```typescript
import { BatchModeSelector } from "./BatchModeSelector";
import { BatchProgressBar } from "./BatchProgressBar";
import { BatchResultReport } from "./BatchResultReport";
```

在组件顶部新增状态选择：

```typescript
  const fileQueue = useWorkspaceStore((s) => s.fileQueue);
  const batchSession = useWorkspaceStore((s) => s.batchSession);
  const centerView = useWorkspaceStore((s) => s.centerView);
```

在 JSX 的 `<FileQueueTabs />` 下方（约第 39 行）按此顺序渲染：

```tsx
        <FileQueueTabs />
        <BatchProgressBar />
        {/* 多文件已导入但未开始（phase=idle 且队列无 processing） → 模式选择器 */}
        {fileQueue.length > 1 && !batchSession && centerView === "dropzone" && (
          <BatchModeSelector />
        )}
        {/* 批量已完成 → 结果报告 */}
        {batchSession?.phase === "finished" && <BatchResultReport />}
        {/* 其余情况：走现有 centerView 切换 */}
```

其余 `centerView` 分支（comparison / restore / dropzone 等）保持，但需添加守卫条件：当 `batchSession?.phase === "finished"` 时不渲染原有视图，避免重叠。

具体方式：在原渲染入口外层包一个 `batchSession?.phase !== "finished" && (...)` 条件；
或者在每个 centerView 分支内检查。推荐外层守卫：

```tsx
{batchSession?.phase !== "finished" && !(fileQueue.length > 1 && !batchSession && centerView === "dropzone") && (
  <>
    {/* 原有按 centerView 渲染 DropzoneView/ComparisonView/RestoreView 的代码 */}
  </>
)}
```

- [ ] **Step 3: 类型检查 + 构建**

Run: `npx tsc --noEmit && npm run build`
Expected: PASS

- [ ] **Step 4: 提交**

```bash
git add src/components/CenterPanel/index.tsx
git commit -m "feat(batch): CenterPanel 按批量会话阶段切换子视图"
```

---

## Task 14: WorkspaceLayout 工具栏行为调整

**Files:**
- Modify: `src/layouts/WorkspaceLayout.tsx`

- [ ] **Step 1: 在 renderToolbarActions 中为全自动模式加守卫**

找到 `renderToolbarActions`（约第 378 行）。在函数顶部提前返回全自动模式下的"只读"状态：

```typescript
  const renderToolbarActions = () => {
    // 全自动模式处理中或已完成：工具栏不显示导出按钮（进度条/报告自己管理）
    const bs = useWorkspaceStore.getState().batchSession;
    if (bs?.mode === "auto") {
      if (bs.phase === "running") return null;
      // phase = finished：对比视图是只读抽查，不提供导出
      if (bs.phase === "finished" && centerView === "comparison") {
        return (
          <span className="text-xs text-slate-400 px-2" data-testid="viewonly-badge">
            {t("fileQueue.batchMode.viewOnly")}
          </span>
        );
      }
    }
    // 原有逻辑...
```

注意 React 订阅问题：用 `useWorkspaceStore((s) => s.batchSession)` 替代 `getState()`，以便状态变化时重渲染。将 `bs` 替换为 hook 订阅值：

```typescript
  const batchSession = useWorkspaceStore((s) => s.batchSession);
  // ...
  const renderToolbarActions = () => {
    if (batchSession?.mode === "auto") {
      if (batchSession.phase === "running") return null;
      if (batchSession.phase === "finished" && centerView === "comparison") {
        return (
          <span className="text-xs text-slate-400 px-2" data-testid="viewonly-badge">
            {t("fileQueue.batchMode.viewOnly")}
          </span>
        );
      }
    }
    // 原有逻辑保留 ...
```

- [ ] **Step 2: WorkspaceList 切换工作区确认文案改为通用**

找到 `src/components/WorkspaceList/index.tsx` 中的 `confirmSwitchQueue` / `confirmDeleteQueue`。当 `batchSession?.phase === "running"` 时使用新文案：

在两处 `if (store.isBatchMode() && store.hasUnfinishedFiles())` 分支内，插入前置判断：

```typescript
const bs = store.batchSession;
if (bs?.phase === "running") {
  const confirmed = window.confirm(t("fileQueue.batchMode.switchConfirm"));
  if (!confirmed) return;
  // 中止正在进行的批量
  useBatchAutoProcess.getState?.().abortAll?.(); // 若不方便，直接调 store.abortBatchAuto()
  store.abortBatchAuto();
}
```

简化：由于 hook 不能在事件回调外直接调用，改为直接用 store action：

```typescript
if (store.batchSession?.phase === "running") {
  if (!window.confirm(t("fileQueue.batchMode.switchConfirm"))) return;
  store.abortBatchAuto();
  store.clearFileQueue();
}
```

删除工作区分支同理，用 `deleteConfirm` 文案。

- [ ] **Step 3: DropzoneView 拒绝拖入的文案升级**

在 `src/components/CenterPanel/DropzoneView.tsx` 的 `handleImportFiles` 开头：

```typescript
const batchSession = useWorkspaceStore.getState().batchSession;
if (batchSession?.phase === "running") {
  toast.error(t("fileQueue.batchMode.dropRejected"));
  return;
}
if (isBatchMode() && hasUnfinishedFiles() && !batchSession) {
  toast.error(t("dropzone.waitQueue"));
  return;
}
```

- [ ] **Step 4: 类型检查 + 构建**

Run: `npx tsc --noEmit && npm run build`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/layouts/WorkspaceLayout.tsx src/components/WorkspaceList/index.tsx src/components/CenterPanel/DropzoneView.tsx
git commit -m "feat(batch): 工具栏/工作区切换/拖入拒绝在批量模式下的行为调整"
```

---

## Task 15: E2E 测试 — 全自动 happy path

**Files:**
- Create: `e2e/tests/test_batch_auto.py`

- [ ] **Step 1: 编写全自动 happy path 测试**

```python
"""批量全自动模式 E2E 测试"""
import time
import pytest
from playwright.sync_api import Page, expect

pytestmark = pytest.mark.usefixtures("page")

E2E_URL = "http://127.0.0.1:1420"


def setup_mocks(page: Page):
    """覆写关键 IPC，返回可控制的延迟和结果"""
    page.add_init_script("""
        window.__E2E_IPC_OVERRIDES__ = {
            check_file_exists: () => false,
            import_file: async (args) => {
                await new Promise(r => setTimeout(r, 50));
                return {
                    type: 'Spreadsheet',
                    file_type: 'xlsx',
                    sheets: [{ name: 'Sheet1', headers: ['姓名'], rows: [[{ text: '张三' }]] }],
                };
            },
            detect_by_regex: async () => [],
            detect_by_ner: async () => {
                await new Promise(r => setTimeout(r, 80));
                return [{
                    id: 't1', text: '张三', sensitive_type: 'PersonName',
                    sheet_index: 0, row: 0, col: 0, confidence: 0.95, source: 'ner',
                }];
            },
            detect_by_dict: async () => [],
            apply_desensitize: async () => ({
                content: { type: 'Spreadsheet', file_type: 'xlsx', sheets: [{ name: 'Sheet1', headers: ['姓名'], rows: [[{ text: 'XX' }]] }] },
                mappings: [{ original_text: '张三', replaced_text: 'XX', strategy: 'Replace', sensitive_type: 'PersonName' }],
                summary: { total: 1, by_type: {} },
            }),
            export_file: async () => null,
            add_processing_record: async () => null,
        };
    """)


def test_batch_auto_happy_path(page: Page):
    """拖入 5 个文件，选全自动，验证结果报告显示 5 成功"""
    setup_mocks(page)
    page.goto(E2E_URL)
    page.wait_for_selector("[data-testid='view-dropzone']", timeout=10000)

    # 选工作区
    page.click("text=E2E 测试")
    page.wait_for_selector("[data-testid='view-dropzone']")

    # 直接通过 store 注入 fileQueue（避开 Tauri dragDrop 事件）
    page.evaluate("""
        const store = window.__DIMKEY_STORE__.getState();
        const files = Array.from({length: 5}, (_, i) => ({
            id: `f${i}`, filePath: `/tmp/file${i}.xlsx`,
            fileName: `file${i}.xlsx`, status: 'pending',
        }));
        store.initFileQueue(files);
    """)

    # 模式选择器应出现
    page.wait_for_selector("[data-testid='batch-mode-selector']", timeout=5000)
    expect(page.locator("[data-testid='mode-auto']")).to_be_visible()

    # 设置输出目录（通过 store 绕过真实 dialog）
    page.evaluate("""
        // BatchModeSelector 通过 useState 存 outputDir；此处直接点击默认策略：
        // 若 workspace.output_dir 已设，则 input 已有值；否则我们 mock 一个
    """)
    # 直接把 batchSession 的期望输出目录注入，然后点击开始
    # 由于 BatchModeSelector 的 outputDir 是本地 state，需真实点击选择；此处改为 mock plugin-dialog
    page.evaluate("""
        // 简化：直接调 startBatchAuto 跳过 UI 选择
        window.__DIMKEY_STORE__.getState();
    """)

    # 点击"开始" — 由于 auto 模式默认要 outputDir，先点选择按钮；我们 mock plugin-dialog
    page.evaluate("""
        // Patch plugin-dialog open 使其直接返回 /tmp/out
        if (!window.__mocked_dialog__) {
            window.__mocked_dialog__ = true;
            // Playwright 无法直接 mock ES module，改用绕过策略
        }
    """)

    # 直接通过 hook 启动（确定性）：调用 store startBatchAuto 并手动跑流水线
    # 这里的目的是验证 UI 流程；我们模拟"用户选好 dir 然后开始"：
    page.evaluate("""
        (async () => {
            const store = window.__DIMKEY_STORE__.getState();
            // 直接调模块级函数：从全局暴露 useBatchAutoProcess 结果
            store.startBatchAuto('/tmp/out');
        })();
    """)

    # 此时 phase=running，但我们没有真正触发 runBatch；补丁：在应用启动时挂一个 helper
    # 实际测试路径：通过 Playwright 在应用内点击 BatchModeSelector 的真实按钮。
    # 为避免 plugin-dialog 阻塞，在 init 脚本中拦截 @tauri-apps/plugin-dialog open
    # 见 conftest 扩展（Task 15 Step 2 处理）

    # 本 test 的可验证点：数据流打通时 UI 能进入结果报告
    # 若当前不能自动跑完 runBatch，则跳过完整断言、仅断言 mode selector 可见（作为烟雾测试）
    expect(page.locator("[data-testid='batch-mode-selector']")).to_be_visible()
```

> **说明**：E2E 在浏览器 + IPC mock 下完整验证并发调度有一定复杂度（`plugin-dialog.open` 是 ES module，不便 mock）。Step 2 提供绕过方案。

- [ ] **Step 2: 在 e2e/tests/conftest.py 的 init 脚本中追加 plugin-dialog mock**

在 `e2e/tests/conftest.py` 的 `context.add_init_script("""` 块末尾（`unregisterCallback` 之后，闭合 `"""` 之前）追加：

```javascript
        // Mock @tauri-apps/plugin-dialog open — 返回测试预设的路径
        window.__E2E_DIALOG_MOCK__ = { directory: '/tmp/e2e-output', file: null };
        // plugin-dialog 的 open 底层通过 __TAURI_INTERNALS__.invoke 'plugin:dialog|open' 调用
```

并在 IPC mock 的 defaults 中追加：

```javascript
'plugin:dialog|open': (args) => {
    if (args?.options?.directory) return window.__E2E_DIALOG_MOCK__.directory;
    return window.__E2E_DIALOG_MOCK__.file;
},
```

- [ ] **Step 3: 完成 test_batch_auto_happy_path 的断言部分**

替换测试末尾为真实点击流程：

```python
    # 点击选择目录 → mock 返回 /tmp/e2e-output
    page.click("[data-testid='btn-choose-dir']")
    page.wait_for_function("() => document.querySelector(\"[data-testid='output-dir-section'] input\").value === '/tmp/e2e-output'")

    # 点击开始
    page.click("[data-testid='btn-start-batch']")

    # 等待进度条出现并最终进入结果报告
    page.wait_for_selector("[data-testid='batch-progress']", timeout=3000)
    page.wait_for_selector("[data-testid='batch-result-report']", timeout=20000)

    # 验证 5 个文件都显示为 confirmed
    confirmed_rows = page.locator("[data-testid='result-row-confirmed']")
    expect(confirmed_rows).to_have_count(5)
```

- [ ] **Step 4: 运行测试**

```bash
TAURI_DEV_HOST=127.0.0.1 npm run dev &
sleep 3
DIMKEY_E2E=1 DIMKEY_TEST_URL=http://127.0.0.1:1420 e2e/.venv/bin/pytest e2e/tests/test_batch_auto.py -v -m "not needs_backend"
```

Expected: `test_batch_auto_happy_path PASSED`

- [ ] **Step 5: 提交**

```bash
git add e2e/tests/test_batch_auto.py e2e/tests/conftest.py
git commit -m "test(batch): 全自动批量处理 E2E happy path"
```

---

## Task 16: E2E 测试 — 中止行为

**Files:**
- Modify: `e2e/tests/test_batch_auto.py`

- [ ] **Step 1: 追加中止场景测试**

在文件末尾追加：

```python
def test_batch_auto_abort(page: Page):
    """批量处理中点击中止 → 未开始的标记为 aborted"""
    # 模拟慢速 import，给中止留时间
    page.add_init_script("""
        window.__E2E_IPC_OVERRIDES__ = {
            check_file_exists: () => false,
            import_file: async () => {
                await new Promise(r => setTimeout(r, 500));
                return { type: 'Spreadsheet', file_type: 'xlsx', sheets: [] };
            },
            detect_by_regex: async () => [],
            detect_by_ner: async () => [],
            detect_by_dict: async () => [],
            apply_desensitize: async () => ({ content: { type: 'Spreadsheet', file_type: 'xlsx', sheets: [] }, mappings: [], summary: { total: 0, by_type: {} } }),
            export_file: async () => null,
            add_processing_record: async () => null,
        };
        // 自动确认 abort 弹窗
        window.confirm = () => true;
    """)
    page.goto(E2E_URL)
    page.wait_for_selector("[data-testid='view-dropzone']", timeout=10000)
    page.click("text=E2E 测试")

    page.evaluate("""
        const store = window.__DIMKEY_STORE__.getState();
        const files = Array.from({length: 10}, (_, i) => ({
            id: `f${i}`, filePath: `/tmp/f${i}.xlsx`, fileName: `f${i}.xlsx`, status: 'pending',
        }));
        store.initFileQueue(files);
    """)

    page.wait_for_selector("[data-testid='batch-mode-selector']")
    page.click("[data-testid='btn-choose-dir']")
    page.click("[data-testid='btn-start-batch']")
    page.wait_for_selector("[data-testid='batch-progress']")

    # 等几个完成后点中止
    page.wait_for_timeout(400)
    page.click("[data-testid='btn-abort-batch']")

    # 进入结果报告
    page.wait_for_selector("[data-testid='batch-result-report']", timeout=10000)

    # aborted 行数 >= 1
    aborted_rows = page.locator("[data-testid='result-row-aborted']")
    expect(aborted_rows.first).to_be_visible()
```

- [ ] **Step 2: 运行并提交**

```bash
DIMKEY_E2E=1 DIMKEY_TEST_URL=http://127.0.0.1:1420 e2e/.venv/bin/pytest e2e/tests/test_batch_auto.py::test_batch_auto_abort -v
git add e2e/tests/test_batch_auto.py
git commit -m "test(batch): 全自动模式中止行为 E2E"
```

---

## Task 17: E2E 测试 — 失败重试

**Files:**
- Modify: `e2e/tests/test_batch_auto.py`

- [ ] **Step 1: 追加失败重试测试**

```python
def test_batch_auto_retry_failed(page: Page):
    """第一轮 apply_desensitize 抛错 → 进入 failed；点重试第二轮成功"""
    page.add_init_script("""
        window.__E2E_RETRY_COUNT__ = 0;
        window.__E2E_IPC_OVERRIDES__ = {
            check_file_exists: () => false,
            import_file: async () => ({ type: 'Spreadsheet', file_type: 'xlsx', sheets: [] }),
            detect_by_regex: async () => [],
            detect_by_ner: async () => [],
            detect_by_dict: async () => [],
            apply_desensitize: async () => {
                window.__E2E_RETRY_COUNT__++;
                if (window.__E2E_RETRY_COUNT__ <= 2) {
                    throw '模拟失败';
                }
                return { content: { type: 'Spreadsheet', file_type: 'xlsx', sheets: [] }, mappings: [], summary: { total: 0, by_type: {} } };
            },
            export_file: async () => null,
            add_processing_record: async () => null,
        };
    """)
    page.goto(E2E_URL)
    page.wait_for_selector("[data-testid='view-dropzone']", timeout=10000)
    page.click("text=E2E 测试")
    page.evaluate("""
        const store = window.__DIMKEY_STORE__.getState();
        store.initFileQueue([
            { id: 'f0', filePath: '/tmp/f0.xlsx', fileName: 'f0.xlsx', status: 'pending' },
            { id: 'f1', filePath: '/tmp/f1.xlsx', fileName: 'f1.xlsx', status: 'pending' },
        ]);
    """)
    page.wait_for_selector("[data-testid='batch-mode-selector']")
    page.click("[data-testid='btn-choose-dir']")
    page.click("[data-testid='btn-start-batch']")

    # 等待结果报告，两个文件都失败
    page.wait_for_selector("[data-testid='batch-result-report']", timeout=10000)
    failed_rows = page.locator("[data-testid='result-row-failed']")
    expect(failed_rows).to_have_count(2)

    # 点击其中一个的重试
    page.locator("[data-testid='btn-retry-file']").first.click()

    # 该行应变为 confirmed
    page.wait_for_selector("[data-testid='result-row-confirmed']", timeout=5000)
    expect(page.locator("[data-testid='result-row-confirmed']").first).to_be_visible()
```

- [ ] **Step 2: 运行并提交**

```bash
DIMKEY_E2E=1 DIMKEY_TEST_URL=http://127.0.0.1:1420 e2e/.venv/bin/pytest e2e/tests/test_batch_auto.py -v
git add e2e/tests/test_batch_auto.py
git commit -m "test(batch): 失败重试 E2E"
```

---

## Task 18: 最终回归 — 逐个确认模式未回归

**Files:**
- 无（仅验证）

- [ ] **Step 1: 运行现有全部 E2E**

```bash
DIMKEY_E2E=1 DIMKEY_TEST_URL=http://127.0.0.1:1420 e2e/.venv/bin/pytest e2e/tests/ -v -m "not needs_backend"
```

Expected: 所有现有用例仍 PASS，包括 `test_basic_desensitize`、`test_workspace_crud` 等。

- [ ] **Step 2: 运行 Rust 测试（确认后端零回归）**

```bash
cd src-tauri && cargo test
```

Expected: 165 个测试全 PASS。

- [ ] **Step 3: 若有失败，修复后重跑；全部通过后最终 commit**

```bash
git log --oneline -20  # 确认提交历史整洁
```

---

## 验证清单（完成所有 Task 后自检）

- [ ] 拖入单文件：走现有 DropzoneView → ComparisonView → 导出流程（逐个确认模式零回归）
- [ ] 拖入 2+ 文件：出现 BatchModeSelector
- [ ] 选"逐个确认"：走原有 FileQueueTabs 串行流程
- [ ] 选"全自动"+ 选输出目录 + 开始：进度条显示、最多 3 并发、完成后结果报告
- [ ] 结果报告：显示成功/失败/取消统计、打开目录按钮、点击 confirmed 行进入只读抽查、点击 failed 行可重试
- [ ] 中止：pending 行变 aborted，in-flight 继续跑完
- [ ] 切换/删除工作区：弹确认并中止当前批量
- [ ] 批量进行中拖入新文件：被拒绝
- [ ] 加密文件：标记 failed，不弹密码框，不阻塞后续
- [ ] Rust 零改动（`git diff main -- src-tauri/` 为空）

---

## 依赖关系

```
Task 1 (types) ─┬─→ Task 3 (outputPath)
                ├─→ Task 4 (batchScheduler)
                ├─→ Task 5 (store) ─→ Task 6 (pipeline) ─→ Task 7 (hook) ─┬─→ Task 8 (selector)
                │                                                          ├─→ Task 9 (progress)
                │                                                          └─→ Task 10 (report)
                └─→ Task 2 (i18n)

Task 8/9/10 ─→ Task 11 (tabs) ─→ Task 12 (dropzone) ─→ Task 13 (center) ─→ Task 14 (layout)
                                                                           ↓
                                                                      Task 15/16/17 (e2e)
                                                                           ↓
                                                                        Task 18 (regression)
```
