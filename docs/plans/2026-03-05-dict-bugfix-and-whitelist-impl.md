# 自定义词典 Bug 修复 + 白名单排除功能 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复自定义词典添加后不高亮/不脱敏的 bug，并新增工作区级白名单排除功能。

**Architecture:** Bug 修复通过让 `useReDetectDict` hook 同时更新 workspaceStore 和 detectStore 来解决两个 store 不同步问题。白名单功能在 Workspace 数据模型中新增 `whitelist` 字段，在前端合并识别结果后按白名单过滤，UI 上在 SensitivePopover 浮层加"加入白名单"按钮，右侧栏加折叠式排除词展示区。

**Tech Stack:** Rust (serde, workspace model), React + TypeScript + Zustand + TailwindCSS

---

## Task 1: 修复 useReDetectDict — 同步 workspaceStore

**Files:**
- Modify: `src/hooks/useReDetectDict.ts`

**Step 1: 修改 useReDetectDict 同时更新 workspaceStore**

将 `useReDetectDict.ts` 全部替换为：

```typescript
import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useWorkspaceStore } from "../stores/workspaceStore";
import { useDetectStore } from "../stores/detectStore";
import type { SensitiveItem } from "../types";

/**
 * 共享 hook：重新执行词典检测并更新 detectStore + workspaceStore
 * 用于词典条目变更后刷新高亮
 */
export function useReDetectDict() {
  const replaceDictItems = useDetectStore((s) => s.replaceDictItems);

  const reDetectDict = useCallback(async () => {
    const store = useWorkspaceStore.getState();
    const fileContent = store.currentFileContent;
    const wsData = store.activeWorkspaceData;
    const dictEntries = wsData?.workspace.dict_entries || [];
    if (!fileContent) return;

    try {
      const dictItems = dictEntries.length > 0
        ? await invoke<SensitiveItem[]>("detect_by_dict", {
            content: fileContent,
            dictEntries,
          })
        : [];

      // 更新 detectStore（保持现有行为）
      replaceDictItems(dictItems);

      // 同步更新 workspaceStore.currentSensitiveItems
      const currentItems = store.currentSensitiveItems;
      const nonDictItems = currentItems.filter((i) => i.source !== "Dict");

      // 白名单过滤（如果已有白名单）
      const whitelist = wsData?.workspace.whitelist || [];
      const filteredDictItems = dictItems.filter((item) =>
        !whitelist.some((w) =>
          w.match_mode === "Exact"
            ? item.text === w.text
            : item.text.toLowerCase() === w.text.toLowerCase()
        )
      );

      // enabledTypes 过滤
      const enabledTypes = wsData?.workspace.enabled_types || [];
      const enabledDictItems = filteredDictItems.filter((item) => {
        const key = typeof item.sensitive_type === "string"
          ? item.sensitive_type
          : "Custom";
        return enabledTypes.includes(key);
      });

      const mergedItems = [...nonDictItems, ...enabledDictItems];
      store.setCurrentSensitiveItems(mergedItems);

      // 同时更新 rawSensitiveItems
      const rawItems = store.rawSensitiveItems;
      const rawNonDict = rawItems.filter((i) => i.source !== "Dict");
      store.setRawSensitiveItems([...rawNonDict, ...dictItems]);
    } catch {
      // 静默处理
    }
  }, [replaceDictItems]);

  return reDetectDict;
}
```

**Step 2: 验证编译通过**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npm run build 2>&1 | head -20`
Expected: 无 TypeScript 编译错误

**Step 3: 提交**

```bash
git add src/hooks/useReDetectDict.ts
git commit -m "fix: reDetectDict 同步更新 workspaceStore，修复词典添加后不高亮"
```

---

## Task 2: 脱敏模式下词典变更后重新脱敏

**Files:**
- Modify: `src/components/StrategyPanel/DictSection.tsx`

**Step 1: DictSection 中词典变更后触发重新脱敏**

在 `DictSection.tsx` 的 `reDetectDict` 调用后，脱敏模式需要重新执行 `apply_desensitize`。

修改 `handleAdd`、`handleRemove`、`handleSaveEdit` 中的 `reDetectDict()` 调用后面，添加重新脱敏逻辑。

在文件顶部添加 import:
```typescript
import { useAutoDesensitize } from "../../hooks/useAutoDesensitize";
```

在 `DictSection` 组件内添加:
```typescript
const { desensitizeManualItems } = useAutoDesensitize();
```

修改 `handleAdd` 函数（72-78行区域），在 `await reDetectDict()` 后面加：
```typescript
// 脱敏模式下重新执行脱敏
if (!isTemplateMode) {
  await desensitizeManualItems();
}
```

同样修改 `handleRemove`（81-89行）和 `handleSaveEdit`（100-117行）中的 `reDetectDict()` 之后。

**Step 2: 验证编译通过**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npm run build 2>&1 | head -20`
Expected: 无 TypeScript 编译错误

