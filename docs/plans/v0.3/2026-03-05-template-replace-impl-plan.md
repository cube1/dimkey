# 模版替换功能实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为Dimkey添加「模版替换」工作区模式，让用户（如律师）通过字典定义原值→替换值映射，一键替换合同模版中的敏感信息。

**Architecture:** 在现有脱敏流程基础上，扩展 DictEntry 增加可选 replacement 字段，扩展 Workspace 增加 mode 字段。模版替换模式下三层引擎全开识别，但执行替换时只处理字典中有 replacement 值的项，不使用一致性映射。

**Tech Stack:** Rust (Tauri v2 后端) + React + TypeScript + TailwindCSS v3 + Zustand

---

## Task 1: Rust — DictEntry 增加 replacement 字段

**Files:**
- Modify: `src-tauri/src/models/strategy.rs:82-89`

**Step 1: 修改 DictEntry 结构体**

在 `src-tauri/src/models/strategy.rs` 的 DictEntry 中添加 `replacement` 字段：

```rust
/// 自定义词典条目（与前端 DictEntry 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictEntry {
    /// 词条文本
    pub text: String,
    /// 敏感信息类型
    pub sensitive_type: SensitiveType,
    /// 匹配模式
    pub match_mode: MatchMode,
    /// 模版替换时的替换值（可选，仅在模版替换模式下使用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
}
```

**Step 2: 运行 cargo check 确认编译通过**

Run: `cd src-tauri && cargo check`
Expected: 编译成功，无错误（replacement 使用 serde default，现有代码无需改动）

**Step 3: Commit**

```bash
git add src-tauri/src/models/strategy.rs
git commit -m "feat: DictEntry 增加可选的 replacement 字段"
```

---

## Task 2: Rust — Workspace 增加 WorkspaceMode

**Files:**
- Modify: `src-tauri/src/models/workspace.rs:44-84`

**Step 1: 添加 WorkspaceMode 枚举和 Workspace 字段**

在 `src-tauri/src/models/workspace.rs` 中，在 `Workspace` 结构体定义之前添加枚举：

```rust
/// 工作区模式
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum WorkspaceMode {
    /// 脱敏模式（默认）
    #[default]
    Desensitize,
    /// 模版替换模式
    TemplateReplace,
}
```

然后在 `Workspace` 结构体中（`replace_counters` 字段之后）添加：

```rust
    /// 工作区模式（脱敏/模版替换），旧版 JSON 缺失时默认脱敏
    #[serde(default)]
    pub mode: WorkspaceMode,
```

**Step 2: 运行 cargo check 确认编译通过**

Run: `cd src-tauri && cargo check`
Expected: 编译成功

**Step 3: Commit**

```bash
git add src-tauri/src/models/workspace.rs
git commit -m "feat: Workspace 增加 WorkspaceMode 模式字段"
```

---

## Task 3: Rust — apply_desensitize 支持模版替换逻辑

**Files:**
- Modify: `src-tauri/src/commands/desensitize.rs:38-261`

**Step 1: 在 apply_desensitize 中添加模版替换分支**

核心改动在 `apply_desensitize` 函数中。在加载工作区数据后（约行 62-80），读取工作区的 mode 和 dict_entries。然后在核心脱敏循环（行 85-124）中根据 mode 走不同逻辑。

改动逻辑：

1. 在行 62-80 的工作区加载块中，额外读取 `mode` 和 `dict_entries`：

```rust
    let (ws_path, replace_seed, replace_counters, ws_mode, ws_dict_entries) = if let Some(ref ws_id) = workspace_id {
        let dir = get_workspaces_dir(&app_handle)?;
        let path = dir.join(format!("{}.json", ws_id));
        if path.exists() {
            let ws_data = read_workspace_data(&path)?;
            for m in &ws_data.workspace.consistency_mappings {
                let st = string_to_sensitive_type(&m.sensitive_type_key);
                let key = (m.original_text.clone(), st);
                consistency_map.insert(key, (m.replaced_text.clone(), m.strategy.clone()));
            }
            let seed = ws_data.workspace.replace_seed;
            let counters = ws_data.workspace.replace_counters.clone();
            let mode = ws_data.workspace.mode.clone();
            let dict = ws_data.workspace.dict_entries.clone();
            (Some(path), seed, counters, mode, dict)
        } else {
            (None, 0, HashMap::new(), WorkspaceMode::Desensitize, vec![])
        }
    } else {
        (None, 0, HashMap::new(), WorkspaceMode::Desensitize, vec![])
    };
```

