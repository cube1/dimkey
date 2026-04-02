use std::fs;
use tauri::Manager;
use crate::models::task::TaskRecord;
use crate::models::workspace::{
    Workspace, WorkspaceData, ProcessingRecord, ProcessingStatus, WorkspaceMode,
};
use crate::commands::workspace::{chrono_now, default_strategies};
use crate::models::strategy::ReplaceStyle;

/// 应用启动时检查 tasks/ 目录，如有数据则迁移为一个 "v0.1 导入记录" 工作区
/// 迁移后将 tasks/ 重命名为 tasks_v1_backup/
/// 返回迁移后的工作区 ID（如有迁移），否则返回 None
pub fn migrate_v1_tasks(app_handle: &tauri::AppHandle) -> Result<Option<String>, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录: {}", e))?;

    let tasks_dir = app_dir.join("tasks");
    let backup_dir = app_dir.join("tasks_v1_backup");

    // 如果 tasks/ 不存在或已经备份过，跳过
    if !tasks_dir.exists() || backup_dir.exists() {
        return Ok(None);
    }

    // 读取所有任务文件
    let entries = fs::read_dir(&tasks_dir)
        .map_err(|e| format!("读取任务目录失败: {}", e))?;

    let mut records: Vec<ProcessingRecord> = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let task: TaskRecord = match serde_json::from_str(&content) {
            Ok(t) => t,
            Err(_) => continue,
        };

        records.push(ProcessingRecord {
            id: task.id,
            file_name: task.original_file_name,
            file_path: String::new(), // v0.1 没有保存原始路径
            file_type: task.file_type,
            processed_at: task.created_at,
            mappings: task.mappings,
            sensitive_count: task.sensitive_count,
            status: ProcessingStatus::Completed,
            codebook_path: None,
        });
    }

    if records.is_empty() {
        // 没有任务需要迁移，也做备份标记
        let _ = fs::rename(&tasks_dir, &backup_dir);
        return Ok(None);
    }

    // 按时间倒序
    records.sort_by(|a, b| b.processed_at.cmp(&a.processed_at));

    let now = chrono_now();
    let id = uuid::Uuid::new_v4().to_string();

    let workspace = Workspace {
        id: id.clone(),
        name: "v0.1 导入记录".to_string(),
        source: crate::models::workspace::WorkspaceSource::File,
        created_at: now.clone(),
        updated_at: now,
        strategies: default_strategies(),
        dict_entries: vec![],
        column_rules: std::collections::HashMap::new(),
        output_dir: None,
        consistency_mappings: vec![],
        enabled_types: crate::models::workspace::default_enabled_types(),
        replace_style: ReplaceStyle::Fake,
        replace_seed: rand::random(),
        replace_counters: std::collections::HashMap::new(),
        mode: WorkspaceMode::Desensitize,
        whitelist: vec![],
        alias_groups: vec![],
    };

    let data = WorkspaceData {
        workspace,
        history: records,
    };

    // 保存工作区文件
    let workspaces_dir = app_dir.join("workspaces");
    fs::create_dir_all(&workspaces_dir)
        .map_err(|e| format!("创建工作区目录失败: {}", e))?;

    let ws_path = workspaces_dir.join(format!("{}.json", id));
    let json = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("序列化工作区数据失败: {}", e))?;
    fs::write(&ws_path, json)
        .map_err(|e| format!("保存工作区文件失败: {}", e))?;

    // 备份旧 tasks 目录
    fs::rename(&tasks_dir, &backup_dir)
        .map_err(|e| format!("备份 tasks 目录失败: {}", e))?;

    println!("v0.1 数据迁移完成：{} 条记录迁移到工作区 {}", data.history.len(), id);

    Ok(Some(id))
}

