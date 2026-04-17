//! 英文场景边界测试（对标中文 boundary.rs）
//! 覆盖：误匹配防护、无敏感文本、空内容、掩码边界、多类型混合

mod common;

use dimkey_lib::desensitizer::mask::apply_mask;
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;

// ============================================================
// BE01: SSN 嵌入在更长数字串中不应误识别
// ============================================================

#[test]
fn test_en_ssn_in_longer_number_not_detected() {
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect_text("Tracking: 12539-48-26714", 0, 0);

    let ssns: Vec<_> = items
        .iter()
        .filter(|i| i.sensitive_type == SensitiveType::Ssn)
        .collect();
    assert!(
        ssns.is_empty(),
        "长数字串中不应误识别出 SSN，但识别出: {:?}",
        ssns.iter().map(|i| &i.text).collect::<Vec<_>>()
    );
}

// ============================================================
// BE02: UsPhone 前后有数字不应误识别
// ============================================================

#[test]
fn test_en_us_phone_in_longer_number_not_detected() {
    let engine = RegexEngine::for_language(Language::En);
    // 连续长数字，不应被识别为电话
    let items = engine.detect_text("Account 94151234567890123", 0, 0);

    let phones: Vec<_> = items
        .iter()
        .filter(|i| i.sensitive_type == SensitiveType::UsPhone)
        .collect();
    assert!(
        phones.is_empty(),
        "长数字串中不应误识别出 UsPhone，实际: {:?}",
        phones.iter().map(|i| &i.text).collect::<Vec<_>>()
    );
}

// ============================================================
// BE03: ZIP Code 在长数字串中不应误识别
// ============================================================

#[test]
fn test_en_zip_code_in_longer_number_not_detected() {
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect_text("Transaction ID 123456789012", 0, 0);

    let zips: Vec<_> = items
        .iter()
        .filter(|i| i.sensitive_type == SensitiveType::ZipCode)
        .collect();
    assert!(
        zips.is_empty(),
        "长数字串中不应误识别出 ZIP Code，实际: {:?}",
        zips.iter().map(|i| &i.text).collect::<Vec<_>>()
    );
}

// ============================================================
// BE04: 无效 IPv4（每段超过 255）不应识别
// ============================================================

#[test]
fn test_en_invalid_ipv4_not_detected() {
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect_text("Host: 999.999.999.999", 0, 0);

    let ips: Vec<_> = items
        .iter()
        .filter(|i| i.sensitive_type == SensitiveType::IpAddress)
        .collect();
    assert!(ips.is_empty(), "无效 IP 不应被识别为 IP 地址");
}

// ============================================================
// BE05: 信用卡 Luhn 校验失败不应识别
// ============================================================

#[test]
fn test_en_invalid_credit_card_luhn_rejected() {
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect_text("Card: 1234-5678-9012-3456", 0, 0);

    let cards: Vec<_> = items
        .iter()
        .filter(|i| i.sensitive_type == SensitiveType::CreditCard)
        .collect();
    assert!(
        cards.is_empty(),
        "Luhn 校验失败的号码不应被识别为信用卡，实际: {:?}",
        cards.iter().map(|i| &i.text).collect::<Vec<_>>()
    );
}

// ============================================================
// BE06: 纯英文文本无敏感信息 — 零误报
// ============================================================

#[test]
fn test_en_no_sensitive_in_pure_text() {
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect_text(
        "The quick brown fox jumps over the lazy dog. This is a regular English sentence \
         with no sensitive information whatsoever. Shakespeare wrote many plays.",
        0, 0,
    );
    assert!(items.is_empty(), "普通英文文本不应识别出敏感信息，实际: {}", items.len());
}

#[test]
fn test_en_no_sensitive_meeting_notes() {
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect_text(
        "Meeting minutes from March 15: The team discussed Q2 roadmap priorities. \
         Action items include reviewing the proposal and scheduling follow-ups.",
        0, 0,
    );
    assert!(items.is_empty(), "会议纪要普通文本不应误报");
}

// ============================================================
// BE07: 空内容不会崩溃
// ============================================================

#[test]
fn test_en_empty_content() {
    let engine = RegexEngine::for_language(Language::En);

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
        file_name: "empty.txt".to_string(),
        file_type: FileType::Txt,
        paragraphs: vec![],
        encoding: None,
    };
    let items = engine.detect(&content);
    assert!(items.is_empty(), "空文档应返回空结果");
}

// ============================================================
// BE08: 掩码边界 — 英文类型短文本
// ============================================================

