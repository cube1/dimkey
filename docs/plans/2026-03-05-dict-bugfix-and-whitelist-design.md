# 自定义词典 Bug 修复 + 白名单排除功能设计

**日期**: 2026-03-05
**状态**: 已确认

---

## 背景

用户在脱敏模式下遇到两个问题：

1. **Bug**: 添加自定义词典条目后，原文中没有高亮标记，脱敏后也没有替换
2. **Feature**: NER 引擎有误识别，需要持久化的白名单排除功能

---

## Part 1: Bug 修复 — 自定义词典不生效

### 根因

`DictSection.reDetectDict()` 调用 `detectStore.replaceDictItems()` 更新了 detectStore，但主处理流程（`useAutoDesensitize.processFile`）将识别结果存储在 `workspaceStore.currentSensitiveItems`。两个 store 没有同步，导致：

- 高亮组件读取 `workspaceStore.currentSensitiveItems`，看不到新的词典项
- 脱敏执行使用 `workspaceStore.currentSensitiveItems`，不包含词典项

### 修复方案

1. `reDetectDict()` 更新 detectStore 后，同步更新 `workspaceStore.currentSensitiveItems`：
   - 从 `currentSensitiveItems` 中移除 `source === "Dict"` 的旧项
   - 加入新的 dictItems
   - 调用 `setCurrentSensitiveItems` 更新

2. 脱敏模式下，词典变更后重新调用 `apply_desensitize` 更新脱敏结果

3. 确保 `Custom` 类型在 `enabled_types` 过滤中被正确处理

---

## Part 2: 白名单排除功能

### 需求

- 工作区级别，持久化存储
- 对所有引擎（正则、NER、词典）生效
- 从识别结果浮层中一键添加
- 右侧栏有小型展示区可查看/删除

### 数据模型

**Rust 后端** (`workspace.rs`):

```rust
pub struct WhitelistEntry {
    pub text: String,           // 排除的文本
    pub match_mode: MatchMode,  // Exact / Fuzzy
}

pub struct Workspace {
    // ... 现有字段
    pub whitelist: Vec<WhitelistEntry>,
}
```

**前端** (`types/index.ts`):

```typescript
interface WhitelistEntry {
  text: string;
  match_mode: "Exact" | "Fuzzy";
}
```

### 过滤时机

在 `useAutoDesensitize.processFile` 的 `mergedItems` 合并完成后、`enabledTypes` 过滤之前，按 whitelist 过滤：

```typescript
// 白名单过滤
const whitelist = ws.whitelist || [];
const afterWhitelist = mergedItems.filter((item) =>
  !whitelist.some((w) =>
    w.match_mode === "Exact"
      ? item.text === w.text
      : item.text.toLowerCase() === w.text.toLowerCase()
  )
);
```

### 前端交互

#### SensitivePopover — "加入白名单"按钮

- 位置：浮层底部，现有"标记为非敏感"旁边
- 点击后：
  1. 将文本加入 `workspace.whitelist`（精确匹配模式）
  2. 从当前所有识别结果中移除所有文本完全匹配的项
  3. toast "已加入白名单"
  4. 关闭浮层

#### WhitelistSection — 右侧栏排除词展示区

- 位置：DictSection（自定义词典）下方
- 折叠标题："排除词 (N)"，Shield 图标
- 条目显示：文本 + 匹配模式 + 删除按钮
- 删除条目后重新检测，让被排除的词重新出现

---

## 修改文件清单

| 文件 | 改动 |
|------|------|
| `src-tauri/src/models/workspace.rs` | 新增 `WhitelistEntry`，Workspace 加 `whitelist` |
| `src/types/index.ts` | 新增 `WhitelistEntry` 类型 |
| `src/stores/workspaceStore.ts` | 新增白名单 CRUD 方法 |
| `src/hooks/useAutoDesensitize.ts` | 合并结果后加白名单过滤 + 修复词典同步 |
| `src/components/StrategyPanel/DictSection.tsx` | `reDetectDict` 修复：同步到 workspaceStore |
| `src/components/StrategyPanel/WhitelistSection.tsx` | 新建：排除词展示区 |
| `src/components/StrategyPanel/index.tsx` | 引入 WhitelistSection |
| `src/components/SensitivePopover/index.tsx` | 加"加入白名单"按钮 |

## 不改动的部分

- Rust 后端引擎（dict_engine/regex_engine/ner_engine）
- 脱敏执行逻辑 (`apply_desensitize`)
- 导出功能
