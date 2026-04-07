#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Mutex;

use dimkey_lib::commands::desensitize::{sensitive_type_to_key, string_to_sensitive_type};
use dimkey_lib::desensitizer::{generalize, mask, replace};
use dimkey_lib::desensitizer::replace::ReplaceState;
use dimkey_lib::engine::backends::onnx_token_classifier::OnnxTokenClassifier;
use dimkey_lib::engine::dict_engine::DictEngine;
use dimkey_lib::engine::ner_engine::NerEngine;
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::models::task::*;

use serde::Deserialize;

// ============================================================
// 全管道测试支持（正则 + NER + 词典 合并检测）
// ============================================================

static NER_ENGINE: std::sync::OnceLock<Mutex<NerEngine>> = std::sync::OnceLock::new();

/// 获取或初始化全局 NER 引擎（真实 ONNX 模型）
fn get_ner_engine() -> &'static Mutex<NerEngine> {
    NER_ENGINE.get_or_init(|| {
        let ner_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("ner");
        let engine = match OnnxTokenClassifier::try_load(&ner_dir) {
            Ok(Some(backend)) => {
                eprintln!("[test] NER 引擎已加载 (ONNX)");
                NerEngine::from_backend(Box::new(backend))
            }
            Ok(None) => {
                eprintln!("[test] ⚠️ NER 模型文件不存在，降级运行");
                NerEngine::degraded()
            }
            Err(e) => {
                eprintln!("[test] ⚠️ NER 引擎加载失败: {}，降级运行", e);
                NerEngine::degraded()
            }
        };
        Mutex::new(engine)
    })
}

/// 全管道检测：正则 + NER + 词典，合并去重
///
/// 复现 `src/hooks/useAutoDesensitize.ts:1058-1112` 的敏感项合并逻辑。
/// 注意：不复现列推断合并（1091-1099）和白名单过滤（1114-1125），
/// 这些不影响 SensitiveItem 基线对照。
///
/// 性能警告：NER 引擎是全局单例 + Mutex，多个测试会在 NER 阶段串行化。
pub fn detect_full_pipeline(content: &FileContent, lang: Language) -> Vec<SensitiveItem> {
    // 1. 正则引擎
    let regex_engine = RegexEngine::for_language(lang);
    let regex_items = regex_engine.detect(content);

    // 2. NER 引擎（poison 容错：如果前一次测试 panic 污染了锁，继续用内部数据）
    let ner_items = {
        let engine_mutex = get_ner_engine();
        let mut engine = match engine_mutex.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                eprintln!("[test] ⚠️ NER 锁被 poison，继续使用 inner 数据");
                poisoned.into_inner()
            }
        };
        engine.detect(content).unwrap_or_default()
    };

    // 3. 词典引擎（内置词典）
    let builtin_dict_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("builtin_dict");
    let dict_json = match lang {
        Language::Zh => std::fs::read_to_string(builtin_dict_path.join("zh.json")).unwrap_or_default(),
        Language::En => std::fs::read_to_string(builtin_dict_path.join("en.json")).unwrap_or_default(),
    };

    #[derive(Deserialize)]
    struct BuiltinDictItem {
        text: String,
        sensitive_type: String,
        match_mode: dimkey_lib::models::strategy::MatchMode,
    }

    let dict_entries: Vec<DictEntry> = serde_json::from_str::<Vec<BuiltinDictItem>>(&dict_json)
        .unwrap_or_default()
        .into_iter()
        .map(|item| DictEntry {
            text: item.text,
            sensitive_type: string_to_sensitive_type(&item.sensitive_type),
            match_mode: item.match_mode,
            replacement: None,
            language: None,
            builtin: true,
        })
        .collect();

    let dict_items = if dict_entries.is_empty() {
        vec![]
    } else {
        DictEngine::new(dict_entries).detect(content)
    };

    // 4. 合并去重（与前端 useAutoDesensitize.ts:1101-1112 一致）
    // 正则优先，词典和 NER 只补充非重叠区域
    let mut merged = regex_items;
    for di in dict_items.into_iter().chain(ner_items.into_iter()) {
        let overlap = merged.iter().any(|ex| {
            ex.sheet_index == di.sheet_index
                && ex.row == di.row
                && ex.col == di.col
                && ex.start < di.end
                && di.start < ex.end
        });
        if !overlap {
            merged.push(di);
        }
    }

    merged
}

