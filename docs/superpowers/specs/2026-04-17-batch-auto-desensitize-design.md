# 批量文件脱敏优化：全自动模式 + 并发处理 设计

> 日期: 2026-04-17 | 版本: v0.7.x | 状态: 待实现

## 背景与目标

当前批量文件脱敏（INK-018）已支持多文件队列，但存在两个痛点：

1. **效率低** — 每个文件都要用户逐个进入对比视图微调并点击导出，20 个文件即 20 次交互
2. **串行慢** — 完全串行处理，未利用后端可并发的能力

本设计在保留现有"**逐个确认模式**"的基础上，新增"**全自动模式**"，并引入文件级并发调度，提升批量处理效率。

## 非目标

- 不改动后端 Rust 代码
- 不改变一致性映射的语义（跨文件一致性由 Rust 后端的工作区级持久化映射保证）
- 不支持 in-flight Tauri 命令的真正取消（Tauri v2 不支持）
- 不实现分层流水线（文件解析/NER/脱敏独立阶段调度），文件级并发足够

## 架构决策

| 决策项 | 选择 | 理由 |
|---|---|---|
| 触发方式 | 多文件拖入后出现模式选择器 | 保留现有逐个确认模式，新老并存 |
| 并发层次 | 文件级并发（默认 3） | NER 已被 `Arc<Mutex>` 串行化，文件级并发天然利用了其他阶段的并行；无需后端改动 |
| 并发数 | `MAX_CONCURRENCY = 3`（常量） | 平衡吞吐与内存峰值（pdfium + ONNX + 大 xlsx 解析） |
| 输出策略 | 全自动模式要求先选统一输出目录 | 避免每文件弹保存框，符合"一键"语义 |
| 输出文件名 | `原名_脱敏.{ext}`，重名自动追加 `_N` | 与用户直觉一致 |
| 结果展示 | 进度条 + FileQueueTabs + 完成后结果报告 | 处理中有整体/细节双视角，处理后可抽查 |
| 中止语义 | pending 不再启动，in-flight 跑完 | Tauri 命令不可取消；避免产生部分损坏的输出文件 |
| 失败重试 | 单文件重试按钮（用当前策略） | 用户改策略后可只重跑失败文件 |
| 后端改动 | 零 | 所有现有命令线程安全；NER Mutex 已提供串行化 |

## 并发模型说明

Rust 侧 NER 引擎为 `NerEngineState(Arc<Mutex<NerEngine>>)`，`detect_by_ner` 通过 `spawn_blocking` + `lock()` 执行推理。多个文件并发调用 NER 时，Tokio 线程池内的任务会在 Mutex 上自动排队串行化。

其他后端命令（`import_file`、`detect_by_regex`、`detect_by_dict`、`apply_desensitize`、`export_file`、`add_processing_record`）无共享可变状态，天然线程安全。

因此 **前端仅需实现一个文件级的 Promise 池调度器，无需任何后端改动**。最终效果是：
- 导入/正则/词典/脱敏/导出 阶段：并发加速
- NER 阶段：自动在 Mutex 排队（等价于 NER 独立串行队列）

## 数据模型

### 新增类型

```typescript
// src/types/index.ts

/** 批量处理模式 */
export type BatchMode = "sequential" | "auto";

/** 批量处理会话（仅全自动模式使用） */
export interface BatchSession {
  mode: BatchMode;
  /** 全自动模式的统一输出目录 */
  outputDir: string | null;
  startedAt: number;
  /** 用户是否已点击"中止全部" */
  aborted: boolean;
  /** 处理状态：未开始 / 处理中 / 已结束（全部完成或被中止） */
  phase: "idle" | "running" | "finished";
}

/** 批量文件队列中的单个文件（扩展） */
export interface QueueFile {
  id: string;
  filePath: string;
  fileName: string;
  status: "pending" | "processing" | "confirmed" | "failed" | "aborted";
  errorMessage?: string;
  // --- 新增（全自动模式下回写，用于结果报告和抽查） ---
  sensitiveCount?: number;
  outputPath?: string;
  result?: DesensitizeResult;
  recordId?: string;
}

/** 批量导入最大文件数（沿用） */
export const MAX_QUEUE_SIZE = 20;
/** 文件级并发数 */
export const MAX_CONCURRENCY = 3;
```

### workspaceStore 扩展

```typescript
interface WorkspaceState {
  // ... 现有字段
  fileQueue: QueueFile[];
  activeQueueIndex: number;
  batchSession: BatchSession | null;   // 新增

  // 新增 actions
  startBatchAuto: (outputDir: string) => void;
  abortBatchAuto: () => void;
  finishBatchAuto: () => void;
  updateQueueFileResult: (id: string, patch: Partial<QueueFile>) => void;
}
```

## 模块设计

### 1. 并发调度器 `src/utils/batchScheduler.ts`（新增）

