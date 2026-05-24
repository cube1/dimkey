//! 英文敏感信息一致性替换测试
//! 验证相同 SSN/Email/Phone 在多行/多列中被一致替换

mod common;

use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;

use common::*;

/// 构造英文测试用 Spreadsheet FileContent
fn make_en_spreadsheet(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> FileContent {
    let col_count = headers.len();
    let row_count = rows.len();
    FileContent::Spreadsheet {
        file_name: "test.csv".to_string(),
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

fn make_item(text: &str, st: SensitiveType, row: usize, col: usize) -> SensitiveItem {
    SensitiveItem {
        id: format!("{}_{}", row, col),
        text: text.into(),
        sensitive_type: st,
        source: DetectSource::Regex,
        pdf_bboxes: None,
        confidence: 0.95,
        start: 0,
        end: text.chars().count(),
        row,
        col,
        sheet_index: 0,
    }
}

/// 测试相同 SSN 被一致性替换为同一个假值
#[test]
fn test_same_ssn_replaced_consistently() {
    let content = make_en_spreadsheet(
        vec!["SSN"],
        vec![
            vec!["123-45-6789"],
            vec!["123-45-6789"],
            vec!["123-45-6789"],
        ],
    );

    let items = vec![
        make_item("123-45-6789", SensitiveType::Ssn, 1, 0),
        make_item("123-45-6789", SensitiveType::Ssn, 2, 0),
        make_item("123-45-6789", SensitiveType::Ssn, 3, 0),
    ];

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Ssn,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];

    let result = desensitize_content(&content, &items, &strategies);

    let rows = get_rows(&result.content);
    assert_eq!(rows[0][0], rows[1][0], "第 1、2 行应替换为相同 SSN");
    assert_eq!(rows[1][0], rows[2][0], "第 2、3 行应替换为相同 SSN");
    assert_ne!(rows[0][0], "123-45-6789", "不应保留原文");

    assert_eq!(result.mappings.len(), 1, "应只有一条映射记录");
    assert_eq!(result.mappings[0].occurrences, 3, "出现次数应为 3");
}

/// 测试不同 SSN 被替换为不同假值
#[test]
fn test_different_ssns_replaced_differently() {
    let content = make_en_spreadsheet(
        vec!["SSN"],
        vec![
            vec!["123-45-6789"],
            vec!["987-65-4321"],
            vec!["123-45-6789"],
        ],
    );

    let items = vec![
        make_item("123-45-6789", SensitiveType::Ssn, 1, 0),
        make_item("987-65-4321", SensitiveType::Ssn, 2, 0),
        make_item("123-45-6789", SensitiveType::Ssn, 3, 0),
    ];

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Ssn,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];

    let result = desensitize_content(&content, &items, &strategies);

    let rows = get_rows(&result.content);
    assert_eq!(rows[0][0], rows[2][0], "相同 SSN 应替换为相同值");
    assert_ne!(rows[0][0], rows[1][0], "不同 SSN 应替换为不同值");
}

/// 测试跨列一致性：同一 Email 出现在不同列
#[test]
fn test_en_consistency_across_columns() {
    let content = make_en_spreadsheet(
        vec!["Primary Email", "CC Email"],
        vec![
            vec!["john@test.com", "jane@test.com"],
            vec!["jane@test.com", "john@test.com"],
        ],
    );

    let items = vec![
        make_item("john@test.com", SensitiveType::Email, 1, 0),
        make_item("jane@test.com", SensitiveType::Email, 1, 1),
        make_item("jane@test.com", SensitiveType::Email, 2, 0),
        make_item("john@test.com", SensitiveType::Email, 2, 1),
    ];

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Email,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];

    let result = desensitize_content(&content, &items, &strategies);

    let rows = get_rows(&result.content);
    // john@test.com 在 (0,0) 和 (1,1) 应相同
    assert_eq!(rows[0][0], rows[1][1], "跨列的 john@test.com 应替换为相同值");
    // jane@test.com 在 (0,1) 和 (1,0) 应相同
    assert_eq!(rows[0][1], rows[1][0], "跨列的 jane@test.com 应替换为相同值");
    // john 和 jane 应不同
    assert_ne!(rows[0][0], rows[0][1], "不同 Email 应替换为不同值");
}

/// 测试多种英文类型混合一致性替换
#[test]
fn test_en_mixed_types_consistency() {
    let content = make_en_spreadsheet(
        vec!["SSN", "Phone", "Email"],
        vec![
            vec!["123-45-6789", "(415) 293-8847", "john@test.com"],
            vec!["123-45-6789", "(415) 293-8847", "john@test.com"],
        ],
    );

    let items = vec![
        make_item("123-45-6789", SensitiveType::Ssn, 1, 0),
        make_item("(415) 293-8847", SensitiveType::UsPhone, 1, 1),
        make_item("john@test.com", SensitiveType::Email, 1, 2),
        make_item("123-45-6789", SensitiveType::Ssn, 2, 0),
        make_item("(415) 293-8847", SensitiveType::UsPhone, 2, 1),
        make_item("john@test.com", SensitiveType::Email, 2, 2),
    ];

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

    let rows = get_rows(&result.content);
    // 每种类型跨行一致
    assert_eq!(rows[0][0], rows[1][0], "SSN 应跨行一致");
    assert_eq!(rows[0][1], rows[1][1], "Phone 应跨行一致");
    assert_eq!(rows[0][2], rows[1][2], "Email 应跨行一致");

    // 不同类型应产生不同值
    assert_ne!(rows[0][0], "123-45-6789", "SSN 不应保留原文");
    assert_ne!(rows[0][2], "john@test.com", "Email 不应保留原文");

    // 应有 3 条映射
    assert_eq!(result.mappings.len(), 3, "应有 3 条映射记录（SSN + Phone + Email）");
}