/// 解析 fixture 文件为 FileContent
pub fn parse_fixture(fixture_abs_path: &str) -> FileContent {
    let path = std::path::Path::new(fixture_abs_path);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "xlsx" | "xls" => dimkey_lib::parser::excel::parse_excel(fixture_abs_path)
            .unwrap_or_else(|e| panic!("Excel 导入失败: {}", e)),
        "csv" => dimkey_lib::parser::excel::parse_csv(fixture_abs_path)
            .unwrap_or_else(|e| panic!("CSV 导入失败: {}", e)),
        "docx" => dimkey_lib::parser::word::parse_docx(fixture_abs_path)
            .unwrap_or_else(|e| panic!("Docx 导入失败: {}", e)),
        "txt" => dimkey_lib::parser::txt::parse_txt(fixture_abs_path)
            .unwrap_or_else(|e| panic!("TXT 导入失败: {}", e)),
        _ => panic!("不支持的格式: {}", ext),
    }
}

/// 全管道基线断言：解析文件 → 三层引擎检测 → 与 sidecar baseline 严格对照
///
/// 如果 NER 引擎降级（模型加载失败），直接 panic 而不是让 soft 项漏检伪装成"NER 能力问题"。
pub fn assert_full_pipeline_baseline(fixture_abs_path: &str, lang: Language) {
    let ner_loaded = {
        let guard = get_ner_engine().lock().unwrap_or_else(|p| p.into_inner());
        guard.is_loaded()
    };
    if !ner_loaded {
        panic!(
            "NER 模型未加载 (resources/ner/model.onnx)，全管道测试无法运行。\n\
             请先准备 NER 模型文件。降级模式会让所有 NER 类基线看起来像漏检。"
        );
    }

    let content = parse_fixture(fixture_abs_path);
    let items = detect_full_pipeline(&content, lang);
    assert_baseline_from_sidecar_strict(&items, fixture_abs_path, None);
}

/// 获取测试数据文件路径（从 e2e/fixtures/scenarios/ 按扩展名查找）
pub fn test_data_path(filename: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let subdir = match ext {
        "xlsx" | "xls" => "xlsx",
        "csv" => "csv",
        "docx" => "docx",
        "pdf" => "pdf",
        "txt" => "txt",
        _ => "csv",
    };
    format!("{}/../e2e/fixtures/scenarios/{}/{}", manifest_dir, subdir, filename)
}

/// 获取 e2e/fixtures/ 下的任意文件路径（用于非 scenarios/ 目录的 fixture）
pub fn fixture_path(relative: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/../e2e/fixtures/{}", manifest_dir, relative)
}

/// 统计识别结果中某种类型的数量
pub fn count_by_type(items: &[SensitiveItem], st: &SensitiveType) -> usize {
    items.iter().filter(|i| &i.sensitive_type == st).count()
}

/// 提取识别结果中某种类型的所有去重文本
pub fn texts_by_type(items: &[SensitiveItem], st: &SensitiveType) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    items
        .iter()
        .filter(|i| &i.sensitive_type == st)
        .filter(|i| seen.insert(i.text.clone()))
        .map(|i| i.text.clone())
        .collect()
}

/// 断言基线中每个 hard 模式的具体值都被识别到
/// expected: &[(&str, &SensitiveType)] — 基线中���记为 hard 的 (文本, 类型) 对
/// 对于未匹配项会 panic 并列出漏检值
pub fn assert_baseline_covered(items: &[SensitiveItem], expected: &[(&str, &SensitiveType)]) {
    let mut missing: Vec<String> = Vec::new();
    for (text, st) in expected {
        // 用 contains ��配：识别结果的 text 可能包含空格差异，做 trim 后精确匹配
        let text_trimmed = text.trim();
        let found = items.iter().any(|i| {
            &i.sensitive_type == *st && i.text.trim() == text_trimmed
        });
        if !found {
            missing.push(format!("  {:?} '{}'", st, text));
        }
    }
    if !missing.is_empty() {
        panic!(
            "基线覆盖检查失败，以下 {} 项未被识别:\n{}",
            missing.len(),
            missing.join("\n")
        );
    }
}

