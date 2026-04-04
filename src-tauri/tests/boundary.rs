mod common;

use dimkey_lib::desensitizer::mask::apply_mask;
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;

use common::*;

/// 测试手机号嵌入在更长数字中不应被误识别
#[test]
fn test_phone_in_longer_number_not_detected() {
    let engine = RegexEngine::new();
    let items = engine.detect_text("订单号20241380013800112", 0, 0);

    let phones: Vec<_> = items
        .iter()
        .filter(|i| i.sensitive_type == SensitiveType::Phone)
        .collect();
    assert!(
        phones.is_empty(),
        "长数字中不应误识别出手机号，但识别出: {:?}",
        phones.iter().map(|i| &i.text).collect::<Vec<_>>()
    );
}

/// 测试身份证号末尾为 X 的识别
#[test]
fn test_idcard_with_trailing_x() {
    let engine = RegexEngine::new();
    let items = engine.detect_text("身份证：11010119900307678X", 0, 0);

    let id_items: Vec<_> = items
        .iter()
        .filter(|i| i.sensitive_type == SensitiveType::IdCard)
        .collect();
    assert_eq!(id_items.len(), 1, "应识别出 1 个身份证号");
    assert_eq!(id_items[0].text, "11010119900307678X");
}

/// 测试同一单元格中包含多种敏感信息
#[test]
fn test_multiple_types_in_one_cell() {
    let engine = RegexEngine::new();
    let items = engine.detect_text("联系方式：13800138001，邮箱 zhangsan@qq.com", 0, 0);

    let phones: Vec<_> = items
        .iter()
        .filter(|i| i.sensitive_type == SensitiveType::Phone)
        .collect();
    let emails: Vec<_> = items
        .iter()
        .filter(|i| i.sensitive_type == SensitiveType::Email)
        .collect();

    assert_eq!(phones.len(), 1, "应识别出 1 个手机号");
    assert_eq!(phones[0].text, "13800138001");
    assert_eq!(emails.len(), 1, "应识别出 1 个邮箱");
    assert_eq!(emails[0].text, "zhangsan@qq.com");
}

/// 测试空内容不会崩溃
#[test]
fn test_empty_content() {
    let engine = RegexEngine::new();

    // 空表格
    let content = FileContent::Spreadsheet {
        file_name: "empty.csv".to_string(),
        file_type: FileType::Csv,
        sheets: vec![SheetData {
            name: String::new(),
            headers: vec![],
            rows: vec![],
            row_count: 0,
            col_count: 0,
        }],
    };
    let items = engine.detect(&content);
    assert!(items.is_empty(), "空内容应返回空结果");

    // 空文档
    let content = FileContent::Document {
        file_name: "empty.docx".to_string(),
        file_type: FileType::Docx,
        paragraphs: vec![],
        encoding: None,
    };
    let items = engine.detect(&content);
    assert!(items.is_empty(), "空文档应返回空结果");
}

/// 测试不含敏感信息的普通文本
#[test]
fn test_no_sensitive_data() {
    let engine = RegexEngine::new();
    let items = engine.detect_text("今天天气不错，适合出门散步", 0, 0);
    assert!(items.is_empty(), "普通文本不应识别出敏感信息");

    let items2 = engine.detect_text("会议纪要：讨论了下半年的工作计划", 0, 0);
    assert!(items2.is_empty(), "无敏感信息的文本不应误报");
}

/// 测试掩码边界：前后缀超过文本长度时全部掩码
#[test]
fn test_mask_short_text() {
    // 2 个字符，keep_prefix=3, keep_suffix=4，超过长度应全掩码
    let result = apply_mask("AB", &SensitiveType::Phone, 3, 4);
    assert_eq!(result, "**", "短文本超限应全部掩码");

    // 空字符串
    let result = apply_mask("", &SensitiveType::Phone, 3, 4);
    assert_eq!(result, "", "空字符串应返回空");

    // 刚好等于 keep_prefix + keep_suffix
    let result = apply_mask("1234567", &SensitiveType::Phone, 3, 4);
    assert_eq!(result, "*******", "前后缀之和等于长度应全掩码");

    // 正常情况
    let result = apply_mask("12345678", &SensitiveType::Phone, 3, 4);
    assert_eq!(result, "123*5678", "前 3 后 4 中间 1 个 *");
}

