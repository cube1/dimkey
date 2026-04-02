# INK-020 别名组一致性替换 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 支持将公司全称和简称手动关联为同一实体，脱敏时统一替换为相同值。

**Architecture:** 在 Workspace 中新增独立的 `AliasGroup` 结构体管理别名组，`ConsistencyMapping` 扩展 `alias_group_id` 字段引用组 ID。脱敏引擎在查找一致性映射时增加别名组反查逻辑。前端在 SensitivePopover 浮框中增加关联入口，并新增独立的别名管理面板。

**Tech Stack:** Rust (Tauri v2 commands, serde), React + TypeScript + Zustand + TailwindCSS

---

## 文件结构

### 新建文件
- `src-tauri/src/commands/alias_group.rs` — 别名组 CRUD Tauri commands
- `src/components/AliasGroupPanel/index.tsx` — 别名管理面板组件
- `src/components/AliasLinkMode/index.tsx` — 预览界面关联模式顶栏组件

### 修改文件
- `src-tauri/src/models/workspace.rs` — 新增 AliasGroup struct，扩展 Workspace 和 ConsistencyMapping
- `src-tauri/src/commands/mod.rs` — 注册 alias_group 模块
- `src-tauri/src/commands/desensitize.rs` — 脱敏时增加别名组反查逻辑
- `src-tauri/src/lib.rs` — 注册新 Tauri commands
- `src/types/index.ts` — 新增 AliasGroup 类型，扩展 ConsistencyMapping 和 Workspace
- `src/stores/workspaceStore.ts` — 新增别名组相关 state 和方法
- `src/components/SensitivePopover/index.tsx` — 浮框增加"关联为同一实体"操作

---

## Task 1: 数据模型扩展（Rust）

**Files:**
- Modify: `src-tauri/src/models/workspace.rs:22-33` (ConsistencyMapping) 和 `64-111` (Workspace)

- [ ] **Step 1: 写测试 — AliasGroup 序列化/反序列化**

在 `src-tauri/src/models/workspace.rs` 文件末尾添加测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_group_serde_roundtrip() {
        let group = AliasGroup {
            id: "g1".to_string(),
            primary: "ABC科技有限公司".to_string(),
            members: vec!["ABC科技有限公司".to_string(), "ABC".to_string()],
            sensitive_type_key: "OrgName".to_string(),
            created_at: "2026-03-27T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&group).unwrap();
        let decoded: AliasGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "g1");
        assert_eq!(decoded.primary, "ABC科技有限公司");
        assert_eq!(decoded.members.len(), 2);
    }

    #[test]
    fn test_workspace_backward_compat_no_alias_groups() {
        // 模拟旧版 JSON（无 alias_groups 字段）
        let json = r#"{
            "id": "ws1", "name": "test", "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z", "strategies": {},
            "dict_entries": [], "column_rules": {}, "output_dir": null,
            "consistency_mappings": [], "enabled_types": ["Phone"]
        }"#;
        let ws: Workspace = serde_json::from_str(json).unwrap();
        assert!(ws.alias_groups.is_empty());
    }

    #[test]
    fn test_consistency_mapping_backward_compat_no_group_id() {
        let json = r#"{
            "original_text": "张三",
            "sensitive_type_key": "PersonName",
            "replaced_text": "李四",
            "strategy": "Replace"
        }"#;
        let m: ConsistencyMapping = serde_json::from_str(json).unwrap();
        assert!(m.alias_group_id.is_none());
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cd src-tauri && cargo test models::workspace::tests -- --nocapture
```

预期：编译失败，`AliasGroup` 未定义，`alias_groups` / `alias_group_id` 字段不存在。

- [ ] **Step 3: 实现数据模型**

在 `src-tauri/src/models/workspace.rs` 中：

1. 在 `ConsistencyMapping` struct（第 23-33 行）后添加 `alias_group_id` 字段：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyMapping {
    pub original_text: String,
    pub sensitive_type_key: String,
    pub replaced_text: String,
    pub strategy: StrategyType,
    /// 关联的别名组 ID（属于某个别名组时非 None）
    #[serde(default)]
    pub alias_group_id: Option<String>,
}
```

2. 在 `ConsistencyMapping` 定义之后、`ProcessingStatus` 之前，添加 `AliasGroup` struct：

```rust
/// 别名组：将同一实体的多个名称关联在一起，脱敏时统一替换
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasGroup {
    /// 唯一标识 (UUID)
    pub id: String,
    /// 主名（组内最长文本，用于生成替换值）
    pub primary: String,
    /// 所有成员文本（含主名）
    pub members: Vec<String>,
    /// 敏感类型键名（如 "OrgName"）
    pub sensitive_type_key: String,
    /// 创建时间 ISO 8601
    pub created_at: String,
}
```

3. 在 `Workspace` struct（第 64-111 行）的 `whitelist` 字段后添加：

