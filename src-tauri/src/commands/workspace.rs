use std::fs;
use std::collections::HashMap;
use tauri::Manager;
use crate::models::workspace::{
    Workspace, WorkspaceData, WorkspaceListItem, ProcessingRecord, ProcessingStatus,
    WorkspaceSource, WorkspaceMode,
};
use crate::models::sensitive::{FileContent, RestoreResult, RestoreItem, SensitiveType};
use crate::models::strategy::{Strategy, ReplaceStyle};
use crate::commands::file::import_file_internal;

/// 获取 workspaces/ 目录路径
pub(crate) fn get_workspaces_dir(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录: {}", e))?;
    Ok(app_dir.join("workspaces"))
}

/// 获取单个工作区文件路径（校验 ID 格式，防止路径穿越）
fn get_workspace_path(app_handle: &tauri::AppHandle, id: &str) -> Result<std::path::PathBuf, String> {
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("工作区 ID 格式非法".to_string());
    }
    let dir = get_workspaces_dir(app_handle)?;
    Ok(dir.join(format!("{}.json", id)))
}

/// 读取工作区数据文件
pub(crate) fn read_workspace_data(path: &std::path::Path) -> Result<WorkspaceData, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("读取工作区文件失败: {}", e))?;
    let mut data: WorkspaceData = serde_json::from_str(&content)
        .map_err(|e| format!("解析工作区数据失败: {}", e))?;
    migrate_legacy_counters(&mut data.workspace.replace_counters);
    Ok(data)
}

/// 写入工作区数据文件
pub(crate) fn write_workspace_data(path: &std::path::Path, data: &WorkspaceData) -> Result<(), String> {
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| format!("序列化工作区数据失败: {}", e))?;
    fs::write(path, json)
        .map_err(|e| format!("保存工作区文件失败: {}", e))
}

/// 构建默认策略配置
pub(crate) fn default_strategies() -> HashMap<String, Strategy> {
    let mut m = HashMap::new();
    m.insert("Phone".into(), Strategy::Mask { keep_prefix: 3, keep_suffix: 4 });
    m.insert("IdCard".into(), Strategy::Mask { keep_prefix: 4, keep_suffix: 4 });
    m.insert("BankCard".into(), Strategy::Mask { keep_prefix: 4, keep_suffix: 4 });
    m.insert("Email".into(), Strategy::Mask { keep_prefix: 1, keep_suffix: 0 });
    m.insert("IpAddress".into(), Strategy::Mask { keep_prefix: 8, keep_suffix: 0 });
    m.insert("LandlinePhone".into(), Strategy::Mask { keep_prefix: 4, keep_suffix: 4 });
    m.insert("LicensePlate".into(), Strategy::Mask { keep_prefix: 2, keep_suffix: 2 });
    m.insert("CreditCode".into(), Strategy::Mask { keep_prefix: 4, keep_suffix: 3 });
    m.insert("PersonName".into(), Strategy::Replace { style: ReplaceStyle::Fake });
    m.insert("OrgName".into(), Strategy::Replace { style: ReplaceStyle::Fake });
    m.insert("Address".into(), Strategy::Replace { style: ReplaceStyle::Fake });
    m.insert("Title".into(), Strategy::Generalize);
    m.insert("Custom".into(), Strategy::Mask { keep_prefix: 1, keep_suffix: 1 });
    m
}

/// 创建工作区
#[tauri::command]
pub async fn create_workspace(
    name: String,
    app_handle: tauri::AppHandle,
) -> Result<Workspace, String> {
    let dir = get_workspaces_dir(&app_handle)?;
    fs::create_dir_all(&dir)
        .map_err(|e| format!("创建工作区目录失败: {}", e))?;

    let now = chrono_now();
    let id = uuid::Uuid::new_v4().to_string();

    let workspace = Workspace {
        id: id.clone(),
        name,
        source: WorkspaceSource::File,
        created_at: now.clone(),
        updated_at: now,
        strategies: default_strategies(),
        dict_entries: vec![],
        column_rules: HashMap::new(),
        output_dir: None,
        consistency_mappings: vec![],
        enabled_types: crate::models::workspace::default_enabled_types(),
        replace_style: ReplaceStyle::Fake,
        replace_seed: rand::random(),
        replace_counters: HashMap::new(),
        mode: WorkspaceMode::Desensitize,
        whitelist: vec![],
        alias_groups: vec![],
    };

    let data = WorkspaceData {
        workspace: workspace.clone(),
        history: vec![],
    };

    let path = dir.join(format!("{}.json", id));
    write_workspace_data(&path, &data)?;

    crate::analytics::track(&app_handle, "workspace_created", None);

    Ok(workspace)
}