2. 在核心循环中（行 85-124），根据模式分支：

```rust
    match ws_mode {
        WorkspaceMode::TemplateReplace => {
            // 模版替换模式：只处理字典中有 replacement 值的项
            // 构建字典查找表：原文 → replacement
            let dict_replacement_map: HashMap<String, String> = ws_dict_entries
                .iter()
                .filter_map(|e| {
                    e.replacement.as_ref().map(|r| (e.text.clone(), r.clone()))
                })
                .collect();

            for item in &items {
                let key = (item.text.clone(), item.sensitive_type.clone());
                if let Some(replacement) = dict_replacement_map.get(&item.text) {
                    // 字典有替换值 → 直接替换
                    consistency_map.insert(key, (replacement.clone(), StrategyType::Replace));
                }
                // 无替换值 → 不插入 consistency_map，后续替换时跳过
            }
        }
        WorkspaceMode::Desensitize => {
            // 现有脱敏逻辑不变（行 85-124 原始代码）
            for item in &items {
                // ... 现有代码 ...
            }
        }
    }
```

3. 模版替换模式下，跳过一致性映射的写回逻辑（行 202-235）：

在写回逻辑块中增加条件判断，模版替换模式下不写回 consistency_mappings：

```rust
    if let Some(ref path) = ws_path {
        if ws_mode == WorkspaceMode::Desensitize {
            // 现有的一致性映射写回逻辑...
        }
        // 模版替换模式不写回一致性映射
    }
```

**Step 2: 需要引入 WorkspaceMode**

在文件顶部 use 语句中添加 `WorkspaceMode`：

```rust
use crate::models::workspace::WorkspaceMode;
```

**Step 3: 运行 cargo check 确认编译通过**

Run: `cd src-tauri && cargo check`
Expected: 编译成功

**Step 4: 运行测试确认现有逻辑不受影响**

Run: `cd src-tauri && cargo test`
Expected: 所有测试通过

**Step 5: Commit**

```bash
git add src-tauri/src/commands/desensitize.rs
git commit -m "feat: apply_desensitize 支持模版替换模式"
```

---

## Task 4: 前端类型定义更新

**Files:**
- Modify: `src/types/index.ts:144-149` (DictEntry)
- Modify: `src/types/index.ts:256-269` (Workspace)

**Step 1: 扩展 DictEntry 接口**

在 `src/types/index.ts` 的 DictEntry 接口中添加 replacement 字段：

```typescript
/** 自定义词典条目 */
export interface DictEntry {
  text: string;
  sensitive_type: SensitiveType;
  match_mode: "Exact" | "Fuzzy";
  replacement?: string;  // 模版替换时的替换值
}
```

**Step 2: 添加 WorkspaceMode 类型并扩展 Workspace 接口**

在 `WorkspaceSource` 类型定义附近（约行 242）添加：

```typescript
/** 工作区模式 */
export type WorkspaceMode = "Desensitize" | "TemplateReplace";
```

在 `Workspace` 接口中添加 mode 字段：

```typescript
export interface Workspace {
  // ...现有字段
  enabled_types: string[];
  mode?: WorkspaceMode;  // 工作区模式，默认 Desensitize
}
```

**Step 3: 运行前端构建检查**

Run: `npm run dev` (验证无 TypeScript 编译错误后 Ctrl+C)
Expected: 无错误

**Step 4: Commit**

```bash
git add src/types/index.ts
git commit -m "feat: 前端类型增加 WorkspaceMode 和 DictEntry.replacement"
```

---

## Task 5: workspaceStore 增加模式切换方法

**Files:**
- Modify: `src/stores/workspaceStore.ts`

**Step 1: 在 WorkspaceState 接口中添加方法**

在 workspaceStore 的 state 接口中（约在 `updateEnabledTypes` 附近）添加：

