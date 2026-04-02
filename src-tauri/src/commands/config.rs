use std::fs;
use std::collections::HashMap;
use tauri::Manager;
use crate::models::strategy::{StrategyMap, DictEntry, ReplaceStyle};

/// 获取配置文件路径
fn get_config_path(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录: {}", e))?;
    fs::create_dir_all(&app_dir)
        .map_err(|e| format!("创建应用数据目录失败: {}", e))?;
    Ok(app_dir.join("config.json"))
}

/// 获取词典文件路径
fn get_dict_path(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录: {}", e))?;
    fs::create_dir_all(&app_dir)
        .map_err(|e| format!("创建应用数据目录失败: {}", e))?;
    Ok(app_dir.join("dict.json"))
}

/// 读取本地策略配置
/// 返回 HashMap<String, Strategy>，与前端 Record<string, Strategy> 对齐
#[tauri::command]
pub async fn load_config(app_handle: tauri::AppHandle) -> Result<StrategyMap, String> {
    let config_path = get_config_path(&app_handle)?;

    if !config_path.exists() {
        // 首次使用，返回空的策略表（前端会用自己的默认值）
        return Ok(StrategyMap {
            strategies: HashMap::new(),
            replace_style: ReplaceStyle::default(),
        });
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置文件失败: {}", e))?;

    let map: StrategyMap = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置文件失败: {}", e))?;

    Ok(map)
}

/// 保存策略配置到本地
#[tauri::command]
pub async fn save_config(config: StrategyMap, app_handle: tauri::AppHandle) -> Result<(), String> {
    let config_path = get_config_path(&app_handle)?;

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化配置失败: {}", e))?;

    fs::write(&config_path, json)
        .map_err(|e| format!("保存配置文件失败: {}", e))?;

    Ok(())
}

/// 读取自定义词典
#[tauri::command]
pub async fn load_dict(app_handle: tauri::AppHandle) -> Result<Vec<DictEntry>, String> {
    let dict_path = get_dict_path(&app_handle)?;

    if !dict_path.exists() {
        return Ok(vec![]);
    }

    let content = fs::read_to_string(&dict_path)
        .map_err(|e| format!("读取词典文件失败: {}", e))?;

    let entries: Vec<DictEntry> = serde_json::from_str(&content)
        .map_err(|e| format!("解析词典文件失败: {}", e))?;

    Ok(entries)
}

/// 保存自定义词典
#[tauri::command]
pub async fn save_dict(entries: Vec<DictEntry>, app_handle: tauri::AppHandle) -> Result<(), String> {
    let dict_path = get_dict_path(&app_handle)?;
    let entry_count = entries.len();

    let json = serde_json::to_string_pretty(&entries)
        .map_err(|e| format!("序列化词典失败: {}", e))?;

    fs::write(&dict_path, json)
        .map_err(|e| format!("保存词典文件失败: {}", e))?;

    crate::analytics::track(&app_handle, "dict_updated", Some(serde_json::json!({
        "entry_count": entry_count,
    })));

    Ok(())
}
