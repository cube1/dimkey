mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::word::parse_docx;

use common::*;

// ============================================================
// C43: 集团高管通讯录.docx — 段落+表格混合 DOCX
// 补充: Title(NER) 中文职位
// ============================================================

/// C43-1: 检测 Phone — 至少 16 个（段落 + 表格内手机号）
#[test]
fn test_executive_docx_detect_phone() {
    let path = fixture_path("scenarios/docx/集团高管通讯录.docx");
    let content = parse_docx(&path).expect("集团高管 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::Phone);
    assert!(
        count >= 16,
        "应识别出至少 16 个手机号，实际: {}",
        count
    );
}

/// C43-2: 检测 Email — 至少 16 个
#[test]
fn test_executive_docx_detect_email() {
    let path = fixture_path("scenarios/docx/集团高管通讯录.docx");
    let content = parse_docx(&path).expect("集团高管 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::Email);
    assert!(
        count >= 16,
        "应识别出至少 16 个邮箱，实际: {}",
        count
    );
}

/// C43-3: 检测 IdCard — 至少 9 个
#[test]
fn test_executive_docx_detect_idcard() {
    let path = fixture_path("scenarios/docx/集团高管通讯录.docx");
    let content = parse_docx(&path).expect("集团高管 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::IdCard);
    assert!(
        count >= 9,
        "应识别出至少 9 个身份证号，实际: {}",
        count
    );
}

/// C43-4: 检测 Landline — 至少 3 个（010 座机 + 400 热线）
#[test]
fn test_executive_docx_detect_landline() {
    let path = fixture_path("scenarios/docx/集团高管通讯录.docx");
    let content = parse_docx(&path).expect("集团高管 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::LandlinePhone);
    assert!(
        count >= 2,
        "应识别出至少 2 个座机号码（400热线格式暂未支持），实际: {}",
        count
    );
}

/// C43-5: 基线覆盖验证 — 400 热线格式暂未支持
#[test]
fn test_executive_docx_baseline_coverage() {
    let path = fixture_path("scenarios/docx/集团高管通讯录.docx");
    let content = parse_docx(&path).expect("集团高管 DOCX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}