```typescript
  updateWorkspaceMode: (mode: WorkspaceMode) => Promise<void>;
```

**Step 2: 实现 updateWorkspaceMode 方法**

在 store 的实现中（在 `updateEnabledTypes` 实现附近，约行 258-260）添加：

```typescript
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

**Step 3: 在 types 中引入 WorkspaceMode**

确保 workspaceStore.ts 中的 import 包含 `WorkspaceMode`：

```typescript
import type { ..., WorkspaceMode } from "../types";
```

**Step 4: Commit**

```bash
git add src/stores/workspaceStore.ts
git commit -m "feat: workspaceStore 增加 updateWorkspaceMode 方法"
```

---

## Task 6: DictSection 组件增加 replacement 输入

**Files:**
- Modify: `src/components/StrategyPanel/DictSection.tsx`

**Step 1: 添加 replacement 输入状态**

在现有的 `text`/`typeKey`/`matchMode` 状态声明后（约行 15-17）添加：

```typescript
  const [replacement, setReplacement] = useState("");
```

**Step 2: 获取工作区模式**

从 workspaceStore 获取当前模式：

```typescript
  const workspaceMode = wsData?.workspace.mode || "Desensitize";
  const isTemplateMode = workspaceMode === "TemplateReplace";
```

**Step 3: 修改 handleAdd 构建 DictEntry 时包含 replacement**

在 `handleAdd` 函数中（约行 33-37），修改 newEntry 构建：

```typescript
    const newEntry: DictEntry = {
      text: text.trim(),
      sensitive_type: sensitiveType,
      match_mode: matchMode,
      ...(replacement.trim() ? { replacement: replacement.trim() } : {}),
    };
```

添加后重置 replacement：

```typescript
      setText("");
      setReplacement("");
```

**Step 4: 在词条列表中展示替换值**

修改词条列表显示（约行 82-104），在词条文本后显示替换值：

```typescript
                    <span className="text-xs text-slate-800 truncate flex-1 min-w-0">
                      {entry.text}
                      {entry.replacement && (
                        <span className="text-primary-600"> → {entry.replacement}</span>
                      )}
                    </span>
```

**Step 5: 在添加表单中增加替换值输入框**

在文本输入框之后（约行 111-122）、类型选择之前，添加替换值输入框：

```typescript
            {isTemplateMode && (
              <input
                type="text"
                value={replacement}
                onChange={(e) => setReplacement(e.target.value)}
                placeholder="替换为（可选）"
                className="w-full px-2.5 py-1.5 text-xs border border-slate-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-400"
              />
            )}
```

**Step 6: Commit**

```bash
git add src/components/StrategyPanel/DictSection.tsx
git commit -m "feat: DictSection 支持设置替换值"
```

---

## Task 7: StrategyPanel 根据模式切换显示

**Files:**
- Modify: `src/components/StrategyPanel/index.tsx`

**Step 1: 读取工作区模式**

添加模式获取逻辑：

```typescript
  const workspaceMode = wsData?.workspace.mode || "Desensitize";
  const isTemplateMode = workspaceMode === "TemplateReplace";
```

**Step 2: 引入模式切换 UI**

在标题区域（行 22-32）的 `<h2>` 标题旁或下方添加模式切换下拉：

```typescript
import { useWorkspaceStore } from "../../stores/workspaceStore";
import type { WorkspaceMode } from "../../types";

// 在标题区域添加模式切换
<div className="px-4 py-3 border-b border-slate-200 shrink-0">
  <div className="flex items-center gap-2">
    <div className="p-1 rounded-md bg-primary-50">
      <Settings2 className="w-3.5 h-3.5 text-primary-500" />
    </div>
    <div className="flex-1">
      <h2 className="text-sm font-bold text-slate-700">策略配置</h2>
      <p className="text-[11px] text-slate-400 leading-tight">{wsData.workspace.name}</p>
    </div>
  </div>
  {/* 模式切换 */}
  <div className="mt-2 flex gap-1">
    <button
      onClick={() => updateWorkspaceMode("Desensitize")}
      className={`flex-1 px-2 py-1 text-xs rounded transition-colors ${
        !isTemplateMode
          ? "bg-primary-500 text-white"
          : "bg-slate-100 text-slate-600 hover:bg-slate-200"
      }`}
    >
      脱敏模式
    </button>
    <button
      onClick={() => updateWorkspaceMode("TemplateReplace")}
      className={`flex-1 px-2 py-1 text-xs rounded transition-colors ${
        isTemplateMode
          ? "bg-primary-500 text-white"
          : "bg-slate-100 text-slate-600 hover:bg-slate-200"
      }`}
    >
      模版替换
    </button>
  </div>
