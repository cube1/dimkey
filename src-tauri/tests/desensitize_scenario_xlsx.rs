mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::excel::parse_excel;

use common::*;

// ============================================================
// C11: 边界测试用例.xlsx — BankCard, CreditCode, Email, IdCard, Landline, Phone
// ============================================================

/// C11: 边界测试场景 — 各类型识别数量 smoke test
#[test]
fn test_xlsx_boundary_detect_counts() {
    let path = test_data_path("边界测试用例.xlsx");
    let content = parse_excel(&path).expect("边界测试 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 5,
        "应识别出至少 5 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 4,
        "应识别出至少 4 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 6,
        "应识别出至少 6 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
    assert!(
        count_by_type(&items, &SensitiveType::BankCard) >= 4,
        "应识别出至少 4 个银行卡号，实际: {}",
        count_by_type(&items, &SensitiveType::BankCard)
    );
}

/// C11: 基线覆盖验证
#[test]
fn test_xlsx_boundary_baseline_coverage() {
    let path = test_data_path("边界测试用例.xlsx");
    let content = parse_excel(&path).expect("边界测试 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C12: 混合敏感信息场景.xlsx（多sheet）— BankCard, Email, IdCard, Phone
// ============================================================

/// C12: 混合场景 — 各类型识别数量 smoke test
#[test]
fn test_xlsx_mixed_detect_counts() {
    let path = test_data_path("混合敏感信息场景.xlsx");
    let content = parse_excel(&path).expect("混合场景 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 7,
        "应识别出至少 7 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 3,
        "应识别出至少 3 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 4,
        "应识别出至少 4 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
    assert!(
        count_by_type(&items, &SensitiveType::BankCard) >= 4,
        "应识别出至少 4 个银行卡号，实际: {}",
        count_by_type(&items, &SensitiveType::BankCard)
    );
}

/// C12: 基线覆盖验证
#[test]
fn test_xlsx_mixed_baseline_coverage() {
    let path = test_data_path("混合敏感信息场景.xlsx");
    let content = parse_excel(&path).expect("混合场景 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C13: 律所案件登记表.xlsx — CreditCode, Email, IdCard, Landline, Phone
// ============================================================

/// C13: 律所案件 — 各类型识别数量 smoke test
#[test]
fn test_xlsx_law_case_detect_counts() {
    let path = test_data_path("律所案件登记表.xlsx");
    let content = parse_excel(&path).expect("律所案件 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 21,
        "应识别出至少 21 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 6,
        "应识别出至少 6 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 8,
        "应识别出至少 8 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
    assert!(
        count_by_type(&items, &SensitiveType::LandlinePhone) >= 3,
        "应识别出至少 3 个座机号，实际: {}",
        count_by_type(&items, &SensitiveType::LandlinePhone)
    );
}

/// C13: 基线覆盖验证
#[test]
fn test_xlsx_law_case_baseline_coverage() {
    let path = test_data_path("律所案件登记表.xlsx");
    let content = parse_excel(&path).expect("律所案件 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C14: 物业业主信息表.xlsx — Email, IdCard, Landline, LicensePlate, Phone
// ============================================================

/// C14: 物业业主 — 各类型识别数量 smoke test
#[test]
fn test_xlsx_property_owner_detect_counts() {
    let path = test_data_path("物业业主信息表.xlsx");
    let content = parse_excel(&path).expect("物业业主 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 8,
        "应识别出至少 8 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 7,
        "应识别出至少 7 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 7,
        "应识别出至少 7 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
}

/// C14: 基线覆盖验证 — 车牌号 "京A·12345" 带中间点格式暂未支持
#[test]
fn test_xlsx_property_owner_baseline_coverage() {
    let path = test_data_path("物业业主信息表.xlsx");
    let content = parse_excel(&path).expect("物业业主 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C15: 学校学生信息登记表.xlsx — Email, IdCard, Phone
// ============================================================

/// C15: 学生信息 — 各类型识别数量 smoke test
#[test]
fn test_xlsx_student_detect_counts() {
    let path = test_data_path("学校学生信息登记表.xlsx");
    let content = parse_excel(&path).expect("学生信息 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 10,
        "应识别出至少 10 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 5,
        "应识别出至少 5 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 5,
        "应识别出至少 5 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
}

/// C15: 基线覆盖验证
#[test]
fn test_xlsx_student_baseline_coverage() {
    let path = test_data_path("学校学生信息登记表.xlsx");
    let content = parse_excel(&path).expect("学生信息 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C16: 医院患者登记表.xlsx — IdCard, Phone（MedicalInsurance 为未知类型，自动跳过）
// ============================================================

/// C16: 医院患者 — 各类型识别数量 smoke test
#[test]
fn test_xlsx_hospital_detect_counts() {
    let path = test_data_path("医院患者登记表.xlsx");
    let content = parse_excel(&path).expect("医院患者 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 12,
        "应识别出至少 12 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 12,
        "应识别出至少 12 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
}

/// C16: 基线覆盖验证（MedicalInsurance 为未知类型，baseline 检查器自动跳过）
#[test]
fn test_xlsx_hospital_baseline_coverage() {
    let path = test_data_path("医院患者登记表.xlsx");
    let content = parse_excel(&path).expect("医院患者 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C17: 银行贷款申请表.xlsx — BankCard, Email, IdCard, Phone
// ============================================================

/// C17: 银行贷款 — 各类型识别数量 smoke test
#[test]
fn test_xlsx_bank_loan_detect_counts() {
    let path = test_data_path("银行贷款申请表.xlsx");
    let content = parse_excel(&path).expect("银行贷款 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 10,
        "应识别出至少 10 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 10,
        "应识别出至少 10 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
    assert!(
        count_by_type(&items, &SensitiveType::BankCard) >= 5,
        "应识别出至少 5 个银行卡号，实际: {}",
        count_by_type(&items, &SensitiveType::BankCard)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 5,
        "应识别出至少 5 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
}

/// C17: 基线覆盖验证
#[test]
fn test_xlsx_bank_loan_baseline_coverage() {
    let path = test_data_path("银行贷款申请表.xlsx");
    let content = parse_excel(&path).expect("银行贷款 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}