通用 Promise 池，与业务解耦：

```typescript
export async function runBatch<T>(
  items: T[],
  concurrency: number,
  worker: (item: T, index: number, signal: AbortSignal) => Promise<void>,
  signal: AbortSignal,
): Promise<void>;
```

- 滚动窗口：任一 worker resolve 就启动下一个 pending
- worker 内部自己处理错误（不应 reject 到调度器，避免中断整体）
- `signal.aborted` 为 true 时不再启动新 worker
- 所有 worker 完成后 resolve

### 2. 全自动处理 Hook `src/hooks/useBatchAutoProcess.ts`（新增）

```typescript
export function useBatchAutoProcess() {
  const startAutoProcess = useCallback(async (queue: QueueFile[], outputDir: string) => {
    const abortController = new AbortController();
    useWorkspaceStore.getState().startBatchAuto(outputDir);

    await runBatch(queue, MAX_CONCURRENCY, async (file, _idx, signal) => {
      if (signal.aborted) {
        wsStore.updateQueueFileResult(file.id, { status: "aborted" });
        return;
      }
      wsStore.updateQueueFileStatus(file.id, "processing");
      try {
        const content = await invoke("import_file", { filePath: file.filePath });
        const regexItems = await invoke("detect_by_regex", { content, ... });
        const nerItems   = await invoke("detect_by_ner",   { content });
        const dictItems  = await invoke("detect_by_dict",  { content, dictEntries });
        const merged = mergeAndDedupe(regexItems, nerItems, dictItems);
        const result = await invoke("apply_desensitize", { content, items: merged, strategies });
        const outputPath = resolveOutputPath(outputDir, file.fileName);
        await invoke("export_file", { result, filePath: outputPath });
        const recordId = await saveProcessingRecord(file, result);
        wsStore.updateQueueFileResult(file.id, {
          status: "confirmed",
          sensitiveCount: result.summary.total,
          outputPath, result, recordId,
        });
      } catch (err) {
        wsStore.updateQueueFileResult(file.id, {
          status: "failed",
          errorMessage: formatError(err),
        });
      }
    }, abortController.signal);

    wsStore.finishBatchAuto();
  }, []);

  return { startAutoProcess, /* abort, retry */ };
}
```

**关键设计**：此 Hook 完全不触碰 `currentFileContent/currentSensitiveItems/currentResult` 这些"单文件视图"状态，避免并发污染。每个文件的结果存在 `QueueFile.result` 里，用户点击 Tab 抽查时才加载到视图。

### 3. `useAutoDesensitize.ts` 重构

提取"处理单文件的完整流水线"为纯函数 `runDesensitizePipeline(filePath, options): Promise<PipelineResult>`，同时被：
- 逐个确认模式的 `processFile()` 使用（后接对比视图）
- 全自动模式的 `useBatchAutoProcess` 使用（后接直接导出）

### 4. UI 组件

#### `BatchModeSelector.tsx`（新增）
多文件拖入后显示在 `DropzoneView` 或 CenterPanel：
- Segmented Control：逐个确认 / 全自动
- 若选全自动：显示输出目录选择器（默认 `workspace.output_dir`，可改）
- 显示文件数、支持的格式列表
- "开始"按钮

#### `BatchProgressBar.tsx`（新增）
全自动模式处理中显示在 CenterPanel 顶部：
- 整体进度条 `<done>/<total>`
- 预计剩余时间（基于已完成文件的平均耗时）
- "中止全部"按钮

#### `FileQueueTabs.tsx`（改造）
- 全自动模式下，`confirmed` Tab 可点击：加载 `QueueFile.result` 到 `currentResult` 并切换到对比视图（**只读抽查**，不可编辑；右上角显示"只读"徽标；如需修改请走失败重试或重新单文件处理）
- `failed` Tab 点击：显示错误详情 + "用当前策略重试"按钮
- `aborted` Tab：灰色显示，点击提示"已取消"

#### `BatchResultReport.tsx`（新增）
`batchSession.phase === "finished"` 时替换 CenterPanel 主体：
- 顶部摘要：`成功 X / 失败 Y / 取消 Z · 输出到 <path>`，带 `[打开目录]` 按钮
- 文件列表：每行 图标 + 文件名 + 敏感项数/错误信息 + 输出路径
- 失败行有 "重试" 按钮
- 底部 "关闭结果报告" 按钮 → `clearFileQueue()` 回到 DropzoneView

### 5. `DropzoneView.tsx` 改造

多文件拖入逻辑：
```
paths.length === 1  → 走现有单文件流程
paths.length >  1   → 构建 fileQueue（pending 状态）
                    → 渲染 BatchModeSelector（不自动开始）
                    → 用户选模式后：
                        sequential: 复用现有逻辑（第一个文件 status=processing，调 processFile）
                        auto: 调 useBatchAutoProcess.startAutoProcess
```

