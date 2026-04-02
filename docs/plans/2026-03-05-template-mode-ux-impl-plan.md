# 模版替换模式 UX 重设计 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 重设计模版替换模式的 UX 流程，实现内联编辑优先、词典行内编辑、前端实时预览联动。

**Architecture:** 所有改动集中在前端。模版模式下预览由前端根据词典映射直接渲染（不调用后端 apply_desensitize），导出时才调用后端。词典变更通过 Zustand store 驱动实时联动。

**Tech Stack:** React + TypeScript + Zustand + TailwindCSS

---

### Task 1: workspaceStore — 添加词典条目 CRUD 辅助方法

**Files:**
- Modify: `src/stores/workspaceStore.ts`

**目标:** 为后续组件提供便捷的词典操作方法。

**Step 1: 添加 updateSingleDictEntry 方法**

在 workspaceStore 中添加按索引更新单个词典条目的方法：

```typescript
// 在 store 类型定义中添加
updateSingleDictEntry: (index: number, entry: DictEntry) => Promise<void>;
addDictEntryFromPopover: (text: string, sensitiveType: SensitiveType, replacement: string, matchMode?: MatchMode) => Promise<void>;

// 实现
updateSingleDictEntry: async (index, entry) => {
  const wsData = get().activeWorkspaceData;
  if (!wsData) return;
  const entries = [...wsData.workspace.dict_entries];
  entries[index] = entry;
  await get().updateDictEntries(entries);
},

addDictEntryFromPopover: async (text, sensitiveType, replacement, matchMode = "Exact") => {
  const wsData = get().activeWorkspaceData;
  if (!wsData) return;
  const entries = [...wsData.workspace.dict_entries];
  const existingIndex = entries.findIndex((e) => e.text === text);
  if (existingIndex >= 0) {
    entries[existingIndex] = { ...entries[existingIndex], replacement };
  } else {
    entries.push({ text, sensitive_type: sensitiveType, match_mode: matchMode, replacement });
  }
  await get().updateDictEntries(entries);
},
```

**Step 2: 验证**

运行 `cargo tauri dev`，打开开发者工具确认 store 方法可用。

**Step 3: 提交**

```bash
git add src/stores/workspaceStore.ts
git commit -m "feat: 添加词典条目 CRUD 辅助方法（updateSingleDictEntry, addDictEntryFromPopover）"
```

---

### Task 2: DictSection — 行内编辑功能

**Files:**
- Modify: `src/components/StrategyPanel/DictSection.tsx`

**目标:** 词典条目支持点击编辑，修改原文和替换值。

**Step 1: 添加编辑状态管理**

在 DictSection 组件中添加编辑状态：

```typescript
const [editingIndex, setEditingIndex] = useState<number | null>(null);
const [editText, setEditText] = useState("");
const [editReplacement, setEditReplacement] = useState("");
```

**Step 2: 添加编辑操作方法**

```typescript
const handleStartEdit = (index: number) => {
  const entry = entries[index];
  setEditingIndex(index);
  setEditText(entry.text);
  setEditReplacement(entry.replacement || "");
};

const handleSaveEdit = async () => {
  if (editingIndex === null || !editText.trim()) return;
  const entry = entries[editingIndex];
  const updated: DictEntry = {
    ...entry,
    text: editText.trim(),
    ...(editReplacement.trim() ? { replacement: editReplacement.trim() } : { replacement: undefined }),
  };
  const newEntries = [...entries];
  newEntries[editingIndex] = updated;
  await updateDictEntries(newEntries);
  setEditingIndex(null);
  reDetectDict();
};

const handleCancelEdit = () => {
  setEditingIndex(null);
};
```

**Step 3: 修改条目列表渲染，添加编辑/普通两种模式**

找到当前的条目列表渲染部分（约 lines 85-120），将每个条目改为支持编辑模式：