```rust
    /// 别名组列表（将全称/简称关联为同一实体）
    #[serde(default)]
    pub alias_groups: Vec<AliasGroup>,
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cd src-tauri && cargo test models::workspace::tests -- --nocapture
```

预期：3 个测试全部 PASS。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/models/workspace.rs
git commit -m "feat(models): 新增 AliasGroup 结构体，扩展 ConsistencyMapping 和 Workspace"
```

---

## Task 2: 别名组 CRUD Commands（Rust）

**Files:**
- Create: `src-tauri/src/commands/alias_group.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs:14-20` (imports) 和 `126-173` (invoke_handler)

- [ ] **Step 1: 创建 alias_group.rs 并写测试**

创建 `src-tauri/src/commands/alias_group.rs`：

```rust
use crate::models::workspace::{AliasGroup, ConsistencyMapping};
use crate::commands::workspace::{
    get_workspaces_dir, read_workspace_data, write_workspace_data, chrono_now,
};

/// 选择组内最长文本为主名
fn select_primary(members: &[String]) -> String {
    members.iter()
        .max_by_key(|m| m.chars().count())
        .cloned()
        .unwrap_or_default()
}

/// 创建别名组
#[tauri::command]
pub async fn create_alias_group(
    workspace_id: String,
    members: Vec<String>,
    sensitive_type_key: String,
    app_handle: tauri::AppHandle,
) -> Result<AliasGroup, String> {
    if members.len() < 2 {
        return Err("别名组至少需要 2 个成员".to_string());
    }

    let dir = get_workspaces_dir(&app_handle)?;
    let path = dir.join(format!("{}.json", workspace_id));
    let mut ws_data = read_workspace_data(&path)?;

    let primary = select_primary(&members);
    let group_id = uuid::Uuid::new_v4().to_string();

    let group = AliasGroup {
        id: group_id.clone(),
        primary: primary.clone(),
        members: members.clone(),
        sensitive_type_key: sensitive_type_key.clone(),
        created_at: chrono_now(),
    };

    // 找到主名的替换值（如果已有一致性映射）
    let primary_replaced = ws_data.workspace.consistency_mappings.iter()
        .find(|m| m.original_text == primary && m.sensitive_type_key == sensitive_type_key)
        .map(|m| (m.replaced_text.clone(), m.strategy.clone()));

    // 同步 consistency_mappings：所有成员的 alias_group_id 设为新组 ID
    for m in &mut ws_data.workspace.consistency_mappings {
        if m.sensitive_type_key == sensitive_type_key && members.contains(&m.original_text) {
            m.alias_group_id = Some(group_id.clone());
            // 如果主名已有替换值，覆盖组内其他成员的替换值
            if let Some((ref replaced, ref strategy)) = primary_replaced {
                m.replaced_text = replaced.clone();
                m.strategy = strategy.clone();
            }
        }
    }

    ws_data.workspace.alias_groups.push(group.clone());
    ws_data.workspace.updated_at = chrono_now();
    write_workspace_data(&path, &ws_data)?;

    Ok(group)
}

/// 向已有组添加成员
#[tauri::command]
pub async fn add_alias_member(
    workspace_id: String,
    group_id: String,
    member: String,
    app_handle: tauri::AppHandle,
) -> Result<AliasGroup, String> {
    let dir = get_workspaces_dir(&app_handle)?;
    let path = dir.join(format!("{}.json", workspace_id));
    let mut ws_data = read_workspace_data(&path)?;

    let group = ws_data.workspace.alias_groups.iter_mut()
        .find(|g| g.id == group_id)
        .ok_or("别名组不存在")?;

    if group.members.contains(&member) {
        return Err("该成员已在组内".to_string());
    }

    group.members.push(member.clone());
    // 重新选择主名（新成员可能更长）
    group.primary = select_primary(&group.members);

    let sensitive_type_key = group.sensitive_type_key.clone();
    let primary = group.primary.clone();
    let group_result = group.clone();

    // 找到主名的替换值
    let primary_replaced = ws_data.workspace.consistency_mappings.iter()
        .find(|m| m.original_text == primary && m.sensitive_type_key == sensitive_type_key)
        .map(|m| (m.replaced_text.clone(), m.strategy.clone()));

    // 同步新成员的 consistency_mapping
    let mut found = false;
    for m in &mut ws_data.workspace.consistency_mappings {
        if m.original_text == member && m.sensitive_type_key == sensitive_type_key {
            m.alias_group_id = Some(group_id.clone());
            if let Some((ref replaced, ref strategy)) = primary_replaced {
                m.replaced_text = replaced.clone();
                m.strategy = strategy.clone();
            }
            found = true;
        }
    }

    // 如果新成员没有 mapping，且主名有替换值，为其创建一条
    if !found {
        if let Some((replaced, strategy)) = primary_replaced {
            ws_data.workspace.consistency_mappings.push(ConsistencyMapping {
                original_text: member,
                sensitive_type_key,
                replaced_text: replaced,
                strategy,
                alias_group_id: Some(group_id.clone()),
            });
        }
    }

    ws_data.workspace.updated_at = chrono_now();
    write_workspace_data(&path, &ws_data)?;

    Ok(group_result)
}

