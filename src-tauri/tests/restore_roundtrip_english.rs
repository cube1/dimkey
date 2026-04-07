//! 英文法律场景端到端往返测试 (C44-C49)
//! 导入 → 识别(英文正则) → 脱敏(Replace) → 导出 → 重导入 → 还原 → 逐格/段对比

mod common;

use dimkey_lib::commands::file::{export_content, import_file_internal};
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use common::*;

/// 构建英文敏感类型的全 Replace 策略（Replace 可逆，用于往返验证）
fn all_replace_strategies_en() -> Vec<StrategyConfig> {
    let types = vec![
        SensitiveType::Ssn,
        SensitiveType::UsPhone,
        SensitiveType::UkPhone,
        SensitiveType::Email,
        SensitiveType::CreditCard,
        SensitiveType::Iban,
        SensitiveType::DriversLicense,
        SensitiveType::ZipCode,
        SensitiveType::Passport,
        SensitiveType::UkPostcode,
        SensitiveType::PersonName,
        SensitiveType::OrgName,
        SensitiveType::Address,
        SensitiveType::Title,
    ];
    types
        .into_iter()
        .map(|st| StrategyConfig {
            sensitive_type: st,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        })
        .collect()
}

/// 表格文件往返辅助：导入 → 识别 → 脱敏 → 导出 → 重导入 → 还原 → 逐格对比
fn assert_spreadsheet_roundtrip(fixture_rel_path: &str, suffix: &str) {
    let path = fixture_path(fixture_rel_path);
    let original = import_file_internal(&path).expect("导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&original);

    assert!(!items.is_empty(), "应检测到敏感信息: {}", fixture_rel_path);

    let strategies = all_replace_strategies_en();
    let result = desensitize_content(&original, &items, &strategies);

    // 脱敏后应有映射产生
    assert!(
        !result.mappings.is_empty(),
        "脱敏后应有映射记录: {}",
        fixture_rel_path
    );

    // 导出到临时文件
    let tmp = tempfile::Builder::new()
        .suffix(suffix)
        .tempfile()
        .expect("创建临时文件失败");
    let tmp_path = tmp.path().to_str().unwrap();
    export_content(&result.content, tmp_path, None).expect("导出失败");

    // 重新导入
    let reimported = import_file_internal(tmp_path).expect("重新导入失败");
    let mut restored = reimported;

    // 使用映射还原
    let restored_count = restore_from_mappings(&mut restored, &result.mappings);
    assert!(restored_count > 0, "应有还原操作发生: {}", fixture_rel_path);

    // 逐格对比
    let original_rows = get_rows(&original);
    let restored_rows = get_rows(&restored);

    assert_eq!(
        original_rows.len(),
        restored_rows.len(),
        "行数应一致: {}",
        fixture_rel_path
    );
    for (row_idx, (orig_row, rest_row)) in
        original_rows.iter().zip(restored_rows.iter()).enumerate()
    {
        for (col_idx, (orig_cell, rest_cell)) in
            orig_row.iter().zip(rest_row.iter()).enumerate()
        {
            assert_eq!(
                orig_cell, rest_cell,
                "{} 第 {} 行第 {} 列不一致: 原文='{}', 还原='{}'",
                fixture_rel_path,
                row_idx + 1,
                col_idx + 1,
                orig_cell,
                rest_cell
            );
        }
    }
}

/// 文档文件往返辅助：导入 → 识别 → 脱敏 → 导出 → 重导入 → 还原 → 逐段落对比
fn assert_document_roundtrip(fixture_rel_path: &str, suffix: &str) {
    let path = fixture_path(fixture_rel_path);
    let original = import_file_internal(&path).expect("导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&original);

    assert!(!items.is_empty(), "应检测到敏感信息: {}", fixture_rel_path);

    let strategies = all_replace_strategies_en();
    let result = desensitize_content(&original, &items, &strategies);

    assert!(
        !result.mappings.is_empty(),
        "脱敏后应有映射记录: {}",
        fixture_rel_path
    );

    // 验证脱敏后内容与原文不同
    let orig_paras = get_paragraphs(&original);
    let new_paras = get_paragraphs(&result.content);
    let has_change = orig_paras
        .iter()
        .zip(new_paras.iter())
        .any(|(o, n)| o.text != n.text);
    assert!(has_change, "脱敏后应有段落发生变化: {}", fixture_rel_path);

    // 导出到临时文件（docx 需要 original_path 做模板）
    let tmp = tempfile::Builder::new()
        .suffix(suffix)
        .tempfile()
        .expect("创建临时文件失败");
    let tmp_path = tmp.path().to_str().unwrap();
    let original_path = if suffix == ".docx" { Some(path.as_str()) } else { None };
    export_content(&result.content, tmp_path, original_path).expect("导出失败");

    // 重新导入
    let reimported = import_file_internal(tmp_path).expect("重新导入失败");
    let mut restored = reimported;

    // 使用映射还原
    let restored_count = restore_from_mappings(&mut restored, &result.mappings);
    assert!(restored_count > 0, "应有还原操作发生: {}", fixture_rel_path);

    // 逐段落对比
    let restored_paras = get_paragraphs(&restored);
    assert_eq!(
        orig_paras.len(),
        restored_paras.len(),
        "段落数量应一致: {}",
        fixture_rel_path
    );
    for (i, (orig, rest)) in orig_paras.iter().zip(restored_paras.iter()).enumerate() {
        assert_eq!(
            orig.text, rest.text,
            "{} 第 {} 段落不一致: 原文='{}', 还原='{}'",
            fixture_rel_path, i, orig.text, rest.text
        );
    }
}

// ============================================================
// C44: law_firm_client_intake.xlsx — 表格往返
// ============================================================

#[test]
fn test_roundtrip_en_law_firm_client_intake() {
    assert_spreadsheet_roundtrip("scenarios/xlsx/law_firm_client_intake.xlsx", ".xlsx");
}

// ============================================================
// C45: legal_case_management.csv — 表格往返
// ============================================================

#[test]
fn test_roundtrip_en_legal_case_management() {
    assert_spreadsheet_roundtrip("scenarios/csv/legal_case_management.csv", ".csv");
}

// ============================================================
// C46: attorney_engagement_letter.docx — 文档往返
// ============================================================

#[test]
fn test_roundtrip_en_attorney_engagement_letter() {
    assert_document_roundtrip("scenarios/docx/attorney_engagement_letter.docx", ".docx");
}

// ============================================================
// C47: litigation_discovery_memo.docx — 文档往返
// ============================================================

#[test]
fn test_roundtrip_en_litigation_discovery_memo() {
    assert_document_roundtrip("scenarios/docx/litigation_discovery_memo.docx", ".docx");
}

// ============================================================
// C48: legal_billing_records.csv — 表格往返
// ============================================================

#[test]
fn test_roundtrip_en_legal_billing_records() {
    assert_spreadsheet_roundtrip("scenarios/csv/legal_billing_records.csv", ".csv");
}

// ============================================================
// C49: english_legal_edge_cases.txt — 文档往返（内存级，TXT 无模板导出）
// ============================================================

#[test]
fn test_roundtrip_en_english_legal_edge_cases() {
    let path = fixture_path("boundary/english_legal_edge_cases.txt");
    let original = import_file_internal(&path).expect("TXT 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&original);

    assert!(!items.is_empty(), "TXT 应检测到敏感信息");

    let strategies = all_replace_strategies_en();
    let result = desensitize_content(&original, &items, &strategies);

    // 验证脱敏后内容与原文不同
    let orig_paras = get_paragraphs(&original);
    let new_paras = get_paragraphs(&result.content);
    let has_change = orig_paras
        .iter()
        .zip(new_paras.iter())
        .any(|(o, n)| o.text != n.text);
    assert!(has_change, "脱敏后应有段落发生变化");

    // 内存还原（TXT 无特殊导出格式，直接在内存中验证）
    let mut restored = result.content.clone();
    let restored_count = restore_from_mappings(&mut restored, &result.mappings);
    assert!(restored_count > 0, "应有还原操作发生");

    // 逐段落对比
    let restored_paras = get_paragraphs(&restored);
    for (i, (orig, rest)) in orig_paras.iter().zip(restored_paras.iter()).enumerate() {
        assert_eq!(
            orig.text, rest.text,
            "第 {} 段落不一致: 原文='{}', 还原='{}'",
            i, orig.text, rest.text
        );
    }
}