/// 创建粘贴板工作区
#[tauri::command]
pub async fn create_clipboard_workspace(
    name: String,
    app_handle: tauri::AppHandle,
) -> Result<Workspace, String> {
    let dir = get_workspaces_dir(&app_handle)?;
    fs::create_dir_all(&dir)
        .map_err(|e| format!("创建工作区目录失败: {}", e))?;

    let now = chrono_now();
    let id = uuid::Uuid::new_v4().to_string();

    let workspace = Workspace {
        id: id.clone(),
        name,
        source: WorkspaceSource::Clipboard,
        created_at: now.clone(),
        updated_at: now,
        strategies: default_strategies(),
        dict_entries: vec![],
        column_rules: HashMap::new(),
        output_dir: None,
        consistency_mappings: vec![],
        enabled_types: crate::models::workspace::default_enabled_types(),
        replace_style: ReplaceStyle::Fake,
        replace_seed: rand::random(),
        replace_counters: HashMap::new(),
        mode: WorkspaceMode::Desensitize,
        whitelist: vec![],
        alias_groups: vec![],
    };

    let data = WorkspaceData {
        workspace: workspace.clone(),
        history: vec![],
    };

    let path = dir.join(format!("{}.json", id));
    write_workspace_data(&path, &data)?;

    crate::analytics::track(&app_handle, "clipboard_workspace_created", None);

    Ok(workspace)
}

/// 获取工作区列表
#[tauri::command]
pub async fn list_workspaces(app_handle: tauri::AppHandle) -> Result<Vec<WorkspaceListItem>, String> {
    let dir = get_workspaces_dir(&app_handle)?;

    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut items: Vec<WorkspaceListItem> = Vec::new();

    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("读取工作区目录失败: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            match read_workspace_data(&path) {
                Ok(data) => {
                    items.push(WorkspaceListItem {
                        id: data.workspace.id,
                        name: data.workspace.name,
                        updated_at: data.workspace.updated_at,
                        history_count: data.history.len(),
                        source: data.workspace.source,
                    });
                }
                Err(e) => {
                    eprintln!("工作区文件读取失败 {:?}: {}", path, e);
                }
            }
        }
    }

    // 按更新时间倒序
    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    Ok(items)
}

/// 获取工作区完整数据
#[tauri::command]
pub async fn get_workspace(
    id: String,
    app_handle: tauri::AppHandle,
) -> Result<WorkspaceData, String> {
    let path = get_workspace_path(&app_handle, &id)?;
    if !path.exists() {
        return Err("工作区不存在".to_string());
    }
    read_workspace_data(&path)
}

/// 更新工作区配置（不动历史记录）
#[tauri::command]
pub async fn update_workspace(
    workspace: Workspace,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let path = get_workspace_path(&app_handle, &workspace.id)?;
    if !path.exists() {
        return Err("工作区不存在".to_string());
    }

    let mut data = read_workspace_data(&path)?;
    let mut updated = workspace;
    updated.updated_at = chrono_now();
    data.workspace = updated;
    write_workspace_data(&path, &data)
}

/// 删除工作区
#[tauri::command]
pub async fn delete_workspace(
    id: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let path = get_workspace_path(&app_handle, &id)?;
    if !path.exists() {
        return Err("工作区不存在".to_string());
    }
    fs::remove_file(&path)
        .map_err(|e| format!("删除工作区失败: {}", e))
}

/// 重命名工作区
#[tauri::command]
pub async fn rename_workspace(
    id: String,
    name: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let path = get_workspace_path(&app_handle, &id)?;
    if !path.exists() {
        return Err("工作区不存在".to_string());
    }

    let mut data = read_workspace_data(&path)?;
    data.workspace.name = name;
    data.workspace.updated_at = chrono_now();
    write_workspace_data(&path, &data)
}

/// 添加处理记录
#[tauri::command]
pub async fn add_processing_record(
    workspace_id: String,
    record: ProcessingRecord,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let path = get_workspace_path(&app_handle, &workspace_id)?;
    if !path.exists() {
        return Err("工作区不存在".to_string());
    }

    let mut data = read_workspace_data(&path)?;

    // 限制历史记录最多 100 条（截断多余的旧记录）
    data.history.truncate(99);

    data.history.insert(0, record);
    data.workspace.updated_at = chrono_now();
    write_workspace_data(&path, &data)
}

/// 更新处理记录的映射（列级重新脱敏后同步映射）
#[tauri::command]
pub async fn update_processing_record_mappings(
    workspace_id: String,
    record_id: String,
    mappings: Vec<crate::models::task::MappingEntry>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let path = get_workspace_path(&app_handle, &workspace_id)?;
    if !path.exists() {
        return Err("工作区不存在".to_string());
    }

    let mut data = read_workspace_data(&path)?;
    let record = data.history.iter_mut().find(|r| r.id == record_id)
        .ok_or_else(|| "处理记录不存在".to_string())?;

    record.mappings = mappings;
    record.sensitive_count = record.mappings.iter().map(|m| m.occurrences).sum();
    data.workspace.updated_at = chrono_now();
    write_workspace_data(&path, &data)
}

