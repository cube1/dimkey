mod common;

use dimkey_lib::desensitizer::mask::apply_mask;
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;

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
