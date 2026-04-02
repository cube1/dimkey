# INK-018 批量文件导入队列 实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在现有工作区中支持多文件拖入，Tab 栏展示文件队列，逐个微调确认并导出。

**Architecture:** 纯前端改动，workspaceStore 新增队列状态，DropzoneView 支持多文件，CenterPanel 顶部新增 FileQueueTabs 组件，WorkspaceLayout 工具栏导出按钮适配批量模式。Rust 后端零改动。

**Tech Stack:** React + TypeScript + Zustand + TailwindCSS + Tauri v2 IPC

**Design Doc:** `docs/plans/2026-03-10-batch-import-queue-design.md`

---

### Task 1: 新增 QueueFile 类型定义

**Files:**
- Modify: `src/types/index.ts`

**Step 1: 在 types/index.ts 中添加 QueueFile 接口和 MAX_QUEUE_SIZE 常量**

在文件末尾（`AutoDesensitizeStep` 类型定义之后）添加：

```typescript
/** 批量导入队列中的单个文件 */
export interface QueueFile {
  id: string;
  filePath: string;
  fileName: string;
  status: "pending" | "processing" | "confirmed" | "failed";
  errorMessage?: string;
}

/** 批量导入最大文件数 */
export const MAX_QUEUE_SIZE = 20;
```

**Step 2: 验证 TypeScript 编译通过**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: 无新增错误

**Step 3: Commit**

```bash
git add src/types/index.ts
git commit -m "feat(INK-018): 新增 QueueFile 类型定义"
```

---

### Task 2: workspaceStore 新增队列状态和 actions

**Files:**
- Modify: `src/stores/workspaceStore.ts`

**Step 1: 在 WorkspaceState 接口中新增字段和方法**

在 `src/stores/workspaceStore.ts` 的 import 中添加 `QueueFile` 和 `MAX_QUEUE_SIZE`：

```typescript
import type {
  // ... 现有 imports
  QueueFile,
} from "../types";
import { MAX_QUEUE_SIZE } from "../types";
```

在 `WorkspaceState` 接口的 `passwordModal` 字段之后、`setPasswordModal` 方法之前，添加：

```typescript
  // --- 批量文件队列 ---
  /** 文件队列（批量导入时使用） */
  fileQueue: QueueFile[];
  /** 当前处理的文件在队列中的索引 */
  activeQueueIndex: number;
```

在接口的 `setActiveSheetIndex` 方法之后，添加：

```typescript
  // --- 批量队列操作 ---
  /** 初始化文件队列（批量导入时调用） */
  initFileQueue: (files: QueueFile[]) => void;
  /** 更新队列中指定文件的状态 */
  updateQueueFileStatus: (id: string, status: QueueFile["status"], errorMessage?: string) => void;
  /** 前进到下一个 pending 文件，返回该文件或 null（全部完成） */
  advanceQueue: () => QueueFile | null;
  /** 清空文件队列 */
  clearFileQueue: () => void;
  /** 是否处于批量模式（队列长度 > 1） */
  isBatchMode: () => boolean;
  /** 队列中是否有未处理的文件 */
  hasUnfinishedFiles: () => boolean;
```

**Step 2: 添加初始值和方法实现**

在 create store 的初始值区域（`activeSheetIndex: 0` 之后）添加：

```typescript
  fileQueue: [],
  activeQueueIndex: -1,
```

在 `setActiveSheetIndex` 实现之后，添加：

```typescript
  initFileQueue: (files) => set({ fileQueue: files, activeQueueIndex: 0 }),

  updateQueueFileStatus: (id, status, errorMessage) =>
    set((s) => ({
      fileQueue: s.fileQueue.map((f) =>
        f.id === id ? { ...f, status, ...(errorMessage !== undefined && { errorMessage }) } : f
      ),
    })),

  advanceQueue: () => {
    const state = get();
    const nextIndex = state.fileQueue.findIndex(
      (f, i) => i > state.activeQueueIndex && f.status === "pending"
    );
    if (nextIndex >= 0) {
      set({ activeQueueIndex: nextIndex });
      return state.fileQueue[nextIndex];
    }
    return null;
  },

  clearFileQueue: () => set({ fileQueue: [], activeQueueIndex: -1 }),

  isBatchMode: () => get().fileQueue.length > 1,

  hasUnfinishedFiles: () =>
    get().fileQueue.some((f) => f.status === "pending" || f.status === "processing"),
```