/// 删除单条处理记录
#[tauri::command]
pub async fn delete_processing_record(
    workspace_id: String,
    record_id: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let path = get_workspace_path(&app_handle, &workspace_id)?;
    if !path.exists() {
        return Err("工作区不存在".to_string());
    }

    let mut data = read_workspace_data(&path)?;
    let before = data.history.len();
    data.history.retain(|r| r.id != record_id);

    if data.history.len() == before {
        return Err("处理记录不存在".to_string());
    }

    data.workspace.updated_at = chrono_now();
    write_workspace_data(&path, &data)
}

/// 还原处理：加载记录映射表 → 导入文件 → 反向替换
#[tauri::command]
pub async fn restore_processing(
    workspace_id: String,
    record_id: String,
    file_path: String,
    app_handle: tauri::AppHandle,
) -> Result<RestoreResult, String> {
    let ws_path = get_workspace_path(&app_handle, &workspace_id)?;
    if !ws_path.exists() {
        return Err("工作区不存在".to_string());
    }

    // 一次性读取工作区数据，避免 TOCTOU 竞态
    let mut data = read_workspace_data(&ws_path)?;
    let record = data.history.iter().find(|r| r.id == record_id)
        .ok_or_else(|| "处理记录不存在".to_string())?
        .clone();

    // 导入待还原文件（同步 IO 放入 spawn_blocking）
    let fp = file_path.clone();
    let content = tokio::task::spawn_blocking(move || {
        import_file_internal(&fp)
    })
    .await
    .map_err(|e| format!("文件导入任务失败: {}", e))??;
    let original_content = content.clone();

    // 构建反向映射（仅 Replace 策略可逆）
    use crate::models::task::StrategyType;
    let mut reverse_mappings: Vec<(String, String)> = record
        .mappings
        .iter()
        .filter(|m| m.strategy == StrategyType::Replace)
        .map(|m| (m.replaced_text.clone(), m.original_text.clone()))
        .collect();
    reverse_mappings.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    // 构建 replaced_text → sensitive_type 映射表
    let type_map: HashMap<String, SensitiveType> = record
        .mappings
        .iter()
        .filter(|m| m.strategy == StrategyType::Replace)
        .map(|m| (m.replaced_text.clone(), m.sensitive_type.clone()))
        .collect();

    // 执行反向替换，同时记录位置信息
    let mut restored_content = content;
    let (matched_count, original_items, restore_items) =
        restore_content(&mut restored_content, &reverse_mappings, &type_map);

    // 更新记录状态（复用同一个 data，避免二次读取的竞态）
    if let Some(r) = data.history.iter_mut().find(|r| r.id == record_id) {
        r.status = ProcessingStatus::Restored;
    }
    data.workspace.updated_at = chrono_now();
    write_workspace_data(&ws_path, &data)?;

    crate::analytics::track(&app_handle, "restore_used", None);

    Ok(RestoreResult {
        original_content,
        restored_content,
        matched_count,
        restore_items,
        original_items,
        file_path,
    })
}

/// 工作区级还原：合并所有历史记录的映射，支持跨格式还原
#[tauri::command]
pub async fn restore_from_workspace(
    workspace_id: String,
    file_path: String,
    app_handle: tauri::AppHandle,
) -> Result<RestoreResult, String> {
    let ws_path = get_workspace_path(&app_handle, &workspace_id)?;
    if !ws_path.exists() {
        return Err("工作区不存在".to_string());
    }

    let data = read_workspace_data(&ws_path)?;

    // 遍历所有历史记录，提取 Replace 策略的映射并去重
    use crate::models::task::StrategyType;
    let mut seen: HashMap<String, (String, SensitiveType)> = HashMap::new();
    for record in &data.history {
        for m in &record.mappings {
            if m.strategy == StrategyType::Replace {
                seen.entry(m.replaced_text.clone())
                    .or_insert_with(|| (m.original_text.clone(), m.sensitive_type.clone()));
            }
        }
    }

    if seen.is_empty() {
        return Err("工作区暂无可还原的映射记录（仅 Replace 策略可还原）".to_string());
    }

    // 构建反向映射，按长度降序排列
    let mut reverse_mappings: Vec<(String, String)> = seen
        .iter()
        .map(|(replaced, (original, _))| (replaced.clone(), original.clone()))
        .collect();
    reverse_mappings.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    // 构建 replaced_text → sensitive_type 映射表
    let type_map: HashMap<String, SensitiveType> = seen
        .into_iter()
        .map(|(replaced, (_, st))| (replaced, st))
        .collect();

    // 导入待还原文件
    let fp = file_path.clone();
    let content = tokio::task::spawn_blocking(move || {
        import_file_internal(&fp)
    })
    .await
    .map_err(|e| format!("文件导入任务失败: {}", e))??;
    let original_content = content.clone();

    // 执行反向替换（使用模糊匹配版本）
    let mut restored_content = content;
    let (matched_count, original_items, restore_items) =
        restore_content_inner(&mut restored_content, &reverse_mappings, &type_map, true);

    crate::analytics::track(&app_handle, "workspace_restore_used", None);

    Ok(RestoreResult {
        original_content,
        restored_content,
        matched_count,
        restore_items,
        original_items,
        file_path,
    })
}

