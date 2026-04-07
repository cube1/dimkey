//! 全管道集成测试 — CSV 场景

mod common;

use common::{assert_full_pipeline_baseline, test_data_path};
use dimkey_lib::models::language::Language;

#[test]
fn test_fp_员工信息表() {
    assert_full_pipeline_baseline(&test_data_path("员工信息表.csv"), Language::Zh);
}

#[test]
fn test_fp_客户通讯录() {
    assert_full_pipeline_baseline(&test_data_path("客户通讯录.csv"), Language::Zh);
}

#[test]
fn test_fp_会议纪要记录() {
    assert_full_pipeline_baseline(&test_data_path("会议纪要记录.csv"), Language::Zh);
}

#[test]
fn test_fp_投诉工单记录() {
    assert_full_pipeline_baseline(&test_data_path("投诉工单记录.csv"), Language::Zh);
}

#[test]
fn test_fp_跨文件一致性_培训签到() {
    assert_full_pipeline_baseline(&test_data_path("跨文件一致性_培训签到.csv"), Language::Zh);
}

#[test]
fn test_fp_english_employee() {
    assert_full_pipeline_baseline(&test_data_path("english_employee.csv"), Language::En);
}

#[test]
fn test_fp_uk_customer_records() {
    assert_full_pipeline_baseline(&test_data_path("uk_customer_records.csv"), Language::En);
}

// --- C45: 英文案件管理台账 ---

#[test]
fn test_fp_legal_case_management() {
    assert_full_pipeline_baseline(
        &test_data_path("legal_case_management.csv"),
        Language::En,
    );
}

// --- C48: 英文法律费用账单 ---

#[test]
fn test_fp_legal_billing_records() {
    assert_full_pipeline_baseline(
        &test_data_path("legal_billing_records.csv"),
        Language::En,
    );
}