/// 从组中移除成员（剩余 ≤1 则自动解散）
#[tauri::command]
pub async fn remove_alias_member(
    workspace_id: String,
    group_id: String,
    member: String,
    app_handle: tauri::AppHandle,
) -> Result<Option<AliasGroup>, String> {
    let dir = get_workspaces_dir(&app_handle)?;
    let path = dir.join(format!("{}.json", workspace_id));
    let mut ws_data = read_workspace_data(&path)?;

    let group = ws_data.workspace.alias_groups.iter_mut()
        .find(|g| g.id == group_id)
        .ok_or("别名组不存在")?;

    group.members.retain(|m| m != &member);

    // 清除该成员 mapping 上的 group_id
    for m in &mut ws_data.workspace.consistency_mappings {
        if m.original_text == member && m.alias_group_id.as_deref() == Some(&group_id) {
            m.alias_group_id = None;
        }
    }

    let result = if group.members.len() <= 1 {
        // 自动解散：清除剩余成员的 group_id
        let remaining: Vec<String> = group.members.clone();
        ws_data.workspace.alias_groups.retain(|g| g.id != group_id);
        for m in &mut ws_data.workspace.consistency_mappings {
            if m.alias_group_id.as_deref() == Some(&group_id) {
                m.alias_group_id = None;
            }
        }
        let _ = remaining; // 解散后不返回组
        None
    } else {
        // 重新选择主名
        group.primary = select_primary(&group.members);
        Some(group.clone())
    };

    ws_data.workspace.updated_at = chrono_now();
    write_workspace_data(&path, &ws_data)?;

    Ok(result)
}

/// 删除整个别名组
#[tauri::command]
pub async fn delete_alias_group(
    workspace_id: String,
    group_id: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let dir = get_workspaces_dir(&app_handle)?;
    let path = dir.join(format!("{}.json", workspace_id));
    let mut ws_data = read_workspace_data(&path)?;

    ws_data.workspace.alias_groups.retain(|g| g.id != group_id);

    // 清除所有关联的 alias_group_id
    for m in &mut ws_data.workspace.consistency_mappings {
        if m.alias_group_id.as_deref() == Some(&group_id) {
            m.alias_group_id = None;
        }
    }

    ws_data.workspace.updated_at = chrono_now();
    write_workspace_data(&path, &ws_data)?;

    Ok(())
}

