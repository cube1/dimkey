mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::excel::parse_csv;

use common::*;

// ============================================================
// C39: uk_customer_records.csv — UK客户记录
// 补充缺失类型: CreditCard, UkPhone
// ============================================================

/// C39-1: 检测 UkPhone — 至少 8 个（+44 和 07 格式混合）
#[test]
fn test_uk_csv_detect_uk_phone() {
    let path = fixture_path("scenarios/csv/uk_customer_records.csv");
    let content = parse_csv(&path).expect("UK CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::UkPhone);
    assert!(
        count >= 8,
        "应识别出至少 8 个 UK 电话号码，实际: {}",
        count
    );
}

/// C39-2: 检测 CreditCard — 当前引擎仅识别部分格式（无空格的卡号）
#[test]
fn test_uk_csv_detect_credit_card() {
    let path = fixture_path("scenarios/csv/uk_customer_records.csv");
    let content = parse_csv(&path).expect("UK CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::CreditCard);
    assert!(
        count >= 2,
        "应识别出至少 2 个信用卡号，实际: {}",
        count
    );
}

/// C39-3: 检测 UkPostcode — 至少 8 个
#[test]
fn test_uk_csv_detect_uk_postcode() {
    let path = fixture_path("scenarios/csv/uk_customer_records.csv");
    let content = parse_csv(&path).expect("UK CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::UkPostcode);
    assert!(
        count >= 8,
        "应识别出至少 8 个英国邮编，实际: {}",
        count
    );
}

/// C39-4: 检测 Email — 至少 8 个
#[test]
fn test_uk_csv_detect_email() {
    let path = fixture_path("scenarios/csv/uk_customer_records.csv");
    let content = parse_csv(&path).expect("UK CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::Email);
    assert!(
        count >= 8,
        "应识别出至少 8 个邮箱，实际: {}",
        count
    );
}

/// C39-5: 检测 IBAN — 至少 1 个（Notes 字段内嵌）
#[test]
fn test_uk_csv_detect_iban() {
    let path = fixture_path("scenarios/csv/uk_customer_records.csv");
    let content = parse_csv(&path).expect("UK CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::Iban);
    assert!(
        count >= 1,
        "应识别出至少 1 个 IBAN，实际: {}",
        count
    );
}

/// C39-6: 检测 DriversLicense — UK驾照格式（WILSO703159GW9IJ）引擎暂未支持
#[test]
fn test_uk_csv_detect_drivers_license() {
    let path = fixture_path("scenarios/csv/uk_customer_records.csv");
    let content = parse_csv(&path).expect("UK CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::DriversLicense);
    assert!(
        count >= 1,
        "应识别出至少 1 个驾照号，实际: {}",
        count
    );
}

/// C39-7: 基线覆盖验证 — CreditCard 带空格格式 + UK DriversLicense 暂不支持
#[test]
fn test_uk_csv_baseline_coverage() {
    let path = fixture_path("scenarios/csv/uk_customer_records.csv");
    let content = parse_csv(&path).expect("UK CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}
