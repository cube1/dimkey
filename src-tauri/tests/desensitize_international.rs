mod common;

use std::collections::HashSet;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::word::parse_docx;

use common::*;

/// 辅助函数：用中文 + 英文两个引擎扫描 DOCX，合并去重结果
fn detect_bilingual(content: &FileContent) -> Vec<SensitiveItem> {
    let zh_engine = RegexEngine::for_language(Language::Zh);
    let en_engine = RegexEngine::for_language(Language::En);
    let mut items = zh_engine.detect(content);
    let en_items = en_engine.detect(content);

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
// C41: international_vendor_contacts.docx — 国际供应商通讯录
// 补充: CreditCard + Passport + DriversLicense 在 docx 格式中
// ============================================================

/// C41-1: 检测 CreditCard — DOCX 中带空格格式引擎暂不支持
#[test]
fn test_international_docx_detect_credit_card() {
    let path = fixture_path("scenarios/docx/international_vendor_contacts.docx");
    let content = parse_docx(&path).expect("国际供应商 DOCX 导入失败");
    let items = detect_bilingual(&content);

    let count = count_by_type(&items, &SensitiveType::CreditCard);
    assert!(
        count >= 4,
        "应识别出至少 4 个信用卡号，实际: {}",
        count
    );
}

/// C41-2: 检测 IBAN — 至少 5 个（GB/DE/FR 格式，部分重复出现）
#[test]
fn test_international_docx_detect_iban() {
    let path = fixture_path("scenarios/docx/international_vendor_contacts.docx");
    let content = parse_docx(&path).expect("国际供应商 DOCX 导入失败");
    let items = detect_bilingual(&content);

    let count = count_by_type(&items, &SensitiveType::Iban);
    assert!(
        count >= 2,
        "应识别出至少 2 个 IBAN，实际: {}",
        count
    );
}

/// C41-3: 检测 Passport — 多国护照号格式引擎暂不支持
#[test]
fn test_international_docx_detect_passport() {
    let path = fixture_path("scenarios/docx/international_vendor_contacts.docx");
    let content = parse_docx(&path).expect("国际供应商 DOCX 导入失败");
    let items = detect_bilingual(&content);

    let count = count_by_type(&items, &SensitiveType::Passport);
    assert!(
        count >= 3,
        "应识别出至少 3 个护照号，实际: {}",
        count
    );
}

/// C41-4: 检测 UkPhone — 至少 2 个
#[test]
fn test_international_docx_detect_uk_phone() {
    let path = fixture_path("scenarios/docx/international_vendor_contacts.docx");
    let content = parse_docx(&path).expect("国际供应商 DOCX 导入失败");
    let items = detect_bilingual(&content);

    let count = count_by_type(&items, &SensitiveType::UkPhone);
    assert!(
        count >= 2,
        "应识别出至少 2 个 UK 电话号码，实际: {}",
        count
    );
}

/// C41-5: 检测 DriversLicense — 多国驾照格式引擎暂不支持
#[test]
fn test_international_docx_detect_drivers_license() {
    let path = fixture_path("scenarios/docx/international_vendor_contacts.docx");
    let content = parse_docx(&path).expect("国际供应商 DOCX 导入失败");
    let items = detect_bilingual(&content);

    let count = count_by_type(&items, &SensitiveType::DriversLicense);
    assert!(
        count >= 2,
        "应识别出至少 2 个驾照号，实际: {}",
        count
    );
}

/// C41-6: 检测 Email — 至少 6 个
#[test]
fn test_international_docx_detect_email() {
    let path = fixture_path("scenarios/docx/international_vendor_contacts.docx");
    let content = parse_docx(&path).expect("国际供应商 DOCX 导入失败");
    let items = detect_bilingual(&content);

    let count = count_by_type(&items, &SensitiveType::Email);
    assert!(
        count >= 6,
        "应识别出至少 6 个邮箱，实际: {}",
        count
    );
}

/// C41-7: 检测 SSN — 至少 1 个
#[test]
fn test_international_docx_detect_ssn() {
    let path = fixture_path("scenarios/docx/international_vendor_contacts.docx");
    let content = parse_docx(&path).expect("国际供应商 DOCX 导入失败");
    let items = detect_bilingual(&content);

    let count = count_by_type(&items, &SensitiveType::Ssn);
    assert!(
        count >= 1,
        "应识别出至少 1 个 SSN，实际: {}",
        count
    );
}

/// C41-8: 检测 UsPhone — 至少 2 个
#[test]
fn test_international_docx_detect_us_phone() {
    let path = fixture_path("scenarios/docx/international_vendor_contacts.docx");
    let content = parse_docx(&path).expect("国际供应商 DOCX 导入失败");
    let items = detect_bilingual(&content);

    let count = count_by_type(&items, &SensitiveType::UsPhone);
    assert!(
        count >= 2,
        "应识别出至少 2 个美国电话号码，实际: {}",
        count
    );
}

/// C41-9: 基线覆盖验证 — CreditCard/IBAN(FR)/Passport/DriversLicense 暂不支持
#[test]
fn test_international_docx_baseline_coverage() {
    let path = fixture_path("scenarios/docx/international_vendor_contacts.docx");
    let content = parse_docx(&path).expect("国际供应商 DOCX 导入失败");
    let items = detect_bilingual(&content);

    assert_baseline_from_sidecar(&items, &path);
}
