//! 英文 CSV 端到端脱敏测试
//! 导入 → 正则识别 → Mask/Replace 脱敏 → 验证效果

mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::models::task::StrategyType;
use dimkey_lib::parser::excel::parse_csv;

use common::*;

/// 测试英文 CSV 导入后的结构正确性
#[test]
fn test_en_csv_import_structure() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");

    if let FileContent::Spreadsheet { sheets, .. } = &content {
        let sheet = &sheets[0];
        assert_eq!(
            sheet.headers,
            vec!["Name", "Department", "SSN", "Phone", "Email", "Address", "Passport", "Bank Account"]
        );
        assert_eq!(sheet.row_count, 10, "应有 10 行数据");
        assert_eq!(sheet.col_count, 8, "应有 8 列");
        // 验证第一行数据
        assert_eq!(sheet.rows[0][0], "James Anderson");
        assert_eq!(sheet.rows[0][2], "539-48-2671");
        assert_eq!(sheet.rows[0][4], "james.anderson@techcorp.com");
    } else {
        panic!("期望 Spreadsheet 类型");
    }
}

/// 测试英文 CSV 正则识别各类型数量
#[test]
fn test_en_csv_regex_detect_counts() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Ssn) >= 10,
        "应识别出至少 10 个 SSN，实际: {}",
        count_by_type(&items, &SensitiveType::Ssn)
    );
    assert!(
        count_by_type(&items, &SensitiveType::UsPhone) >= 10,
        "应识别出至少 10 个美国电话，实际: {}",
        count_by_type(&items, &SensitiveType::UsPhone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 10,
        "应识别出至少 10 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
}

/// 测试英文 SSN 掩码：539-**-**** 格式
#[test]
fn test_en_csv_mask_ssn() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let all_items = engine.detect(&content);

    let ssn_items: Vec<_> = all_items
        .into_iter()
        .filter(|i| i.sensitive_type == SensitiveType::Ssn)
        .collect();

    assert!(!ssn_items.is_empty(), "应识别出 SSN");

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Ssn,
        strategy: Strategy::Mask {
            keep_prefix: 3,
            keep_suffix: 0,
        },
        consistent: true,
    }];

    let result = desensitize_content(&content, &ssn_items, &strategies);

    for mapping in &result.mappings {
        assert_eq!(mapping.strategy, StrategyType::Mask);
        let replaced = &mapping.replaced_text;
        let original = &mapping.original_text;
        // SSN 格式: xxx-xx-xxxx (11 字符)
        assert_eq!(replaced.len(), original.len(), "掩码后长度应不变: {}", replaced);
        assert_eq!(
            &replaced[..3],
            &original[..3],
            "前 3 位应保留: {}",
            replaced
        );
        assert!(
            replaced[3..].contains('*'),
            "后半部分应包含掩码字符: {}",
            replaced
        );
    }
}

/// 测试英文 CSV Replace 策略：SSN 替换后格式合法
#[test]
fn test_en_csv_replace_ssn() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let all_items = engine.detect(&content);

    let ssn_items: Vec<_> = all_items
        .into_iter()
        .filter(|i| i.sensitive_type == SensitiveType::Ssn)
        .collect();

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Ssn,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];

    let result = desensitize_content(&content, &ssn_items, &strategies);

    for mapping in &result.mappings {
        assert_eq!(mapping.strategy, StrategyType::Replace);
        assert_ne!(
            mapping.replaced_text, mapping.original_text,
            "替换后应不同于原文"
        );
    }
}

/// 测试英文 CSV Replace 策略：Email 替换后包含 @
#[test]
fn test_en_csv_replace_email() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let all_items = engine.detect(&content);

    let email_items: Vec<_> = all_items
        .into_iter()
        .filter(|i| i.sensitive_type == SensitiveType::Email)
        .collect();

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Email,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];

    let result = desensitize_content(&content, &email_items, &strategies);

    for mapping in &result.mappings {
        assert_ne!(mapping.replaced_text, mapping.original_text);
        assert!(
            mapping.replaced_text.contains('@'),
            "假邮箱应包含 @: {}",
            mapping.replaced_text
        );
    }
}

/// 测试脱敏后非敏感列（Department）不被修改
#[test]
fn test_en_csv_preserves_unrelated_columns() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    // 找到 Department 列索引
    let headers = get_headers(&content);
    let dept_col = headers
        .iter()
        .position(|h| h == "Department")
        .expect("应有 Department 列");

    // 记录原始 Department 值
    let original_rows = get_rows(&content);
    let original_depts: Vec<String> = original_rows.iter().map(|r| r[dept_col].text.clone()).collect();

    // 用 Mask 策略脱敏所有英文类型
    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::Ssn,
            strategy: Strategy::Mask { keep_prefix: 3, keep_suffix: 0 },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::UsPhone,
            strategy: Strategy::Mask { keep_prefix: 3, keep_suffix: 4 },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::Email,
            strategy: Strategy::Mask { keep_prefix: 1, keep_suffix: 0 },
            consistent: true,
        },
    ];

    let result = desensitize_content(&content, &items, &strategies);

    // 验证 Department 列完全不变
    let new_rows = get_rows(&result.content);
    for (i, row) in new_rows.iter().enumerate() {
        assert_eq!(
            row[dept_col], original_depts[i],
            "第 {} 行 Department 不应被修改",
            i + 1
        );
    }
}

/// 测试脱敏后不存在原始敏感信息泄漏
#[test]
fn test_en_csv_no_sensitive_leak() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert!(!items.is_empty(), "应识别到敏感信息");

    let original_texts: Vec<String> = items.iter().map(|i| i.text.clone()).collect();

    // 全 Replace 脱敏
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
        StrategyConfig {
            sensitive_type: SensitiveType::Iban,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::Passport,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::ZipCode,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
    ];

    let result = desensitize_content(&content, &items, &strategies);

    // 收集脱敏后全文
    let new_rows = get_rows(&result.content);
    let all_new_text: String = new_rows
        .iter()
        .flat_map(|r| r.iter().map(|c| c.text.clone()))
        .collect::<Vec<_>>()
        .join(" ");

    for text in &original_texts {
        assert!(
            !all_new_text.contains(text.as_str()),
            "脱敏后不应包含原始敏感信息: {}",
            text
        );
    }
}