/// 还原 AI 回复文本：用工作区所有映射的反向替换还原假数据
#[tauri::command]
pub async fn restore_ai_response(
    workspace_id: String,
    ai_text: String,
    app_handle: tauri::AppHandle,
) -> Result<RestoreResult, String> {
    if ai_text.is_empty() {
        return Err("AI 回复文本为空".to_string());
    }

    let ws_path = get_workspace_path(&app_handle, &workspace_id)?;
    if !ws_path.exists() {
        return Err("工作区不存在".to_string());
    }

    let data = read_workspace_data(&ws_path)?;

    // 遍历所有历史记录，提取 Replace 策略的映射并去重
    use crate::models::task::StrategyType;
    let mut seen: HashMap<String, (String, SensitiveType)> = HashMap::new();
    for record in &data.history {
        for m in &record.mappings {
            if m.strategy == StrategyType::Replace {
                seen.entry(m.replaced_text.clone())
                    .or_insert_with(|| (m.original_text.clone(), m.sensitive_type.clone()));
            }
        }
    }

    if seen.is_empty() {
        return Err("工作区暂无可还原的映射记录（仅替换策略可还原，建议使用替换策略脱敏）".to_string());
    }

    // 构建反向映射，按长度降序排列
    let mut reverse_mappings: Vec<(String, String)> = seen
        .iter()
        .map(|(replaced, (original, _))| (replaced.clone(), original.clone()))
        .collect();
    reverse_mappings.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    // 构建 replaced_text → sensitive_type 映射表
    let type_map: HashMap<String, SensitiveType> = seen
        .into_iter()
        .map(|(replaced, (_, st))| (replaced, st))
        .collect();

    // 将 AI 文本解析为 FileContent（复用 TXT 段落逻辑）
    let paragraphs: Vec<crate::models::sensitive::Paragraph> = ai_text
        .lines()
        .enumerate()
        .map(|(i, line)| crate::models::sensitive::Paragraph {
            index: i,
            text: line.to_string(),
            style: "normal".to_string(),
            table_position: None,
            pdf_position: None,
        })
        .collect();

    let original_content = FileContent::Document {
        file_name: "ai_response.txt".to_string(),
        file_type: crate::models::sensitive::FileType::Txt,
        paragraphs: paragraphs.clone(),
        encoding: Some("utf-8".to_string()),
    };

    let mut restored_content = FileContent::Document {
        file_name: "ai_response.txt".to_string(),
        file_type: crate::models::sensitive::FileType::Txt,
        paragraphs,
        encoding: Some("utf-8".to_string()),
    };

    // 执行模糊匹配反向替换
    let (matched_count, original_items, restore_items) =
        restore_content_inner(&mut restored_content, &reverse_mappings, &type_map, true);

    crate::analytics::track(&app_handle, "ai_response_restored", Some(serde_json::json!({
        "matched_count": matched_count,
    })));

    Ok(RestoreResult {
        original_content,
        restored_content,
        matched_count,
        restore_items,
        original_items,
        file_path: "clipboard".to_string(),
    })
}

/// 清空工作区的一致性替换映射表
#[tauri::command]
pub async fn clear_consistency_mappings(
    workspace_id: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let path = get_workspace_path(&app_handle, &workspace_id)?;
    if !path.exists() {
        return Err("工作区不存在".to_string());
    }

    let mut data = read_workspace_data(&path)?;
    data.workspace.consistency_mappings.clear();
    data.workspace.updated_at = chrono_now();
    write_workspace_data(&path, &data)
}

/// 清除工作区中指定类型的一致性替换映射
#[tauri::command]
pub async fn clear_type_consistency_mappings(
    workspace_id: String,
    sensitive_type_key: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let path = get_workspace_path(&app_handle, &workspace_id)?;
    if !path.exists() {
        return Err("工作区不存在".to_string());
    }

    let mut data = read_workspace_data(&path)?;
    data.workspace.consistency_mappings
        .retain(|m| m.sensitive_type_key != sensitive_type_key);
    data.workspace.updated_at = chrono_now();
    write_workspace_data(&path, &data)
}

/// 单次替换的位置记录
struct ReplaceMatch {
    /// 替换前文本中的起始位置
    orig_start: usize,
    /// 替换后文本中的起始位置
    new_start: usize,
    from_text: String,
    to_text: String,
}

/// 在内容中执行反向替换，返回 (总次数, 原始侧位置, 还原侧位置)
fn restore_content(
    content: &mut FileContent,
    mappings: &[(String, String)],
    type_map: &HashMap<String, SensitiveType>,
) -> (usize, Vec<RestoreItem>, Vec<RestoreItem>) {
    restore_content_inner(content, mappings, type_map, false)
}

