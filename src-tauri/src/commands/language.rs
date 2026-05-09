use std::sync::RwLock;
use tauri::State;
use crate::models::language::Language;

/// 全局语言状态。
///
/// 语言由 Cargo feature 在编译期决定（`lang-zh` / `lang-en`），
/// 运行时不可变更。仍以 `RwLock` 包装，是为了兼容下游 `*state.0.read()` 的调用形式，
/// 避免一处改动牵动十几个 command。
pub struct AppLanguage(pub RwLock<Language>);

impl AppLanguage {
    pub fn from_build() -> Self {
        Self(RwLock::new(Language::current()))
    }
}

#[tauri::command]
pub fn get_language(state: State<AppLanguage>) -> Result<String, String> {
    let current = state.0.read().map_err(|e| format!("语言状态锁失败: {}", e))?;
    Ok(current.code().to_string())
}