**Step 3: 在 selectWorkspace 中清空队列**

在 `selectWorkspace` 方法的 `set({...})` 调用中，添加 `fileQueue: [], activeQueueIndex: -1,`：

```typescript
  selectWorkspace: async (id) => {
    try {
      const data = await invoke<WorkspaceData>("get_workspace", { id });
      set({
        // ... 现有字段
        fileQueue: [],
        activeQueueIndex: -1,
      });
    } catch (e) {
      // ...
    }
  },
```

同样在 `deleteWorkspace` 的 `set({...})` 中添加 `fileQueue: [], activeQueueIndex: -1,`。

**Step 4: 验证 TypeScript 编译通过**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: 无新增错误

**Step 5: Commit**

```bash
git add src/stores/workspaceStore.ts src/types/index.ts
git commit -m "feat(INK-018): workspaceStore 新增批量队列状态和 actions"
```

---

### Task 3: DropzoneView 支持多文件拖入

**Files:**
- Modify: `src/components/CenterPanel/DropzoneView.tsx`

**Step 1: 改造拖放事件处理，支持多文件**

在文件顶部 import 中添加：

```typescript
import { MAX_QUEUE_SIZE } from "../../types";
import type { QueueFile } from "../../types";
```

从 workspaceStore 中取出新增的方法。在 `DropzoneView` 函数体中已有的 store 读取之后添加：

```typescript
const fileQueue = useWorkspaceStore((s) => s.fileQueue);
const initFileQueue = useWorkspaceStore((s) => s.initFileQueue);
const isBatchMode = useWorkspaceStore((s) => s.isBatchMode);
const hasUnfinishedFiles = useWorkspaceStore((s) => s.hasUnfinishedFiles);
```

**Step 2: 新增 handleImportFiles 批量处理函数**

在 `handleImportFile` 之后添加：

```typescript
const handleImportFiles = useCallback(
  async (paths: string[]) => {
    // 批量模式进行中，拒绝新增
    if (isBatchMode() && hasUnfinishedFiles()) {
      toast.error("请先完成当前队列中的文件处理");
      return;
    }

    // 校验并过滤
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
      toast.error(`${invalidNames.length} 个文件格式不支持，已跳过`);
    }

    if (validPaths.length === 0) return;

    // 单文件走原有流程
    if (validPaths.length === 1) {
      await handleImportFile(validPaths[0]);
      return;
    }

    // 截断到上限
    const filesToProcess = validPaths.slice(0, MAX_QUEUE_SIZE);
    if (validPaths.length > MAX_QUEUE_SIZE) {
      toast(`最多同时处理 ${MAX_QUEUE_SIZE} 个文件，已取前 ${MAX_QUEUE_SIZE} 个`, { icon: "ℹ️" });
    }

    // 构建队列
    const queue: QueueFile[] = filesToProcess.map((p, i) => ({
      id: `q_${Date.now()}_${i}`,
      filePath: p,
      fileName: p.split(/[/\\]/).pop() || p,
      status: i === 0 ? "processing" : "pending",
    }));

    initFileQueue(queue);
    await processFile(filesToProcess[0]);
  },
  [handleImportFile, processFile, isBatchMode, hasUnfinishedFiles, initFileQueue]
);
```

**Step 3: 修改 onDragDropEvent 使用新函数**

将拖放事件监听中的 `handleImportFile(paths[0])` 改为 `handleImportFiles(paths)`：

```typescript
// 原来：
// if (paths.length > 0) {
//   handleImportFile(paths[0]);
// }

// 改为：
if (paths.length > 0) {
  handleImportFiles(paths);
}
```

同时更新 useEffect 的依赖数组，将 `handleImportFile` 替换为 `handleImportFiles`。

**Step 4: 修改"选择文件"按钮支持多选**