// ============================================================
// .baseline.json sidecar 支持
// ============================================================

/// .baseline.json 文件结构
#[derive(Deserialize)]
pub struct BaselineFile {
    pub fixture: String,
    pub expected: Vec<BaselineEntry>,
}

/// 基线条目
#[derive(Deserialize)]
pub struct BaselineEntry {
    pub value: String,
    #[serde(rename = "type")]
    pub sensitive_type: String,
    pub count: usize,
    #[serde(default)]
    pub note: String,
    #[serde(rename = "assert", default = "default_assert_mode")]
    pub assert_mode: String,
}

fn default_assert_mode() -> String {
    "hard".to_string()
}

/// 从 fixture 路径自动加载对应的 .baseline.json
/// fixture_path: fixture 文件的绝对路径
pub fn load_baseline(fixture_abs_path: &str) -> BaselineFile {
    let sidecar = format!("{}.baseline.json", fixture_abs_path);
    let content = std::fs::read_to_string(&sidecar)
        .unwrap_or_else(|e| panic!("无法读取 baseline 文件 {}: {}", sidecar, e));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("解析 baseline JSON 失败 {}: {}", sidecar, e))
}

/// 将基线中的类型字符串映射为 SensitiveType 枚举
fn parse_sensitive_type(s: &str) -> Option<SensitiveType> {
    match s {
        "Phone" => Some(SensitiveType::Phone),
        "IdCard" => Some(SensitiveType::IdCard),
        "Email" => Some(SensitiveType::Email),
        "Address" => Some(SensitiveType::Address),
        "PersonName" => Some(SensitiveType::PersonName),
        "OrgName" => Some(SensitiveType::OrgName),
        "BankCard" => Some(SensitiveType::BankCard),
        "CreditCode" => Some(SensitiveType::CreditCode),
        "LicensePlate" => Some(SensitiveType::LicensePlate),
        "IpAddress" => Some(SensitiveType::IpAddress),
        "LandlinePhone" | "Landline" => Some(SensitiveType::LandlinePhone),
        "Title" => Some(SensitiveType::Title),
        "Ssn" | "SSN" => Some(SensitiveType::Ssn),
        "UsPhone" => Some(SensitiveType::UsPhone),
        "UkPhone" => Some(SensitiveType::UkPhone),
        "Passport" => Some(SensitiveType::Passport),
        "CreditCard" => Some(SensitiveType::CreditCard),
        "ZipCode" => Some(SensitiveType::ZipCode),
        "DriversLicense" => Some(SensitiveType::DriversLicense),
        "Iban" | "IBAN" => Some(SensitiveType::Iban),
        "UkPostcode" => Some(SensitiveType::UkPostcode),
        _ => None,
    }
}

/// 自动从 .baseline.json 加载基线并断言（全类型模式）
///
/// soft 项未命中只 warning（适合只跑正则引擎的集成测试）。
/// 全管道测试请用 `assert_full_pipeline_baseline` 或 `assert_baseline_strict`。
pub fn assert_baseline_from_sidecar(items: &[SensitiveItem], fixture_abs_path: &str) {
    assert_baseline_from_sidecar_filtered(items, fixture_abs_path, None);
}

/// 自动从 .baseline.json 加载基线并断言（可选类型过滤）
///
/// - `enabled_types: None` → 校验 baseline 中所有类型
/// - `enabled_types: Some(&[Phone, Email])` → 只校验 Phone 和 Email，其余类型跳过
///
/// 适用场景：
/// - 全类型扫描测试 → `None`
/// - 类型过滤测试（如 T02 只启用手机号）→ `Some(&[SensitiveType::Phone])`
/// - 关闭单一类型（如 T04 关闭身份证）→ `Some(&[除 IdCard 外的所有类型])`
///
/// 行为（宽松模式，供单引擎测试使用）：
/// - hard 项且类型在 enabled 范围内：必须命中，否则 panic
/// - hard 项但类型不在 enabled 范围内：跳过（不检查也不 warning）
/// - soft 项且类型在 enabled 范围内：未命中只打 warning（不 panic）
///
/// 严格模式（soft 项未命中也 panic）请用 `assert_baseline_from_sidecar_strict`。
pub fn assert_baseline_from_sidecar_filtered(
    items: &[SensitiveItem],
    fixture_abs_path: &str,
    enabled_types: Option<&[SensitiveType]>,
) {
    assert_baseline_from_sidecar_impl(items, fixture_abs_path, enabled_types, false);
}

