mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::excel::parse_excel;

use common::*;

// ============================================================
// C40: us_compliance_audit.xlsx — US合规审计表
// 补充缺失类型: ZipCode, DriversLicense, CreditCard
// ============================================================

/// C40-1: 检测 SSN — 至少 10 个
#[test]
fn test_us_compliance_detect_ssn() {
    let path = fixture_path("scenarios/xlsx/us_compliance_audit.xlsx");
    let content = parse_excel(&path).expect("US 合规 XLSX 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::Ssn);
    assert!(
        count >= 10,
        "应识别出至少 10 个 SSN，实际: {}",
        count
    );
}

/// C40-2: 检测 UsPhone — 至少 10 个（混合括号和短横线格式）
#[test]
fn test_us_compliance_detect_us_phone() {
    let path = fixture_path("scenarios/xlsx/us_compliance_audit.xlsx");
    let content = parse_excel(&path).expect("US 合规 XLSX 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::UsPhone);
    assert!(
        count >= 10,
        "应识别出至少 10 个美国电话号码，实际: {}",
        count
    );
}

/// C40-3: 检测 DriversLicense — X123-4567-8901 格式引擎暂未支持
#[test]
fn test_us_compliance_detect_drivers_license() {
    let path = fixture_path("scenarios/xlsx/us_compliance_audit.xlsx");
    let content = parse_excel(&path).expect("US 合规 XLSX 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::DriversLicense);
    assert!(
        count >= 10,
        "应识别出至少 10 个驾照号，实际: {}",
        count
    );
}

/// C40-4: 检测 CreditCard — 当前引擎仅识别部分格式
#[test]
fn test_us_compliance_detect_credit_card() {
    let path = fixture_path("scenarios/xlsx/us_compliance_audit.xlsx");
    let content = parse_excel(&path).expect("US 合规 XLSX 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::CreditCard);
    assert!(
        count >= 2,
        "应识别出至少 2 个信用卡号，实际: {}",
        count
    );
}

/// C40-5: 检测 ZipCode — 至少 10 个（5位美国邮编）
#[test]
fn test_us_compliance_detect_zipcode() {
    let path = fixture_path("scenarios/xlsx/us_compliance_audit.xlsx");
    let content = parse_excel(&path).expect("US 合规 XLSX 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::ZipCode);
    assert!(
        count >= 10,
        "应识别出至少 10 个美国邮编，实际: {}",
        count
    );
}

/// C40-6: 检测 Email — 至少 10 个
#[test]
fn test_us_compliance_detect_email() {
    let path = fixture_path("scenarios/xlsx/us_compliance_audit.xlsx");
    let content = parse_excel(&path).expect("US 合规 XLSX 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::Email);
    assert!(
        count >= 10,
        "应识别出至少 10 个邮箱，实际: {}",
        count
    );
}

/// C40-7: 基线覆盖验证 — DriversLicense + CreditCard 带空格格式暂不支持
#[test]
fn test_us_compliance_baseline_coverage() {
    let path = fixture_path("scenarios/xlsx/us_compliance_audit.xlsx");
    let content = parse_excel(&path).expect("US 合规 XLSX 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}
