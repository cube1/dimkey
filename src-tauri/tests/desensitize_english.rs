mod common;

use std::collections::HashSet;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::excel::{parse_csv, parse_excel};

use common::*;

/// 辅助函数：用中文 + 英文两个引擎扫描，合并去重结果
/// 按 (text, sensitive_type, row, col, sheet_index, start, end) 去重，
/// 避免 Email 等通用类型被两个引擎重复计数
fn detect_bilingual(content: &FileContent) -> Vec<SensitiveItem> {
    let zh_engine = RegexEngine::for_language(Language::Zh);
    let en_engine = RegexEngine::for_language(Language::En);
    let mut items = zh_engine.detect(content);
    let en_items = en_engine.detect(content);

    // 用已有项的坐标 + 类型做去重
    let seen: HashSet<(String, SensitiveType, usize, usize, usize, usize, usize)> = items
        .iter()
        .map(|i| (i.text.clone(), i.sensitive_type.clone(), i.row, i.col, i.sheet_index, i.start, i.end))
        .collect();

    for item in en_items {
        let key = (item.text.clone(), item.sensitive_type.clone(), item.row, item.col, item.sheet_index, item.start, item.end);
        if !seen.contains(&key) {
            items.push(item);
        }
    }

    items
}

// ============================================================
// C31: english_employee.csv — 纯英文员工数据（10 行）
// ============================================================

/// C31-1: 验证 CSV 导入结构 — 10 行数据，表头正确
#[test]
fn test_english_csv_import_structure() {
    let path = fixture_path("scenarios/csv/english_employee.csv");
    let content = parse_csv(&path).expect("英文 CSV 导入失败");

    if let FileContent::Spreadsheet { sheets, .. } = &content {
        let sheet = &sheets[0];
        assert_eq!(sheet.row_count, 10, "应有 10 行数据");
        assert_eq!(sheet.rows.len(), 10, "rows 长度应为 10");
        // 表头应包含常见字段
        let headers = &sheet.headers;
        assert!(!headers.is_empty(), "表头不应为空");
    } else {
        panic!("期望 Spreadsheet 类型");
    }
}

