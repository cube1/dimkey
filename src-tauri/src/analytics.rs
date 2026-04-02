use std::fs;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;
use tauri_plugin_aptabase::EventTracker;

/// 缓存统计开关状态，避免每次都读文件
static ANALYTICS_ENABLED: OnceLock<AtomicBool> = OnceLock::new();

/// 获取统计配置文件路径
fn config_path(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录: {}", e))?;
    fs::create_dir_all(&app_dir)
        .map_err(|e| format!("创建应用数据目录失败: {}", e))?;
    Ok(app_dir.join("analytics.json"))
}

/// 从文件读取 analytics_enabled 值
fn read_enabled_from_file(app_handle: &tauri::AppHandle) -> bool {
    let path = match config_path(app_handle) {
        Ok(p) => p,
        Err(_) => return true,
    };
    if !path.exists() {
        return true;
    }
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return true,
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return true,
    };
    json.get("analytics_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

/// 初始化并获取缓存的开关值
fn is_enabled(app_handle: &tauri::AppHandle) -> bool {
    let flag = ANALYTICS_ENABLED.get_or_init(|| {
        AtomicBool::new(read_enabled_from_file(app_handle))
    });
    flag.load(Ordering::Relaxed)
}

/// 上报事件（检查开关，静默失败）
pub fn track(app_handle: &tauri::AppHandle, event: &str, props: Option<serde_json::Value>) {
    if !is_enabled(app_handle) {
        return;
    }
    let _ = app_handle.track_event(event, props);
}

/// 查询统计开关状态（Tauri command）
#[tauri::command]
pub async fn get_analytics_enabled(app_handle: tauri::AppHandle) -> Result<bool, String> {
    Ok(is_enabled(&app_handle))
}

/// 设置统计开关状态（Tauri command）
#[tauri::command]
pub async fn set_analytics_enabled(enabled: bool, app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(flag) = ANALYTICS_ENABLED.get() {
        flag.store(enabled, Ordering::Relaxed);
    }

    let path = config_path(&app_handle)?;
    let json = serde_json::json!({ "analytics_enabled": enabled });
    let content = serde_json::to_string_pretty(&json)
        .map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("保存统计配置失败: {}", e))?;
    Ok(())
}
