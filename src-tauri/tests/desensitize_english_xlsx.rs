//! 英文 XLSX 端到端脱敏测试
//! 导入 → 正则识别 → Mask/Replace 脱敏 → 验证效果

mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::models::task::StrategyType;
use dimkey_lib::parser::excel::parse_excel;

use common::*;

/// 测试英文 XLSX 导入后的结构正确性
#[test]
fn test_en_xlsx_import_structure() {
    let path = test_data_path("us_compliance_audit.xlsx");
    let content = parse_excel(&path).expect("Excel 导入失败");

    if let FileContent::Spreadsheet { sheets, .. } = &content {
        let sheet = &sheets[0];
        assert!(
            !sheet.headers.is_empty(),
            "表头不应为空"
        );
        assert!(
            sheet.row_count > 0,
            "应有数据行"
        );
    } else {
        panic!("期望 Spreadsheet 类型");
    }
}

/// 测试英文 XLSX 正则引擎能识别出主要英文类型
#[test]
fn test_en_xlsx_regex_detect_types() {
    let path = test_data_path("us_compliance_audit.xlsx");
    let content = parse_excel(&path).expect("Excel 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert!(!items.is_empty(), "应识别出敏感信息");

    // 合规审计表中应有 SSN 和 Email
    let types: Vec<SensitiveType> = items.iter().map(|i| i.sensitive_type.clone()).collect();
    let has_ssn = types.contains(&SensitiveType::Ssn);
    let has_email = types.contains(&SensitiveType::Email);
    assert!(
        has_ssn || has_email,
        "合规审计表应至少包含 SSN 或 Email，实际类型: {:?}",
        types.iter().collect::<std::collections::HashSet<_>>()
    );
}

/// 测试英文 XLSX 全类型 Mask 脱敏
#[test]
fn test_en_xlsx_mask_all_types() {
    let path = test_data_path("us_compliance_audit.xlsx");
    let content = parse_excel(&path).expect("Excel 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert!(!items.is_empty(), "应识别出敏感信息");

    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::Ssn,
            strategy: Strategy::Mask { keep_prefix: 3, keep_suffix: 0 },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::UsPhone,
            strategy: Strategy::Mask { keep_prefix: 0, keep_suffix: 4 },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::Email,
            strategy: Strategy::Mask { keep_prefix: 1, keep_suffix: 0 },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::CreditCard,
            strategy: Strategy::Mask { keep_prefix: 0, keep_suffix: 4 },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::Iban,
            strategy: Strategy::Mask { keep_prefix: 4, keep_suffix: 0 },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::ZipCode,
            strategy: Strategy::Mask { keep_prefix: 2, keep_suffix: 0 },
            consistent: true,
        },
    ];

    let result = desensitize_content(&content, &items, &strategies);

    assert!(
        !result.mappings.is_empty(),
        "应有脱敏映射记录"
    );

    for mapping in &result.mappings {
        assert_eq!(mapping.strategy, StrategyType::Mask);
        assert_ne!(
            mapping.replaced_text, mapping.original_text,
            "掩码后应不同于原文: {}",
            mapping.original_text
        );
        assert!(
            mapping.replaced_text.contains('*'),
            "掩码结果应包含 *: {}",
            mapping.replaced_text
        );
    }
}

/// 测试英文 law_firm XLSX 导入 + 识别 + Replace 脱敏
#[test]
fn test_en_xlsx_law_firm_replace() {
    let path = test_data_path("law_firm_client_intake.xlsx");
    let content = parse_excel(&path).expect("Excel 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert!(!items.is_empty(), "律所客户登记表应识别出敏感信息");

    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::Ssn,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::UsPhone,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::Email,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
    ];

    let result = desensitize_content(&content, &items, &strategies);

    // 验证脱敏后原始数据不存在
    let original_texts: Vec<String> = items.iter().map(|i| i.text.clone()).collect();
    let new_rows = get_rows(&result.content);
    let all_new_text: String = new_rows
        .iter()
        .flat_map(|r| r.iter().map(|c| c.text.clone()))
        .collect::<Vec<_>>()
        .join(" ");

    for text in &original_texts {
        if result.mappings.iter().any(|m| m.original_text == *text) {
            assert!(
                !all_new_text.contains(text.as_str()),
                "脱敏后不应包含原始敏感信息: {}",
                text
            );
        }
    }
}
