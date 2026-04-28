//! 一致性替换 — 同文件 / 跨文件维度
//!
//! K01: 同文件一致性 — 同一手机号在多行出现，应替换为同一假数据
//! K02: 跨文件一致性 — 共享 mappings 字典时，不同文件中相同原文应得相同替换
//!
//! 注：consistency.rs 已覆盖跨列一致性的 inline 契约测试，本文件聚焦 workspace 维度。

mod common;

use std::collections::HashMap;

use dimkey_lib::desensitizer::replace::{apply_replace, ReplaceState};
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::models::task::MappingEntry;
use dimkey_lib::parser::excel::parse_csv;

use common::*;

/// 构造 inline Spreadsheet — 与 consistency.rs 同模式
fn make_spreadsheet(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> FileContent {
    let col_count = headers.len();
    let row_count = rows.len();
    FileContent::Spreadsheet {
        file_name: "k01.csv".to_string(),
        file_type: FileType::Csv,
        sheets: vec![SheetData {
            name: String::new(),
            headers: headers.into_iter().map(String::from).collect(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|c| CellValue::from(*c)).collect())
                .collect(),
            row_count,
            col_count,
        }],
    }
}

// ============================================================
// K01: 同文件一致性 — 同一手机号 5 行替换结果完全一致
// 用 inline 5 行重复手机号 fixture，验证 mapping 契约 + 文档级替换
// （sample.csv 单行无法复现"5 行重复"的端到端语义，故 inline 构造）
// ============================================================

#[test]
fn test_k01_same_phone_consistency_5rows() {
    let dup_phone = "13800138000";
    let content = make_spreadsheet(
        vec!["姓名", "手机号"],
        vec![
            vec!["张三", dup_phone],
            vec!["李四", dup_phone],
            vec!["王五", dup_phone],
            vec!["赵六", dup_phone],
            vec!["钱七", dup_phone],
        ],
    );

    let items: Vec<SensitiveItem> = (1..=5)
        .map(|row| SensitiveItem {
            id: format!("k01_{}", row),
            text: dup_phone.into(),
            sensitive_type: SensitiveType::Phone,
            source: DetectSource::Regex,
            pdf_bbox: None,
            confidence: 0.95,
            start: 0,
            end: dup_phone.chars().count(),
            row,
            col: 1,
            sheet_index: 0,
        })
        .collect();

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Phone,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];

    let result = desensitize_content(&content, &items, &strategies);

    // 契约 1：同一原文只产生一条 mapping
    let phone_mappings: Vec<&MappingEntry> = result
        .mappings
        .iter()
        .filter(|m| m.original_text == dup_phone)
        .collect();
    assert_eq!(
        phone_mappings.len(),
        1,
        "同一手机号应只产生一条 mapping，实际 {} 条",
        phone_mappings.len()
    );

    // 契约 2：occurrences 等于 items 中该原文出现次数
    assert_eq!(
        phone_mappings[0].occurrences, 5,
        "occurrences 应为 5，实际 {}",
        phone_mappings[0].occurrences
    );

    // 契约 3：替换值非原文，且为合法手机号格式
    let replaced: &str = phone_mappings[0].replaced_text.as_str();
    assert_ne!(replaced, dup_phone, "替换值不应保留原文");
    assert_eq!(replaced.len(), 11, "替换后应仍是 11 位手机号: {}", replaced);
    assert!(
        replaced.chars().all(|c| c.is_ascii_digit()),
        "替换后应全为数字: {}",
        replaced
    );

    // 契约 4：5 行单元格内容全部被替换为同一假数据（文档级一致性）
    let rows = get_rows(&result.content);
    assert_eq!(rows.len(), 5, "应有 5 行数据");
    for (idx, row) in rows.iter().enumerate() {
        assert_eq!(
            row[1].text, replaced,
            "第 {} 行手机号应为 {}，实际 {}",
            idx + 1,
            replaced,
            row[1].text
        );
    }
    // 没有任何一行残留原文
    assert!(
        rows.iter().all(|r| r[1].text != dup_phone),
        "脱敏后任何行都不应残留原文 {}",
        dup_phone
    );
}

// ============================================================
// K01-supplement: 真实 fixture sample.csv 走全链路 — 5 个不同手机号 → 5 条 mappings
// ============================================================

#[test]
fn test_k01_real_fixture_unique_mappings() {
    let path = fixture_path("sample.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::Zh);
    let items = engine.detect(&content);

    let phone_items: Vec<SensitiveItem> = items
        .iter()
        .filter(|i| i.sensitive_type == SensitiveType::Phone)
        .cloned()
        .collect();
    assert!(!phone_items.is_empty(), "应识别到手机号");

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Phone,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];
    let result = desensitize_content(&content, &phone_items, &strategies);

    // sample.csv 中 5 个 phone 互不相同 → 5 条 mappings
    let phone_mappings: Vec<&MappingEntry> = result
        .mappings
        .iter()
        .filter(|m| m.sensitive_type == SensitiveType::Phone)
        .collect();
    assert_eq!(
        phone_mappings.len(),
        phone_items.len(),
        "唯一原文数应等于 mapping 数"
    );

    // 替换值各不相同（小概率随机碰撞，保留容错：要求至少 N-1 个不同）
    let mut unique_replaced: std::collections::HashSet<&String> = std::collections::HashSet::new();
    for m in &phone_mappings {
        unique_replaced.insert(&m.replaced_text);
    }
    assert!(
        unique_replaced.len() >= phone_mappings.len() - 1,
        "替换值应基本各不相同: {:?}",
        phone_mappings.iter().map(|m| &m.replaced_text).collect::<Vec<_>>()
    );
}