</div>
```

**Step 3: 根据模式条件渲染子组件**

修改可滚动配置面板（行 40-44）：

```typescript
      <div className="flex-1 overflow-auto">
        {!isTemplateMode && <RulesSection />}
        {!isTemplateMode && <TypeSelector />}  {/* TypeSelector 也移到这里 */}
        <DictSection />
        <OutputSection />
      </div>
```

注：脱敏模式下显示 RulesSection + TypeSelector + DictSection + OutputSection；模版替换模式下只显示 DictSection + OutputSection。

**Step 4: 获取 updateWorkspaceMode**

```typescript
  const updateWorkspaceMode = useWorkspaceStore((s) => s.updateWorkspaceMode);
```

**Step 5: Commit**

```bash
git add src/components/StrategyPanel/index.tsx
git commit -m "feat: StrategyPanel 支持模式切换显示"
```

---

## Task 8: SensitivePopover 模版替换模式增强

**Files:**
- Modify: `src/components/SensitivePopover/index.tsx`

**Step 1: 获取工作区模式和字典更新方法**

在组件中添加：

```typescript
  const wsData = useWorkspaceStore((s) => s.activeWorkspaceData);
  const updateDictEntries = useWorkspaceStore((s) => s.updateDictEntries);
  const workspaceMode = wsData?.workspace.mode || "Desensitize";
  const isTemplateMode = workspaceMode === "TemplateReplace";
```

**Step 2: 添加替换值本地状态**

```typescript
  const [replacementValue, setReplacementValue] = useState("");
```

当 item 变化时重置：

```typescript
  useEffect(() => {
    if (!item) return;
    setReplacementValue("");
    // 如果字典已有此项的替换值，自动填充
    if (isTemplateMode && wsData) {
      const dictEntry = wsData.workspace.dict_entries.find(
        (e) => e.text === item.text
      );
      if (dictEntry?.replacement) {
        setReplacementValue(dictEntry.replacement);
      }
    }
  }, [item, isTemplateMode, wsData]);
```

**Step 3: 添加「加入字典并替换」处理函数**

```typescript
  const handleAddToDict = async () => {
    if (!item || !wsData || !replacementValue.trim()) return;

    const typeKey = getSensitiveTypeKey(item.sensitive_type);
    const entries = wsData.workspace.dict_entries;

    // 查找是否已有此词条
    const existingIndex = entries.findIndex((e) => e.text === item.text);

    let newEntries: DictEntry[];
    if (existingIndex >= 0) {
      // 已有词条 → 更新替换值
      newEntries = entries.map((e, i) =>
        i === existingIndex
          ? { ...e, replacement: replacementValue.trim() }
          : e
      );
    } else {
      // 新增词条
      newEntries = [
        ...entries,
        {
          text: item.text,
          sensitive_type: item.sensitive_type,
          match_mode: "Exact" as const,
          replacement: replacementValue.trim(),
        },
      ];
    }

    try {
      await updateDictEntries(newEntries);
      toast.success(`已设置替换：${item.text} → ${replacementValue.trim()}`);
      onClose();
    } catch {
      toast.error("设置替换值失败");
    }
  };