// ============================================================
// C06: 密码保护文件 — import_file_internal 应返回错误
// ============================================================

/// C06: 加密 XLSX 直接导入应失败，提示需要密码
#[test]
fn test_encrypted_xlsx_import_fails() {
    use dimkey_lib::commands::file::import_file_internal;

    let path = common::fixture_path("sample_encrypted.xlsx");
    let result = import_file_internal(&path);

    assert!(
        result.is_err(),
        "加密文件直接导入应返回错误"
    );
}

/// C07: 密码错误重试 — 错误密码应返回 WRONG_PASSWORD 错误
#[test]
fn test_encrypted_xlsx_wrong_password() {
    use dimkey_lib::commands::file::import_file_with_password_internal;

    let path = fixture_path("sample_encrypted.xlsx");

    // 错误密码
    let result = import_file_with_password_internal(&path, "wrong_password_123");
    assert!(result.is_err(), "错误密码应返回错误");
    let err = result.unwrap_err();
    assert!(
        err.contains("WRONG_PASSWORD") || err.contains("密码") || err.contains("password"),
        "错误信息应提示密码错误: {}",
        err
    );

    // 空密码
    let result2 = import_file_with_password_internal(&path, "");
    assert!(result2.is_err(), "空密码应返回错误");
}

// ============================================================
// C08: 空文件 — 不应崩溃
// ============================================================

/// C08: 空 Excel 文件应能导入但不包含数据行
#[test]
fn test_empty_xlsx_import() {
    use dimkey_lib::commands::file::import_file_internal;

    let path = common::fixture_path("empty.xlsx");
    let result = import_file_internal(&path);

    match result {
        Ok(content) => {
            // 空文件导入成功时，应无数据行或段落
            match &content {
                FileContent::Spreadsheet { sheets, .. } => {
                    let total_rows: usize = sheets.iter().map(|s| s.row_count).sum();
                    assert_eq!(total_rows, 0, "空文件应无数据行");
                }
                FileContent::Document { paragraphs, .. } => {
                    // 空文档可能有 0 或少量空段落
                    let non_empty: Vec<_> = paragraphs.iter()
                        .filter(|p| !p.text.trim().is_empty())
                        .collect();
                    assert!(non_empty.is_empty(), "空文件应无有效段落");
                }
            }

            // 识别结果应为空
            let engine = RegexEngine::new();
            let items = engine.detect(&content);
            assert!(items.is_empty(), "空文件不应识别出敏感信息");
        }
        Err(e) => {
            // 空文件导入失败也是可接受的行为，记录错误
            eprintln!("[C08] 空文件导入返回错误（可接受）: {}", e);
        }
    }
}

// ============================================================
// C09: 大文件 — 不应崩溃或超时
// ============================================================

/// C09: 大 CSV 文件应能正常导入和识别
#[test]
fn test_large_csv_import_and_detect() {
    use dimkey_lib::commands::file::import_file_internal;

    let path = common::fixture_path("large.csv");
    let content = import_file_internal(&path).expect("大文件导入失败");

    if let FileContent::Spreadsheet { sheets, .. } = &content {
        assert!(
            sheets[0].row_count > 0,
            "大文件应有数据行"
        );
    }

    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    // 大文件（500 行）应识别出大量敏感信息
    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 400,
        "大文件应识别出至少 400 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 400,
        "大文件应识别出至少 400 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 400,
        "大文件应识别出至少 400 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );

    // 基线覆盖验证（抽样 15 个 hard 值）
    assert_baseline_from_sidecar(&items, &path);
}
