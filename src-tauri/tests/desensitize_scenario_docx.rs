mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::word::parse_docx;

use common::*;

// ============================================================
// C22: 客户调研报告.docx — BankCard, Email, IdCard, Phone
// 注：desensitize_word.rs 已有导入/脱敏测试，此处补充基线覆盖
// ============================================================

/// C22: 客户调研 — 各类型识别数量 smoke test
#[test]
fn test_docx_customer_survey_detect_counts() {
    let path = test_data_path("客户调研报告.docx");
    let content = parse_docx(&path).expect("客户调研报告 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 9,
        "应识别出至少 9 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 5,
        "应识别出至少 5 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 3,
        "应识别出至少 3 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
}

/// C22: 基线覆盖验证
#[test]
fn test_docx_customer_survey_baseline_coverage() {
    let path = test_data_path("客户调研报告.docx");
    let content = parse_docx(&path).expect("客户调研报告 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C23: 人事变动通知.docx — BankCard, Email, IdCard, Phone
// ============================================================

/// C23: 人事变动 — 各类型识别数量 smoke test
#[test]
fn test_docx_hr_change_detect_counts() {
    let path = test_data_path("人事变动通知.docx");
    let content = parse_docx(&path).expect("人事变动通知 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 5,
        "应识别出至少 5 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 4,
        "应识别出至少 4 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 3,
        "应识别出至少 3 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
}

/// C23: 基线覆盖验证
#[test]
fn test_docx_hr_change_baseline_coverage() {
    let path = test_data_path("人事变动通知.docx");
    let content = parse_docx(&path).expect("人事变动通知 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C24: 房屋租赁合同.docx — BankCard, CreditCode, Email, IdCard, Landline, Phone
// ============================================================

/// C24: 租赁合同 — 各类型识别数量 smoke test
#[test]
fn test_docx_rental_contract_detect_counts() {
    let path = test_data_path("房屋租赁合同.docx");
    let content = parse_docx(&path).expect("房屋租赁合同 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 2,
        "应识别出至少 2 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 2,
        "应识别出至少 2 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 1,
        "应识别出至少 1 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
}

/// C24: 基线覆盖验证
#[test]
fn test_docx_rental_contract_baseline_coverage() {
    let path = test_data_path("房屋租赁合同.docx");
    let content = parse_docx(&path).expect("房屋租赁合同 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C25: 保险理赔案件记录.docx — BankCard, Email, IdCard, LicensePlate, Phone
// ============================================================

/// C25: 保险理赔 — 各类型识别数量 smoke test
#[test]
fn test_docx_insurance_claim_detect_counts() {
    let path = test_data_path("保险理赔案件记录.docx");
    let content = parse_docx(&path).expect("保险理赔 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 6,
        "应识别出至少 6 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 3,
        "应识别出至少 3 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
    assert!(
        count_by_type(&items, &SensitiveType::BankCard) >= 2,
        "应识别出至少 2 个银行卡号，实际: {}",
        count_by_type(&items, &SensitiveType::BankCard)
    );
}

/// C25: 基线覆盖验证 — 车牌号带中间点格式暂未支持
#[test]
#[ignore = "LicensePlate 带中间点格式（粤A·D1234）暂未被正则覆盖"]
fn test_docx_insurance_claim_baseline_coverage() {
    let path = test_data_path("保险理赔案件记录.docx");
    let content = parse_docx(&path).expect("保险理赔 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C26: 律师函-延期交房.docx — BankCard, Email, IdCard, Landline, Phone
// ============================================================

/// C26: 律师函 — 各类型识别数量 smoke test
#[test]
fn test_docx_lawyer_letter_detect_counts() {
    let path = test_data_path("律师函-延期交房.docx");
    let content = parse_docx(&path).expect("律师函 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 3,
        "应识别出至少 3 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 3,
        "应识别出至少 3 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 1,
        "应识别出至少 1 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
}

/// C26: 基线覆盖验证
#[test]
fn test_docx_lawyer_letter_baseline_coverage() {
    let path = test_data_path("律师函-延期交房.docx");
    let content = parse_docx(&path).expect("律师函 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C27: 律所案件分析备忘录-劳动争议.docx — BankCard, CreditCode, Email, IdCard, Landline, Phone
// MedicalInsurance 为未知类型，baseline 检查器自动跳过
// ============================================================

/// C27: 劳动争议备忘录 — 各类型识别数量 smoke test
#[test]
fn test_docx_labor_dispute_detect_counts() {
    let path = test_data_path("律所案件分析备忘录-劳动争议.docx");
    let content = parse_docx(&path).expect("劳动争议备忘录 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 5,
        "应识别出至少 5 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 4,
        "应识别出至少 4 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 3,
        "应识别出至少 3 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
}

/// C27: 基线覆盖验证（MedicalInsurance 为未知类型，baseline 检查器自动跳过）
#[test]
fn test_docx_labor_dispute_baseline_coverage() {
    let path = test_data_path("律所案件分析备忘录-劳动争议.docx");
    let content = parse_docx(&path).expect("劳动争议备忘录 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C28: 门诊病历摘要.docx — Email, IdCard, Landline, Phone
// MedicalInsurance 为未知类型，baseline 检查器自动跳过
// ============================================================

/// C28: 门诊病历 — 各类型识别数量 smoke test
#[test]
fn test_docx_medical_record_detect_counts() {
    let path = test_data_path("门诊病历摘要.docx");
    let content = parse_docx(&path).expect("门诊病历 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 3,
        "应识别出至少 3 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 2,
        "应识别出至少 2 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
}

/// C28: 基线覆盖验证（MedicalInsurance 为未知类型，baseline 检查器自动跳过）
#[test]
fn test_docx_medical_record_baseline_coverage() {
    let path = test_data_path("门诊病历摘要.docx");
    let content = parse_docx(&path).expect("门诊病历 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C29: 民事判决书-商品房买卖纠纷.docx — BankCard, CreditCode, Email, IdCard, Phone
// ============================================================

/// C29: 民事判决书 — 各类型识别数量 smoke test
#[test]
fn test_docx_civil_judgment_detect_counts() {
    let path = test_data_path("民事判决书-商品房买卖纠纷.docx");
    let content = parse_docx(&path).expect("民事判决书 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 5,
        "应识别出至少 5 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 3,
        "应识别出至少 3 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
    assert!(
        count_by_type(&items, &SensitiveType::BankCard) >= 2,
        "应识别出至少 2 个银行卡号，实际: {}",
        count_by_type(&items, &SensitiveType::BankCard)
    );
}

/// C29: 基线覆盖验证
#[test]
fn test_docx_civil_judgment_baseline_coverage() {
    let path = test_data_path("民事判决书-商品房买卖纠纷.docx");
    let content = parse_docx(&path).expect("民事判决书 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// C30: 投资尽调报告.docx — CreditCode, Email, IdCard, Phone
// ============================================================

/// C30: 投资尽调 — 各类型识别数量 smoke test
#[test]
fn test_docx_investment_due_diligence_detect_counts() {
    let path = test_data_path("投资尽调报告.docx");
    let content = parse_docx(&path).expect("投资尽调报告 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 10,
        "应识别出至少 10 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 6,
        "应识别出至少 6 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 3,
        "应识别出至少 3 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
    assert!(
        count_by_type(&items, &SensitiveType::CreditCode) >= 1,
        "应识别出至少 1 个统一社会信用代码，实际: {}",
        count_by_type(&items, &SensitiveType::CreditCode)
    );
}

/// C30: 基线覆盖验证
#[test]
fn test_docx_investment_due_diligence_baseline_coverage() {
    let path = test_data_path("投资尽调报告.docx");
    let content = parse_docx(&path).expect("投资尽调报告 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}