```

**Step 4: 根据模式切换弹窗 UI**

在 return 语句中，根据 isTemplateMode 渲染不同内容：

```typescript
  // 模版替换模式下的弹窗
  if (isTemplateMode) {
    return (
      <div ref={popoverRef} style={popoverStyle}>
        <div className="w-72 rounded-lg border border-gray-200 bg-white p-4 shadow-lg">
          <p className="mb-2 text-base font-bold text-gray-900 break-all">
            {item.text}
          </p>
          <span
            className={`inline-block rounded-full px-2 py-0.5 text-xs font-medium ${typeInfo.bgClass} ${typeInfo.textClass}`}
          >
            {typeInfo.label}
          </span>

          <div className="mt-3">
            <label className="mb-1 block text-xs text-gray-500">替换为</label>
            <input
              type="text"
              value={replacementValue}
              onChange={(e) => setReplacementValue(e.target.value)}
              placeholder="输入替换值"
              className="w-full rounded border border-gray-300 px-2 py-1.5 text-sm text-gray-700 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
              autoFocus
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  handleAddToDict();
                }
              }}
            />
          </div>

          <div className="mt-3 flex gap-2">
            <button
              onClick={handleAddToDict}
              disabled={!replacementValue.trim()}
              className="flex-1 rounded bg-primary-600 px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-primary-700 disabled:opacity-50"
            >
              加入字典
            </button>
            <button
              onClick={onClose}
              className="flex-1 rounded bg-gray-100 px-3 py-1.5 text-sm font-medium text-gray-600 transition-colors hover:bg-gray-200"
            >
              跳过
            </button>
          </div>
        </div>
      </div>
    );
  }

  // 脱敏模式下的弹窗（现有代码不变）
  return (
    // ... 现有 return ...
  );
```

**Step 5: 添加必要的 import**

```typescript
import { useWorkspaceStore } from "../../stores/workspaceStore";
import type { DictEntry } from "../../types";
import toast from "react-hot-toast";
```

**Step 6: Commit**

```bash
git add src/components/SensitivePopover/index.tsx
git commit -m "feat: SensitivePopover 支持模版替换模式"
```

---

## Task 9: useAutoDesensitize 适配模版替换模式

**Files:**
- Modify: `src/hooks/useAutoDesensitize.ts`

**Step 1: 修改 processFile 中的策略构建逻辑**

在 `processFile` 函数中（约行 277-293），根据模式构建不同的策略配置。

模版替换模式下，策略配置需要传递给 Rust 后端（Rust 后端已在 Task 3 中处理了模式分支），但前端需要确保 items 正确传递。

关键修改：模版替换模式下，只传递字典中有 replacement 值的匹配项给 apply_desensitize。

在构建 strategies 和调用 apply_desensitize 之前，添加模式判断：

```typescript
      const ws = wsData.workspace;
      const isTemplateMode = ws.mode === "TemplateReplace";

      if (isTemplateMode) {
        // 模版替换模式：过滤只保留字典中有 replacement 的匹配项
        const dictReplacements = new Set(
          ws.dict_entries
            .filter((e) => e.replacement)
            .map((e) => e.text)
        );

        const templateItems = mergedItems.filter((item) =>
          dictReplacements.has(item.text)
        );

        if (templateItems.length === 0) {
          store.setCurrentSensitiveItems(mergedItems); // 仍展示全部识别结果
          const emptyResult: DesensitizeResult = {
            content: content,
            mappings: [],
            summary: { total: 0, by_type: {} },
          };
          store.setCurrentResult(emptyResult);
          store.setCenterView("comparison");
          store.setProcessingStep("done");
          toast("字典中没有设置替换值的匹配项", { icon: "ℹ️" });
          return;
        }

        // 使用 Replace 策略（Rust 后端会根据 mode 使用字典替换值）
        const strategies: StrategyConfig[] = templateItems
          .reduce<string[]>((acc, item) => {
            const key = typeof item.sensitive_type === "string"
              ? item.sensitive_type : "Custom";
            if (!acc.includes(key)) acc.push(key);
            return acc;
          }, [])
          .map((key) => ({
            sensitive_type: key === "Custom"
              ? { Custom: "自定义" }
              : (key as SensitiveItem["sensitive_type"]),
            strategy: { Replace: { style: "Fake" as const } }, // 占位，Rust 会用字典值
            consistent: false, // 模版替换不使用一致性
          }));

        const result = await invoke<DesensitizeResult>("apply_desensitize", {
          content,
          items: templateItems,
          strategies,
          workspaceId: ws.id,
        });

        // 保存全部识别项（用于高亮显示）和替换结果
        store.setRawSensitiveItems(mergedItems);
        store.setCurrentSensitiveItems(mergedItems);
        store.setCurrentResult(result);
        store.setCenterView("comparison");
        store.setProcessingStep("done");
        toast.success(`已替换 ${result.summary.total} 处`);

        // 保存处理记录
        const record: ProcessingRecord = {
          id: generateRecordId(),
          file_name: name,
          file_path: filePath,
          file_type: content.file_type,
          processed_at: new Date().toISOString(),
          mappings: result.mappings,
          sensitive_count: result.summary.total,
          status: "Completed",
        };
        await invoke("add_processing_record", {
          workspaceId: ws.id,
          record,
        });
        store.setCurrentRecordId(record.id);
        await store.refreshActiveWorkspace();
        return;
      }

      // 脱敏模式：现有逻辑不变