非编辑状态（原有 + 编辑按钮）：
```tsx
{editingIndex !== index ? (
  <div className="flex items-center gap-1 group">
    <span className="text-xs text-slate-800 truncate flex-1 min-w-0">
      {entry.text}
      {entry.replacement && (
        <span className="text-primary-600"> → {entry.replacement}</span>
      )}
    </span>
    <button onClick={() => handleStartEdit(index)}
      className="text-slate-400 hover:text-slate-600 opacity-0 group-hover:opacity-100 transition-opacity"
      title="编辑">
      <PencilIcon className="w-3 h-3" />
    </button>
    <button onClick={() => handleRemove(index)}
      className="text-slate-400 hover:text-red-500 opacity-0 group-hover:opacity-100 transition-opacity"
      title="删除">
      <XMarkIcon className="w-3 h-3" />
    </button>
  </div>
) : (
  /* 编辑模式 */
  <div className="flex flex-col gap-1 p-1 bg-slate-50 rounded">
    <input value={editText} onChange={(e) => setEditText(e.target.value)}
      className="text-xs px-1.5 py-0.5 border border-slate-300 rounded w-full"
      placeholder="原文" />
    {isTemplateMode && (
      <input value={editReplacement} onChange={(e) => setEditReplacement(e.target.value)}
        className="text-xs px-1.5 py-0.5 border border-slate-300 rounded w-full"
        placeholder="替换为（必填）" />
    )}
    <div className="flex gap-1 justify-end">
      <button onClick={handleSaveEdit}
        className="text-xs px-1.5 py-0.5 bg-primary-500 text-white rounded hover:bg-primary-600">
        保存
      </button>
      <button onClick={handleCancelEdit}
        className="text-xs px-1.5 py-0.5 bg-slate-200 text-slate-600 rounded hover:bg-slate-300">
        取消
      </button>
    </div>
  </div>
)}
```

**Step 4: 添加映射统计（模版模式下）**

在词典列表上方添加统计信息：

```tsx
{isTemplateMode && entries.length > 0 && (
  <div className="text-xs text-slate-500 px-1">
    词典映射 ({entries.filter(e => e.replacement).length}/{entries.length} 已设替换值)
  </div>
)}
```

**Step 5: 验证**

`cargo tauri dev` → 切换到模版替换模式 → 添加词典条目 → 验证编辑按钮出现 → 点击编辑 → 修改并保存 → 确认条目已更新。

**Step 6: 提交**

```bash
git add src/components/StrategyPanel/DictSection.tsx
git commit -m "feat: 词典条目支持行内编辑，模版模式显示映射统计"
```

---

### Task 3: SensitivePopover — 模版模式内联设替换值

**Files:**
- Modify: `src/components/SensitivePopover/index.tsx`

**目标:** 模版模式下点击高亮项，Popover 简化为替换值输入 + 确认，确认后自动同步到词典。

**Step 1: 引入 addDictEntryFromPopover 方法**

```typescript
const addDictEntryFromPopover = useWorkspaceStore((s) => s.addDictEntryFromPopover);
```

**Step 2: 修改 handleAddToDict，使用新方法简化逻辑**

当前 `handleAddToDict` (lines 191-233) 手动构建 entries 数组。改为使用 `addDictEntryFromPopover`：

```typescript
const handleConfirmReplacement = async () => {
  if (!item || !replacementValue.trim()) return;
  await addDictEntryFromPopover(
    item.text,
    item.sensitive_type,
    replacementValue.trim()
  );
  // 重新检测词典项
  const fileContent = useWorkspaceStore.getState().currentFileContent;
  const wsData = useWorkspaceStore.getState().activeWorkspaceData;
  if (fileContent && wsData) {
    const dictItems = await invoke<SensitiveItem[]>("detect_by_dict", {
      content: fileContent,
      dictEntries: wsData.workspace.dict_entries,
    });
    replaceDictItems(dictItems);
  }
  onClose();
};
```

**Step 3: 简化模版模式 Popover UI**

当前模版模式 UI 部分（约 lines 243-293）改造为更简洁的布局：

```tsx
{isTemplateMode && (
  <div className="p-3 space-y-2">
    <div className="text-xs text-slate-500">
      {info.label} · {item.text}
      {existingDictEntry && (
        <span className="ml-1 text-teal-600">（已在词典中）</span>
      )}
    </div>
    <div>
      <label className="text-xs text-slate-500 mb-0.5 block">替换为</label>
      <input
        value={replacementValue}
        onChange={(e) => setReplacementValue(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && handleConfirmReplacement()}
        className="w-full text-sm px-2 py-1 border border-slate-300 rounded focus:outline-none focus:ring-1 focus:ring-teal-400"
        placeholder="输入替换值..."
        autoFocus
      />
    </div>
    <div className="flex gap-2 justify-end">
      <button onClick={onClose}
        className="text-xs px-2 py-1 text-slate-500 hover:text-slate-700">
        取消
      </button>
      <button onClick={handleConfirmReplacement}
        disabled={!replacementValue.trim()}
        className="text-xs px-2 py-1 bg-teal-500 text-white rounded hover:bg-teal-600 disabled:opacity-50">
        确认
      </button>
    </div>
  </div>
)}
```

**Step 4: 添加"已在词典中"检测变量**

在 useEffect 同步 replacementValue 的地方（约 lines 104-116），添加：