/// 参数化内部函数：fuzzy=true 时使用模糊匹配版替换
fn restore_content_inner(
    content: &mut FileContent,
    mappings: &[(String, String)],
    type_map: &HashMap<String, SensitiveType>,
    fuzzy: bool,
) -> (usize, Vec<RestoreItem>, Vec<RestoreItem>) {
    let replace_fn = if fuzzy {
        replace_all_in_text_with_positions_fuzzy
    } else {
        replace_all_in_text_with_positions
    };

    let mut total = 0;
    let mut original_items = Vec::new();
    let mut restore_items = Vec::new();

    match content {
        FileContent::Spreadsheet { sheets, .. } => {
            for (sheet_idx, sheet) in sheets.iter_mut().enumerate() {
                for (col, header) in sheet.headers.iter_mut().enumerate() {
                    let matches = replace_fn(header, mappings);
                    for m in &matches {
                        let st = type_map.get(&m.from_text).cloned()
                            .unwrap_or(SensitiveType::PersonName);
                        original_items.push(RestoreItem {
                            row: 0, col,
                            start: m.orig_start,
                            end: m.orig_start + m.from_text.chars().count(),
                            text: m.from_text.clone(),
                            replaced_text: m.to_text.clone(),
                            sensitive_type: st.clone(),
                            sheet_index: sheet_idx,
                        });
                        restore_items.push(RestoreItem {
                            row: 0, col,
                            start: m.new_start,
                            end: m.new_start + m.to_text.chars().count(),
                            text: m.to_text.clone(),
                            replaced_text: m.from_text.clone(),
                            sensitive_type: st,
                            sheet_index: sheet_idx,
                        });
                    }
                    total += matches.len();
                }
                for (row_idx, row) in sheet.rows.iter_mut().enumerate() {
                    for (col, cell) in row.iter_mut().enumerate() {
                        let matches = replace_fn(&mut cell.text, mappings);
                        for m in &matches {
                            let st = type_map.get(&m.from_text).cloned()
                                .unwrap_or(SensitiveType::PersonName);
                            original_items.push(RestoreItem {
                                row: row_idx, col,
                                start: m.orig_start,
                                end: m.orig_start + m.from_text.chars().count(),
                                text: m.from_text.clone(),
                                replaced_text: m.to_text.clone(),
                                sensitive_type: st.clone(),
                                sheet_index: sheet_idx,
                            });
                            restore_items.push(RestoreItem {
                                row: row_idx, col,
                                start: m.new_start,
                                end: m.new_start + m.to_text.chars().count(),
                                text: m.to_text.clone(),
                                replaced_text: m.from_text.clone(),
                                sensitive_type: st,
                                sheet_index: sheet_idx,
                            });
                        }
                        total += matches.len();
                    }
                }
            }
        }
        FileContent::Document { paragraphs, .. } => {
            for para in paragraphs.iter_mut() {
                let row = para.index;
                let matches = replace_fn(&mut para.text, mappings);
                for m in &matches {
                    let st = type_map.get(&m.from_text).cloned()
                        .unwrap_or(SensitiveType::PersonName);
                    original_items.push(RestoreItem {
                        row, col: 0,
                        start: m.orig_start,
                        end: m.orig_start + m.from_text.chars().count(),
                        text: m.from_text.clone(),
                        replaced_text: m.to_text.clone(),
                        sensitive_type: st.clone(),
                        sheet_index: 0,
                    });
                    restore_items.push(RestoreItem {
                        row, col: 0,
                        start: m.new_start,
                        end: m.new_start + m.to_text.chars().count(),
                        text: m.to_text.clone(),
                        replaced_text: m.from_text.clone(),
                        sensitive_type: st,
                        sheet_index: 0,
                    });
                }
                total += matches.len();
            }
        }
    }

    (total, original_items, restore_items)
}