### 6. `WorkspaceLayout.tsx` 工具栏

全自动模式处理中：隐藏所有导出/对比按钮，仅显示批量进度控件。
全自动模式抽查时（用户点了 confirmed Tab）：显示"查看模式，不可编辑"提示。

## 输出文件命名

使用 `@tauri-apps/plugin-fs` 的 `exists` 在前端做重名检测，**零 Rust 改动**：

```typescript
import { exists } from "@tauri-apps/plugin-fs";

async function resolveOutputPath(outputDir: string, originalName: string): Promise<string> {
  const { base, ext } = splitExt(originalName);  // "data.xlsx" → { base: "data", ext: ".xlsx" }
  let candidate = `${base}_脱敏${ext}`;
  let n = 1;
  while (await exists(join(outputDir, candidate))) {
    candidate = `${base}_脱敏_${n}${ext}`;
    n++;
  }
  return join(outputDir, candidate);
}
```

> 注：并发场景下两个 worker 可能同时检测到相同文件名不存在并选取同名输出。可接受的权衡是：实际冲突极罕见（同一批 20 个文件内文件名基本不重名），且 `export_file` 写入时会覆盖，不会造成数据损坏。若未来需要严格唯一，可在前端加 Mutex 锁定路径解析阶段。

## 边界与错误处理

| 场景 | 行为 |
|---|---|
| 全自动进行中又拖入文件 | Toast "批量处理进行中，请先完成或中止"，拒绝 |
| 切换/删除工作区 | 确认框"批量处理进行中，切换将中止并保留已完成结果"，确认后广播 abort |
| 关闭应用 | 正常关闭；in-flight 的 invoke 在 Rust 侧完成或被操作系统终止 |
| 加密文件 | `import_file` 抛 EncryptedError → 标记 failed，errorMessage = "加密文件已跳过" |
| 输出文件重名 | 自动追加 `_1`/`_2`... |
| 输出目录无权限 | 首个文件 export 失败时提示；其余文件继续（可能全失败） |
| 中止后重试 | 进入结果报告视图后点失败文件的"重试"按钮，单独跑一次（不受并发池管辖） |
| 一致性映射跨文件 | 由 Rust 后端工作区级映射保证；并发下同一文本首次写入谁先谁后可能略有差异，但不影响最终一致性 |

## 测试策略

### 单元测试
- `batchScheduler.ts`：并发数控制、abort 行为、worker 错误不影响整体
- `resolveOutputPath`：重名追加逻辑

### 集成/E2E 测试（Playwright + IPC Mock）
- 拖入 5 个文件 → 选全自动 → 验证进度条、3 个并发、最终结果报告
- 中途中止 → 验证 pending 不再启动、已完成保留
- 失败文件重试 → 验证单独重跑
- 点击 confirmed Tab 进入抽查 → 验证 result 正确加载

### 手动验收
- 20 个混合格式文件（xlsx/docx/pdf/csv）全自动处理，对比串行与并发耗时
- 加密文件混入，验证跳过行为
- 输出目录重名场景

## 改动清单

| 文件 | 类型 |
|---|---|
| `src/types/index.ts` | 扩展 `QueueFile`、新增 `BatchMode`/`BatchSession`、`MAX_CONCURRENCY` |
| `src/stores/workspaceStore.ts` | 新增 `batchSession` + actions |
| `src/utils/batchScheduler.ts` | **新增** |
| `src/hooks/useBatchAutoProcess.ts` | **新增** |
| `src/hooks/useAutoDesensitize.ts` | 抽取 `runDesensitizePipeline` 共享函数 |
| `src/components/CenterPanel/BatchModeSelector.tsx` | **新增** |
| `src/components/CenterPanel/BatchProgressBar.tsx` | **新增** |
| `src/components/CenterPanel/BatchResultReport.tsx` | **新增** |
| `src/components/CenterPanel/FileQueueTabs.tsx` | 扩展点击行为 |
| `src/components/CenterPanel/DropzoneView.tsx` | 多文件分支改为展示模式选择器 |
| `src/components/CenterPanel/index.tsx` | 根据 `batchSession.phase` 切换子视图 |
| `src/layouts/WorkspaceLayout.tsx` | 批量模式下工具栏行为 |
| `src/locales/{zh,en}.json` | 新增批量模式文案 |
| `src-tauri/` | **零改动**（理想路径）|

## 实施顺序建议（留给 writing-plans 细化）

1. 数据模型 + store actions（可独立提交，不影响现有功能）
2. `batchScheduler.ts` + 单元测试
3. 抽取 `runDesensitizePipeline` 共享函数，验证逐个确认模式不回归
4. `useBatchAutoProcess` + 最小 UI（先能跑起来）
5. 完善 UI（进度条、模式选择器、结果报告、抽查）
6. 边界与错误处理、重试、中止
7. i18n + E2E 测试