```typescript
const [existingDictEntry, setExistingDictEntry] = useState<DictEntry | null>(null);

useEffect(() => {
  if (!item) return;
  setReplacementValue("");
  setExistingDictEntry(null);
  if (isTemplateMode && wsData) {
    const dictEntry = wsData.workspace.dict_entries.find(
      (e) => e.text === item.text
    );
    if (dictEntry) {
      setExistingDictEntry(dictEntry);
      if (dictEntry.replacement) {
        setReplacementValue(dictEntry.replacement);
      }
    }
  }
}, [item, isTemplateMode, wsData]);
```

**Step 5: 验证**

`cargo tauri dev` → 导入文件 → 切换模版模式 → 点击高亮项 → 验证 Popover 显示简化 UI → 输入替换值 → 确认 → 验证词典同步更新 → 验证高亮变 teal。

**Step 6: 提交**

```bash
git add src/components/SensitivePopover/index.tsx
git commit -m "feat: 模版模式 Popover 简化为替换值输入，确认后同步词典"
```

---

### Task 4: ComparisonView + HighlightedText — 前端实时预览

**Files:**
- Modify: `src/components/CenterPanel/ComparisonView.tsx`
- Modify: `src/components/HighlightedText/index.tsx`

**目标:** 模版模式下，词典变更后左侧高亮即时更新，右侧预览直接显示替换后文本（前端渲染，不调后端）。

**Step 1: ComparisonView — 改造 templateReplacements 的依赖**

当前 `templateReplacements` useMemo（lines 204-214）已经正确地从 dict_entries 构建映射。确保它在词典变更时重新计算（wsData 依赖已有）。

**Step 2: ComparisonView — 模版模式下合并手动词典项到 sensitiveItems**

添加一个 useMemo 来将词典条目也作为高亮项，确保即使三层引擎没检测到的项也能高亮：

```typescript
const templateModeItems = useMemo(() => {
  if (!wsData || wsData.workspace.mode !== "TemplateReplace" || !currentFileContent) {
    return sensitiveItems;
  }

  // 已检测到的项的文本集合
  const detectedTexts = new Set(sensitiveItems.map((item) => item.text));

  // 从词典中找出未被检测到但有替换值的条目
  const additionalItems: SensitiveItem[] = [];
  for (const entry of wsData.workspace.dict_entries) {
    if (detectedTexts.has(entry.text)) continue;
    // 在内容中搜索该文本的出现位置
    // 这里需要遍历 fileContent 的每个 cell/paragraph 来查找
    // 具体实现取决于 FileContent 结构
  }

  return [...sensitiveItems, ...additionalItems];
}, [sensitiveItems, wsData, currentFileContent]);
```

注意：这里需要根据 FileContent 的结构（Excel cells / Word paragraphs）来搜索文本出现位置。具体搜索逻辑参考现有的 `detect_by_dict` Rust 实现，但在前端做简化版（精确匹配即可）。

**Step 3: HighlightedText — 支持在右侧预览面板显示替换后文本**

在 HighlightedText 组件中添加 `showReplacedText` prop：

```typescript
interface HighlightedTextProps {
  // ... existing props
  templateReplacements?: Map<string, string>;
  showReplacedText?: boolean; // 新增：右侧预览时为 true，显示替换后的文本
}
```

渲染逻辑修改（约 lines 84-95）：

```typescript
if (templateReplacements) {
  const replacement = templateReplacements.get(seg.item.text);
  if (replacement !== undefined) {
    if (showReplacedText) {
      // 右侧预览：直接显示替换后的文本，用绿色标记
      displayText = replacement;
      highlightClass = `bg-green-100 text-green-800 rounded px-0.5 py-px ring-1 ring-inset ring-green-300/30`;
    } else {
      // 左侧原文：teal 高亮，保持原文
      highlightClass = `bg-teal-100 text-teal-800 ...`;  // 保持现有
    }
    titleText = `${seg.item.text} → ${replacement}`;
  }
}
```

**Step 4: ComparisonView — 右侧面板传入 showReplacedText**

在右侧 ContentRenderer 中，模版模式下也传入 templateReplacements 和 showReplacedText：

```tsx
{/* 右侧预览面板 */}
<ContentRenderer
  // ... existing props
  items={isTemplateMode ? templateModeItems : desensitizedItems}
  templateReplacements={templateReplacements}
  showReplacedText={isTemplateMode}
  // 模版模式下不需要 diffMode
  diffMode={isTemplateMode ? undefined : "added"}
/>
```

注意：如果模版模式下右侧不使用 `currentResult`（因为没调后端），需要传入原始 `currentFileContent` 作为右侧内容，让 HighlightedText 渲染时用替换值替代原文。

