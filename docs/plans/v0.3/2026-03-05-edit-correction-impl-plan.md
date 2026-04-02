# 脱敏预览编辑纠正 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 提升脱敏预览视图中"取消标记"和"手动标记"功能的可发现性，修复词典保存后检测不生效的 bug。

**Architecture:** 三处独立改动：(1) HighlightedText 高亮项添加 hover 边框反馈 (2) SensitivePopover 脱敏模式将"取消标记"移到顶部 (3) 词典变更后触发 `detect_by_dict` 重检测并更新 store。

**Tech Stack:** React + TypeScript + TailwindCSS + Zustand + Tauri IPC

---

### Task 1: HighlightedText hover 交互增强

**Files:**
- Modify: `src/components/HighlightedText/index.tsx:82-102`

**Step 1: 修改普通模式高亮样式**

在 `src/components/HighlightedText/index.tsx` 第 100 行，为普通模式（非 diffMode、非 templateReplacements）的高亮 span 添加 `hover:ring-2 hover:ring-current/20`：

```tsx
// 第 99-101 行，将：
} else {
  highlightClass = `${info.bgClass} ${info.textClass} rounded px-0.5 py-px ring-1 ring-inset ring-current/5 cursor-pointer hover:brightness-95 transition-all${activeRing}`;
  titleText = `${info.label} (${seg.item.source})`;
}

// 改为：
} else {
  highlightClass = `${info.bgClass} ${info.textClass} rounded px-0.5 py-px ring-1 ring-inset ring-current/5 cursor-pointer hover:ring-2 hover:ring-current/20 transition-all${activeRing}`;
  titleText = `点击编辑 · ${info.label}`;
}
```

**Step 2: 修改模版替换模式高亮样式**

在同文件第 89 行和第 93 行，为模版替换模式的两种高亮也添加一致的 hover ring：

```tsx
// 第 89 行（有替换值），将：
highlightClass = `bg-teal-100 text-teal-800 rounded px-0.5 py-px ring-1 ring-inset ring-teal-300/30 cursor-pointer hover:brightness-95 transition-all${activeRing}`;

// 改为：
highlightClass = `bg-teal-100 text-teal-800 rounded px-0.5 py-px ring-1 ring-inset ring-teal-300/30 cursor-pointer hover:ring-2 hover:ring-teal-400/40 transition-all${activeRing}`;

// 第 93 行（无替换值），将：
highlightClass = `bg-slate-100/60 text-slate-400 rounded px-0.5 py-px cursor-pointer hover:brightness-95 transition-all${activeRing}`;

// 改为：
highlightClass = `bg-slate-100/60 text-slate-400 rounded px-0.5 py-px cursor-pointer hover:ring-2 hover:ring-slate-300/40 transition-all${activeRing}`;
```

**Step 3: 修改 diff 模式高亮样式**

在同文件第 97 行，为 diff 模式也添加 hover ring：

```tsx
// 第 97 行，将：
highlightClass = `${DIFF_STYLES[diffMode]} rounded px-0.5 py-px cursor-pointer hover:brightness-95 transition-all${activeRing}`;

// 改为：
highlightClass = `${DIFF_STYLES[diffMode]} rounded px-0.5 py-px cursor-pointer hover:ring-2 hover:ring-current/20 transition-all${activeRing}`;
```

**Step 4: 验证编译**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npm run dev`
Expected: 无 TypeScript 编译错误，页面正常加载

**Step 5: 提交**

```bash
git add src/components/HighlightedText/index.tsx
git commit -m "feat: 高亮项添加 hover ring 效果增强可发现性"
```

---

### Task 2: SensitivePopover "取消标记"移到顶部

**Files:**
- Modify: `src/components/SensitivePopover/index.tsx:281-373`

**Step 1: 重构脱敏模式顶部布局**

在 `src/components/SensitivePopover/index.tsx` 的脱敏模式 return 分支（第 281 行起），将原始文本和"取消标记"改为同行 flex 布局，并移除底部的独立按钮。

将第 283-294 行（原始文本 + 类型标签）替换为：

```tsx
<div className="w-72 rounded-lg border border-gray-200 bg-white p-4 shadow-lg">
  {/* 顶部：原文 + 取消标记 */}
  <div className="flex items-start justify-between gap-2 mb-2">
    <p className="text-base font-bold text-gray-900 break-all flex-1 min-w-0">
      {item.text}
    </p>
    <button
      onClick={handleRemove}
      className="shrink-0 flex items-center gap-0.5 px-1.5 py-0.5 text-xs text-red-500 hover:text-red-700 hover:bg-red-50 rounded transition-colors"
      title="取消标记此敏感项"
    >
      <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
      </svg>
      取消
    </button>
  </div>

  {/* 类型标签 */}
  <span
    className={`inline-block rounded-full px-2 py-0.5 text-xs font-medium ${typeInfo.bgClass} ${typeInfo.textClass}`}
  >
    {typeInfo.label}
  </span>
