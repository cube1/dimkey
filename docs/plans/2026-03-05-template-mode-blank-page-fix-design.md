# 模版替换模式页面空白修复 — 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复切换到模版替换模式后页面空白的 bug，并实现切换模式后自动重新处理当前文件。

**Architecture:** 修复 ComparisonView 中违反 React Hooks 规则的条件性 useMemo；修改 updateWorkspaceMode 保留文件路径并返回；StrategyPanel 切换模式后自动调用 processFile 重处理。

**Tech Stack:** React + TypeScript, Zustand

---

### Task 1: 修复 ComparisonView 条件性 useMemo

**Files:**
- Modify: `src/components/CenterPanel/ComparisonView.tsx:285-294`

**Step 1: 将条件内 useMemo 移到顶层**

当前 `sheetColumnRules` 的 `useMemo`（第 285-294 行）在 `if (hasCurrentResult)` 条件分支内，违反 React Hooks 规则。将其移到第 214 行之后（`templateReplacements` 的 useMemo 之后），即所有条件返回之前。

从 `if (hasCurrentResult && currentResult && currentFileContent)` 分支内（第 285-294 行）**删除**：
```tsx
    const sheetColumnRules = useMemo(() => {
      const result: Record<number, ColumnRule> = {};
      for (const [key, rule] of Object.entries(confirmedColumnRules)) {
        if (key.startsWith(`${activeSheetIndex}:`)) {
          const col = Number(key.split(":")[1]);
          result[col] = rule;
        }
      }
      return result;
    }, [confirmedColumnRules, activeSheetIndex]);
```

在第 214 行（`templateReplacements` useMemo 之后）**插入**同样的代码：
```tsx
  // 当前 sheet 的已确认列规则
  const sheetColumnRules = useMemo(() => {
    const result: Record<number, ColumnRule> = {};
    for (const [key, rule] of Object.entries(confirmedColumnRules)) {
      if (key.startsWith(`${activeSheetIndex}:`)) {
        const col = Number(key.split(":")[1]);
        result[col] = rule;
      }
    }
    return result;
  }, [confirmedColumnRules, activeSheetIndex]);
```

同时删除原 `if` 分支内第 284 行的注释 `// 当前 sheet 的已确认列规则（提取为 Record<number, ColumnRule>）`。

**Step 2: 验证编译通过**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npm run build`
Expected: 编译成功，无错误

**Step 3: Commit**

```bash
git add src/components/CenterPanel/ComparisonView.tsx
git commit -m "fix: 将条件分支内的 useMemo 移到组件顶层，修复 React Hooks 规则违反"
```

---

### Task 2: 修改 updateWorkspaceMode 保留文件路径并返回

**Files:**
- Modify: `src/stores/workspaceStore.ts:87,264-275`

**Step 1: 修改类型签名和实现**

将 `updateWorkspaceMode` 的类型签名（第 87 行）从：
```tsx
updateWorkspaceMode: (mode: WorkspaceMode) => Promise<void>;
```
改为：
```tsx
updateWorkspaceMode: (mode: WorkspaceMode) => Promise<string | null>;
```

将实现（第 264-275 行）从：
```tsx
  updateWorkspaceMode: async (mode) => {
    await updateWorkspaceField(get, set, { mode });
    // 切换模式时清除当前文件的识别结果和脱敏结果
    set({
      currentSensitiveItems: [],
      rawSensitiveItems: [],
      currentResult: null,
      currentFileContent: null,
      currentFilePath: null,
      processingStep: "idle",
    });
  },
```
改为：
```tsx
  updateWorkspaceMode: async (mode) => {
    const filePath = get().currentFilePath;
    await updateWorkspaceField(get, set, { mode });
    // 切换模式时清除识别结果和脱敏结果，但保留 filePath 用于重新处理
    set({
      currentSensitiveItems: [],
      rawSensitiveItems: [],
      currentResult: null,
      currentFileContent: null,
      currentFilePath: null,
      processingStep: "idle",
    });
    return filePath;
  },
```

**Step 2: 验证编译通过**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npm run build`
Expected: 编译成功

**Step 3: Commit**

```bash
git add src/stores/workspaceStore.ts
git commit -m "fix: updateWorkspaceMode 返回当前文件路径，支持模式切换后重新处理"
```

---

### Task 3: StrategyPanel 切换模式后自动重新处理

**Files:**
- Modify: `src/components/StrategyPanel/index.tsx`

**Step 1: 添加 processFile 依赖和模式切换处理函数**

添加 `useAutoDesensitize` 导入和 `useCallback`：
```tsx
import { useCallback } from "react";
import { Settings2 } from "lucide-react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useAutoDesensitize } from "../../hooks/useAutoDesensitize";
import { TypeSelector } from "../TypeSelector";
import { RulesSection } from "./RulesSection";
import { DictSection } from "./DictSection";
import { OutputSection } from "./OutputSection";
```

在组件内添加 `processFile` 和 `handleModeSwitch`：
```tsx
export function StrategyPanel() {
  const wsData = useWorkspaceStore((s) => s.activeWorkspaceData);
  const updateWorkspaceMode = useWorkspaceStore((s) => s.updateWorkspaceMode);
  const { processFile } = useAutoDesensitize();
  const workspaceMode = wsData?.workspace.mode || "Desensitize";
  const isTemplateMode = workspaceMode === "TemplateReplace";

  const handleModeSwitch = useCallback(async (mode: "Desensitize" | "TemplateReplace") => {
    const filePath = await updateWorkspaceMode(mode);
    if (filePath) {
      await processFile(filePath);
    }
  }, [updateWorkspaceMode, processFile]);
```

将两个 `onClick` 从：
```tsx
onClick={() => updateWorkspaceMode("Desensitize")}
onClick={() => updateWorkspaceMode("TemplateReplace")}
```
改为：
```tsx
onClick={() => handleModeSwitch("Desensitize")}
onClick={() => handleModeSwitch("TemplateReplace")}
```

**Step 2: 验证编译通过**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npm run build`
Expected: 编译成功

**Step 3: 手动测试**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && cargo tauri dev`

测试步骤：
1. 创建工作区并导入一个文件
2. 确认看到脱敏结果（comparison 视图）
3. 切换到"模版替换"模式
4. **预期**：自动重新处理文件，显示 processing → comparison，不再白屏
5. 切换回"脱敏模式"
6. **预期**：同样自动重新处理，显示正常结果

**Step 4: Commit**

```bash
git add src/components/StrategyPanel/index.tsx
git commit -m "fix: 切换工作模式后自动重新处理当前文件，避免页面空白"
```