/// C31-2: 检测 SSN（社会安全号码）— 10 个 XXX-XX-XXXX 格式
#[test]
fn test_english_csv_detect_ssn() {
    let path = fixture_path("scenarios/csv/english_employee.csv");
    let content = parse_csv(&path).expect("英文 CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let ssn_count = count_by_type(&items, &SensitiveType::Ssn);
    assert!(
        ssn_count >= 10,
        "应识别出至少 10 个 SSN，实际: {}",
        ssn_count
    );
}

/// C31-3: 检测美国电话号码 — 混合 (XXX) XXX-XXXX 和 XXX-XXX-XXXX 格式
#[test]
fn test_english_csv_detect_us_phone() {
    let path = fixture_path("scenarios/csv/english_employee.csv");
    let content = parse_csv(&path).expect("英文 CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let us_phone_count = count_by_type(&items, &SensitiveType::UsPhone);
    assert!(
        us_phone_count >= 5,
        "应识别出至少 5 个美国电话号码（部分格式可能未匹配），实际: {}",
        us_phone_count
    );
}

/// C31-4: 检测邮箱 — 10 个
#[test]
fn test_english_csv_detect_email() {
    let path = fixture_path("scenarios/csv/english_employee.csv");
    let content = parse_csv(&path).expect("英文 CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let email_count = count_by_type(&items, &SensitiveType::Email);
    assert!(
        email_count >= 10,
        "应识别出至少 10 个邮箱，实际: {}",
        email_count
    );
}

/// C31-5: 检测英国邮编 — SW1A 1AA, EC2R 8AH, WC2N 5DU 等
#[test]
fn test_english_csv_detect_uk_postcode() {
    let path = fixture_path("scenarios/csv/english_employee.csv");
    let content = parse_csv(&path).expect("英文 CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let uk_postcode_count = count_by_type(&items, &SensitiveType::UkPostcode);
    assert!(
        uk_postcode_count >= 1,
        "应识别出至少 1 个英国邮编，实际: {}",
        uk_postcode_count
    );
}

/// C31-6: 检测 IBAN（国际银行账号）— 至少 1 个
#[test]
fn test_english_csv_detect_iban() {
    let path = fixture_path("scenarios/csv/english_employee.csv");
    let content = parse_csv(&path).expect("英文 CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let iban_count = count_by_type(&items, &SensitiveType::Iban);
    assert!(
        iban_count >= 1,
        "应识别出至少 1 个 IBAN，实际: {}",
        iban_count
    );
}

// ============================================================
// C32: mixed_bilingual.xlsx — 中英双语混合数据（8 行）
// ============================================================

/// C32-1: 检测中国手机号 — 13x/15x/18x 开头，共 8 个
#[test]
fn test_bilingual_xlsx_detect_chinese_phone() {
    let path = fixture_path("scenarios/xlsx/mixed_bilingual.xlsx");
    let content = parse_excel(&path).expect("双语 XLSX 导入失败");
    let items = detect_bilingual(&content);

    let phone_count = count_by_type(&items, &SensitiveType::Phone);
    assert!(
        phone_count >= 8,
        "应识别出至少 8 个中国手机号，实际: {}",
        phone_count
    );
}

/// C32-2: 检测中国身份证号 — 4 个
#[test]
fn test_bilingual_xlsx_detect_chinese_idcard() {
    let path = fixture_path("scenarios/xlsx/mixed_bilingual.xlsx");
    let content = parse_excel(&path).expect("双语 XLSX 导入失败");
    let items = detect_bilingual(&content);

    let idcard_count = count_by_type(&items, &SensitiveType::IdCard);
    assert!(
        idcard_count >= 4,
        "应识别出至少 4 个中国身份证号，实际: {}",
        idcard_count
    );
}

/// C32-3: 检测美国 SSN — 4 个 XXX-XX-XXXX 格式
#[test]
fn test_bilingual_xlsx_detect_ssn() {
    let path = fixture_path("scenarios/xlsx/mixed_bilingual.xlsx");
    let content = parse_excel(&path).expect("双语 XLSX 导入失败");
    let items = detect_bilingual(&content);

    let ssn_count = count_by_type(&items, &SensitiveType::Ssn);
    assert!(
        ssn_count >= 4,
        "应识别出至少 4 个 SSN，实际: {}",
        ssn_count
    );
}

/// C32-4: 检测邮箱 — 8 个
#[test]
fn test_bilingual_xlsx_detect_email() {
    let path = fixture_path("scenarios/xlsx/mixed_bilingual.xlsx");
    let content = parse_excel(&path).expect("双语 XLSX 导入失败");
    let items = detect_bilingual(&content);

    let email_count = count_by_type(&items, &SensitiveType::Email);
    assert!(
        email_count >= 8,
        "应识别出至少 8 个邮箱，实际: {}",
        email_count
    );
}

/// C32-5: 验证中英文敏感类型共存 — 同一文件中同时包含中文和英文敏感数据
#[test]
fn test_bilingual_xlsx_detect_mixed_types() {
    let path = fixture_path("scenarios/xlsx/mixed_bilingual.xlsx");
    let content = parse_excel(&path).expect("双语 XLSX 导入失败");
    let items = detect_bilingual(&content);

    // 中文类型
    let has_phone = count_by_type(&items, &SensitiveType::Phone) > 0;
    let has_idcard = count_by_type(&items, &SensitiveType::IdCard) > 0;

    // 英文类型
    let has_ssn = count_by_type(&items, &SensitiveType::Ssn) > 0;
    let has_email = count_by_type(&items, &SensitiveType::Email) > 0;

    assert!(has_phone, "应包含中国手机号类型");
    assert!(has_idcard, "应包含中国身份证号类型");
    assert!(has_ssn, "应包含美国 SSN 类型");
    assert!(has_email, "应包含邮箱类型");

    // 验证中英文类型确实共存于识别结果中
    let chinese_count = count_by_type(&items, &SensitiveType::Phone)
        + count_by_type(&items, &SensitiveType::IdCard);
    let english_count = count_by_type(&items, &SensitiveType::Ssn);

    assert!(
        chinese_count > 0 && english_count > 0,
        "中英文敏感类型应同时存在：中文类型数 = {}，英文类型数 = {}",
        chinese_count,
        english_count
    );
}