**Step 3: 提交**

```bash
git add src/components/StrategyPanel/DictSection.tsx
git commit -m "fix: 脱敏模式下词典变更后自动重新脱敏"
```

---

## Task 3: Rust 后端 — Workspace 新增 whitelist 字段

**Files:**
- Modify: `src-tauri/src/models/workspace.rs`

**Step 1: 添加 WhitelistEntry 和 whitelist 字段**

在 `workspace.rs` 的 `WorkspaceMode` 枚举之后、`Workspace` 结构之前，添加：

```rust
/// 白名单排除条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistEntry {
    /// 排除的文本
    pub text: String,
    /// 匹配模式（精确/模糊）
    pub match_mode: MatchMode,
}
```

需要在文件顶部 import 中加入 `MatchMode`:
```rust
use super::strategy::{Strategy, DictEntry, ReplaceStyle, MatchMode};
```

在 `Workspace` 结构体的 `mode` 字段之后，添加：
```rust
    /// 白名单排除列表（工作区级别，所有引擎生效）
    #[serde(default)]
    pub whitelist: Vec<WhitelistEntry>,
```

**Step 2: 验证 Rust 编译通过**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool/src-tauri && cargo check 2>&1 | tail -5`
Expected: `Finished`

**Step 3: 提交**

```bash
git add src-tauri/src/models/workspace.rs
git commit -m "feat: Workspace 新增 whitelist 白名单字段"
```

---

## Task 4: 前端类型 — 新增 WhitelistEntry

**Files:**
- Modify: `src/types/index.ts`

**Step 1: 添加 WhitelistEntry 接口**

在 `DictEntry` 接口之后（约 150 行），添加：

```typescript
/** 白名单排除条目 */
export interface WhitelistEntry {
  text: string;
  match_mode: "Exact" | "Fuzzy";
}
```

**Step 2: Workspace 接口新增 whitelist 字段**

在 `Workspace` 接口的 `mode` 字段之后（约 273 行），添加：

```typescript
  whitelist?: WhitelistEntry[];  // 白名单排除列表
```

**Step 3: 验证编译通过**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npm run build 2>&1 | head -20`
Expected: 无 TypeScript 编译错误

**Step 4: 提交**

```bash
git add src/types/index.ts
git commit -m "feat: 前端类型新增 WhitelistEntry"
```

---

## Task 5: workspaceStore — 白名单 CRUD 方法

**Files:**
- Modify: `src/stores/workspaceStore.ts`

**Step 1: 在 WorkspaceState 接口中添加白名单方法声明**

在 `updateWorkspaceMode` 方法声明之后（约 92 行），添加：

```typescript
  /** 添加白名单条目 */
  addWhitelistEntry: (text: string, matchMode?: "Exact" | "Fuzzy") => Promise<void>;
  /** 删除白名单条目 */
  removeWhitelistEntry: (index: number) => Promise<void>;
```

**Step 2: 在 store 实现中添加白名单方法**

在 `updateWorkspaceMode` 实现之后（约 303 行），添加：

```typescript
  addWhitelistEntry: async (text, matchMode = "Exact") => {
    const wsData = get().activeWorkspaceData;
    if (!wsData) return;
    const whitelist = [...(wsData.workspace.whitelist || [])];
    // 查重
    if (whitelist.some((w) => w.text === text && w.match_mode === matchMode)) return;
    whitelist.push({ text, match_mode: matchMode });
    await updateWorkspaceField(get, set, { whitelist });
  },

  removeWhitelistEntry: async (index) => {
    const wsData = get().activeWorkspaceData;
    if (!wsData) return;
    const whitelist = [...(wsData.workspace.whitelist || [])];
    whitelist.splice(index, 1);
    await updateWorkspaceField(get, set, { whitelist });
  },
```

