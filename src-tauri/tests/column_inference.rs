mod common;

use dimkey_lib::commands::detect::detect_columns_internal;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::excel::{parse_csv, parse_excel};

use common::*;

// ============================================================
// L01: 自动列推断 — 员工花名册.xlsx
// ============================================================

/// L01: 自动列推断应正确识别手机号、身份证号、邮箱等列
#[test]
fn test_auto_column_inference() {
    let path = test_data_path("员工花名册.xlsx");
    let content = parse_excel(&path).expect("Excel 导入失败");

    let inferences = detect_columns_internal(&content, None, Language::Zh)
        .expect("列推断失败");

    assert!(!inferences.is_empty(), "应有列推断结果");

    // 找到已推断的列
    let inferred: Vec<_> = inferences.iter()
        .filter(|i| i.inferred_type.is_some())
        .collect();
    assert!(
        inferred.len() >= 3,
        "至少应推断出 3 列为敏感类型（手机/身份证/邮箱），实际: {}",
        inferred.len()
    );

    // 验证手机号列
    let phone_col = inferences.iter()
        .find(|i| i.header == "手机号");
    if let Some(col) = phone_col {
        assert_eq!(
            col.inferred_type,
            Some(SensitiveType::Phone),
            "手机号列应推断为 Phone"
        );
        assert!(col.confidence >= 0.8, "手机号列置信度应 >= 0.8，实际: {:.2}", col.confidence);
    }

    // 验证身份证号列
    let id_col = inferences.iter()
        .find(|i| i.header == "身份证号");
    if let Some(col) = id_col {
        assert_eq!(
            col.inferred_type,
            Some(SensitiveType::IdCard),
            "身份证号列应推断为 IdCard"
        );
        assert!(col.confidence >= 0.8, "身份证号列置信度应 >= 0.8");
    }

    // 验证邮箱列
    let email_col = inferences.iter()
        .find(|i| i.header == "邮箱");
    if let Some(col) = email_col {
        assert_eq!(
            col.inferred_type,
            Some(SensitiveType::Email),
            "邮箱列应推断为 Email"
        );
    }
}

// ============================================================
// L02: 修改列策略 — 推断结果可用于构建策略
// ============================================================

/// L02: 推断结果包含正确的列索引和 sheet 索引，可用于构建列级策略
#[test]
fn test_column_inference_metadata() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");

    let inferences = detect_columns_internal(&content, None, Language::Zh)
        .expect("列推断失败");

    // CSV 只有一个 sheet
    for inf in &inferences {
        assert_eq!(inf.sheet_index, 0, "CSV 应为 sheet 0");
    }

    // 列索引应连续
    let cols: Vec<usize> = inferences.iter().map(|i| i.col).collect();
    for (i, col) in cols.iter().enumerate() {
        assert_eq!(*col, i, "列索引应连续");
    }

    // sample_total 应等于数据行数
    let row_count = match &content {
        FileContent::Spreadsheet { sheets, .. } => sheets[0].row_count,
        _ => panic!("期望 Spreadsheet"),
    };
    for inf in &inferences {
        assert_eq!(inf.sample_total, row_count, "采样总数应等于行数");
    }
}

// ============================================================
// L03: 覆盖列类型为不敏感 — 非敏感列推断结果应为 None
// ============================================================

/// L03: 不含敏感数据的列（如工号、备注）推断类型应为 None
#[test]
fn test_non_sensitive_column_inferred_as_none() {
    let path = test_data_path("员工花名册.xlsx");
    let content = parse_excel(&path).expect("Excel 导入失败");

    let inferences = detect_columns_internal(&content, None, Language::Zh)
        .expect("列推断失败");

    // "工号" 列不应推断为任何敏感类型
    let emp_id_col = inferences.iter()
        .find(|i| i.header == "工号");
    if let Some(col) = emp_id_col {
        assert!(
            col.inferred_type.is_none() || col.confidence < 0.3,
            "工号列不应推断为敏感类型，推断: {:?}, 置信度: {:.2}",
            col.inferred_type, col.confidence
        );
    }
}

// ============================================================
// L04: 导入导出列规则 — 推断结果结构完整性
// ============================================================

/// L04: 每个列推断结果应包含完整的元数据（header, col, confidence, sample_hits）
#[test]
fn test_column_inference_structure_complete() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");

    let inferences = detect_columns_internal(&content, Some(5), Language::Zh)
        .expect("列推断失败");

    let headers = match &content {
        FileContent::Spreadsheet { sheets, .. } => &sheets[0].headers,
        _ => panic!("期望 Spreadsheet"),
    };

    // 推断结果数量应等于列数
    assert_eq!(inferences.len(), headers.len(), "推断结果数应等于列数");

    for inf in &inferences {
        // header 不应为空
        assert!(!inf.header.is_empty(), "header 不应为空");
        // confidence 在 [0, 1] 范围
        assert!(
            inf.confidence >= 0.0 && inf.confidence <= 1.0,
            "置信度应在 [0, 1]，实际: {}",
            inf.confidence
        );
        // sample_hits <= sample_total
        assert!(
            inf.sample_hits <= inf.sample_total,
            "命中数不应超过采样数: {} > {}",
            inf.sample_hits, inf.sample_total
        );
        // sample_total <= 5（指定了 sample_size=5）
        assert!(
            inf.sample_total <= 5,
            "采样数不应超过指定的 sample_size=5，实际: {}",
            inf.sample_total
        );
    }
}

/// L04 补充: Document 类型应返回错误
#[test]
fn test_column_inference_rejects_document() {
    let content = FileContent::Document {
        file_name: "test.txt".to_string(),
        file_type: FileType::Txt,
        paragraphs: vec![],
        encoding: None,
    };

    let result = detect_columns_internal(&content, None, Language::Zh);
    assert!(result.is_err(), "Document 类型应返回错误");
}
