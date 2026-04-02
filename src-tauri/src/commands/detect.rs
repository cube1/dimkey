use std::collections::HashMap;
use std::sync::OnceLock;
use crate::models::sensitive::{FileContent, SensitiveItem, ColumnInference};
use crate::models::language::Language;
use crate::engine::regex_engine::RegexEngine;
use crate::commands::desensitize::sensitive_type_to_key;
use crate::commands::language::AppLanguage;

/// 全局正则引擎单例（避免每次调用重新编译正则表达式）
static REGEX_ENGINE: OnceLock<RegexEngine> = OnceLock::new();
/// 英文正则引擎单例
static REGEX_ENGINE_EN: OnceLock<RegexEngine> = OnceLock::new();

/// 运行规则引擎识别（快速，同步返回）
/// enabled_types: 可选，仅扫描指定的敏感类型（不传则全量扫描）
#[tauri::command]
pub async fn detect_by_regex(
    content: FileContent,
    enabled_types: Option<Vec<String>>,
    app_handle: tauri::AppHandle,
    language_state: tauri::State<'_, AppLanguage>,
) -> Result<Vec<SensitiveItem>, String> {
    let lang = *language_state.0.read().map_err(|e| format!("语言状态锁失败: {}", e))?;
    let engine = match lang {
        Language::En => REGEX_ENGINE_EN.get_or_init(|| RegexEngine::for_language(Language::En)),
        Language::Zh => REGEX_ENGINE.get_or_init(RegexEngine::new),
    };
    let items = engine.detect(&content);
    // 按启用类型过滤
    let result = if let Some(ref types) = enabled_types {
        items
            .into_iter()
            .filter(|item| {
                let key = sensitive_type_to_key(&item.sensitive_type);
                types.contains(&key)
            })
            .collect()
    } else {
        items
    };
    crate::analytics::track(&app_handle, "detection_completed", Some(serde_json::json!({
        "engine": "regex",
        "sensitive_count": result.len(),
    })));
    Ok(result)
}

/// 运行 NER 模型识别（慢速，异步补充）
#[tauri::command]
pub async fn detect_by_ner(
    content: FileContent,
    ner_state: tauri::State<'_, crate::NerEngineState>,
    app_handle: tauri::AppHandle,
) -> Result<Vec<SensitiveItem>, String> {
    let engine_arc = ner_state.0.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut engine = engine_arc.lock().map_err(|e| format!("获取 NER 引擎失败: {}", e))?;
        engine.detect(&content)
    })
    .await
    .map_err(|e| format!("NER 任务执行失败: {}", e))?;

    if let Ok(ref items) = result {
        crate::analytics::track(&app_handle, "detection_completed", Some(serde_json::json!({
            "engine": "ner",
            "sensitive_count": items.len(),
        })));
    }
    result
}

/// 内置词典（编译时嵌入）
const BUILTIN_DICT_ZH: &str = include_str!("../../resources/builtin_dict/zh.json");
const BUILTIN_DICT_EN: &str = include_str!("../../resources/builtin_dict/en.json");

/// 内置词典条目（简化格式，不含 language/builtin/replacement 字段）
#[derive(serde::Deserialize)]
struct BuiltinDictItem {
    text: String,
    sensitive_type: String,
    match_mode: crate::models::strategy::MatchMode,
}

/// 加载内置词典并转换为 DictEntry
fn load_builtin_dict(lang: Language) -> Vec<crate::models::strategy::DictEntry> {
    let json = match lang {
        Language::Zh => BUILTIN_DICT_ZH,
        Language::En => BUILTIN_DICT_EN,
    };
    let items: Vec<BuiltinDictItem> = serde_json::from_str(json).unwrap_or_default();
    items
        .into_iter()
        .map(|item| crate::models::strategy::DictEntry {
            text: item.text,
            sensitive_type: crate::commands::desensitize::string_to_sensitive_type(&item.sensitive_type),
            match_mode: item.match_mode,
            replacement: None,
            language: None, // 内置词典已按语言文件分开，不需要再标记
            builtin: true,
        })
        .collect()
}

