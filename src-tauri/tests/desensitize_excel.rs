mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::parser::excel::parse_excel;

use common::*;

/// 测试 Excel 导入后的结构正确性
#[test]
fn test_xlsx_import_structure() {
    let path = test_data_path("员工花名册.xlsx");
    let content = parse_excel(&path).expect("Excel 导入失败");

    if let FileContent::Spreadsheet {
        sheets,
        ..
    } = &content
    {
        let sheet = &sheets[0];
        let headers = &sheet.headers;
        let rows = &sheet.rows;
        let row_count = sheet.row_count;
        // 员工花名册有 12 列
        assert!(
            headers.contains(&"工号".to_string()),
            "表头应包含'工号'"
        );
        assert!(
            headers.contains(&"姓名".to_string()),
            "表头应包含'姓名'"
        );
        assert!(
            headers.contains(&"手机号".to_string()),
            "表头应包含'手机号'"
        );
        assert!(
            headers.contains(&"身份证号".to_string()),
            "表头应包含'身份证号'"
        );
        assert!(
            headers.contains(&"邮箱".to_string()),
            "表头应包含'邮箱'"
        );
        assert_eq!(row_count, 10, "应有 10 行数据");
        assert_eq!(rows.len(), 10);
    } else {
        panic!("期望 Spreadsheet 类型");
    }
}

/// 测试 Excel 识别数量（10 行，每行有手机/身份证/邮箱/银行卡 + 紧急联系电话）
#[test]
fn test_xlsx_regex_detect_counts() {
    let path = test_data_path("员工花名册.xlsx");
    let content = parse_excel(&path).expect("Excel 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    // 每行有主手机 + 紧急联系电话 = 至少 20 个 Phone
    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 10,
        "应识别出至少 10 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 10,
        "应识别出至少 10 个身份证号"
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 10,
        "应识别出至少 10 个邮箱"
    );
    assert!(
        count_by_type(&items, &SensitiveType::BankCard) >= 10,
        "应识别出至少 10 个银行卡号"
    );
}

/// 测试脱敏后非敏感列（工号列）不被修改
#[test]
fn test_xlsx_mask_preserves_unrelated_columns() {
    let path = test_data_path("员工花名册.xlsx");
    let content = parse_excel(&path).expect("Excel 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    // 找到工号列的索引
    let headers = get_headers(&content);
    let emp_id_col = headers
        .iter()
        .position(|h| h == "工号")
        .expect("应有'工号'列");

    // 记录原始工号值
    let original_rows = get_rows(&content);
    let original_emp_ids: Vec<String> = original_rows.iter().map(|r| r[emp_id_col].text.clone()).collect();

    // 用 Mask 策略脱敏所有类型
    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::Phone,
            strategy: Strategy::Mask {
                keep_prefix: 3,
                keep_suffix: 4,
            },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::IdCard,
            strategy: Strategy::Mask {
                keep_prefix: 6,
                keep_suffix: 4,
            },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::Email,
            strategy: Strategy::Mask {
                keep_prefix: 1,
                keep_suffix: 0,
            },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::BankCard,
            strategy: Strategy::Mask {
                keep_prefix: 4,
                keep_suffix: 4,
            },
            consistent: true,
        },
    ];

    let result = desensitize_content(&content, &items, &strategies);

    // 验证脱敏后工号列完全不变
    let new_rows = get_rows(&result.content);
    for (i, row) in new_rows.iter().enumerate() {
        assert_eq!(
            row[emp_id_col], original_emp_ids[i],
            "第 {} 行工号不应被修改",
            i + 1
        );
    }
}

/// 测试地址泛化：长地址截断到较高行政级别，短地名用占位符替换
#[test]
fn test_xlsx_generalize_address() {
    use dimkey_lib::desensitizer::generalize::apply_generalize;

    // 长地址（市+区+详细）：截断到市级
    let result = apply_generalize(
        "北京市海淀区中关村大街1号院2号楼301室",
        &SensitiveType::Address,
    );
    assert_eq!(result, "北京市", "应截断到市级: {}", result);

    // 长地址（省+市+区+详细）：截断到省级
    let result2 = apply_generalize(
        "广东省深圳市南山区科技园南区高新南一道008号",
        &SensitiveType::Address,
    );
    assert_eq!(result2, "广东省", "应截断到省级: {}", result2);

    // 短地名"深圳市"：无法进一步截断，替换为"某市"
    let result3 = apply_generalize("深圳市", &SensitiveType::Address);
    assert_eq!(result3, "某市", "短地名应替换为占位符: {}", result3);

    // 无后缀的短地名：替换为"某地"
    let result4 = apply_generalize("北京", &SensitiveType::Address);
    assert_eq!(result4, "某地", "无后缀短地名应替换为'某地': {}", result4);
}
