mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::excel::{parse_csv, parse_excel};
use dimkey_lib::parser::word::parse_docx;
use dimkey_lib::parser::txt::parse_txt;

use common::*;

// ============================================================
// C44: law_firm_client_intake.xlsx — 英文律所客户登记表
// Ssn, UsPhone, Email, ZipCode + NER(PersonName, Title, Address)
// ============================================================

/// C44: 律所客户登记表 — 各类型识别数量 smoke test
#[test]
fn test_xlsx_law_firm_client_intake_detect_counts() {
    let path = fixture_path("scenarios/xlsx/law_firm_client_intake.xlsx");
    let content = parse_excel(&path).expect("law_firm_client_intake XLSX 导入失败");
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
    assert!(
        count_by_type(&items, &SensitiveType::ZipCode) >= 10,
        "应识别出至少 10 个邮编，实际: {}",
        count_by_type(&items, &SensitiveType::ZipCode)
    );
}

/// C44: 基线覆盖验证
#[test]
fn test_xlsx_law_firm_client_intake_baseline_coverage() {
    let path = fixture_path("scenarios/xlsx/law_firm_client_intake.xlsx");
    let content = parse_excel(&path).expect("law_firm_client_intake XLSX 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C45: legal_case_management.csv — 英文案件管理台账
// Ssn, UsPhone, UkPhone, Email, DriversLicense, Passport
// + NER(PersonName, OrgName)
// ============================================================

/// C45: 案件管理台账 — 各类型识别数量 smoke test
#[test]
fn test_csv_legal_case_management_detect_counts() {
    let path = fixture_path("scenarios/csv/legal_case_management.csv");
    let content = parse_csv(&path).expect("legal_case_management CSV 导入失败");
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
        count_by_type(&items, &SensitiveType::UkPhone) >= 5,
        "应识别出至少 5 个英国电话，实际: {}",
        count_by_type(&items, &SensitiveType::UkPhone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 15,
        "应识别出至少 15 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
    assert!(
        count_by_type(&items, &SensitiveType::DriversLicense) >= 5,
        "应识别出至少 5 个驾照号，实际: {}",
        count_by_type(&items, &SensitiveType::DriversLicense)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Passport) >= 3,
        "应识别出至少 3 个护照号，实际: {}",
        count_by_type(&items, &SensitiveType::Passport)
    );
}

/// C45: 基线覆盖验证
#[test]
fn test_csv_legal_case_management_baseline_coverage() {
    let path = fixture_path("scenarios/csv/legal_case_management.csv");
    let content = parse_csv(&path).expect("legal_case_management CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C46: attorney_engagement_letter.docx — 英文委托代理协议书
// Ssn, UsPhone, Email, CreditCard, Iban, DriversLicense, ZipCode
// + NER(PersonName, Title, OrgName, Address)
// ============================================================

/// C46: 委托代理协议书 — 各类型识别数量 smoke test
#[test]
fn test_docx_attorney_engagement_letter_detect_counts() {
    let path = fixture_path("scenarios/docx/attorney_engagement_letter.docx");
    let content = parse_docx(&path).expect("attorney_engagement_letter DOCX 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Ssn) >= 1,
        "应识别出至少 1 个 SSN，实际: {}",
        count_by_type(&items, &SensitiveType::Ssn)
    );
    assert!(
        count_by_type(&items, &SensitiveType::UsPhone) >= 3,
        "应识别出至少 3 个美国电话，实际: {}",
        count_by_type(&items, &SensitiveType::UsPhone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 3,
        "应识别出至少 3 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
    assert!(
        count_by_type(&items, &SensitiveType::CreditCard) >= 1,
        "应识别出至少 1 个信用卡号，实际: {}",
        count_by_type(&items, &SensitiveType::CreditCard)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Iban) >= 1,
        "应识别出至少 1 个 IBAN，实际: {}",
        count_by_type(&items, &SensitiveType::Iban)
    );
    assert!(
        count_by_type(&items, &SensitiveType::DriversLicense) >= 1,
        "应识别出至少 1 个驾照号，实际: {}",
        count_by_type(&items, &SensitiveType::DriversLicense)
    );
    assert!(
        count_by_type(&items, &SensitiveType::ZipCode) >= 2,
        "应识别出至少 2 个邮编，实际: {}",
        count_by_type(&items, &SensitiveType::ZipCode)
    );
}

/// C46: 基线覆盖验证
#[test]
fn test_docx_attorney_engagement_letter_baseline_coverage() {
    let path = fixture_path("scenarios/docx/attorney_engagement_letter.docx");
    let content = parse_docx(&path).expect("attorney_engagement_letter DOCX 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C47: litigation_discovery_memo.docx — 英文诉讼发现备忘录
// Ssn, UsPhone, UkPhone, Email, Iban, Passport, DriversLicense,
// ZipCode, UkPostcode + NER(PersonName, Address)
// ============================================================

/// C47: 诉讼发现备忘录 — 各类型识别数量 smoke test
#[test]
fn test_docx_litigation_discovery_memo_detect_counts() {
    let path = fixture_path("scenarios/docx/litigation_discovery_memo.docx");
    let content = parse_docx(&path).expect("litigation_discovery_memo DOCX 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Ssn) >= 3,
        "应识别出至少 3 个 SSN，实际: {}",
        count_by_type(&items, &SensitiveType::Ssn)
    );
    assert!(
        count_by_type(&items, &SensitiveType::UsPhone) >= 5,
        "应识别出至少 5 个美国电话，实际: {}",
        count_by_type(&items, &SensitiveType::UsPhone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::UkPhone) >= 2,
        "应识别出至少 2 个英国电话，实际: {}",
        count_by_type(&items, &SensitiveType::UkPhone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 8,
        "应识别出至少 8 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Iban) >= 1,
        "应识别出至少 1 个 IBAN，实际: {}",
        count_by_type(&items, &SensitiveType::Iban)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Passport) >= 2,
        "应识别出至少 2 个护照号，实际: {}",
        count_by_type(&items, &SensitiveType::Passport)
    );
    assert!(
        count_by_type(&items, &SensitiveType::DriversLicense) >= 1,
        "应识别出至少 1 个驾照号，实际: {}",
        count_by_type(&items, &SensitiveType::DriversLicense)
    );
    assert!(
        count_by_type(&items, &SensitiveType::ZipCode) >= 3,
        "应识别出至少 3 个邮编，实际: {}",
        count_by_type(&items, &SensitiveType::ZipCode)
    );
    assert!(
        count_by_type(&items, &SensitiveType::UkPostcode) >= 1,
        "应识别出至少 1 个英国邮编，实际: {}",
        count_by_type(&items, &SensitiveType::UkPostcode)
    );
}

/// C47: 基线覆盖验证
#[test]
fn test_docx_litigation_discovery_memo_baseline_coverage() {
    let path = fixture_path("scenarios/docx/litigation_discovery_memo.docx");
    let content = parse_docx(&path).expect("litigation_discovery_memo DOCX 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C48: legal_billing_records.csv — 英文法律费用账单
// CreditCard, Iban, UsPhone, UkPhone, Email, UkPostcode, ZipCode
// + NER(PersonName, OrgName, Title)
// ============================================================

/// C48: 法律费用账单 — 各类型识别数量 smoke test
#[test]
fn test_csv_legal_billing_records_detect_counts() {
    let path = fixture_path("scenarios/csv/legal_billing_records.csv");
    let content = parse_csv(&path).expect("legal_billing_records CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::CreditCard) >= 5,
        "应识别出至少 5 个信用卡号，实际: {}",
        count_by_type(&items, &SensitiveType::CreditCard)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Iban) >= 5,
        "应识别出至少 5 个 IBAN，实际: {}",
        count_by_type(&items, &SensitiveType::Iban)
    );
    assert!(
        count_by_type(&items, &SensitiveType::UsPhone) >= 5,
        "应识别出至少 5 个美国电话，实际: {}",
        count_by_type(&items, &SensitiveType::UsPhone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::UkPhone) >= 2,
        "应识别出至少 2 个英国电话，实际: {}",
        count_by_type(&items, &SensitiveType::UkPhone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 10,
        "应识别出至少 10 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
    assert!(
        count_by_type(&items, &SensitiveType::UkPostcode) >= 2,
        "应识别出至少 2 个英国邮编，实际: {}",
        count_by_type(&items, &SensitiveType::UkPostcode)
    );
    assert!(
        count_by_type(&items, &SensitiveType::ZipCode) >= 5,
        "应识别出至少 5 个美国邮编，实际: {}",
        count_by_type(&items, &SensitiveType::ZipCode)
    );
}

/// C48: 基线覆盖验证
#[test]
fn test_csv_legal_billing_records_baseline_coverage() {
    let path = fixture_path("scenarios/csv/legal_billing_records.csv");
    let content = parse_csv(&path).expect("legal_billing_records CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C49: english_legal_edge_cases.txt — 英文法律边界测试
// 多格式电话、信用卡、IBAN 多国、护照多国、ZIP+4、法律术语误报控制
// ============================================================

/// C49: 英文法律边界 — 各类型识别数量 smoke test
#[test]
fn test_txt_english_legal_edge_cases_detect_counts() {
    let path = fixture_path("boundary/english_legal_edge_cases.txt");
    let content = parse_txt(&path).expect("english_legal_edge_cases TXT 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    // SSN: 3 个不同值（其中 1 个出现 2 次）
    assert!(
        count_by_type(&items, &SensitiveType::Ssn) >= 3,
        "应识别出至少 3 个 SSN，实际: {}",
        count_by_type(&items, &SensitiveType::Ssn)
    );
    // 美国电话: 多种格式
    assert!(
        count_by_type(&items, &SensitiveType::UsPhone) >= 5,
        "应识别出至少 5 个美国电话，实际: {}",
        count_by_type(&items, &SensitiveType::UsPhone)
    );
    // 英国电话
    assert!(
        count_by_type(&items, &SensitiveType::UkPhone) >= 2,
        "应识别出至少 2 个英国电话，实际: {}",
        count_by_type(&items, &SensitiveType::UkPhone)
    );
    // 信用卡: 3 种格式（含 1 个无分隔符 edge case，可能未命中）
    assert!(
        count_by_type(&items, &SensitiveType::CreditCard) >= 2,
        "应识别出至少 2 个信用卡号，实际: {}",
        count_by_type(&items, &SensitiveType::CreditCard)
    );
    // IBAN: 多国格式
    assert!(
        count_by_type(&items, &SensitiveType::Iban) >= 4,
        "应识别出至少 4 个 IBAN，实际: {}",
        count_by_type(&items, &SensitiveType::Iban)
    );
    // 护照
    assert!(
        count_by_type(&items, &SensitiveType::Passport) >= 2,
        "应识别出至少 2 个护照号，实际: {}",
        count_by_type(&items, &SensitiveType::Passport)
    );
    // 驾照
    assert!(
        count_by_type(&items, &SensitiveType::DriversLicense) >= 2,
        "应识别出至少 2 个驾照号，实际: {}",
        count_by_type(&items, &SensitiveType::DriversLicense)
    );
    // 邮箱
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 1,
        "应识别出至少 1 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
    // ZIP Code
    assert!(
        count_by_type(&items, &SensitiveType::ZipCode) >= 3,
        "应识别出至少 3 个邮编，实际: {}",
        count_by_type(&items, &SensitiveType::ZipCode)
    );
}

/// C49: 基线覆盖验证
#[test]
fn test_txt_english_legal_edge_cases_baseline_coverage() {
    let path = fixture_path("boundary/english_legal_edge_cases.txt");
    let content = parse_txt(&path).expect("english_legal_edge_cases TXT 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}
