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

    // 校验：成员不能已属于同类型的其他别名组
    for m in &members {
        let already = ws_data.workspace.alias_groups.iter()
            .any(|g| g.sensitive_type_key == sensitive_type_key && g.members.contains(m));
        if already {
            return Err(format!("成员「{}」已属于其他别名组", m));
        }
    }

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

    // 先用不可变引用做校验，避免借用冲突
    {
        let group = ws_data.workspace.alias_groups.iter()
            .find(|g| g.id == group_id)
            .ok_or("别名组不存在")?;

        if group.members.contains(&member) {
            return Err("该成员已在组内".to_string());
        }

        // 校验：成员不能已属于同类型的其他别名组
        let already_in_other = ws_data.workspace.alias_groups.iter()
            .any(|g| g.id != group_id && g.sensitive_type_key == group.sensitive_type_key && g.members.contains(&member));
        if already_in_other {
            return Err("该成员已属于其他别名组".to_string());
        }
    }

    let group = ws_data.workspace.alias_groups.iter_mut()
        .find(|g| g.id == group_id)
        .unwrap(); // 上面已校验存在
    group.members.push(member.clone());
    group.primary = select_primary(&group.members);

    let sensitive_type_key = group.sensitive_type_key.clone();
    let primary = group.primary.clone();
    let group_result = group.clone();

    let primary_replaced = ws_data.workspace.consistency_mappings.iter()
        .find(|m| m.original_text == primary && m.sensitive_type_key == sensitive_type_key)
        .map(|m| (m.replaced_text.clone(), m.strategy.clone()));

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

    for m in &mut ws_data.workspace.consistency_mappings {
        if m.original_text == member && m.alias_group_id.as_deref() == Some(&group_id) {
            m.alias_group_id = None;
        }
    }

    let result = if group.members.len() <= 1 {
        ws_data.workspace.alias_groups.retain(|g| g.id != group_id);
        for m in &mut ws_data.workspace.consistency_mappings {
            if m.alias_group_id.as_deref() == Some(&group_id) {
                m.alias_group_id = None;
            }
        }
        None
    } else {
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
