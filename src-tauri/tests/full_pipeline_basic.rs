//! 全管道集成测试 — 基础 fixture + 边界场景
//!
//! 不包含以下已知问题场景：
//! - sample.pdf: PDFium 动态库未部署（ENV-001）
//! - boundary/gbk_sample.csv: CSV parser 不支持 GBK 编码（BUG-014）
//! - sample_encrypted.xlsx: 需要密码
//! - empty.xlsx: 空文件无基线
//! - large.csv: 大文件，单独测试

mod common;

use common::{assert_full_pipeline_baseline, fixture_path};
use dimkey_lib::models::language::Language;

// --- 基础 sample 文件 ---

#[test]
fn test_fp_sample_xlsx() {
    assert_full_pipeline_baseline(&fixture_path("sample.xlsx"), Language::Zh);
}

#[test]
fn test_fp_sample_csv() {
    assert_full_pipeline_baseline(&fixture_path("sample.csv"), Language::Zh);
}

#[test]
fn test_fp_sample_docx() {
    assert_full_pipeline_baseline(&fixture_path("sample.docx"), Language::Zh);
}

#[test]
fn test_fp_sample_txt() {
    assert_full_pipeline_baseline(&fixture_path("sample.txt"), Language::Zh);
}

// --- 边界场景 ---

#[test]
fn test_fp_boundary_utf8bom() {
    assert_full_pipeline_baseline(&fixture_path("boundary/utf8bom_sample.csv"), Language::Zh);
}

#[test]
fn test_fp_boundary_fullwidth() {
    assert_full_pipeline_baseline(&fixture_path("boundary/fullwidth_digits.csv"), Language::Zh);
}

#[test]
fn test_fp_boundary_large_cell() {
    assert_full_pipeline_baseline(&fixture_path("boundary/large_cell.xlsx"), Language::Zh);
}

// --- batch 文件 ---

#[test]
fn test_fp_batch_01_xlsx() {
    assert_full_pipeline_baseline(&fixture_path("batch/batch_01.xlsx"), Language::Zh);
}

#[test]
fn test_fp_batch_02_csv() {
    assert_full_pipeline_baseline(&fixture_path("batch/batch_02.csv"), Language::Zh);
}

#[test]
fn test_fp_batch_03_docx() {
    assert_full_pipeline_baseline(&fixture_path("batch/batch_03.docx"), Language::Zh);
}

// --- C49: 英文法律边界测试 ---

#[test]
fn test_fp_english_legal_edge_cases() {
    assert_full_pipeline_baseline(
        &fixture_path("boundary/english_legal_edge_cases.txt"),
        Language::En,
    );
}
