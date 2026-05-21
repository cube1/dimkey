//! Regression: 防"打开 UI 没替换"静默 passthrough（Rust 侧后端验证）
//!
//! 这个文件是用户痛点 #1 的回归网。每个用户实际报来的"打开后没替换"文件
//! 都应该在这里加一行测试，永不复发。
//!
//! ## 加新回归用例的流程
//!
//! 1. 把出错的真实文件（先脱敏成 fake 数据）放到 `e2e/fixtures/regression/`
//! 2. 在本文件追加一个测试，使用 `assert_no_silent_passthrough`
//! 3. 跑 `cargo test --test regression_no_passthrough` 确认会过/会失败
//! 4. 提交 fixture + 测试，CI 后续自动拦截
//!
//! ## 为什么需要这个测试
//!
//! `full_pipeline_*.rs` 用 sidecar baseline JSON 做严格匹配 — 适合验证
//! "识别结果与预期一致"。但当用户报"打开后没替换"时，作者拿到的是一个
//! 没有 baseline 的全新文件。这套 helper 不要求 baseline，只要求
//! **业务行为正确**：识别非空 + 替换非 noop + content 真改了。

mod common;

use common::{assert_no_silent_passthrough, fixture_path};
use dimkey_lib::models::language::Language;

// ============================================================
// 基线回归: 确保现有 sample 文件永不退化为 silent passthrough
// ============================================================

// 阈值 = 当前实际识别数（作为基线锁定，跌破即视为回归）
// 不是"理想识别数"。e2e Playwright 那边的 25 是 highlight 数 ≠ Rust 全管线 item 数，
// 因为合并去重前的展示和合并后的 item 不一一对应。

#[test]
fn regression_sample_xlsx_no_passthrough() {
    assert_no_silent_passthrough(&fixture_path("sample.xlsx"), Language::Zh, 20);
}

#[test]
fn regression_sample_csv_no_passthrough() {
    assert_no_silent_passthrough(&fixture_path("sample.csv"), Language::Zh, 20);
}

#[test]
fn regression_sample_docx_no_passthrough() {
    assert_no_silent_passthrough(&fixture_path("sample.docx"), Language::Zh, 8);
}

#[test]
fn regression_sample_txt_no_passthrough() {
    assert_no_silent_passthrough(&fixture_path("sample.txt"), Language::Zh, 8);
}

// ============================================================
// 用户报告的真实回归用例 — 在此追加
// ============================================================
//
// 模板:
//   #[test]
//   fn regression_issue_<N>_<short_desc>() {
//       assert_no_silent_passthrough(
//           &fixture_path("regression/issue_<N>_<short_desc>.<ext>"),
//           Language::Zh,
//           <expected_min_replacements>,
//       );
//   }
//
// 第一个真实 case 加进来时，把这段示例改成实际测试。