/// 在单个文本中执行所有映射的替换，记录每次替换的位置
/// 使用 Aho-Corasick 多模式匹配，单次扫描完成所有替换
/// 返回替换后各匹配的位置信息（start/end 均为字符索引，适配中文等多字节字符）
fn replace_all_in_text_with_positions(
    text: &mut String,
    mappings: &[(String, String)],
) -> Vec<ReplaceMatch> {
    // 过滤空模式
    let valid: Vec<(usize, &str, &str)> = mappings
        .iter()
        .enumerate()
        .filter(|(_, (from, _))| !from.is_empty())
        .map(|(i, (from, to))| (i, from.as_str(), to.as_str()))
        .collect();

    if valid.is_empty() || text.is_empty() {
        return Vec::new();
    }

    let patterns: Vec<&str> = valid.iter().map(|(_, from, _)| *from).collect();

    // LeftmostLongest: 同一位置优先匹配最长模式，不重叠
    let ac = aho_corasick::AhoCorasick::builder()
        .match_kind(aho_corasick::MatchKind::LeftmostLongest)
        .build(&patterns)
        .expect("构建 AC 自动机失败");

    let mut all_matches: Vec<ReplaceMatch> = Vec::new();
    let mut result = String::new();
    let mut last_byte_end: usize = 0;
    let mut running_char_count: usize = 0;
    let mut char_offset: isize = 0;

    for mat in ac.find_iter(text.as_str()) {
        let idx = mat.pattern().as_usize();
        let (_, from, to) = valid[idx];
        let byte_start = mat.start();
        let from_char_len = from.chars().count();
        let to_char_len = to.chars().count();
        let diff = to_char_len as isize - from_char_len as isize;

        result.push_str(&text[last_byte_end..byte_start]);
        running_char_count += text[last_byte_end..byte_start].chars().count();
        let char_idx = running_char_count;
        let new_pos = (char_idx as isize + char_offset) as usize;
        result.push_str(to);
        all_matches.push(ReplaceMatch {
            orig_start: char_idx,
            new_start: new_pos,
            from_text: from.to_string(),
            to_text: to.to_string(),
        });
        char_offset += diff;
        last_byte_end = mat.end();
        running_char_count += from_char_len;
    }

    if last_byte_end > 0 {
        result.push_str(&text[last_byte_end..]);
        *text = result;
    }

    all_matches
}

/// 在单个文本中执行所有映射的替换，返回替换次数（仅测试使用）
#[cfg(test)]
fn replace_all_in_text(text: &mut String, mappings: &[(String, String)]) -> usize {
    let matches = replace_all_in_text_with_positions(text, mappings);
    matches.len()
}

/// 字符级归一化：全角 → 半角，保持字符数不变（1:1 映射）
fn normalize_text(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            // 全角数字/字母 (０-９ Ａ-Ｚ ａ-ｚ) → 半角
            '\u{FF10}'..='\u{FF19}' | '\u{FF21}'..='\u{FF3A}' | '\u{FF41}'..='\u{FF5A}' => {
                char::from_u32(c as u32 - 0xFEE0).unwrap_or(c)
            }
            // 全角空格 → 半角空格
            '\u{3000}' => ' ',
            // 全角括号 → 半角
            '\u{FF08}' => '(',
            '\u{FF09}' => ')',
            // 全角连字符 → 半角
            '\u{FF0D}' => '-',
            _ => c,
        })
        .collect()
}

/// 模糊匹配版替换：先精确匹配，再对归一化文本补充匹配
fn replace_all_in_text_with_positions_fuzzy(
    text: &mut String,
    mappings: &[(String, String)],
) -> Vec<ReplaceMatch> {
    // Phase 1: 精确匹配
    let mut all_matches = replace_all_in_text_with_positions(text, mappings);

    // Phase 2: 归一化匹配
    let norm_text = normalize_text(text);
    if norm_text == *text {
        // 无归一化差异，直接返回
        return all_matches;
    }

    // 收集 Phase 1 已匹配的 from_text，避免重复
    let matched_froms: std::collections::HashSet<&str> =
        all_matches.iter().map(|m| m.from_text.as_str()).collect();

    // 构建未命中映射的归一化版本
    let fuzzy_mappings: Vec<(String, String)> = mappings
        .iter()
        .filter(|(from, _)| !matched_froms.contains(from.as_str()))
        .map(|(from, to)| (normalize_text(from), to.clone()))
        .filter(|(nf, _)| !nf.is_empty())
        .collect();

    if fuzzy_mappings.is_empty() {
        return all_matches;
    }

    // 对归一化文本执行匹配，定位位置
    let mut norm_clone = norm_text.clone();
    let fuzzy_hits = replace_all_in_text_with_positions(&mut norm_clone, &fuzzy_mappings);

    if fuzzy_hits.is_empty() {
        return all_matches;
    }

    // 构建归一化 from → 原始 from 的映射，用于还原 from_text
    let norm_to_orig_from: HashMap<String, String> = mappings
        .iter()
        .filter(|(from, _)| !matched_froms.contains(from.as_str()))
        .map(|(from, _)| (normalize_text(from), from.clone()))
        .collect();

    // 利用字符索引 1:1 对应关系，在原始文本中定位并替换
    // 需要从后往前替换，避免位置偏移
    let mut result_chars: Vec<char> = text.chars().collect();

    // 按 orig_start 排序（正序），然后从后往前处理
    let mut sorted_hits = fuzzy_hits;
    sorted_hits.sort_by_key(|m| m.orig_start);

    for hit in sorted_hits.iter().rev() {
        let start = hit.orig_start;
        let from_norm = &hit.from_text;
        let to = &hit.to_text;
        let orig_from = norm_to_orig_from.get(from_norm).unwrap_or(from_norm);
        let from_char_len = orig_from.chars().count();

        // 从原始文本中提取对应位置的子串
        let end = start + from_char_len;
        if end > result_chars.len() {
            continue;
        }

        // 替换
        let to_chars: Vec<char> = to.chars().collect();
        result_chars.splice(start..end, to_chars);
    }

    // 重建文本
    let new_text: String = result_chars.into_iter().collect();
    *text = new_text;

    // 重新计算位置（Phase 2 的匹配需要重新计算 new_start）
    // 合并所有匹配，按 orig_start 正序排列后重新计算 new_start
    for hit in sorted_hits.iter() {
        let orig_from = norm_to_orig_from.get(&hit.from_text).unwrap_or(&hit.from_text);
        all_matches.push(ReplaceMatch {
            orig_start: hit.orig_start,
            new_start: 0, // 稍后重新计算
            from_text: orig_from.clone(),
            to_text: hit.to_text.clone(),
        });
    }

    // 按 orig_start 排序所有匹配，重新计算 new_start
    all_matches.sort_by_key(|m| m.orig_start);
    let mut offset: isize = 0;
    for m in all_matches.iter_mut() {
        m.new_start = (m.orig_start as isize + offset) as usize;
        let from_len = m.from_text.chars().count() as isize;
        let to_len = m.to_text.chars().count() as isize;
        offset += to_len - from_len;
    }

    all_matches
}