**Step 5: 处理模版模式下右侧面板的数据源**

模版模式下右侧面板应该：
- 数据源：使用 `currentFileContent`（和左侧一样的原始内容）
- 渲染方式：HighlightedText 将有替换值的项渲染为替换后文本
- 没替换值的项：保持原文显示

在 ComparisonView 中修改右侧面板的 content prop：

```typescript
const rightContent = useMemo(() => {
  if (isTemplateMode) {
    return currentFileContent; // 使用原始内容，替换在渲染层处理
  }
  return currentResult; // 普通模式使用后端结果
}, [isTemplateMode, currentFileContent, currentResult]);
```

**Step 6: 验证**

`cargo tauri dev` → 导入文件 → 切换模版模式 → 在 Popover 设置替换值 → 验证：
1. 左侧高亮即时变 teal
2. 右侧显示替换后的文本（绿色标记）
3. 修改词典条目 → 两侧即时更新

**Step 7: 提交**

```bash
git add src/components/CenterPanel/ComparisonView.tsx src/components/HighlightedText/index.tsx
git commit -m "feat: 模版模式前端实时预览，词典变更即时联动两侧面板"
```

---

### Task 5: TextSelectionToolbar — 模版模式框选集成

**Files:**
- Modify: `src/components/TextSelectionToolbar/index.tsx`
- Modify: `src/components/CenterPanel/ComparisonView.tsx`

**目标:** 模版模式下框选文本后，弹出的工具栏支持直接输入替换值并加入词典。

**Step 1: TextSelectionToolbar 添加模版模式支持**

添加 props：

```typescript
interface TextSelectionToolbarProps {
  containerRef: React.RefObject<HTMLElement | null>;
  onAddItem?: (item: SensitiveItem) => void;
  isTemplateMode?: boolean; // 新增
  onAddTemplateMapping?: (text: string, sensitiveType: SensitiveType, replacement: string) => void; // 新增
}
```

**Step 2: 模版模式下的工具栏 UI 改造**

选中文本后，模版模式显示替换值输入：

```tsx
{isTemplateMode ? (
  /* 模版模式：类型选择 + 替换值输入 */
  <div className="bg-white rounded-lg shadow-lg border p-2 space-y-1.5" style={toolbarStyle}>
    {!selectedType ? (
      /* 第一步：选择类型（复用现有类型按钮） */
      <div className="flex flex-wrap gap-1">
        {QUICK_TYPES.map((type) => (
          <button key={type} onClick={() => setSelectedType(type)} ...>
            {SENSITIVE_TYPE_CONFIG[type].label}
          </button>
        ))}
      </div>
    ) : (
      /* 第二步：输入替换值 */
      <div className="space-y-1">
        <div className="text-xs text-slate-500">
          {selectionInfo.text} · {SENSITIVE_TYPE_CONFIG[selectedType].label}
        </div>
        <input
          value={templateReplacement}
          onChange={(e) => setTemplateReplacement(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleTemplateAdd()}
          className="w-full text-xs px-2 py-1 border rounded"
          placeholder="替换为..."
          autoFocus
        />
        <div className="flex gap-1 justify-end">
          <button onClick={() => setSelectedType(null)} className="text-xs text-slate-500">返回</button>
          <button onClick={handleTemplateAdd} className="text-xs px-2 py-0.5 bg-teal-500 text-white rounded">
            添加
          </button>
        </div>
      </div>
    )}
  </div>
) : (
  /* 普通模式：保持现有行为 */
  // ... existing code
)}
```

**Step 3: 添加模版模式状态和处理方法**

```typescript
const [selectedType, setSelectedType] = useState<string | null>(null);
const [templateReplacement, setTemplateReplacement] = useState("");

const handleTemplateAdd = () => {
  if (!selectionInfo || !selectedType || !templateReplacement.trim()) return;
  const sensitiveType = selectedType === "Custom"
    ? { Custom: "自定义" }
    : selectedType;
  onAddTemplateMapping?.(
    selectionInfo.text,
    sensitiveType as SensitiveType,
    templateReplacement.trim()
  );
  // 同时也添加为 SensitiveItem（用于高亮）
  const item: SensitiveItem = {
    id: `manual_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
    text: selectionInfo.text,
    sensitive_type: sensitiveType as SensitiveType,
    confidence: 1.0,
    source: "Manual",
    positions: [{
      start: selectionInfo.start,
      end: selectionInfo.end,
      row: selectionInfo.row,
      col: selectionInfo.col,
      sheet_index: 0,
    }],
  };
  onAddItem?.(item);
  // 清理
  setSelectionInfo(null);
  setSelectedType(null);
  setTemplateReplacement("");
  window.getSelection()?.removeAllRanges();
};
```

**Step 4: ComparisonView 传入模版模式 props**

```tsx
<TextSelectionToolbar
  containerRef={leftPanelRef}
  onAddItem={handleManualAddItem}
  isTemplateMode={isTemplateMode}
  onAddTemplateMapping={async (text, type, replacement) => {
    await addDictEntryFromPopover(text, type, replacement);
  }}