**Step 3: 在顶部 import 中加入 WhitelistEntry**

确认 `import type` 列表中包含 `WhitelistEntry`:

```typescript
import type {
  // ... 现有类型
  WhitelistEntry,
} from "../types";
```

注意：`WhitelistEntry` 只在类型层面使用，如果 `updateWorkspaceField` 的 `fields: Partial<Workspace>` 已经覆盖了 `whitelist` 字段，则不需要额外 import（Workspace 接口已包含 `whitelist?`）。检查是否需要显式 import。

**Step 4: 验证编译通过**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npm run build 2>&1 | head -20`

**Step 5: 提交**

```bash
git add src/stores/workspaceStore.ts
git commit -m "feat: workspaceStore 新增白名单 CRUD 方法"
```

---

## Task 6: useAutoDesensitize — 合并结果后加白名单过滤

**Files:**
- Modify: `src/hooks/useAutoDesensitize.ts`

**Step 1: 在 processFile 的 mergedItems 合并后加白名单过滤**

在 `processFile` 函数中，找到合并去重代码（约 209-221 行）之后、enabledTypes 过滤（约 224 行）之前，添加白名单过滤：

```typescript
      // 白名单过滤
      const whitelist = ws.whitelist || [];
      if (whitelist.length > 0) {
        const beforeCount = mergedItems.length;
        const afterWhitelist = mergedItems.filter((item) =>
          !whitelist.some((w) =>
            w.match_mode === "Exact"
              ? item.text === w.text
              : item.text.toLowerCase() === w.text.toLowerCase()
          )
        );
        mergedItems.length = 0;
        mergedItems.push(...afterWhitelist);
      }