/// 严格基线断言：soft 项未命中也 panic。
///
/// 仅用于全管道测试（正则+NER+词典 都跑）。如果只跑单引擎就用这个，所有 soft
/// 项都会被标记为失败 —— 这是测试语义错误而不是被测代码错误。
pub fn assert_baseline_from_sidecar_strict(
    items: &[SensitiveItem],
    fixture_abs_path: &str,
    enabled_types: Option<&[SensitiveType]>,
) {
    assert_baseline_from_sidecar_impl(items, fixture_abs_path, enabled_types, true);
}

fn assert_baseline_from_sidecar_impl(
    items: &[SensitiveItem],
    fixture_abs_path: &str,
    enabled_types: Option<&[SensitiveType]>,
    strict_soft: bool,
) {
    let baseline = load_baseline(fixture_abs_path);
    let mut hard_missing: Vec<String> = Vec::new();
    let mut soft_missing: Vec<String> = Vec::new();
    let mut skipped_types: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 按类型汇总 hard 项的期望数量
    let mut hard_counts: HashMap<SensitiveType, usize> = HashMap::new();

    for entry in &baseline.expected {
        let st = match parse_sensitive_type(&entry.sensitive_type) {
            Some(st) => st,
            None => {
                eprintln!(
                    "[baseline] 跳过未知类型: {} '{}'",
                    entry.sensitive_type, entry.value
                );
                continue;
            }
        };

        // 类型过滤：不在启用范围内的类型直接跳过
        if let Some(enabled) = enabled_types {
            if !enabled.contains(&st) {
                skipped_types.insert(entry.sensitive_type.clone());
                continue;
            }
        }

        let text_trimmed = entry.value.trim();
        let found = items
            .iter()
            .any(|i| i.sensitive_type == st && i.text.trim() == text_trimmed);

        if entry.assert_mode == "hard" {
            *hard_counts.entry(st.clone()).or_default() += entry.count;
            if !found {
                hard_missing.push(format!("  {} '{}'", entry.sensitive_type, entry.value));
            }
        } else if !found {
            soft_missing.push(format!("  {} '{}'", entry.sensitive_type, entry.value));
        }
    }

    // 被过滤掉的类型打印提示
    if !skipped_types.is_empty() {
        let mut types: Vec<&String> = skipped_types.iter().collect();
        types.sort();
        eprintln!(
            "[baseline] 跳过未启用的类型: {}",
            types.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(", ")
        );
    }

    // 组装错误报告
    let mut all_missing: Vec<String> = Vec::new();
    if !hard_missing.is_empty() {
        all_missing.push(format!("硬断言项未命中 ({}):", hard_missing.len()));
        all_missing.extend(hard_missing.iter().cloned());
    }
    if !soft_missing.is_empty() {
        if strict_soft {
            all_missing.push(format!("软断言项未命中 ({}):", soft_missing.len()));
            all_missing.extend(soft_missing.iter().cloned());
        } else {
            // 宽松模式：soft 未命中只 warning
            eprintln!(
                "[baseline] ⚠️  {} 个软断言项未命中（宽松模式，不影响测试通过）:\n{}",
                soft_missing.len(),
                soft_missing.join("\n")
            );
        }
    }

    let has_hard_failure = !hard_missing.is_empty();
    let has_soft_failure_strict = strict_soft && !soft_missing.is_empty();
    if has_hard_failure || has_soft_failure_strict {
        panic!(
            "基线覆盖检查失败，以下项未被识别:\n{}\n\n实际识别到 {} 项",
            all_missing.join("\n"),
            items.len()
        );
    }

    // 按类型验证数量下限
    for (st, expected_count) in &hard_counts {
        let actual = count_by_type(items, st);
        assert!(
            actual >= *expected_count,
            "{:?} 数量不足: 期望 >= {}, 实际 {}",
            st, expected_count, actual
        );
    }
}