/// 运行词典匹配（合并内置词典 + 用户词典，按语言过滤）
#[tauri::command]
pub async fn detect_by_dict(
    content: FileContent,
    dict_entries: Vec<crate::models::strategy::DictEntry>,
    app_handle: tauri::AppHandle,
    language_state: tauri::State<'_, AppLanguage>,
) -> Result<Vec<SensitiveItem>, String> {
    use crate::engine::dict_engine::DictEngine;

    let lang = *language_state.0.read().map_err(|e| format!("语言状态锁失败: {}", e))?;
    let lang_str = match lang {
        Language::Zh => "zh",
        Language::En => "en",
    };

    // 过滤用户词典：只保留匹配当前语言或无语言标记的条目
    let mut entries: Vec<crate::models::strategy::DictEntry> = dict_entries
        .into_iter()
        .filter(|e| {
            match &e.language {
                None => true, // 无语言标记，所有语言生效
                Some(l) => l == lang_str,
            }
        })
        .collect();

    // 合并内置词典
    entries.extend(load_builtin_dict(lang));

    if entries.is_empty() {
        return Ok(vec![]);
    }

    let result = tokio::task::spawn_blocking(move || {
        let engine = DictEngine::new(entries);
        Ok(engine.detect(&content))
    })
    .await
    .map_err(|e| format!("词典匹配任务失败: {}", e))?;

    if let Ok(ref items) = result {
        crate::analytics::track(&app_handle, "detection_completed", Some(serde_json::json!({
            "engine": "dict",
            "sensitive_count": items.len(),
        })));
    }
    result
}

/// 获取当前语言的内置词典（供前端展示）
#[tauri::command]
pub async fn get_builtin_dict(
    language_state: tauri::State<'_, AppLanguage>,
) -> Result<Vec<crate::models::strategy::DictEntry>, String> {
    let lang = *language_state.0.read().map_err(|e| format!("语言状态锁失败: {}", e))?;
    Ok(load_builtin_dict(lang))
}

/// 列级类型推断：对表格每列采样前 N 行，统计各类型命中率
#[tauri::command]
pub async fn detect_columns(
    content: FileContent,
    sample_size: Option<usize>,
    language_state: tauri::State<'_, AppLanguage>,
) -> Result<Vec<ColumnInference>, String> {
    let sheets = match &content {
        FileContent::Spreadsheet { sheets, .. } => sheets.clone(),
        _ => return Err("列级推断仅支持表格类型文件".to_string()),
    };

    let sample_n = sample_size.unwrap_or(100);
    let lang = *language_state.0.read().map_err(|e| format!("语言状态锁失败: {}", e))?;
    let engine = match lang {
        Language::En => REGEX_ENGINE_EN.get_or_init(|| RegexEngine::for_language(Language::En)),
        Language::Zh => REGEX_ENGINE.get_or_init(RegexEngine::new),
    };
    let mut inferences = Vec::new();

    for (sheet_idx, sheet) in sheets.iter().enumerate() {
        let col_count = sheet.headers.len();

        for col_idx in 0..col_count {
            let header = sheet.headers[col_idx].clone();
            let sample_total = sheet.rows.len().min(sample_n);
            let mut type_hits: HashMap<String, usize> = HashMap::new();
            let mut hit_count = 0usize;

            for row_idx in 0..sample_total {
                let cell = match sheet.rows[row_idx].get(col_idx) {
                    Some(cv) if !cv.text.is_empty() => &cv.text,
                    _ => continue,
                };

                let items = engine.detect_text(cell, row_idx + 1, col_idx);
                if items.is_empty() {
                    continue;
                }

                // 计算敏感项覆盖的字符数占单元格总字符数的比例
                let cell_chars = cell.chars().count();
                let covered_chars: usize = items.iter().map(|i| i.end - i.start).sum();
                // 覆盖率 >= 50% 才计为命中
                if covered_chars * 2 >= cell_chars {
                    hit_count += 1;
                    // 取覆盖最多字符的类型
                    let mut type_coverage: HashMap<String, usize> = HashMap::new();
                    for item in &items {
                        let key = sensitive_type_to_key(&item.sensitive_type);
                        *type_coverage.entry(key).or_default() += item.end - item.start;
                    }
                    if let Some((best_type, _)) = type_coverage.into_iter().max_by_key(|(_, v)| *v) {
                        *type_hits.entry(best_type).or_default() += 1;
                    }
                }
            }

            let confidence = if sample_total > 0 {
                hit_count as f64 / sample_total as f64
            } else {
                0.0
            };

            let inferred_type = if confidence >= 0.3 {
                type_hits
                    .iter()
                    .max_by_key(|(_, v)| *v)
                    .map(|(k, _)| crate::commands::desensitize::string_to_sensitive_type(k))
            } else {
                None
            };

            inferences.push(ColumnInference {
                col: col_idx,
                header,
                inferred_type,
                confidence,
                sample_hits: hit_count,
                sample_total,
                sheet_index: sheet_idx,
            });
        }
    }

    Ok(inferences)
}