在 `handleClickSelect` 函数中，将 `open` 调用的 `multiple: false` 改为 `multiple: true`，并适配返回值：

```typescript
const handleClickSelect = async () => {
  // 批量模式进行中，拒绝新增
  if (isBatchMode() && hasUnfinishedFiles()) {
    toast.error("请先完成当前队列中的文件处理");
    return;
  }

  const selected = await open({
    multiple: true,
    filters: [
      {
        name: "支持的文件",
        extensions: ["xlsx", "xls", "csv", "tsv", "docx", "txt"],
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
```

**Step 5: 批量模式下隐藏还原入口**

在还原操作区域外包一个条件判断：

```typescript
{/* 还原操作 - 批量模式下隐藏 */}
{!(isBatchMode() && hasUnfinishedFiles()) && (
  <div className="px-6 pb-3 shrink-0">
    {/* ... 现有还原按钮 ... */}
  </div>
)}
```

**Step 6: 验证编译**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: 无新增错误

**Step 7: Commit**

```bash
git add src/components/CenterPanel/DropzoneView.tsx
git commit -m "feat(INK-018): DropzoneView 支持多文件拖入和选择"
```

---

### Task 4: 创建 FileQueueTabs 组件

**Files:**
- Create: `src/components/CenterPanel/FileQueueTabs.tsx`

**Step 1: 创建组件文件**

```typescript
import { useEffect, useRef } from "react";
import { CheckCircle2, XCircle, Circle, Loader2 } from "lucide-react";
import toast from "react-hot-toast";
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
};

export function FileQueueTabs() {
  const fileQueue = useWorkspaceStore((s) => s.fileQueue);
  const activeQueueIndex = useWorkspaceStore((s) => s.activeQueueIndex);
  const scrollRef = useRef<HTMLDivElement>(null);
  const activeTabRef = useRef<HTMLButtonElement>(null);

  // 当前处理的 Tab 自动滚动到可见区域
  useEffect(() => {
    if (activeTabRef.current) {
      activeTabRef.current.scrollIntoView({ behavior: "smooth", block: "nearest", inline: "center" });
    }
  }, [activeQueueIndex]);

  if (fileQueue.length <= 1) return null;

  const handleTabClick = (file: QueueFile) => {
    switch (file.status) {
      case "confirmed":
        toast("该文件已导出", { icon: "✓" });
        break;
      case "failed":
        toast.error(file.errorMessage || "处理失败");
        break;
      case "pending":
        toast("请按顺序处理文件", { icon: "ℹ️" });
        break;
      case "processing":
        // 当前正在处理，无操作
        break;
    }
  };

  // 统计进度
  const doneCount = fileQueue.filter((f) => f.status === "confirmed" || f.status === "failed").length;

  return (
    <div className="bg-white border-b border-slate-200 px-3 py-1.5 shrink-0">
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
                  ${file.status === "pending" ? "opacity-60 cursor-default" : "cursor-pointer hover:shadow-xs"}
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
```

**Step 2: 验证编译**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: 无新增错误

**Step 3: Commit**

```bash
git add src/components/CenterPanel/FileQueueTabs.tsx
git commit -m "feat(INK-018): 创建 FileQueueTabs 队列 Tab 栏组件"
```

---

### Task 5: CenterPanel 集成 FileQueueTabs

**Files:**
- Modify: `src/components/CenterPanel/index.tsx`

**Step 1: 导入并渲染 FileQueueTabs**

在 import 区域添加：

```typescript
import { FileQueueTabs } from "./FileQueueTabs";
```

在 CenterPanel 的 JSX 中，将 FileQueueTabs 放在 centerView 路由之前。修改 return 部分为：

```typescript
return (
  <>
    {centerView === "empty" ? (
      <EmptyDropzoneView />
    ) : (
      <>
        <FileQueueTabs />
        {centerView === "dropzone" && <DropzoneView />}
        {centerView === "processing" && <ProcessingView />}
        {centerView === "comparison" && <ComparisonView />}
        {centerView === "restore" && <RestoreView />}
      </>
    )}
    <PasswordModal
      visible={passwordModal.visible}
      fileType={passwordModal.fileType}
      attemptsLeft={passwordModal.attemptsLeft}
      errorMessage={passwordModal.errorMessage}
      onSubmit={handlePasswordSubmit}
      onCancel={handlePasswordCancel}
    />
  </>
);
```

