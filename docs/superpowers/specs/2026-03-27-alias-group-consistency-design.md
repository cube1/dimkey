# INK-020 — 公司全称与简称关联一致性替换

## 概述

支持将公司全称（如"ABC科技有限公司"）和简称（如"ABC"）关联为同一实体，脱敏时统一替换为相同值。本期只做手动关联，不做自动识别。

## 数据模型

### AliasGroup（新增）

```rust
pub struct AliasGroup {
    pub id: String,                    // UUID
    pub primary: String,               // 主名（组内最长文本）
    pub members: Vec<String>,          // 所有成员（含主名）
    pub sensitive_type: SensitiveType, // 组内统一敏感类型
    pub created_at: String,            // ISO 8601
}
```

### Workspace 扩展

```rust
pub struct Workspace {
    // ... 现有字段
    pub alias_groups: Vec<AliasGroup>,  // 新增，#[serde(default)]
}
```

### ConsistencyMapping 扩展

```rust
pub struct ConsistencyMapping {
    // ... 现有字段
    pub alias_group_id: Option<String>,  // 新增，关联到 AliasGroup.id
}
```

## 核心逻辑

### 主名选择

组内最长文本自动成为主名。所有成员共用主名的替换值。

### 脱敏时查找流程

```
查找 (original_text, sensitive_type) 在 consistency_map 中：
  → 找到且有 alias_group_id → 用 group 主名的替换值
  → 找到但无 alias_group_id → 现有逻辑不变
  → 未找到 → 检查是否属于某个 alias_group 的 member
    → 是 → 用主名的替换值，并写入新 mapping
    → 否 → 生成新替换值（现有逻辑）
```

### 构建辅助索引

脱敏开始时一次性构建 `member_to_group: HashMap<(String, SensitiveType), String>`，从成员文本到 group_id 的反查索引，查找 O(1)。

## Tauri Commands

### 创建别名组

```rust
#[tauri::command]
fn create_alias_group(
    workspace_id: String,
    members: Vec<String>,
    sensitive_type: SensitiveType,
) -> Result<AliasGroup, String>
```

副作用：组内所有成员若已有 consistency_mapping，将其 `alias_group_id` 设为新组 ID，`replaced_text` 统一覆盖为主名的替换值。

### 添加成员

```rust
#[tauri::command]
fn add_alias_member(
    workspace_id: String,
    group_id: String,
    member: String,
) -> Result<AliasGroup, String>
```

副作用：新成员的映射覆盖为主名替换值。

### 移除成员

```rust
#[tauri::command]
fn remove_alias_member(
    workspace_id: String,
    group_id: String,
    member: String,
) -> Result<Option<AliasGroup>, String>
```

副作用：清除该成员映射的 `alias_group_id`，保留其当前替换值。若剩余成员 ≤1 则自动解散。

### 删除别名组

```rust
#[tauri::command]
fn delete_alias_group(
    workspace_id: String,
    group_id: String,
) -> Result<(), String>
```

副作用：清除所有成员映射的 `alias_group_id`。

### 查询别名组

```rust
#[tauri::command]
fn list_alias_groups(
    workspace_id: String,
) -> Result<Vec<AliasGroup>, String>
```

## 前端交互

### 预览界面快速合并

用户选中一个 OrgName 敏感项，左键浮框中新增：

- **"关联为同一实体"按钮** — 点击后进入关联模式，用户可继续点选其他敏感项加入，顶部显示已选成员列表和确认/取消按钮
- 若选中项已属于别名组，浮框显示"已关联：XXX组（N个成员）"，提供"查看/编辑"和"解除关联"

### 别名管理面板

侧边栏或设置区域增加"别名管理"入口：

- 列表展示当前工作区所有别名组，每组显示主名（加粗）+ 成员数
- 展开可见所有成员，支持手动输入添加、删除单个成员、删除整个组
- 可从此面板新建别名组（手动输入多个文本）

### 状态管理

```typescript
interface AliasGroup {
  id: string
  primary: string
  members: string[]
  sensitiveType: string
  createdAt: string
}
```

Zustand store 新增 `aliasGroups` 及对应 CRUD 方法。

## 持久化与兼容性

- `alias_groups` 序列化在工作区 JSON 中，与 `consistency_mappings` 同级
- `alias_groups` 缺失时反序列化为空 Vec（`#[serde(default)]`）
- `alias_group_id` 缺失时反序列化为 None（Option 天然兼容）
- 旧版工作区文件无需迁移

## 不改动的部分

- 三层识别引擎（regex / ner / dict）
- 各脱敏策略实现（mask / replace / generalize）
- ReplaceState 计数器逻辑

## 测试策略

- **单元测试**：别名组 CRUD、主名选择逻辑、consistency_map 同步
- **集成测试**：创建别名组后执行脱敏，验证组内成员替换值一致

## 决策记录

| 决策 | 选择 | 原因 |
|------|------|------|
| 自动关联 | 本期不做 | 先把数据结构和手动流程做扎实 |
| 交互方式 | 预览浮框 + 独立管理面板 | 两种场景都需要 |
| 替换值 | 全组共用主名（最长文本）替换值 | 简单一致 |
| 作用域 | 工作区级别 | 跨文件一致 |
| 合并冲突 | 主名替换值覆盖 | 避免冲突复杂度 |