```

**Step 2: 同样修改 processClipboardText 和 desensitizeManualItems**

对 `processClipboardText`（行 662-811）和 `desensitizeManualItems`（行 490-559）做类似的模式判断。如果是模版替换模式，走模版替换逻辑。

**Step 3: Commit**

```bash
git add src/hooks/useAutoDesensitize.ts
git commit -m "feat: useAutoDesensitize 适配模版替换模式"
```

---

## Task 10: 预览高亮区分模版替换项

**Files:**
- 需要查找并修改高亮渲染组件（ContentRenderer 或 SpreadsheetView 中的高亮逻辑）

**Step 1: 查找高亮渲染逻辑**

高亮渲染在 `ContentRenderer` 和 `SpreadsheetView` 中。需要根据工作区模式和字典替换值，为有替换值的项使用不同高亮色（蓝色/绿色）。

**Step 2: 添加模版替换项的高亮样式**

在高亮渲染中，判断当前敏感项是否在字典中有 replacement 值：
- 有替换值 → 使用蓝色/绿色高亮
- 无替换值 → 使用正常高亮（或在模版替换模式下使用灰色/淡色高亮表示不会被替换）

在 hover tooltip 中显示 `张三 → 李四` 的映射关系。

**Step 3: Commit**

```bash
git add src/components/
git commit -m "feat: 预览高亮区分模版替换项"
```

---

## Task 11: 集成测试

**Step 1: 运行 Rust 测试**

Run: `cd src-tauri && cargo test`
Expected: 所有测试通过

**Step 2: 运行 cargo check**

Run: `cd src-tauri && cargo check`
Expected: 编译成功

**Step 3: 手动功能验证**

Run: `cargo tauri dev`

验证清单：
- [ ] 创建工作区，默认为脱敏模式
- [ ] 切换到模版替换模式，RulesSection 和 TypeSelector 隐藏
- [ ] 在字典中添加带替换值的词条
- [ ] 导入含敏感信息的合同文件
- [ ] 三层引擎正常识别
- [ ] 点击敏感项弹出模版替换弹窗（替换值输入 + 加入字典）
- [ ] 执行替换后只替换字典有替换值的项
- [ ] 导出文件格式正确
- [ ] 切换回脱敏模式，功能正常
- [ ] 旧工作区打开无报错（序列化兼容）

**Step 4: Commit**

```bash
git commit -m "test: 模版替换功能集成测试通过"
```

---

## 实施顺序与依赖关系

```
Task 1 (DictEntry.replacement)  ──┐
Task 2 (WorkspaceMode)            ├──→ Task 3 (Rust apply_desensitize)
                                  │
Task 4 (前端类型) ────────────────┤
                                  ├──→ Task 5 (workspaceStore)
                                  │       ├──→ Task 6 (DictSection)
                                  │       ├──→ Task 7 (StrategyPanel)
                                  │       ├──→ Task 8 (SensitivePopover)
                                  │       └──→ Task 9 (useAutoDesensitize)
                                  │
                                  └──→ Task 10 (预览高亮) ──→ Task 11 (集成测试)
```

Task 1-4 可以并行（Rust 和前端类型互不依赖）。
Task 5 依赖 Task 4。
Task 6-9 依赖 Task 5，且彼此独立可并行。
Task 10-11 最后执行。