/// 在文本中替换敏感项（从后往前替换，避免偏移问题）
fn replace_in_text(
    text: &str,
    items: &[&SensitiveItem],
    consistency_map: &HashMap<(String, SensitiveType), (String, StrategyType)>,
) -> String {
    let mut sorted: Vec<&SensitiveItem> = items.to_vec();
    sorted.sort_by_key(|i| i.start);

    let mut non_overlapping: Vec<&SensitiveItem> = Vec::new();
    let mut last_end = 0usize;
    for item in &sorted {
        if item.start >= last_end {
            non_overlapping.push(item);
            last_end = item.end;
        }
    }

    non_overlapping.sort_by(|a, b| b.start.cmp(&a.start));

    let mut chars: Vec<char> = text.chars().collect();
    for item in non_overlapping {
        let key = (item.text.clone(), item.sensitive_type.clone());
        if let Some((replacement, _)) = consistency_map.get(&key) {
            let end = item.end.min(chars.len());
            let start = item.start.min(end);
            let replacement_chars: Vec<char> = replacement.chars().collect();
            chars.splice(start..end, replacement_chars);
        }
    }

    chars.into_iter().collect()
}

/// 对 FileContent 执行脱敏（不依赖 app_handle，纯内存操作）
/// 复制自 commands/desensitize.rs 核心逻辑，去除 workspace 持久化部分
pub fn desensitize_content(
    content: &FileContent,
    items: &[SensitiveItem],
    strategies: &[StrategyConfig],
) -> DesensitizeResult {
    // 1. 构建策略查找表
    let strategy_map: HashMap<SensitiveType, (Strategy, bool)> = strategies
        .iter()
        .map(|s| (s.sensitive_type.clone(), (s.strategy.clone(), s.consistent)))
        .collect();

    // 2. 构建一致性映射
    let mut consistency_map: HashMap<(String, SensitiveType), (String, StrategyType)> =
        HashMap::new();
    let mut replace_state = ReplaceState::new(42, HashMap::new());

    for item in items {
        let key = (item.text.clone(), item.sensitive_type.clone());
        let (strategy, consistent) = strategy_map
            .get(&item.sensitive_type)
            .cloned()
            .unwrap_or((
                Strategy::Mask {
                    keep_prefix: 1,
                    keep_suffix: 1,
                },
                true,
            ));

        if consistent && consistency_map.contains_key(&key) {
            continue;
        }

        let (replaced, st_type) = match &strategy {
            Strategy::Mask {
                keep_prefix,
                keep_suffix,
            } => {
                let r =
                    mask::apply_mask(&item.text, &item.sensitive_type, *keep_prefix, *keep_suffix);
                (r, StrategyType::Mask)
            }
            Strategy::Replace { ref style } => {
                let r = replace::apply_replace(&item.text, &item.sensitive_type, &mut replace_state, style);
                (r, StrategyType::Replace)
            }
            Strategy::Generalize => {
                let r = generalize::apply_generalize(&item.text, &item.sensitive_type);
                (r, StrategyType::Generalize)
            }
        };

        consistency_map.insert(key, (replaced, st_type));
    }

    // 3. 克隆内容并替换
    let mut new_content = content.clone();
    match &mut new_content {
        FileContent::Spreadsheet {
            sheets, ..
        } => {
            let mut cell_items: HashMap<(usize, usize, usize), Vec<&SensitiveItem>> = HashMap::new();
            for item in items {
                cell_items
                    .entry((item.sheet_index, item.row, item.col))
                    .or_default()
                    .push(item);
            }

            for ((sheet_idx, row, col), ref cell_items) in &cell_items {
                if let Some(sheet) = sheets.get_mut(*sheet_idx) {
                    if *row == 0 {
                        if let Some(header) = sheet.headers.get_mut(*col) {
                            *header = replace_in_text(header, cell_items, &consistency_map);
                        }
                    } else {
                        if let Some(cell_value) = sheet.rows.get_mut(row - 1).and_then(|r| r.get_mut(*col)) {
                            cell_value.text = replace_in_text(&cell_value.text, cell_items, &consistency_map);
                            cell_value.cell_type = CellType::Text;
                        }
                    }
                }
            }
        }
        FileContent::Document { paragraphs, .. } => {
            let mut para_items: HashMap<usize, Vec<&SensitiveItem>> = HashMap::new();
            for item in items {
                para_items.entry(item.row).or_default().push(item);
            }

            for (para_idx, ref p_items) in &para_items {
                if let Some(para) = paragraphs.iter_mut().find(|p| p.index == *para_idx) {
                    para.text = replace_in_text(&para.text, p_items, &consistency_map);
                }
            }
        }
    }

    // 4. 构建映射记录
    let mut mapping_map: HashMap<(String, SensitiveType), MappingEntry> = HashMap::new();
    for item in items {
        let key = (item.text.clone(), item.sensitive_type.clone());
        if let Some((replaced, st_type)) = consistency_map.get(&key) {
            let entry = mapping_map.entry(key).or_insert(MappingEntry {
                original_text: item.text.clone(),
                replaced_text: replaced.clone(),
                sensitive_type: item.sensitive_type.clone(),
                strategy: st_type.clone(),
                occurrences: 0,
            });
            entry.occurrences += 1;
        }
    }
    let mappings: Vec<MappingEntry> = mapping_map.into_values().collect();

    // 5. 构建统计摘要
    let mut by_type: HashMap<String, usize> = HashMap::new();
    for item in items {
        let key = sensitive_type_to_key(&item.sensitive_type);
        *by_type.entry(key).or_default() += 1;
    }
    let total = items.len();
    let summary = DesensitizeSummary { total, by_type };

    DesensitizeResult {
        content: new_content,
        mappings,
        summary,
    }
}

