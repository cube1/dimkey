mod common;

use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;

use common::*;

/// 构造测试用 Spreadsheet FileContent
fn make_spreadsheet(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> FileContent {
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

/// 测试相同手机号被一致性替换为同一个假号码
#[test]
fn test_same_phone_replaced_consistently() {
    let content = make_spreadsheet(
        vec!["手机号"],
        vec![
            vec!["13800138001"],
            vec!["13800138001"],
            vec!["13800138001"],
        ],
    );

    // 构造 3 个识别项（模拟引擎识别结果）
    let items = vec![
        SensitiveItem {
            id: "1".into(),
            text: "13800138001".into(),
            sensitive_type: SensitiveType::Phone,
            source: DetectSource::Regex,
            confidence: 0.95,
            start: 0,
            end: 11,
            row: 1,
            col: 0,
            sheet_index: 0,
        },
        SensitiveItem {
            id: "2".into(),
            text: "13800138001".into(),
            sensitive_type: SensitiveType::Phone,
            source: DetectSource::Regex,
            confidence: 0.95,
            start: 0,
            end: 11,
            row: 2,
            col: 0,
            sheet_index: 0,
        },
        SensitiveItem {
            id: "3".into(),
            text: "13800138001".into(),
            sensitive_type: SensitiveType::Phone,
            source: DetectSource::Regex,
            confidence: 0.95,
            start: 0,
            end: 11,
            row: 3,
            col: 0,
            sheet_index: 0,
        },
    ];

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Phone,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];

    let result = desensitize_content(&content, &items, &strategies);

    // 3 个单元格应替换为同一个值
    let rows = get_rows(&result.content);
    assert_eq!(rows[0][0], rows[1][0], "第 1、2 行应替换为相同值");
    assert_eq!(rows[1][0], rows[2][0], "第 2、3 行应替换为相同值");
    assert_ne!(rows[0][0], "13800138001", "不应保留原文");

    // mappings 中 occurrences 应为 3
    assert_eq!(result.mappings.len(), 1, "应只有一条映射记录");
    assert_eq!(result.mappings[0].occurrences, 3, "出现次数应为 3");
}

/// 测试不同手机号被替换为不同假号码，相同手机号仍一致
#[test]
fn test_different_phones_replaced_differently() {
    let content = make_spreadsheet(
        vec!["手机号"],
        vec![
            vec!["13800138001"],
            vec!["15912345678"],
            vec!["13800138001"],
        ],
    );

    let items = vec![
        SensitiveItem {
            id: "1".into(),
            text: "13800138001".into(),
            sensitive_type: SensitiveType::Phone,
            source: DetectSource::Regex,
            confidence: 0.95,
            start: 0,
            end: 11,
            row: 1,
            col: 0,
            sheet_index: 0,
        },
        SensitiveItem {
            id: "2".into(),
            text: "15912345678".into(),
            sensitive_type: SensitiveType::Phone,
            source: DetectSource::Regex,
            confidence: 0.95,
            start: 0,
            end: 11,
            row: 2,
            col: 0,
            sheet_index: 0,
        },
        SensitiveItem {
            id: "3".into(),
            text: "13800138001".into(),
            sensitive_type: SensitiveType::Phone,
            source: DetectSource::Regex,
            confidence: 0.95,
            start: 0,
            end: 11,
            row: 3,
            col: 0,
            sheet_index: 0,
        },
    ];

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Phone,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];

    let result = desensitize_content(&content, &items, &strategies);

    let rows = get_rows(&result.content);
    // 第 1 行和第 3 行是相同原文，应一致
    assert_eq!(rows[0][0], rows[2][0], "相同手机号应替换为相同值");
    // 第 2 行是不同原文，应不同
    assert_ne!(rows[0][0], rows[1][0], "不同手机号应替换为不同值");
}

/// 测试跨列一致性：同一姓名出现在不同列，也应一致替换
#[test]
fn test_consistency_across_columns() {
    let content = make_spreadsheet(
        vec!["姓名", "紧急联系人"],
        vec![
            vec!["张三", "李四"],
            vec!["李四", "张三"],
        ],
    );

    // "张三" 出现在 (1,0) 和 (2,1)
    // "李四" 出现在 (1,1) 和 (2,0)
    let items = vec![
        SensitiveItem {
            id: "1".into(),
            text: "张三".into(),
            sensitive_type: SensitiveType::PersonName,
            source: DetectSource::Regex,
            confidence: 0.95,
            start: 0,
            end: 2,
            row: 1,
            col: 0,
            sheet_index: 0,
        },
        SensitiveItem {
            id: "2".into(),
            text: "李四".into(),
            sensitive_type: SensitiveType::PersonName,
            source: DetectSource::Regex,
            confidence: 0.95,
            start: 0,
            end: 2,
            row: 1,
            col: 1,
            sheet_index: 0,
        },
        SensitiveItem {
            id: "3".into(),
            text: "李四".into(),
            sensitive_type: SensitiveType::PersonName,
            source: DetectSource::Regex,
            confidence: 0.95,
            start: 0,
            end: 2,
            row: 2,
            col: 0,
            sheet_index: 0,
        },
        SensitiveItem {
            id: "4".into(),
            text: "张三".into(),
            sensitive_type: SensitiveType::PersonName,
            source: DetectSource::Regex,
            confidence: 0.95,
            start: 0,
            end: 2,
            row: 2,
            col: 1,
            sheet_index: 0,
        },
    ];

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::PersonName,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];

    let result = desensitize_content(&content, &items, &strategies);

    let rows = get_rows(&result.content);
    // "张三" 在 (0,0) 和 (1,1)，应相同
    assert_eq!(rows[0][0], rows[1][1], "跨列的 '张三' 应替换为相同值");
    // "李四" 在 (0,1) 和 (1,0)，应相同
    assert_eq!(rows[0][1], rows[1][0], "跨列的 '李四' 应替换为相同值");
    // "张三" 和 "李四" 应不同
    assert_ne!(rows[0][0], rows[0][1], "'张三' 和 '李四' 应替换为不同值");
}
