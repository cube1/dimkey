mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::models::task::StrategyType;
use dimkey_lib::parser::excel::parse_csv;

use common::*;

/// 测试 CSV 导入后的结构正确性
#[test]
fn test_csv_import_structure() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");

    if let FileContent::Spreadsheet {
        sheets,
        ..
    } = &content
    {
        let sheet = &sheets[0];
        let headers = &sheet.headers;
        let rows = &sheet.rows;
        let row_count = sheet.row_count;
        let col_count = sheet.col_count;
        assert_eq!(
            headers,
            &["姓名", "手机号", "身份证号", "邮箱", "银行卡号", "家庭住址", "所属公司"]
        );
        assert_eq!(row_count, 8);
        assert_eq!(col_count, 7);
        assert_eq!(rows.len(), 8);
        // 验证第一行具体数据
        assert_eq!(rows[0][0], "张三");
        assert_eq!(rows[0][1], "13800138001");
        assert_eq!(rows[0][2], "110101199003076789");
        assert_eq!(rows[0][3], "zhangsan@qq.com");
    } else {
        panic!("期望 Spreadsheet 类型");
    }
}

/// 测试正则引擎对 CSV 的识别数量（每行都有手机/身份证/邮箱/银行卡）
#[test]
fn test_csv_regex_detect_counts() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 8,
        "应识别出至少 8 个手机号"
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 8,
        "应识别出至少 8 个身份证号"
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 8,
        "应识别出至少 8 个邮箱"
    );
    assert!(
        count_by_type(&items, &SensitiveType::BankCard) >= 8,
        "应识别出至少 8 个银行卡号"
    );
}

/// 测试第一行数据的精确识别结果（具体文本 + 类型）
#[test]
fn test_csv_regex_detect_exact_first_row() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    // row=1 是第一行数据（row=0 是表头）
    let row1_items: Vec<_> = items.iter().filter(|i| i.row == 1).collect();

    assert!(
        row1_items
            .iter()
            .any(|i| i.text == "13800138001" && i.sensitive_type == SensitiveType::Phone),
        "应识别出手机号 13800138001"
    );
    assert!(
        row1_items
            .iter()
            .any(|i| i.text == "110101199003076789" && i.sensitive_type == SensitiveType::IdCard),
        "应识别出身份证号 110101199003076789"
    );
    assert!(
        row1_items
            .iter()
            .any(|i| i.text == "zhangsan@qq.com" && i.sensitive_type == SensitiveType::Email),
        "应识别出邮箱 zhangsan@qq.com"
    );
    assert!(
        row1_items
            .iter()
            .any(|i| i.text == "6222021234567890123"
                && i.sensitive_type == SensitiveType::BankCard),
        "应识别出银行卡号 6222021234567890123"
    );
}

/// 测试手机号掩码：138****8001 格式
#[test]
fn test_csv_mask_phone() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let all_items = engine.detect(&content);

    let phone_items: Vec<_> = all_items
        .into_iter()
        .filter(|i| i.sensitive_type == SensitiveType::Phone)
        .collect();

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Phone,
        strategy: Strategy::Mask {
            keep_prefix: 3,
            keep_suffix: 4,
        },
        consistent: true,
    }];

    let result = desensitize_content(&content, &phone_items, &strategies);

    for mapping in &result.mappings {
        assert_eq!(mapping.strategy, StrategyType::Mask);
        // 手机号 11 位，前 3 后 4，中间 4 个 *
        let replaced = &mapping.replaced_text;
        let original = &mapping.original_text;
        assert_eq!(replaced.len(), 11, "掩码后长度应为 11: {}", replaced);
        assert_eq!(
            &replaced[..3],
            &original[..3],
            "前 3 位应保留: {}",
            replaced
        );
        assert_eq!(&replaced[3..7], "****", "中间应为 ****: {}", replaced);
        assert_eq!(
            &replaced[7..],
            &original[7..],
            "后 4 位应保留: {}",
            replaced
        );
    }
}

/// 测试身份证掩码：1101**********6789 格式
#[test]
fn test_csv_mask_idcard() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let all_items = engine.detect(&content);

    let id_items: Vec<_> = all_items
        .into_iter()
        .filter(|i| i.sensitive_type == SensitiveType::IdCard)
        .collect();

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::IdCard,
        strategy: Strategy::Mask {
            keep_prefix: 6,
            keep_suffix: 4,
        },
        consistent: true,
    }];

    let result = desensitize_content(&content, &id_items, &strategies);

    for mapping in &result.mappings {
        let replaced = &mapping.replaced_text;
        let original = &mapping.original_text;
        assert_eq!(replaced.len(), 18, "身份证掩码后长度应为 18: {}", replaced);
        assert_eq!(&replaced[..6], &original[..6], "前 6 位应保留");
        assert_eq!(&replaced[14..], &original[14..], "后 4 位应保留");
        assert!(
            replaced[6..14].chars().all(|c| c == '*'),
            "中间 8 位应全为 *"
        );
    }
}

/// 测试 Replace 策略生成的假手机号格式合法
#[test]
fn test_csv_replace_phone() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let all_items = engine.detect(&content);

    let phone_items: Vec<_> = all_items
        .into_iter()
        .filter(|i| i.sensitive_type == SensitiveType::Phone)
        .collect();

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Phone,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];

    let result = desensitize_content(&content, &phone_items, &strategies);

    for mapping in &result.mappings {
        assert_eq!(mapping.strategy, StrategyType::Replace);
        assert_ne!(
            mapping.replaced_text, mapping.original_text,
            "替换后应不同于原文"
        );
        // 假手机号应为 11 位数字，1 开头
        let replaced = &mapping.replaced_text;
        assert_eq!(replaced.len(), 11, "假手机号应为 11 位: {}", replaced);
        assert!(
            replaced.starts_with('1'),
            "假手机号应以 1 开头: {}",
            replaced
        );
        assert!(
            replaced.chars().all(|c| c.is_ascii_digit()),
            "假手机号应全为数字: {}",
            replaced
        );
    }
}

/// 测试 Replace 策略生成的假邮箱格式合法
#[test]
fn test_csv_replace_email() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
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

/// 测试脱敏统计摘要的正确性
#[test]
fn test_csv_desensitize_summary() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    // 全量用 Replace 策略
    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::Phone,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::IdCard,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::Email,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::BankCard,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
    ];

    let result = desensitize_content(&content, &items, &strategies);

    // 至少 8 行 × 4 种 = 32 个敏感项
    assert!(
        result.summary.total >= 32,
        "脱敏总数应 >= 32，实际: {}",
        result.summary.total
    );
    assert!(
        result
            .summary
            .by_type
            .get("Phone")
            .copied()
            .unwrap_or(0)
            >= 8,
        "Phone 统计应 >= 8"
    );
    assert!(
        result
            .summary
            .by_type
            .get("IdCard")
            .copied()
            .unwrap_or(0)
            >= 8,
        "IdCard 统计应 >= 8"
    );
}
