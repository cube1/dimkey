use std::fs;
use tauri::Manager;
use crate::models::task::{TaskRecord, StrategyType};
use crate::models::sensitive::{FileContent, RestoreResult};
use crate::commands::file::import_file_internal;

/// 获取 tasks/ 目录路径
fn get_tasks_dir(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录: {}", e))?;
    Ok(app_dir.join("tasks"))
}

/// 保存脱敏任务记录
#[tauri::command]
pub async fn save_task(task: TaskRecord, app_handle: tauri::AppHandle) -> Result<(), String> {
    let tasks_dir = get_tasks_dir(&app_handle)?;
    fs::create_dir_all(&tasks_dir)
        .map_err(|e| format!("创建任务目录失败: {}", e))?;

    let file_path = tasks_dir.join(format!("{}.json", task.id));
    let json = serde_json::to_string_pretty(&task)
        .map_err(|e| format!("序列化任务记录失败: {}", e))?;

    fs::write(&file_path, json)
        .map_err(|e| format!("保存任务文件失败: {}", e))?;

    Ok(())
}

/// 获取历史任务列表，按创建时间倒序
#[tauri::command]
pub async fn list_tasks(app_handle: tauri::AppHandle) -> Result<Vec<TaskRecord>, String> {
    let tasks_dir = get_tasks_dir(&app_handle)?;

    if !tasks_dir.exists() {
        return Ok(vec![]);
    }

    let mut tasks: Vec<TaskRecord> = Vec::new();

    let entries = fs::read_dir(&tasks_dir)
        .map_err(|e| format!("读取任务目录失败: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    if let Ok(task) = serde_json::from_str::<TaskRecord>(&content) {
                        tasks.push(task);
                    }
                }
                Err(_) => continue,
            }
        }
    }

    // 按创建时间倒序
    tasks.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(tasks)
}

/// 删除指定任务
#[tauri::command]
pub async fn delete_task(task_id: String, app_handle: tauri::AppHandle) -> Result<(), String> {
    let tasks_dir = get_tasks_dir(&app_handle)?;
    let file_path = tasks_dir.join(format!("{}.json", task_id));

    if !file_path.exists() {
        return Err("任务不存在".to_string());
    }

    fs::remove_file(&file_path)
        .map_err(|e| format!("删除任务失败: {}", e))?;

    Ok(())
}

/// 反向还原：加载任务映射表 → 导入文件 → 反向替换 Replace 类条目
#[tauri::command]
pub async fn restore_file(
    task_id: String,
    file_path: String,
    app_handle: tauri::AppHandle,
) -> Result<RestoreResult, String> {
    // 1. 加载任务记录
    let tasks_dir = get_tasks_dir(&app_handle)?;
    let task_path = tasks_dir.join(format!("{}.json", task_id));
    let task_json = fs::read_to_string(&task_path)
        .map_err(|e| format!("读取任务记录失败: {}", e))?;
    let task: TaskRecord = serde_json::from_str(&task_json)
        .map_err(|e| format!("解析任务记录失败: {}", e))?;

    // 2. 导入待还原文件
    let content = import_file_internal(&file_path)?;
    let original_content = content.clone();

    // 3. 构建反向映射（仅 Replace 策略可逆）
    // 按 replaced_text 长度降序排列，优先匹配较长的文本
    let mut reverse_mappings: Vec<(String, String)> = task
        .mappings
        .iter()
        .filter(|m| m.strategy == StrategyType::Replace)
        .map(|m| (m.replaced_text.clone(), m.original_text.clone()))
        .collect();
    reverse_mappings.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    // 4. 执行反向替换
    let mut restored_content = content;
    let matched_count = restore_content(&mut restored_content, &reverse_mappings);

    Ok(RestoreResult {
        original_content,
        restored_content,
        matched_count,
        restore_items: vec![],
        original_items: vec![],
        file_path,
    })
}

/// 在内容中执行反向替换，返回匹配替换的总次数
fn restore_content(content: &mut FileContent, mappings: &[(String, String)]) -> usize {
    let mut total = 0;

    match content {
        FileContent::Spreadsheet { sheets, .. } => {
            for sheet in sheets.iter_mut() {
                for header in sheet.headers.iter_mut() {
                    total += replace_all_in_text(header, mappings);
                }
                for row in sheet.rows.iter_mut() {
                    for cell in row.iter_mut() {
                        total += replace_all_in_text(&mut cell.text, mappings);
                    }
                }
            }
        }
        FileContent::Document { paragraphs, .. } => {
            for para in paragraphs.iter_mut() {
                total += replace_all_in_text(&mut para.text, mappings);
            }
        }
    }

    total
}

/// 在单个文本中执行所有映射的替换，返回替换次数
fn replace_all_in_text(text: &mut String, mappings: &[(String, String)]) -> usize {
    let mut count = 0;
    for (from, to) in mappings {
        let matches = text.matches(from.as_str()).count();
        if matches > 0 {
            *text = text.replace(from.as_str(), to.as_str());
            count += matches;
        }
    }
    count
}
