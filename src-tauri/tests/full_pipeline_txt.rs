//! 全管道集成测试 — TXT 场景

mod common;

use common::{assert_full_pipeline_baseline, test_data_path};
use dimkey_lib::models::language::Language;

#[test]
fn test_fp_会议纪要() {
    assert_full_pipeline_baseline(&test_data_path("会议纪要.txt"), Language::Zh);
}

#[test]
fn test_fp_通知公告() {
    assert_full_pipeline_baseline(&test_data_path("通知公告.txt"), Language::Zh);
}

#[test]
fn test_fp_IT运维事件报告() {
    assert_full_pipeline_baseline(&test_data_path("IT运维事件报告.txt"), Language::Zh);
}
