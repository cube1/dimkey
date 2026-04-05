//! 全管道集成测试 — DOCX 场景

mod common;

use common::{assert_full_pipeline_baseline, test_data_path};
use dimkey_lib::models::language::Language;

#[test]
fn test_fp_客户调研报告() {
    assert_full_pipeline_baseline(&test_data_path("客户调研报告.docx"), Language::Zh);
}

#[test]
fn test_fp_人事变动通知() {
    assert_full_pipeline_baseline(&test_data_path("人事变动通知.docx"), Language::Zh);
}

#[test]
fn test_fp_房屋租赁合同() {
    assert_full_pipeline_baseline(&test_data_path("房屋租赁合同.docx"), Language::Zh);
}

#[test]
fn test_fp_保险理赔案件记录() {
    assert_full_pipeline_baseline(&test_data_path("保险理赔案件记录.docx"), Language::Zh);
}

#[test]
fn test_fp_律师函_延期交房() {
    assert_full_pipeline_baseline(&test_data_path("律师函-延期交房.docx"), Language::Zh);
}

#[test]
fn test_fp_律所案件分析备忘录_劳动争议() {
    assert_full_pipeline_baseline(
        &test_data_path("律所案件分析备忘录-劳动争议.docx"),
        Language::Zh,
    );
}

#[test]
fn test_fp_门诊病历摘要() {
    assert_full_pipeline_baseline(&test_data_path("门诊病历摘要.docx"), Language::Zh);
}

#[test]
fn test_fp_民事判决书_商品房买卖纠纷() {
    assert_full_pipeline_baseline(
        &test_data_path("民事判决书-商品房买卖纠纷.docx"),
        Language::Zh,
    );
}

#[test]
fn test_fp_投资尽调报告() {
    assert_full_pipeline_baseline(&test_data_path("投资尽调报告.docx"), Language::Zh);
}

#[test]
fn test_fp_集团高管通讯录() {
    assert_full_pipeline_baseline(&test_data_path("集团高管通讯录.docx"), Language::Zh);
}

#[test]
fn test_fp_international_vendor_contacts() {
    assert_full_pipeline_baseline(
        &test_data_path("international_vendor_contacts.docx"),
        Language::En,
    );
}