/// 获取当前 ISO 8601 时间字符串
pub(crate) fn chrono_now() -> String {
    // 使用标准库实现，避免额外依赖
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();

    // 简单的 ISO 8601 格式化
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // 计算年月日（从 1970-01-01 起算）
    let mut y = 1970i64;
    let mut remaining_days = days as i64;

    loop {
        let days_in_year = if is_leap_year(y) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        y += 1;
    }

    let month_days = if is_leap_year(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 0usize;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining_days < md as i64 {
            m = i;
            break;
        }
        remaining_days -= md as i64;
    }

    let d = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m + 1,
        d,
        hours,
        minutes,
        seconds
    )
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_strategies() {
        let strategies = default_strategies();
        assert!(strategies.contains_key("Phone"));
        assert!(strategies.contains_key("PersonName"));
        assert_eq!(strategies.len(), 13);
    }

    #[test]
    fn test_chrono_now_format() {
        let now = chrono_now();
        // 应该是 ISO 8601 格式
        assert!(now.contains("T"));
        assert!(now.ends_with("Z"));
        assert_eq!(now.len(), 20);
    }

    #[test]
    fn test_replace_all_in_text() {
        let mut text = "张三是一个好人，张三很棒".to_string();
        let mappings = vec![("张三".to_string(), "王五".to_string())];
        let count = replace_all_in_text(&mut text, &mappings);
        assert_eq!(count, 2);
        assert_eq!(text, "王五是一个好人，王五很棒");
    }

    #[test]
    fn test_restore_content_spreadsheet() {
        use crate::models::sensitive::CellValue;
        let mut content = FileContent::Spreadsheet {
            file_name: "test.csv".to_string(),
            file_type: crate::models::sensitive::FileType::Csv,
            sheets: vec![crate::models::sensitive::SheetData {
                name: String::new(),
                headers: vec!["姓名".to_string(), "电话".to_string()],
                rows: vec![vec![CellValue::text("假名A".to_string()), CellValue::text("138****8888".to_string())]],
                row_count: 1,
                col_count: 2,
            }],
        };
        let mappings = vec![("假名A".to_string(), "张三".to_string())];
        let type_map = HashMap::new();
        let (count, orig_items, rest_items) = restore_content(&mut content, &mappings, &type_map);
        assert_eq!(count, 1);
        assert_eq!(orig_items.len(), 1);
        assert_eq!(rest_items.len(), 1);
        if let FileContent::Spreadsheet { sheets, .. } = &content {
            assert_eq!(sheets[0].rows[0][0].text, "张三");
        }
    }

    #[test]
    fn test_replace_positions_chinese_text() {
        // 验证中文文本的字符索引（而非字节索引）
        let mut text = "天翼云科技有限公司云密码服务".to_string();
        let mappings = vec![("天翼云科技有限公司".to_string(), "某某公司".to_string())];
        let matches = replace_all_in_text_with_positions(&mut text, &mappings);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].orig_start, 0); // 字符位置 0
        assert_eq!(matches[0].new_start, 0);
        assert_eq!(text, "某某公司云密码服务");

        // 验证中间位置的中文替换
        let mut text2 = "我是张三，张三很好".to_string();
        let mappings2 = vec![("张三".to_string(), "李四丰".to_string())];
        let matches2 = replace_all_in_text_with_positions(&mut text2, &mappings2);
        assert_eq!(matches2.len(), 2);
        // 第一个 "张三" 在字符位置 2
        assert_eq!(matches2[0].orig_start, 2);
        assert_eq!(matches2[0].new_start, 2);
        // 第二个 "张三" 在原文字符位置 5（"我是张三，" = 5个字符之后）
        assert_eq!(matches2[1].orig_start, 5);
        // 替换后偏移：第一个替换把2字符变成3字符，偏移+1
        assert_eq!(matches2[1].new_start, 6);
        assert_eq!(text2, "我是李四丰，李四丰很好");
    }

    #[test]
    fn test_normalize_text() {
        // 全角数字 → 半角
        assert_eq!(normalize_text("１２３"), "123");
        // 全角字母 → 半角
        assert_eq!(normalize_text("ＡＢｃｄ"), "ABcd");
        // 全角空格/括号/连字符
        assert_eq!(normalize_text("（Ａ　Ｂ－Ｃ）"), "(A B-C)");
        // 中文字符不变
        assert_eq!(normalize_text("张三"), "张三");
        // 混合文本
        assert_eq!(normalize_text("电话：１３８００１３８０００"), "电话：13800138000");
        // 字符数不变（1:1 映射）
        assert_eq!(normalize_text("１２３").chars().count(), "１２３".chars().count());
    }

    #[test]
    fn test_fuzzy_replace_fullwidth_digits() {
        // 模拟 AI 将半角数字变成全角数字的场景
        let mut text = "电话：１３８１２３４５６７８".to_string();
        let mappings = vec![("13812345678".to_string(), "真实号码".to_string())];
        let matches = replace_all_in_text_with_positions_fuzzy(&mut text, &mappings);
        assert_eq!(matches.len(), 1);
        assert_eq!(text, "电话：真实号码");
    }

    #[test]
    fn test_fuzzy_replace_exact_first() {
        // 精确匹配优先，不走模糊
        let mut text = "张三是好人".to_string();
        let mappings = vec![("张三".to_string(), "李四".to_string())];
        let matches = replace_all_in_text_with_positions_fuzzy(&mut text, &mappings);
        assert_eq!(matches.len(), 1);
        assert_eq!(text, "李四是好人");
    }

    #[test]
    fn test_fuzzy_replace_no_change_needed() {
        // 文本无全角字符，归一化后相同，直接返回精确匹配结果
        let mut text = "hello world".to_string();
        let mappings = vec![("hello".to_string(), "hi".to_string())];
        let matches = replace_all_in_text_with_positions_fuzzy(&mut text, &mappings);
        assert_eq!(matches.len(), 1);
        assert_eq!(text, "hi world");
    }
}