/// 使用映射记录还原 FileContent（仅 Replace 策略可逆）
/// 返回还原的替换次数
pub fn restore_from_mappings(content: &mut FileContent, mappings: &[MappingEntry]) -> usize {
    let mut reverse_mappings: Vec<(String, String)> = mappings
        .iter()
        .filter(|m| m.strategy == StrategyType::Replace)
        .map(|m| (m.replaced_text.clone(), m.original_text.clone()))
        .collect();
    // 按 replaced_text 长度降序，优先匹配长文本
    reverse_mappings.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    let mut total = 0;
    match content {
        FileContent::Spreadsheet {
            sheets, ..
        } => {
            for sheet in sheets.iter_mut() {
                for header in sheet.headers.iter_mut() {
                    for (from, to) in &reverse_mappings {
                        let count = header.matches(from.as_str()).count();
                        if count > 0 {
                            *header = header.replace(from.as_str(), to.as_str());
                            total += count;
                        }
                    }
                }
                for row in sheet.rows.iter_mut() {
                    for cell in row.iter_mut() {
                        for (from, to) in &reverse_mappings {
                            let count = cell.text.matches(from.as_str()).count();
                            if count > 0 {
                                cell.text = cell.text.replace(from.as_str(), to.as_str());
                                total += count;
                            }
                        }
                    }
                }
            }
        }
        FileContent::Document { paragraphs, .. } => {
            for para in paragraphs.iter_mut() {
                for (from, to) in &reverse_mappings {
                    let count = para.text.matches(from.as_str()).count();
                    if count > 0 {
                        para.text = para.text.replace(from.as_str(), to.as_str());
                        total += count;
                    }
                }
            }
        }
    }

    total
}

/// 提取 Spreadsheet 第一个 Sheet 的行数据（便于断言）
pub fn get_rows(content: &FileContent) -> &Vec<Vec<CellValue>> {
    match content {
        FileContent::Spreadsheet { sheets, .. } => &sheets[0].rows,
        _ => panic!("期望 Spreadsheet 类型"),
    }
}

/// 提取 Spreadsheet 第一个 Sheet 的表头（便于断言）
pub fn get_headers(content: &FileContent) -> &Vec<String> {
    match content {
        FileContent::Spreadsheet { sheets, .. } => &sheets[0].headers,
        _ => panic!("期望 Spreadsheet 类型"),
    }
}

/// 提取 Document 的段落（便于断言）
pub fn get_paragraphs(content: &FileContent) -> &Vec<Paragraph> {
    match content {
        FileContent::Document { paragraphs, .. } => paragraphs,
        _ => panic!("期望 Document 类型"),
    }
}
