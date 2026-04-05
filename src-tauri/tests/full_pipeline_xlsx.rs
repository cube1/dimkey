//! 全管道集成测试 — XLSX 场景
//! 三层引擎（正则+NER+词典）合并检测，与 baseline sidecar 对照

mod common;

use common::{assert_full_pipeline_baseline, test_data_path};
use dimkey_lib::models::language::Language;

#[test]
fn test_fp_员工花名册() {
    assert_full_pipeline_baseline(&test_data_path("员工花名册.xlsx"), Language::Zh);
}

#[test]
fn test_fp_边界测试用例() {
    assert_full_pipeline_baseline(&test_data_path("边界测试用例.xlsx"), Language::Zh);
}

#[test]
fn test_fp_混合敏感信息场景() {
    assert_full_pipeline_baseline(&test_data_path("混合敏感信息场景.xlsx"), Language::Zh);
}

#[test]
fn test_fp_律所案件登记表() {
    assert_full_pipeline_baseline(&test_data_path("律所案件登记表.xlsx"), Language::Zh);
}

#[test]
fn test_fp_物业业主信息表() {
    assert_full_pipeline_baseline(&test_data_path("物业业主信息表.xlsx"), Language::Zh);
}

#[test]
fn test_fp_学校学生信息登记表() {
    assert_full_pipeline_baseline(&test_data_path("学校学生信息登记表.xlsx"), Language::Zh);
}

#[test]
fn test_fp_医院患者登记表() {
    assert_full_pipeline_baseline(&test_data_path("医院患者登记表.xlsx"), Language::Zh);
}

#[test]
fn test_fp_银行贷款申请表() {
    assert_full_pipeline_baseline(&test_data_path("银行贷款申请表.xlsx"), Language::Zh);
}

#[test]
fn test_fp_mixed_bilingual() {
    assert_full_pipeline_baseline(&test_data_path("mixed_bilingual.xlsx"), Language::Zh);
}

#[test]
fn test_fp_us_compliance_audit() {
    assert_full_pipeline_baseline(&test_data_path("us_compliance_audit.xlsx"), Language::En);
}

#[test]
fn test_fp_跨文件一致性_入职信息() {
    assert_full_pipeline_baseline(&test_data_path("跨文件一致性_入职信息.xlsx"), Language::Zh);
}