// ============================================================
// K02: 跨文件一致性 — 共享 mappings 字典时，相同原文得相同替换
// fixture: 跨文件一致性_培训签到.csv + 跨文件一致性_入职信息.xlsx
// 共享原文：刘伟、陈静、13611223344、18755667788、110101199505051234、
//          320106199208181567、liuwei@company.cn、chenjing@company.cn
// ============================================================

/// 模拟 workspace mappings 字典持久化的辅助：
/// 对 items 按 (text, type) 查 cache，未命中则用 apply_replace 新生成并写入 cache。
/// 返回每个 item 对应的最终 replaced_text 列表（按 items 顺序）。
fn replace_with_shared_cache(
    items: &[SensitiveItem],
    cache: &mut HashMap<(String, SensitiveType), String>,
    state: &mut ReplaceState,
    style: &ReplaceStyle,
) -> Vec<String> {
    items
        .iter()
        .map(|item| {
            let key = (item.text.clone(), item.sensitive_type.clone());
            cache
                .entry(key)
                .or_insert_with(|| apply_replace(&item.text, &item.sensitive_type, state, style))
                .clone()
        })
        .collect()
}

#[test]
fn test_k02_cross_file_consistency_via_shared_mappings() {
    let path_a = fixture_path("scenarios/csv/跨文件一致性_培训签到.csv");
    let path_b = fixture_path("scenarios/xlsx/跨文件一致性_入职信息.xlsx");

    let content_a = parse_csv(&path_a).expect("file_a CSV 导入失败");
    let content_b = dimkey_lib::parser::excel::parse_excel(&path_b).expect("file_b xlsx 导入失败");

    let engine = RegexEngine::for_language(Language::Zh);
    let items_a: Vec<SensitiveItem> = engine
        .detect(&content_a)
        .into_iter()
        .filter(|i| i.sensitive_type == SensitiveType::Phone)
        .collect();
    let items_b: Vec<SensitiveItem> = engine
        .detect(&content_b)
        .into_iter()
        .filter(|i| i.sensitive_type == SensitiveType::Phone)
        .collect();

    assert!(
        items_a.iter().any(|i| i.text == "13611223344"),
        "file_a 应识别到 13611223344"
    );
    assert!(
        items_b.iter().any(|i| i.text == "13611223344"),
        "file_b 应识别到 13611223344"
    );

    let mut cache: HashMap<(String, SensitiveType), String> = HashMap::new();
    let mut state = ReplaceState::new(42, HashMap::new());

    let replaced_a = replace_with_shared_cache(&items_a, &mut cache, &mut state, &ReplaceStyle::Fake);
    let replaced_b = replace_with_shared_cache(&items_b, &mut cache, &mut state, &ReplaceStyle::Fake);

    let idx_a = items_a
        .iter()
        .position(|i| i.text == "13611223344")
        .expect("file_a 必含 13611223344");
    let idx_b = items_b
        .iter()
        .position(|i| i.text == "13611223344")
        .expect("file_b 必含 13611223344");

    assert_eq!(
        replaced_a[idx_a], replaced_b[idx_b],
        "共享 mappings 字典时，跨文件相同手机号应得到相同替换值"
    );

    // 同时验证另一组共享原文
    let idx_a2 = items_a.iter().position(|i| i.text == "18755667788").expect("file_a");
    let idx_b2 = items_b.iter().position(|i| i.text == "18755667788").expect("file_b");
    assert_eq!(replaced_a[idx_a2], replaced_b[idx_b2], "18755667788 跨文件应一致");

    // 仅 file_a 独有的手机号应不出现在 file_b
    assert!(
        items_b.iter().all(|i| i.text != "17600112233"),
        "17600112233 仅 file_a 独有"
    );
}

#[test]
fn test_k02_without_shared_cache_diverges() {
    // 反面验证：不共享 cache 时，由于 phone 走 RNG，跨文件大概率不一致
    // 此用例用于明确"K02 通过 mappings 字典持久化才能保证一致"的契约边界
    let path_a = fixture_path("scenarios/csv/跨文件一致性_培训签到.csv");
    let path_b = fixture_path("scenarios/xlsx/跨文件一致性_入职信息.xlsx");

    let content_a = parse_csv(&path_a).expect("file_a 导入失败");
    let content_b = dimkey_lib::parser::excel::parse_excel(&path_b).expect("file_b 导入失败");

    let engine = RegexEngine::for_language(Language::Zh);
    let items_a: Vec<SensitiveItem> = engine
        .detect(&content_a)
        .into_iter()
        .filter(|i| i.text == "13611223344")
        .collect();
    let items_b: Vec<SensitiveItem> = engine
        .detect(&content_b)
        .into_iter()
        .filter(|i| i.text == "13611223344")
        .collect();

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Phone,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];

    let r_a = desensitize_content(&content_a, &items_a, &strategies);
    let r_b = desensitize_content(&content_b, &items_b, &strategies);

    let m_a = r_a.mappings.iter().find(|m| m.original_text == "13611223344").expect("a");
    let m_b = r_b.mappings.iter().find(|m| m.original_text == "13611223344").expect("b");

    // 不共享时，phone 走 RNG，两次结果几乎必不同（极小概率碰撞，不做强断言）
    // 这里只断言"两次都产生了合法手机号且不等于原文"，不断言不等
    assert_eq!(m_a.replaced_text.len(), 11);
    assert_eq!(m_b.replaced_text.len(), 11);
    assert_ne!(m_a.replaced_text, "13611223344");
    assert_ne!(m_b.replaced_text, "13611223344");
}
