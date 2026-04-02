use std::sync::RwLock;
use tauri::State;
use crate::models::language::Language;

/// 全局语言状态
pub struct AppLanguage(pub RwLock<Language>);

#[tauri::command]
pub fn set_language(lang: String, state: State<AppLanguage>) -> Result<(), String> {
    let language = Language::from_str_loose(&lang);
    let mut current = state.0.write().map_err(|e| format!("语言状态锁失败: {}", e))?;
    *current = language;
    Ok(())
}

#[tauri::command]
pub fn get_language(state: State<AppLanguage>) -> Result<String, String> {
    let current = state.0.read().map_err(|e| format!("语言状态锁失败: {}", e))?;
    let s = match *current {
        Language::Zh => "zh",
        Language::En => "en",
    };
    Ok(s.to_string())
}