```

**Step 2: 在 processClipboardText 中也添加同样的白名单过滤**

在 `processClipboardText` 函数中，找到合并去重代码（约 733-744 行）之后、enabledTypes 过滤之前，添加同样的白名单过滤代码。

**Step 3: 在 reDesensitizeWithFilteredItems 中也添加白名单过滤**

在 `reDesensitizeWithFilteredItems` 函数中，`rawItems` 过滤 `enabledTypes` 之前（约 864 行），先做白名单过滤：

```typescript
    // 白名单过滤
    const whitelist = ws.whitelist || [];
    const afterWhitelist = whitelist.length > 0
      ? rawItems.filter((item) =>
          !whitelist.some((w) =>
            w.match_mode === "Exact"
              ? item.text === w.text
              : item.text.toLowerCase() === w.text.toLowerCase()
          )
        )
      : rawItems;

    // 按 enabledTypes 过滤
    const filtered = afterWhitelist.filter((item) => {
```

**Step 4: 验证编译通过**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npm run build 2>&1 | head -20`

**Step 5: 提交**

```bash
git add src/hooks/useAutoDesensitize.ts
git commit -m "feat: 识别结果合并后按白名单过滤"
```

---

## Task 7: SensitivePopover — 加"加入白名单"按钮

**Files:**
- Modify: `src/components/SensitivePopover/index.tsx`

**Step 1: 引入 workspaceStore 白名单方法和 toast**

在组件内（约 61-67 行区域），添加：

```typescript
  const addWhitelistEntry = useWorkspaceStore((s) => s.addWhitelistEntry);
```

确认 `toast` 已 import（文件中已有通过 `useConfigStore` 等，检查是否有 `react-hot-toast`）。如果没有，添加：

```typescript
import toast from "react-hot-toast";
```

**Step 2: 添加 handleAddToWhitelist 函数**

在 `handleRemove` 函数之后（约 192 行），添加：

```typescript
  // 加入白名单（持久化排除）
  const handleAddToWhitelist = async () => {
    if (!item) return;
    await addWhitelistEntry(item.text);
    // 从当前识别结果中移除所有同文本的项
    const store = useWorkspaceStore.getState();
    const filtered = store.currentSensitiveItems.filter(
      (i) => i.text !== item.text
    );
    store.setCurrentSensitiveItems(filtered);
    // 同步更新 rawSensitiveItems
    const rawFiltered = store.rawSensitiveItems.filter(
      (i) => i.text !== item.text
    );
    store.setRawSensitiveItems(rawFiltered);
    toast.success(`"${item.text}" 已加入白名单`);
    onClose();
  };
```

**Step 3: 在脱敏模式浮层中添加"加入白名单"按钮**

在脱敏模式浮层的"取消"按钮旁（约 263-272 行区域），将按钮区域改为包含两个按钮：

找到现有的取消按钮 JSX（在 `<div className="flex items-start justify-between gap-2 mb-2">` 内），在取消按钮后面添加白名单按钮：

```tsx
          <button
            onClick={handleAddToWhitelist}
            className="shrink-0 flex items-center gap-0.5 px-1.5 py-0.5 text-xs text-amber-600 hover:text-amber-800 hover:bg-amber-50 rounded transition-colors"
            title="加入白名单，不再识别此文本"
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
            </svg>
            白名单
          </button>
```

注意：需要将原有的两个按钮（取消 + 白名单）放在一个 flex 容器中。修改顶部 flex 容器，把按钮区域改为：

```tsx
        <div className="flex items-start justify-between gap-2 mb-2">
          <p className="text-base font-bold text-gray-900 break-all flex-1 min-w-0">
            {item.text}
          </p>
          <div className="shrink-0 flex items-center gap-1">
            <button
              onClick={handleAddToWhitelist}
              className="flex items-center gap-0.5 px-1.5 py-0.5 text-xs text-amber-600 hover:text-amber-800 hover:bg-amber-50 rounded transition-colors"
              title="加入白名单，不再识别此文本"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
              </svg>
              白名单
            </button>
            <button
              onClick={handleRemove}
              className="flex items-center gap-0.5 px-1.5 py-0.5 text-xs text-red-500 hover:text-red-700 hover:bg-red-50 rounded transition-colors"
              title="取消标记此敏感项"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
              取消
            </button>
          </div>
        </div>
```

**Step 4: 验证编译通过**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npm run build 2>&1 | head -20`

**Step 5: 提交**

```bash
git add src/components/SensitivePopover/index.tsx
git commit -m "feat: 敏感项浮层添加加入白名单按钮"
```

---

## Task 8: 新建 WhitelistSection 组件

**Files:**
- Create: `src/components/StrategyPanel/WhitelistSection.tsx`

**Step 1: 创建 WhitelistSection 组件**

```typescript
import { useState } from "react";
import { ChevronDown, ChevronRight, ShieldCheck, X } from "lucide-react";
import toast from "react-hot-toast";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useReDetectDict } from "../../hooks/useReDetectDict";
import { useAutoDesensitize } from "../../hooks/useAutoDesensitize";

export function WhitelistSection() {
  const [collapsed, setCollapsed] = useState(true);
  const wsData = useWorkspaceStore((s) => s.activeWorkspaceData);
  const removeWhitelistEntry = useWorkspaceStore((s) => s.removeWhitelistEntry);
  const reDetectDict = useReDetectDict();
  const { reDesensitizeWithFilteredItems } = useAutoDesensitize();

  const whitelist = wsData?.workspace.whitelist || [];
  const workspaceMode = wsData?.workspace.mode || "Desensitize";
  const isTemplateMode = workspaceMode === "TemplateReplace";

  if (whitelist.length === 0) return null;

  const handleRemove = async (index: number) => {
    const entry = whitelist[index];
    try {
      await removeWhitelistEntry(index);
      toast.success(`"${entry.text}" 已从白名单移除`);
      // 重新处理当前文件，让被排除的词重新出现
      const store = useWorkspaceStore.getState();
      const filePath = store.currentFilePath;
      if (filePath) {
        // 简单方案：触发重新检测词典（词典检测中包含白名单过滤逻辑）
        await reDetectDict();
        if (!isTemplateMode) {
          await reDesensitizeWithFilteredItems();
        }
      }
    } catch {
      toast.error("移除白名单条目失败");
    }
  };

  return (
    <div className="border-b border-slate-100">
      <button
        onClick={() => setCollapsed(!collapsed)}
        className="w-full flex items-center gap-2 px-4 py-2.5 text-xs font-semibold text-slate-600 hover:bg-slate-50 transition-colors"
      >
        {collapsed ? (
          <ChevronRight className="w-3.5 h-3.5 text-slate-400" />
        ) : (
          <ChevronDown className="w-3.5 h-3.5 text-slate-400" />
        )}
        <ShieldCheck className="w-3.5 h-3.5 text-amber-500" />
        排除词
        <span className="text-[11px] font-normal text-slate-400">({whitelist.length})</span>
      </button>

      {!collapsed && (
        <div className="px-4 pb-3">
          <div className="space-y-1.5 max-h-32 overflow-auto">
            {whitelist.map((entry, index) => (
              <div
                key={`${entry.text}-${index}`}
                className="flex items-center gap-2 px-2.5 py-1.5 bg-amber-50 rounded group"
              >
                <span className="text-xs text-slate-800 truncate flex-1 min-w-0">
                  {entry.text}
                </span>
                <span className="shrink-0 text-xs text-slate-400">
                  {entry.match_mode === "Exact" ? "精确" : "模糊"}
                </span>
                <button
                  onClick={() => handleRemove(index)}
                  className="shrink-0 p-0.5 text-slate-300 hover:text-rose-500 rounded transition-colors opacity-0 group-hover:opacity-100"
                  title="从白名单移除"
                >
                  <X className="w-3 h-3" />
                </button>
              </div>
            ))}
          </div>
          <p className="text-[11px] text-slate-400 mt-2 px-1">
            白名单中的文本不会被任何引擎识别为敏感信息
          </p>
        </div>
      )}
    </div>
  );
}
```

**Step 2: 验证编译通过**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npm run build 2>&1 | head -20`

**Step 3: 提交**

```bash
git add src/components/StrategyPanel/WhitelistSection.tsx
git commit -m "feat: 新建 WhitelistSection 排除词展示组件"
```

---

## Task 9: StrategyPanel 引入 WhitelistSection

**Files:**
- Modify: `src/components/StrategyPanel/index.tsx`

**Step 1: 添加 import**

在现有 import（约 7-8 行）后添加：

```typescript
import { WhitelistSection } from "./WhitelistSection";
```

**Step 2: 在可滚动配置面板中添加 WhitelistSection**

在 `<DictSection />` 之后、`<OutputSection />` 之前（约 84-85 行），添加：

```tsx
        <WhitelistSection />
```

完整的可滚动区域变为：

```tsx
      <div className="flex-1 overflow-auto">
        {!isTemplateMode && <RulesSection />}
        <DictSection />
        <WhitelistSection />
        <OutputSection />
      </div>
```

**Step 3: 验证编译通过**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && npm run build 2>&1 | head -20`

**Step 4: 提交**

```bash
git add src/components/StrategyPanel/index.tsx
git commit -m "feat: StrategyPanel 引入 WhitelistSection"
```

---

## Task 10: 端到端手动验证

**Step 1: 启动开发环境**

Run: `cd /Users/tanzeshun/workpath/git/desensitize-tool && cargo tauri dev`

**Step 2: 验证 Bug 修复**

1. 打开应用，创建或选择工作区
2. 拖入一个包含已知敏感信息的文件（如含有手机号的 Excel）
3. 等待识别完成，确认原文高亮正常
4. 在右侧栏"自定义词典"中添加一个词条（如文件中某个特定词语）
5. **验证**：添加后原文中该词语应立即出现高亮
6. **验证**：脱敏结果中该词语应被替换

**Step 3: 验证白名单功能**

1. 点击一个已识别的敏感项（如被 NER 误识别的人名）
2. 在浮层中点击"白名单"按钮
3. **验证**：该文本从所有高亮中消失
4. **验证**：右侧栏出现"排除词"区域，显示刚加入的词
5. 重新导入同一文件
6. **验证**：该词不再被识别为敏感信息
7. 在"排除词"区域删除该条目
8. **验证**：重新检测后该词重新出现在识别结果中

**Step 4: 最终提交（如有修复）**

如果手动验证发现问题，修复后提交。