/// 迁移老工作区计数器：旧 key（如 "PersonName"）→ 新 key（"PersonName_zh"）。
///
/// 老用户绝大多数是中文场景，旧 counter 实际记录的就是中文池消费记录。
/// 迁移到 `_zh` 后缀键，保持中文序号连续性；不影响英文池（从 0 开始）。
pub(crate) fn migrate_legacy_counters(counters: &mut HashMap<String, usize>) {
    for legacy_key in ["PersonName", "OrgName", "Address", "Title"] {
        if let Some(v) = counters.remove(legacy_key) {
            counters.entry(format!("{}_zh", legacy_key)).or_insert(v);
        }
    }
}

#[cfg(test)]
mod migrate_tests {
    use super::*;

    #[test]
    fn test_migrate_moves_legacy_keys() {
        let mut counters = HashMap::new();
        counters.insert("PersonName".to_string(), 5);
        counters.insert("OrgName".to_string(), 3);
        counters.insert("Address".to_string(), 1);
        counters.insert("Title".to_string(), 2);
        // 无关 key 不动
        counters.insert("mou_surname_张".to_string(), 1);

        migrate_legacy_counters(&mut counters);

        assert_eq!(counters.get("PersonName"), None);
        assert_eq!(counters.get("OrgName"), None);
        assert_eq!(counters.get("Address"), None);
        assert_eq!(counters.get("Title"), None);
        assert_eq!(counters.get("PersonName_zh"), Some(&5));
        assert_eq!(counters.get("OrgName_zh"), Some(&3));
        assert_eq!(counters.get("Address_zh"), Some(&1));
        assert_eq!(counters.get("Title_zh"), Some(&2));
        // 无关 key 保留
        assert_eq!(counters.get("mou_surname_张"), Some(&1));
    }

    #[test]
    fn test_migrate_idempotent_when_zh_already_set() {
        let mut counters = HashMap::new();
        counters.insert("PersonName".to_string(), 5);
        counters.insert("PersonName_zh".to_string(), 10); // 已迁移过
        migrate_legacy_counters(&mut counters);
        // 已存在的 _zh 不被覆盖
        assert_eq!(counters.get("PersonName_zh"), Some(&10));
        assert_eq!(counters.get("PersonName"), None);
    }

    #[test]
    fn test_migrate_no_legacy_keys_does_nothing() {
        let mut counters = HashMap::new();
        counters.insert("PersonName_zh".to_string(), 7);
        counters.insert("PersonName_en".to_string(), 3);
        let before = counters.clone();
        migrate_legacy_counters(&mut counters);
        assert_eq!(counters, before);
    }

    #[test]
    fn test_migrate_empty_counters_no_op() {
        let mut counters: HashMap<String, usize> = HashMap::new();
        migrate_legacy_counters(&mut counters);
        assert!(counters.is_empty());
    }
}