**Step 2: 验证编译**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: 无新增错误

**Step 3: Commit**

```bash
git add src/components/CenterPanel/index.tsx
git commit -m "feat(INK-018): CenterPanel 集成 FileQueueTabs"
```

---

### Task 6: useAutoDesensitize 适配批量模式失败处理

**Files:**
- Modify: `src/hooks/useAutoDesensitize.ts`

**Step 1: 在 processFile 的通用错误处理中，批量模式下标记失败并跳到下一个**

在 `processFile` 的 catch 块中，现有通用错误处理（`else` 分支，约 382-390 行）之后的逻辑需要改造。

找到 processFile 的 catch 块中的通用错误处理部分：

```typescript
      } else {
        // 通用错误（保留现有逻辑）
        const message =
          typeof err === "string" ? err :
          err instanceof Error ? err.message : "处理失败";
        toast.error(message);
        store.setProcessingStep("idle");
        store.setCenterView("dropzone");
      }
```

改为：

```typescript
      } else {
        const message =
          typeof err === "string" ? err :
          err instanceof Error ? err.message : "处理失败";

        // 批量模式：标记失败，自动跳到下一个
        const wsStore = useWorkspaceStore.getState();
        if (wsStore.isBatchMode()) {
          const currentFile = wsStore.fileQueue[wsStore.activeQueueIndex];
          if (currentFile) {
            wsStore.updateQueueFileStatus(currentFile.id, "failed", message);
          }
          toast.error(`${filePath.split(/[/\\]/).pop()} 处理失败：${message}`);
          store.setProcessingStep("idle");

          // 尝试处理下一个文件
          const nextFile = wsStore.advanceQueue();
          if (nextFile) {
            wsStore.updateQueueFileStatus(nextFile.id, "processing");
            // 释放锁后异步调用下一个（避免递归栈溢出）
            isProcessingRef.current = false;
            setTimeout(() => processFile(nextFile.filePath), 0);
            return; // 跳过 finally 中的 isProcessingRef = false
          } else {
            toast.success("所有文件已处理完成");
            store.setCenterView("dropzone");
          }
        } else {
          toast.error(message);
          store.setProcessingStep("idle");
          store.setCenterView("dropzone");
        }
      }
```

**Step 2: 在 processFile 成功完成后，更新队列状态**

在 processFile 的 try 块末尾（`store.setProcessingStep("done")` 之后、catch 之前），添加：

```typescript
      // 批量模式：标记当前文件为 processing（等待用户确认导出）
      const wsStoreAfter = useWorkspaceStore.getState();
      if (wsStoreAfter.isBatchMode()) {
        const currentFile = wsStoreAfter.fileQueue[wsStoreAfter.activeQueueIndex];
        if (currentFile) {
          wsStoreAfter.updateQueueFileStatus(currentFile.id, "processing");
        }
      }
```

注意：这段代码放在 `store.setProcessingStep("done")` 和 `store.setCenterView("comparison")` 之后。

**Step 3: 验证编译**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: 无新增错误

**Step 4: Commit**

```bash
git add src/hooks/useAutoDesensitize.ts
git commit -m "feat(INK-018): useAutoDesensitize 适配批量模式失败处理和队列推进"
```

---

### Task 7: WorkspaceLayout 工具栏适配批量导出按钮

**Files:**
- Modify: `src/layouts/WorkspaceLayout.tsx`

**Step 1: 从 store 读取队列状态**

在 `WorkspaceLayout` 函数体中现有的 store 读取之后添加：

```typescript
const fileQueue = useWorkspaceStore((s) => s.fileQueue);
const activeQueueIndex = useWorkspaceStore((s) => s.activeQueueIndex);
const updateQueueFileStatus = useWorkspaceStore((s) => s.updateQueueFileStatus);
const advanceQueue = useWorkspaceStore((s) => s.advanceQueue);
const isBatchMode = useWorkspaceStore((s) => s.isBatchMode);
```