/// 查询工作区内所有别名组
#[tauri::command]
pub async fn list_alias_groups(
    workspace_id: String,
    app_handle: tauri::AppHandle,
) -> Result<Vec<AliasGroup>, String> {
    let dir = get_workspaces_dir(&app_handle)?;
    let path = dir.join(format!("{}.json", workspace_id));
    let ws_data = read_workspace_data(&path)?;
    Ok(ws_data.workspace.alias_groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_primary_longest() {
        let members = vec![
            "ABC".to_string(),
            "ABC科技有限公司".to_string(),
            "ABC科技".to_string(),
        ];
        assert_eq!(select_primary(&members), "ABC科技有限公司");
    }

    #[test]
    fn test_select_primary_empty() {
        let members: Vec<String> = vec![];
        assert_eq!(select_primary(&members), "");
    }
}
```

- [ ] **Step 2: 运行测试确认通过**

```bash
cd src-tauri && cargo test commands::alias_group::tests -- --nocapture
```

预期：编译失败，因为 `mod.rs` 尚未注册模块。

- [ ] **Step 3: 注册模块**

修改 `src-tauri/src/commands/mod.rs`，添加：

```rust
pub mod alias_group;
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cd src-tauri && cargo test commands::alias_group::tests -- --nocapture
```

预期：2 个测试 PASS（`select_primary_longest` 和 `select_primary_empty`）。

- [ ] **Step 5: 注册 Tauri commands**

修改 `src-tauri/src/lib.rs`：

1. 在 imports 区（第 14-20 行）添加：
```rust
use commands::alias_group::{
    create_alias_group, add_alias_member, remove_alias_member,
    delete_alias_group, list_alias_groups,
};
```

2. 在 `invoke_handler`（第 126-173 行）的 `clear_type_consistency_mappings,` 后添加：
```rust
            // 别名组
            create_alias_group,
            add_alias_member,
            remove_alias_member,
            delete_alias_group,
            list_alias_groups,
```

- [ ] **Step 6: 编译检查**

```bash
cd src-tauri && cargo check
```

预期：无错误。

- [ ] **Step 7: 提交**

```bash
git add src-tauri/src/commands/alias_group.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(commands): 别名组 CRUD Tauri commands"
```

---

## Task 3: 脱敏引擎别名组反查逻辑

**Files:**
- Modify: `src-tauri/src/commands/desensitize.rs:57-82` (一致性映射加载) 和 `105-148` (脱敏模式生成)

- [ ] **Step 1: 理解改动点**

`apply_desensitize` 函数需要在两处增加别名组逻辑：
1. **加载阶段**（第 62-82 行）：额外构建 `member_to_group` 反查索引
2. **生成阶段**（第 107-146 行）：查找时先检查别名组

`apply_desensitize_by_columns` 函数需要类似改动：
1. **加载阶段**（第 386-393 行）：构建反查索引
2. **查找阶段**（第 417-420 行）：增加别名组查找

- [ ] **Step 2: 修改 apply_desensitize — 加载阶段**

在 `src-tauri/src/commands/desensitize.rs` 的加载阶段（第 66-76 行区间），在 `for m in &ws_data.workspace.consistency_mappings` 循环之后，添加别名组反查索引构建：

```rust
            // 构建别名组反查索引：(member_text, sensitive_type_key) → (group_id, primary)
            let mut member_to_group: HashMap<(String, String), (String, String)> = HashMap::new();
            for group in &ws_data.workspace.alias_groups {
                for member in &group.members {
                    member_to_group.insert(
                        (member.clone(), group.sensitive_type_key.clone()),
                        (group.id.clone(), group.primary.clone()),
                    );
                }
            }
```

同时修改返回的元组，增加 `member_to_group`。在无 workspace 分支返回空 HashMap。

- [ ] **Step 3: 修改 apply_desensitize — 脱敏模式生成阶段**

在 `WorkspaceMode::Desensitize` 分支（第 105-148 行）中，替换查找逻辑。原来是：

```rust
if consistent && consistency_map.contains_key(&key) {
    continue;
}
```

改为：

```rust
// 一致性模式下，先检查直接映射，再检查别名组
if consistent {
    if consistency_map.contains_key(&key) {
        continue;
    }
    // 检查是否属于某个别名组
    let type_key = crate::commands::desensitize::sensitive_type_to_key(&item.sensitive_type);
    if let Some((_group_id, primary)) = member_to_group.get(&(item.text.clone(), type_key)) {
        let primary_key = (primary.clone(), item.sensitive_type.clone());
        if let Some((replaced, st_type)) = consistency_map.get(&primary_key) {
            // 复用主名的替换值
            consistency_map.insert(key, (replaced.clone(), st_type.clone()));
            continue;
        }
        // 主名也没有映射 → 为主名生成，然后当前成员复用
    }
}
```

- [ ] **Step 4: 修改 apply_desensitize_by_columns — 类似逻辑**

在 `apply_desensitize_by_columns` 函数中（第 386-393 行区间），加载 alias_groups 构建反查索引。在查找阶段（第 417-420 行区间）增加别名组查找：

```rust
} else if let Some((_gid, primary)) = member_to_group.get(&(cell_text.clone(), st_key.clone())) {
    if let Some(existing) = global_consistency.get(&(primary.clone(), st.clone())) {
        let r = existing.clone();
        unique_map.insert(cell_text.clone(), r.clone());
        global_consistency.insert((cell_text.clone(), st.clone()), r.clone());
        r
    } else {
        // 主名也没有映射，走正常生成，然后也写入主名映射
        let result = match &rule.strategy { /* ... 同已有逻辑 ... */ };
        unique_map.insert(cell_text.clone(), result.clone());
        global_consistency.insert((cell_text.clone(), st.clone()), result.clone());
        global_consistency.insert((primary.clone(), st.clone()), result.clone());
        result
    }
```

- [ ] **Step 5: 编译检查**

```bash
cd src-tauri && cargo check
```

预期：无错误。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/commands/desensitize.rs
git commit -m "feat(desensitize): 脱敏引擎增加别名组反查，同组成员统一替换值"
```

---

## Task 4: 前端类型定义

**Files:**
- Modify: `src/types/index.ts:268-291`

- [ ] **Step 1: 添加 AliasGroup 类型并扩展现有类型**

在 `src/types/index.ts` 中：

1. 在 `ConsistencyMapping` 接口（第 268 行）添加字段：

```typescript
export interface ConsistencyMapping {
  original_text: string;
  sensitive_type_key: string;
  replaced_text: string;
  strategy: StrategyType;
  alias_group_id?: string;  // 新增
}
```

2. 在 `ConsistencyMapping` 之后、`Workspace` 之前，添加 `AliasGroup` 接口：

```typescript
/** 别名组：将同一实体的多个名称关联 */
export interface AliasGroup {
  id: string;
  primary: string;
  members: string[];
  sensitive_type_key: string;
  created_at: string;
}
```

3. 在 `Workspace` 接口（第 276 行）中添加字段：

```typescript
export interface Workspace {
  // ... 现有字段
  alias_groups?: AliasGroup[];  // 新增
}
```

- [ ] **Step 2: 提交**

```bash
git add src/types/index.ts
git commit -m "feat(types): 前端类型新增 AliasGroup，扩展 ConsistencyMapping"
```

---

## Task 5: Zustand Store 扩展

**Files:**
- Modify: `src/stores/workspaceStore.ts`

- [ ] **Step 1: 在 WorkspaceState 接口中新增状态和方法**

在 `src/stores/workspaceStore.ts` 的 `WorkspaceState` 接口中添加：

```typescript
  /** 当前工作区的别名组 */
  aliasGroups: AliasGroup[];
  /** 是否处于关联模式 */
  aliasLinkMode: boolean;
  /** 关联模式中已选的成员 */
  aliasLinkMembers: SensitiveItem[];
  /** 进入关联模式 */
  enterAliasLinkMode: (initialItem: SensitiveItem) => void;
  /** 退出关联模式 */
  exitAliasLinkMode: () => void;
  /** 关联模式中添加成员 */
  addAliasLinkMember: (item: SensitiveItem) => void;
  /** 关联模式中移除成员 */
  removeAliasLinkMember: (itemId: string) => void;
  /** 确认创建别名组 */
  confirmAliasGroup: () => Promise<void>;
  /** 加载别名组列表 */
  fetchAliasGroups: () => Promise<void>;
  /** 添加成员到已有组 */
  addMemberToGroup: (groupId: string, member: string) => Promise<void>;
  /** 从组中移除成员 */
  removeMemberFromGroup: (groupId: string, member: string) => Promise<void>;
  /** 删除别名组 */
  deleteAliasGroup: (groupId: string) => Promise<void>;
```

- [ ] **Step 2: 实现 store 方法**

在 store 的 `create` 调用中添加初始值和方法实现：

```typescript
  aliasGroups: [],
  aliasLinkMode: false,
  aliasLinkMembers: [],

  enterAliasLinkMode: (initialItem) => {
    set({ aliasLinkMode: true, aliasLinkMembers: [initialItem] });
  },

  exitAliasLinkMode: () => {
    set({ aliasLinkMode: false, aliasLinkMembers: [] });
  },

  addAliasLinkMember: (item) => {
    const { aliasLinkMembers } = get();
    // 避免重复添加（按 text 去重）
    if (aliasLinkMembers.some((m) => m.text === item.text)) return;
    set({ aliasLinkMembers: [...aliasLinkMembers, item] });
  },

  removeAliasLinkMember: (itemId) => {
    const { aliasLinkMembers } = get();
    set({ aliasLinkMembers: aliasLinkMembers.filter((m) => m.id !== itemId) });
  },

  confirmAliasGroup: async () => {
    const { activeWorkspaceId, aliasLinkMembers } = get();
    if (!activeWorkspaceId || aliasLinkMembers.length < 2) return;

    const members = aliasLinkMembers.map((m) => m.text);
    // 取第一个成员的 sensitive_type_key
    const sensitiveTypeKey = getSensitiveTypeKey(aliasLinkMembers[0].sensitive_type);

    await invoke("create_alias_group", {
      workspaceId: activeWorkspaceId,
      members,
      sensitiveTypeKey,
    });

    // 刷新工作区数据和别名组
    const wsId = activeWorkspaceId;
    const data = await invoke<WorkspaceData>("get_workspace", { id: wsId });
    set({
      activeWorkspaceData: data,
      aliasGroups: data.workspace.alias_groups ?? [],
      aliasLinkMode: false,
      aliasLinkMembers: [],
    });
  },

  fetchAliasGroups: async () => {
    const { activeWorkspaceId } = get();
    if (!activeWorkspaceId) return;
    const groups = await invoke<AliasGroup[]>("list_alias_groups", {
      workspaceId: activeWorkspaceId,
    });
    set({ aliasGroups: groups });
  },

  addMemberToGroup: async (groupId, member) => {
    const { activeWorkspaceId } = get();
    if (!activeWorkspaceId) return;
    await invoke("add_alias_member", {
      workspaceId: activeWorkspaceId,
      groupId,
      member,
    });
    await get().fetchAliasGroups();
  },

  removeMemberFromGroup: async (groupId, member) => {
    const { activeWorkspaceId } = get();
    if (!activeWorkspaceId) return;
    await invoke("remove_alias_member", {
      workspaceId: activeWorkspaceId,
      groupId,
      member,
    });
    await get().fetchAliasGroups();
  },

  deleteAliasGroup: async (groupId) => {
    const { activeWorkspaceId } = get();
    if (!activeWorkspaceId) return;
    await invoke("delete_alias_group", {
      workspaceId: activeWorkspaceId,
      groupId,
    });
    await get().fetchAliasGroups();
  },
```

- [ ] **Step 3: 在 selectWorkspace 中同步加载别名组**

在 `selectWorkspace` 方法中，加载工作区数据后同步设置 `aliasGroups`：

```typescript
set({ aliasGroups: data.workspace.alias_groups ?? [] });
```

- [ ] **Step 4: 编译检查**

```bash
npm run dev
```

预期：前端正常启动无类型错误。按 Ctrl+C 关闭。

- [ ] **Step 5: 提交**

```bash
git add src/stores/workspaceStore.ts
git commit -m "feat(store): workspaceStore 新增别名组状态和 CRUD 方法"
```

---

## Task 6: SensitivePopover 增加关联入口

**Files:**
- Modify: `src/components/SensitivePopover/index.tsx:376-398` (底部操作栏)

- [ ] **Step 1: 添加"关联实体"按钮**

在 `SensitivePopover` 组件中：

1. 从 workspaceStore 引入新的方法：

```typescript
const aliasLinkMode = useWorkspaceStore((s) => s.aliasLinkMode);
const enterAliasLinkMode = useWorkspaceStore((s) => s.enterAliasLinkMode);
const addAliasLinkMember = useWorkspaceStore((s) => s.addAliasLinkMember);
const aliasGroups = useWorkspaceStore((s) => s.aliasGroups);
```

2. 计算当前 item 是否已属于某个别名组：

```typescript
const belongsToGroup = useMemo(() => {
  if (!item) return null;
  const typeKey = getSensitiveTypeKey(item.sensitive_type);
  return aliasGroups.find((g) =>
    g.sensitive_type_key === typeKey && g.members.includes(item.text)
  ) ?? null;
}, [item, aliasGroups]);
```

3. 在底部操作栏（第 377-398 行），"加入白名单"按钮前添加关联按钮：

```tsx
{/* 关联实体按钮（仅 OrgName/PersonName 类型显示） */}
{(typeKey === "OrgName" || typeKey === "PersonName") && !isTemplateMode && (
  belongsToGroup ? (
    <button
      onClick={() => {
        // 已在组内：显示组信息（可跳转到别名管理面板）
        toast(`已关联：${belongsToGroup.primary}（${belongsToGroup.members.length}个成员）`);
        onClose();
      }}
      className="flex items-center gap-1 px-2.5 py-1.5 text-xs text-indigo-600 hover:bg-indigo-50 rounded-lg transition-colors"
      title={`已关联到：${belongsToGroup.primary}`}
    >
      <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101" />
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M10.172 13.828a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.102 1.101" />
      </svg>
      已关联
    </button>
  ) : (
    <button
      onClick={() => {
        if (aliasLinkMode) {
          addAliasLinkMember(item);
        } else {
          enterAliasLinkMode(item);
        }
        onClose();
      }}
      className="flex items-center gap-1 px-2.5 py-1.5 text-xs text-indigo-600 hover:bg-indigo-50 rounded-lg transition-colors"
      title="将此项与其他项关联为同一实体"
    >
      <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101" />
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M10.172 13.828a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.102 1.101" />
      </svg>
      关联实体
    </button>
  )
)}
```

- [ ] **Step 2: 处理关联模式下的点击行为**

当 `aliasLinkMode` 为 true 时，点击敏感项浮框时，不应打开策略编辑，而是直接将该项加入关联候选。在 `SensitivePopover` 顶部渲染逻辑中添加关联模式分支：

```tsx
// 关联模式下的简化浮框
if (aliasLinkMode && !isTemplateMode) {
  return (
    <div ref={popoverRef} style={popoverStyle}>
      <div className="w-64 rounded-xl border border-indigo-200 bg-white shadow-float animate-slide-up overflow-hidden">
        <div className="px-4 py-3">
          <p className="text-sm font-medium text-slate-700">{item.text}</p>
          <p className="text-xs text-slate-400 mt-1">{typeInfo.label}</p>
        </div>
        <div className="border-t border-slate-100 px-3 py-2 flex justify-end gap-2">
          <button onClick={onClose}
            className="text-xs px-3 py-1.5 text-slate-400 hover:text-slate-600 rounded-lg hover:bg-slate-100 transition-colors">
            取消
          </button>
          <button
            onClick={() => { addAliasLinkMember(item); onClose(); }}
            className="text-xs px-3 py-1.5 bg-indigo-500 text-white rounded-lg hover:bg-indigo-600 transition-colors">
            加入关联
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: 编译检查**

```bash
npm run dev
```

预期：无类型错误，浮框正常渲染。

- [ ] **Step 4: 提交**

```bash
git add src/components/SensitivePopover/index.tsx
git commit -m "feat(popover): SensitivePopover 新增关联实体入口和关联模式"
```

---

## Task 7: 关联模式顶栏组件

**Files:**
- Create: `src/components/AliasLinkMode/index.tsx`
- 集成到预览视图（需找到预览区域的父组件并添加）

- [ ] **Step 1: 创建 AliasLinkMode 组件**

创建 `src/components/AliasLinkMode/index.tsx`：

```tsx
import { useWorkspaceStore } from "../../stores/workspaceStore";
import toast from "react-hot-toast";

export function AliasLinkBar() {
  const aliasLinkMode = useWorkspaceStore((s) => s.aliasLinkMode);
  const aliasLinkMembers = useWorkspaceStore((s) => s.aliasLinkMembers);
  const exitAliasLinkMode = useWorkspaceStore((s) => s.exitAliasLinkMode);
  const removeAliasLinkMember = useWorkspaceStore((s) => s.removeAliasLinkMember);
  const confirmAliasGroup = useWorkspaceStore((s) => s.confirmAliasGroup);

  if (!aliasLinkMode) return null;

  const handleConfirm = async () => {
    if (aliasLinkMembers.length < 2) {
      toast.error("至少需要选择 2 个成员");
      return;
    }
    try {
      await confirmAliasGroup();
      toast.success("别名组创建成功");
    } catch (e) {
      toast.error(`创建失败: ${e}`);
    }
  };

  return (
    <div className="flex items-center gap-2 px-4 py-2 bg-indigo-50 border-b border-indigo-100 text-sm">
      <span className="text-indigo-700 font-medium shrink-0">关联模式</span>
      <div className="flex items-center gap-1.5 flex-wrap flex-1 min-w-0">
        {aliasLinkMembers.map((m) => (
          <span
            key={m.id}
            className="inline-flex items-center gap-1 px-2 py-0.5 bg-white border border-indigo-200 rounded-full text-xs text-indigo-700"
          >
            <span className="truncate max-w-[120px]">{m.text}</span>
            <button
              onClick={() => removeAliasLinkMember(m.id)}
              className="text-indigo-400 hover:text-red-500 transition-colors"
            >
              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </span>
        ))}
        <span className="text-xs text-indigo-400">点击敏感项继续添加...</span>
      </div>
      <div className="flex items-center gap-1.5 shrink-0">
        <button
          onClick={exitAliasLinkMode}
          className="px-3 py-1 text-xs text-slate-500 hover:text-slate-700 hover:bg-slate-100 rounded-lg transition-colors"
        >
          取消
        </button>
        <button
          onClick={handleConfirm}
          disabled={aliasLinkMembers.length < 2}
          className="px-3 py-1 text-xs bg-indigo-500 text-white rounded-lg hover:bg-indigo-600 disabled:opacity-40 transition-colors"
        >
          确认关联（{aliasLinkMembers.length}）
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: 集成到预览视图**

找到预览区域的父组件（通常是中间面板/主内容区），在预览内容上方插入 `<AliasLinkBar />`。具体文件需根据代码库中预览区域的位置确定——可能是 `App.tsx` 或某个 `PreviewPanel` 组件。

在预览区域的最顶部（文件内容之上）添加：

```tsx
import { AliasLinkBar } from "./components/AliasLinkMode";

// 在预览内容区最顶部
<AliasLinkBar />
```

- [ ] **Step 3: 编译检查**

```bash
npm run dev
```

预期：关联模式顶栏正常显示/隐藏。

- [ ] **Step 4: 提交**

```bash
git add src/components/AliasLinkMode/index.tsx
# 以及集成文件
git commit -m "feat(ui): 关联模式顶栏组件，显示已选成员和确认/取消按钮"
```

---

## Task 8: 别名管理面板

**Files:**
- Create: `src/components/AliasGroupPanel/index.tsx`
- 集成到侧边栏或设置区域

- [ ] **Step 1: 创建 AliasGroupPanel 组件**

创建 `src/components/AliasGroupPanel/index.tsx`：

```tsx
import { useState } from "react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { getSensitiveTypeInfo } from "../../types";
import toast from "react-hot-toast";

export function AliasGroupPanel() {
  const aliasGroups = useWorkspaceStore((s) => s.aliasGroups);
  const addMemberToGroup = useWorkspaceStore((s) => s.addMemberToGroup);
  const removeMemberFromGroup = useWorkspaceStore((s) => s.removeMemberFromGroup);
  const deleteAliasGroup = useWorkspaceStore((s) => s.deleteAliasGroup);

  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [newMemberText, setNewMemberText] = useState("");

  const handleAddMember = async (groupId: string) => {
    const text = newMemberText.trim();
    if (!text) return;
    try {
      await addMemberToGroup(groupId, text);
      setNewMemberText("");
      toast.success("成员已添加");
    } catch (e) {
      toast.error(`添加失败: ${e}`);
    }
  };

  const handleRemoveMember = async (groupId: string, member: string) => {
    try {
      await removeMemberFromGroup(groupId, member);
    } catch (e) {
      toast.error(`移除失败: ${e}`);
    }
  };

  const handleDeleteGroup = async (groupId: string) => {
    try {
      await deleteAliasGroup(groupId);
      toast.success("别名组已删除");
    } catch (e) {
      toast.error(`删除失败: ${e}`);
    }
  };

  if (aliasGroups.length === 0) {
    return (
      <div className="px-4 py-6 text-center text-sm text-slate-400">
        暂无别名组
        <p className="text-xs mt-1 text-slate-300">在预览中选择敏感项，点击"关联实体"创建</p>
      </div>
    );
  }

  return (
    <div className="space-y-2 p-2">
      {aliasGroups.map((group) => {
        const expanded = expandedId === group.id;
        return (
          <div key={group.id} className="border border-slate-200 rounded-lg overflow-hidden">
            {/* 组头 */}
            <button
              onClick={() => setExpandedId(expanded ? null : group.id)}
              className="w-full flex items-center justify-between px-3 py-2 hover:bg-slate-50 transition-colors"
            >
              <div className="flex items-center gap-2 min-w-0">
                <svg className={`w-3.5 h-3.5 text-slate-400 transition-transform ${expanded ? "rotate-90" : ""}`}
                  fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                </svg>
                <span className="text-sm font-medium text-slate-700 truncate">{group.primary}</span>
                <span className="text-xs text-slate-400 shrink-0">{group.members.length}个</span>
              </div>
              <button
                onClick={(e) => { e.stopPropagation(); handleDeleteGroup(group.id); }}
                className="text-slate-300 hover:text-red-500 transition-colors p-1"
                title="删除别名组"
              >
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5}
                    d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                </svg>
              </button>
            </button>

            {/* 展开内容 */}
            {expanded && (
              <div className="border-t border-slate-100 px-3 py-2 space-y-1.5">
                {group.members.map((member) => (
                  <div key={member} className="flex items-center justify-between group">
                    <span className={`text-sm ${member === group.primary ? "font-medium text-indigo-600" : "text-slate-600"}`}>
                      {member}
                      {member === group.primary && (
                        <span className="ml-1 text-[10px] text-indigo-400">主名</span>
                      )}
                    </span>
                    {member !== group.primary && (
                      <button
                        onClick={() => handleRemoveMember(group.id, member)}
                        className="opacity-0 group-hover:opacity-100 text-slate-300 hover:text-red-500 transition-all p-0.5"
                      >
                        <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                        </svg>
                      </button>
                    )}
                  </div>
                ))}

                {/* 添加新成员 */}
                <div className="flex items-center gap-1.5 mt-2 pt-2 border-t border-slate-100">
                  <input
                    value={newMemberText}
                    onChange={(e) => setNewMemberText(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && handleAddMember(group.id)}
                    className="flex-1 text-xs px-2 py-1 border border-slate-200 rounded-md focus:outline-none focus:ring-1 focus:ring-indigo-300"
                    placeholder="添加别名..."
                  />
                  <button
                    onClick={() => handleAddMember(group.id)}
                    disabled={!newMemberText.trim()}
                    className="text-xs px-2 py-1 text-indigo-600 hover:bg-indigo-50 rounded-md disabled:opacity-40 transition-colors"
                  >
                    添加
                  </button>
                </div>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: 集成到侧边栏**

在侧边栏配置区域（存放策略配置、词典等的位置）添加一个"别名组"折叠区块，渲染 `<AliasGroupPanel />`。具体集成文件需根据当前侧边栏结构确定。

- [ ] **Step 3: 编译检查和视觉验证**

```bash
npm run dev
```

预期：别名管理面板在侧边栏正常渲染，空状态提示正确。

- [ ] **Step 4: 提交**

```bash
git add src/components/AliasGroupPanel/index.tsx
# 以及集成文件
git commit -m "feat(ui): 别名管理面板，支持查看/添加/删除成员和组"
```

---

## Task 9: 端到端集成测试

**Files:**
- 无新文件，手动测试验证

- [ ] **Step 1: 启动应用**

```bash
cargo tauri dev
```

- [ ] **Step 2: 验证完整流程**

1. 导入一个包含"ABC科技有限公司"和"ABC"的文件
2. 识别后，点击"ABC科技有限公司"的浮框 → 点击"关联实体"
3. 点击"ABC"的浮框 → 点击"加入关联"
4. 顶栏确认创建别名组
5. 执行脱敏 → 验证两者替换为相同值
6. 在别名管理面板中查看组信息
7. 尝试添加新成员、删除成员、删除组

- [ ] **Step 3: 验证向后兼容**

1. 打开一个旧的工作区（无 alias_groups 字段）
2. 确认正常加载无报错
3. 脱敏功能正常（不使用别名组时）

- [ ] **Step 4: 最终提交（如有修复）**

```bash
git add -A
git commit -m "fix: 别名组功能端到端修复"
```