```

同时删除底部的取消标记按钮（第 364-370 行）：

```tsx
// 删除这段：
{/* 取消标记按钮 */}
<button
  onClick={handleRemove}
  className="mt-3 w-full rounded bg-red-50 px-3 py-1.5 text-sm font-medium text-red-600 transition-colors hover:bg-red-100"
>
  取消标记
</button>
```

**Step 2: 验证编译**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npm run dev`
Expected: 无 TypeScript 编译错误

**Step 3: 提交**

```bash
git add src/components/SensitivePopover/index.tsx
git commit -m "feat: Popover 取消标记移至顶部提升可发现性"
```

---

### Task 3: 修复词典保存后重检测

**Files:**
- Modify: `src/components/StrategyPanel/DictSection.tsx`
- Modify: `src/components/SensitivePopover/index.tsx`（模版替换模式的 handleAddToDict）
- Read reference: `src/stores/detectStore.ts:112-118`（`replaceDictItems` 已存在）

**背景：**
`detectStore.replaceDictItems()` 方法已存在但从未被调用。词典变更后需要：
1. 调用 `invoke("detect_by_dict", { content: fileContent })` 重新检测
2. 用结果调用 `replaceDictItems()` 更新 store

**Step 1: DictSection 添加重检测逻辑**

在 `src/components/StrategyPanel/DictSection.tsx` 中：

1. 导入需要的依赖：

```tsx
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../../stores/appStore";
import { useDetectStore } from "../../stores/detectStore";
import type { SensitiveItem } from "../../types";
```

2. 在 `DictSection` 组件内获取 store 方法：

```tsx
const fileContent = useAppStore((s) => s.fileContent);
const replaceDictItems = useDetectStore((s) => s.replaceDictItems);
```

3. 添加重检测辅助函数：

```tsx
const reDetectDict = useCallback(async () => {
  if (!fileContent) return;
  try {
    const dictItems = await invoke<SensitiveItem[]>("detect_by_dict", { content: fileContent });
    replaceDictItems(dictItems);
  } catch {
    // 静默处理
  }
}, [fileContent, replaceDictItems]);
```

4. 在 `handleAdd` 和 `handleRemove` 的 `await updateDictEntries(...)` 成功后调用 `reDetectDict()`：

```tsx
// handleAdd 中，在 setText("") 后添加：
await reDetectDict();

// handleRemove 中，在 updateDictEntries 成功后添加：
await reDetectDict();
```

**Step 2: SensitivePopover 模版替换模式也触发重检测**

在 `src/components/SensitivePopover/index.tsx` 中：

1. 导入：

```tsx
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../../stores/appStore";
import type { SensitiveItem } from "../../types";
```

（注意：`SensitiveItem` 已在现有 import 中，只需补充 `invoke` 和 `useAppStore`）

2. 在组件内获取 store：

```tsx
const fileContent = useAppStore((s) => s.fileContent);
const replaceDictItems = useDetectStore((s) => s.replaceDictItems);
```

（注意：`useDetectStore` 已导入，`replaceDictItems` 需要从中取出）

3. 在 `handleAddToDict` 的 `await updateDictEntries(newEntries)` 成功后，触发重检测：

```tsx
// 在 toast.success(...) 之后、onClose() 之前添加：
if (fileContent) {
  try {
    const dictItems = await invoke<SensitiveItem[]>("detect_by_dict", { content: fileContent });
    replaceDictItems(dictItems);
  } catch {
    // 静默处理
  }
}
```

**Step 3: 验证编译**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npm run dev`
Expected: 无 TypeScript 编译错误

**Step 4: 提交**

```bash
git add src/components/StrategyPanel/DictSection.tsx src/components/SensitivePopover/index.tsx
git commit -m "fix: 词典变更后触发重检测更新识别结果"
```
