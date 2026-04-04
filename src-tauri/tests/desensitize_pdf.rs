mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::pdf::parse_pdf;

use common::*;

// ============================================================
// C05: 基础脱敏 - pdf (sample.pdf)
// ============================================================

/// C05: PDF 导入后应解析为 Document 类型
#[test]
fn test_pdf_import_structure() {
    let path = fixture_path("sample.pdf");
    let content = parse_pdf(&path).expect("PDF 导入失败");

    if let FileContent::Document { paragraphs, .. } = &content {
        assert!(!paragraphs.is_empty(), "PDF 应解析出至少一个段落");
    } else {
        panic!("期望 Document 类型");
    }
}

/// C05: PDF 中应识别出手机号、身份证、邮箱
#[test]
fn test_pdf_regex_detect_counts() {
    let path = fixture_path("sample.pdf");
    let content = parse_pdf(&path).expect("PDF 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 2,
        "应识别出至少 2 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 2,
        "应识别出至少 2 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 2,
        "应识别出至少 2 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
}

/// C05: PDF 基线覆盖验证
#[test]
fn test_pdf_baseline_coverage() {
    let path = fixture_path("sample.pdf");
    let content = parse_pdf(&path).expect("PDF 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}