#[test]
fn test_en_mask_short_text_ssn() {
    // 超短文本
    let result = apply_mask("AB", &SensitiveType::Ssn, 3, 4);
    assert_eq!(result, "**", "短文本超限应全部掩码");

    // 空字符串
    let result = apply_mask("", &SensitiveType::Ssn, 3, 4);
    assert_eq!(result, "", "空字符串应返回空");
}

#[test]
fn test_en_mask_short_text_email() {
    // 短 email
    let result = apply_mask("a@b.co", &SensitiveType::Email, 3, 4);
    assert_eq!(
        result, "******",
        "前 3 + 后 4 > 6 时应全部掩码: {}", result
    );

    // 正常情况
    let result = apply_mask("user@test.com", &SensitiveType::Email, 2, 4);
    assert_eq!(&result[..2], "us");
    assert_eq!(&result[result.len() - 4..], ".com");
}

// ============================================================
// BE09: 多类型混合 — 同一文本包含多种英文敏感信息
// ============================================================

#[test]
fn test_en_multiple_types_in_one_cell() {
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect_text(
        "Employee: John Doe, SSN 539-48-2671, Phone (415) 293-8847, Email john@test.com, ZIP 94102",
        0, 0,
    );

    let ssns: Vec<_> = items.iter().filter(|i| i.sensitive_type == SensitiveType::Ssn).collect();
    let phones: Vec<_> = items.iter().filter(|i| i.sensitive_type == SensitiveType::UsPhone).collect();
    let emails: Vec<_> = items.iter().filter(|i| i.sensitive_type == SensitiveType::Email).collect();
    let zips: Vec<_> = items.iter().filter(|i| i.sensitive_type == SensitiveType::ZipCode).collect();

    assert_eq!(ssns.len(), 1, "应识别出 1 个 SSN");
    assert_eq!(ssns[0].text, "539-48-2671");
    assert_eq!(phones.len(), 1, "应识别出 1 个 UsPhone");
    assert_eq!(emails.len(), 1, "应识别出 1 个 Email");
    assert_eq!(emails[0].text, "john@test.com");
    assert_eq!(zips.len(), 1, "应识别出 1 个 ZIP Code");
    assert_eq!(zips[0].text, "94102");
}

// ============================================================
// BE10: 引擎独立性 — 英文引擎不匹配中文手机号/身份证
// ============================================================

#[test]
fn test_en_engine_ignores_chinese_patterns() {
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect_text(
        "客户：张三，手机 13812345678，身份证 110101199001011234，车牌 京A12345",
        0, 0,
    );

    // 英文引擎不应匹配中文类型
    assert!(
        !items.iter().any(|i| i.sensitive_type == SensitiveType::Phone),
        "英文引擎不应匹配中文手机号"
    );
    assert!(
        !items.iter().any(|i| i.sensitive_type == SensitiveType::IdCard),
        "英文引擎不应匹配中文身份证"
    );
    assert!(
        !items.iter().any(|i| i.sensitive_type == SensitiveType::LicensePlate),
        "英文引擎不应匹配中文车牌"
    );
}

// ============================================================
// BE11: SSN 格式差异 — 非标准分隔符不应匹配
// ============================================================

#[test]
fn test_en_ssn_non_standard_format_rejected() {
    let engine = RegexEngine::for_language(Language::En);

    // 空格分隔（非标准格式）
    let items = engine.detect_text("SSN: 539 48 2671", 0, 0);
    let ssns: Vec<_> = items.iter().filter(|i| i.sensitive_type == SensitiveType::Ssn).collect();
    assert!(ssns.is_empty(), "空格分隔的 SSN 不应匹配（标准要求 xxx-xx-xxxx）");

    // 点号分隔
    let items2 = engine.detect_text("SSN: 539.48.2671", 0, 0);
    let ssns2: Vec<_> = items2.iter().filter(|i| i.sensitive_type == SensitiveType::Ssn).collect();
    assert!(ssns2.is_empty(), "点号分隔的 SSN 不应匹配");
}

// ============================================================
// BE12: Email 边界 — 无效格式不应匹配
// ============================================================

#[test]
fn test_en_email_invalid_format_rejected() {
    let engine = RegexEngine::for_language(Language::En);

    // 缺少 @
    let items = engine.detect_text("Contact: john.test.com", 0, 0);
    let emails: Vec<_> = items.iter().filter(|i| i.sensitive_type == SensitiveType::Email).collect();
    assert!(emails.is_empty(), "缺少 @ 的文本不应匹配 Email");

    // 缺少 TLD
    let items2 = engine.detect_text("Contact: john@test", 0, 0);
    let emails2: Vec<_> = items2.iter().filter(|i| i.sensitive_type == SensitiveType::Email).collect();
    assert!(emails2.is_empty(), "缺少 TLD 的文本不应匹配 Email");
}