在 import 中添加 `SkipForward` 图标：

```typescript
import { PanelLeft, PanelRight, ArrowLeft, Download, ClipboardCopy, Check, SkipForward } from "lucide-react";
```

同时导入 `useAutoDesensitize`：

```typescript
import { useAutoDesensitize } from "../hooks/useAutoDesensitize";
```

在函数体中取出 `processFile`：

```typescript
const { processFile } = useAutoDesensitize();
```

**Step 2: 新增 handleExportAndNext 函数**

在 `handleComparisonExport` 之后添加：

```typescript
// 批量模式：导出并处理下一个
const handleExportAndNext = useCallback(async () => {
  await handleComparisonExport();

  // 导出成功后标记 confirmed 并推进队列
  const store = useWorkspaceStore.getState();
  const currentFile = store.fileQueue[store.activeQueueIndex];
  if (currentFile) {
    store.updateQueueFileStatus(currentFile.id, "confirmed");
  }

  const nextFile = store.advanceQueue();
  if (nextFile) {
    store.updateQueueFileStatus(nextFile.id, "processing");
    await processFile(nextFile.filePath);
  } else {
    toast.success("所有文件已处理完成");
    store.setCenterView("dropzone");
  }
}, [handleComparisonExport, processFile]);

// 批量模式：仅导出（不自动跳下一个）
const handleExportOnly = useCallback(async () => {
  await handleComparisonExport();

  const store = useWorkspaceStore.getState();
  const currentFile = store.fileQueue[store.activeQueueIndex];
  if (currentFile) {
    store.updateQueueFileStatus(currentFile.id, "confirmed");
  }
}, [handleComparisonExport]);
```

**Step 3: 修改 renderToolbarActions 适配批量模式**

将现有的 comparison 分支改为：

```typescript
if (centerView === "comparison" && (currentResult || (isTemplateMode && currentFileContent))) {
  const batchMode = isBatchMode();
  const isLastFile = batchMode && !fileQueue.some((f, i) => i > activeQueueIndex && f.status === "pending");

  if (batchMode && !isLastFile) {
    return (
      <div className="flex items-center gap-2">
        <button
          onClick={handleExportOnly}
          disabled={exporting}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 bg-white text-slate-600 text-xs font-medium rounded-md border border-slate-300 hover:bg-slate-50 shadow-sm transition-colors disabled:opacity-50"
        >
          <Download className="w-3.5 h-3.5" />
          {exporting ? "导出中…" : "仅导出"}
        </button>
        <button
          onClick={handleExportAndNext}
          disabled={exporting}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 bg-primary-600 text-white text-xs font-medium rounded-md hover:bg-primary-700 shadow-sm transition-colors disabled:opacity-50"
        >
          <SkipForward className="w-3.5 h-3.5" />
          {exporting ? "导出中…" : "导出并处理下一个"}
        </button>
      </div>
    );
  }

  // 单文件模式 或 批量模式最后一个文件：现有导出按钮
  return (
    <button
      onClick={batchMode ? handleExportOnly : handleComparisonExport}
      disabled={exporting}
      className="inline-flex items-center gap-1.5 px-3 py-1.5 bg-primary-600 text-white text-xs font-medium rounded-md hover:bg-primary-700 shadow-sm transition-colors disabled:opacity-50"
    >
      <Download className="w-3.5 h-3.5" />
      {exporting ? "导出中…" : "导出文件"}
    </button>
  );
}
```

**Step 4: 验证编译**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: 无新增错误

**Step 5: Commit**

```bash
git add src/layouts/WorkspaceLayout.tsx
git commit -m "feat(INK-018): WorkspaceLayout 适配批量模式导出按钮"
```

---

### Task 8: 切换工作区时的队列保护

**Files:**
- Modify: `src/components/WorkspaceList/index.tsx`（或工作区切换的触发组件）

**Step 1: 找到工作区切换和删除的触发位置**