/>
```

**Step 5: 验证**

`cargo tauri dev` → 模版模式 → 在左侧文档中框选文本 → 工具栏出现 → 选择类型 → 输入替换值 → 添加 → 验证：
1. 高亮出现在选中位置
2. 右侧词典面板同步更新
3. 右侧预览显示替换值

**Step 6: 提交**

```bash
git add src/components/TextSelectionToolbar/index.tsx src/components/CenterPanel/ComparisonView.tsx
git commit -m "feat: 模版模式框选文本直接输入替换值并加入词典"
```

---

### Task 6: useAutoDesensitize — 模版模式处理流程简化

**Files:**
- Modify: `src/hooks/useAutoDesensitize.ts`

**目标:** 模版模式下，processFile 只做检测不做 apply_desensitize（预览由前端处理），导出时才调后端。

**Step 1: 修改 processFile 的模版模式分支**

当前模版模式分支（约 lines 277-353）会过滤词典项并调用 `apply_desensitize`。改为：

```typescript
if (isTemplateMode) {
  // 模版模式：只做检测，不调用 apply_desensitize
  // 前端根据词典映射实时渲染预览
  store.setCurrentSensitiveItems(mergedItems);
  store.setProcessingStep("done");

  // 不需要 toast 提示"没有匹配项"，因为用户可能还没设置词典
  return;
}
```

**Step 2: 确保导出逻辑仍调用后端**

导出（export）功能不在 useAutoDesensitize 中，而是通过 OutputSection 或导出按钮触发。确认导出时仍然会调用 `apply_desensitize` 来生成真正的文件。

查看导出流程：导出按钮触发 `handleExport` → 调用 `apply_desensitize`（或 `export_file`） → 生成文件。

如果当前模版模式导出已经正确调用后端，则无需修改。如果导出依赖 `currentResult`（此时模版模式下为 null），则需要在导出时单独调用一次 `apply_desensitize`。

**Step 3: 修改 desensitizeManualItems 和 processClipboardText**

这两个方法中也有模版模式分支，同样简化为只更新 items，不调后端：

`desensitizeManualItems()` (约 lines 587-624)：
```typescript
if (isTemplateMode) {
  // 手动项直接加入列表，不需要调后端
  store.setCurrentSensitiveItems(items);
  return;
}
```

`processClipboardText()` (约 lines 909-981)：类似处理。

**Step 4: 验证**

`cargo tauri dev` → 导入文件 → 切换模版模式 → 确认：
1. 检测正常运行（高亮项出现）
2. 不会自动调 apply_desensitize（不出现"脱敏中"loading）
3. 词典设置替换值后，预览实时更新
4. 点击导出 → 正常生成文件

**Step 5: 提交**

```bash
git add src/hooks/useAutoDesensitize.ts
git commit -m "feat: 模版模式简化为仅检测，预览由前端处理"
```

---

### Task 7: 集成测试与收尾

**Files:**
- 可能微调上述所有文件

**Step 1: 端到端流程验证**

`cargo tauri dev`，按以下完整流程验证：

1. 导入一个 Excel 模版文件（含假数据如"张三"、"13800138000"）
2. 切换到「模版替换」模式
3. 系统自动检测 → 项以灰色高亮显示
4. 点击高亮的"张三" → Popover 出现 → 输入"李明" → 确认
5. 验证：左侧"张三"变 teal，右侧预览显示"李明"，右侧词典新增条目
6. 框选一个未检测到的文本"XX有限公司" → 选类型 → 输入"ABC科技" → 添加
7. 验证：新项高亮 + 词典同步
8. 在右侧词典面板点击编辑 → 修改替换值 → 保存
9. 验证：两侧即时更新
10. 点击导出 → 验证文件正确生成

**Step 2: 检查脱敏模式不受影响**

切换回脱敏模式，验证原有功能完全正常。

**Step 3: 修复发现的问题**

如有 bug，逐个修复并提交。

**Step 4: 最终提交**

```bash
git commit -m "fix: 模版替换模式 UX 重设计集成修复"
```