Run: `grep -n "selectWorkspace\|deleteWorkspace" src/components/WorkspaceList/index.tsx | head -20`

**Step 2: 在切换工作区前检查队列状态**

在调用 `selectWorkspace` 的地方，添加确认逻辑：

```typescript
const handleSelectWorkspace = async (id: string) => {
  const store = useWorkspaceStore.getState();
  if (store.isBatchMode() && store.hasUnfinishedFiles()) {
    const unfinished = store.fileQueue.filter((f) => f.status === "pending" || f.status === "processing").length;
    const confirmed = window.confirm(`当前有 ${unfinished} 个文件未处理，切换工作区将放弃队列，是否继续？`);
    if (!confirmed) return;
    store.clearFileQueue();
  }
  await selectWorkspace(id);
};
```

对 `deleteWorkspace` 做同样处理。

**Step 3: 验证编译**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: 无新增错误

**Step 4: Commit**

```bash
git add src/components/WorkspaceList/index.tsx
git commit -m "feat(INK-018): 切换/删除工作区时保护未完成的批量队列"
```

---

### Task 9: 返回按钮适配批量模式

**Files:**
- Modify: `src/layouts/WorkspaceLayout.tsx`

**Step 1: 修改 handleGoBack 在批量模式下的行为**

将现有 `handleGoBack` 修改为：

```typescript
const handleGoBack = useCallback(() => {
  const store = useWorkspaceStore.getState();

  if (centerView === "restore") {
    setRestoreResult(null);
    setCenterView("dropzone");
    return;
  }

  // 批量模式：返回等于放弃当前文件，跳到下一个
  if (store.isBatchMode() && store.hasUnfinishedFiles()) {
    const currentFile = store.fileQueue[store.activeQueueIndex];
    if (currentFile && currentFile.status === "processing") {
      // 当前文件未导出就返回，标记为 failed
      store.updateQueueFileStatus(currentFile.id, "failed", "用户跳过");
    }
    setCurrentResult(null);

    const nextFile = store.advanceQueue();
    if (nextFile) {
      store.updateQueueFileStatus(nextFile.id, "processing");
      processFile(nextFile.filePath);
      return;
    } else {
      toast.success("所有文件已处理完成");
    }
  }

  setCurrentResult(null);
  setCenterView("dropzone");
}, [centerView, setRestoreResult, setCurrentResult, setCenterView, processFile]);
```

**Step 2: 验证编译**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: 无新增错误

**Step 3: Commit**

```bash
git add src/layouts/WorkspaceLayout.tsx
git commit -m "feat(INK-018): 返回按钮适配批量模式，支持跳过当前文件"
```

---

### Task 10: 端到端手动测试

**Files:** 无代码改动

**Step 1: 启动开发模式**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && cargo tauri dev`

**Step 2: 手动测试矩阵**

| # | 场景 | 预期结果 |
|---|------|---------|
| 1 | 拖入单个文件 | 行为与之前完全一致，无 Tab 栏 |
| 2 | 拖入 3 个 xlsx 文件 | Tab 栏显示 3 个文件，第一个自动处理 |
| 3 | 第一个文件微调后点"导出并处理下一个" | 弹出保存对话框 → 导出 → Tab 标记绿色 → 自动处理第二个 |
| 4 | 最后一个文件 | 只显示"导出文件"按钮 |
| 5 | 拖入包含 1 个不支持格式的文件 | Toast 提示跳过，其余正常进队列 |
| 6 | 拖入 > 20 个文件 | Toast 提示截断，取前 20 个 |
| 7 | 队列进行中再拖入文件 | Toast 提示"请先完成当前队列" |
| 8 | 队列进行中切换工作区 | 弹出确认框 |
| 9 | 文件处理失败（如损坏文件） | Tab 标记红色，自动跳到下一个 |
| 10 | 点击各状态的 Tab | confirmed→Toast, failed→Toast显示原因, pending→提示按顺序 |

**Step 3: 修复发现的问题（如有）**

根据测试结果修复 bug。

**Step 4: 最终 Commit**

```bash
git add -A
git commit -m "fix(INK-018): 修复端到端测试中发现的问题"
```
