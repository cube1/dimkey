mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::txt::parse_txt;

use common::*;

// ============================================================
// C36 — 会议纪要.txt
// ============================================================

/// 测试 TXT 导入后的结构正确性（Document 类型，包含段落）
#[test]
fn test_txt_meeting_import_structure() {
    let path = test_data_path("会议纪要.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");

    if let FileContent::Document { paragraphs, .. } = &content {
        assert!(
            !paragraphs.is_empty(),
            "应解析出至少一个段落"
        );
    } else {
        panic!("期望 Document 类型");
    }
}

/// 测试会议纪要中手机号识别数量
#[test]
fn test_txt_meeting_detect_phones() {
    let path = test_data_path("会议纪要.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 7,
        "应识别出至少 7 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
}

/// 测试会议纪要中身份证号识别数量
#[test]
fn test_txt_meeting_detect_idcard() {
    let path = test_data_path("会议纪要.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 3,
        "应识别出至少 3 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
}

/// 测试会议纪要中邮箱识别数量
#[test]
fn test_txt_meeting_detect_email() {
    let path = test_data_path("会议纪要.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 4,
        "应识别出至少 4 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
}

/// 测试会议纪要中统一社会信用代码识别
#[test]
fn test_txt_meeting_detect_creditcode() {
    let path = test_data_path("会议纪要.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::CreditCode) >= 1,
        "应识别出至少 1 个统一社会信用代码，实际: {}",
        count_by_type(&items, &SensitiveType::CreditCode)
    );
}

/// 测试会议纪要中 IP 地址、座机号、银行卡号的识别
#[test]
fn test_txt_meeting_detect_various() {
    let path = test_data_path("会议纪要.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::IpAddress) >= 1,
        "应识别出至少 1 个 IP 地址，实际: {}",
        count_by_type(&items, &SensitiveType::IpAddress)
    );
    assert!(
        count_by_type(&items, &SensitiveType::LandlinePhone) >= 1,
        "应识别出至少 1 个座机号，实际: {}",
        count_by_type(&items, &SensitiveType::LandlinePhone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::BankCard) >= 1,
        "应识别出至少 1 个银行卡号，实际: {}",
        count_by_type(&items, &SensitiveType::BankCard)
    );
}

// ============================================================
// C37 — 通知公告.txt
// ============================================================

/// 测试通知公告中手机号识别数量
#[test]
fn test_txt_notice_detect_phones() {
    let path = test_data_path("通知公告.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 7,
        "应识别出至少 7 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
}

/// 测试通知公告中车牌号识别数量
#[test]
fn test_txt_notice_detect_license_plate() {
    let path = test_data_path("通知公告.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::LicensePlate) >= 2,
        "应识别出至少 2 个车牌号，实际: {}",
        count_by_type(&items, &SensitiveType::LicensePlate)
    );
}

/// 测试通知公告中座机号识别数量
#[test]
fn test_txt_notice_detect_landline() {
    let path = test_data_path("通知公告.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::LandlinePhone) >= 2,
        "应识别出至少 2 个座机号，实际: {}",
        count_by_type(&items, &SensitiveType::LandlinePhone)
    );
}

/// 测试通知公告中银行卡号识别数量
#[test]
fn test_txt_notice_detect_bankcard() {
    let path = test_data_path("通知公告.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::BankCard) >= 1,
        "应识别出至少 1 个银行卡号，实际: {}",
        count_by_type(&items, &SensitiveType::BankCard)
    );
}

/// 测试通知公告中统一社会信用代码识别数量
#[test]
fn test_txt_notice_detect_creditcode() {
    let path = test_data_path("通知公告.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::CreditCode) >= 1,
        "应识别出至少 1 个统一社会信用代码，实际: {}",
        count_by_type(&items, &SensitiveType::CreditCode)
    );
}

// ============================================================
// C42 — IT运维事件报告.txt
// 补充: IpAddress 多样化（IPv4 内网/公网 + IPv6），Title(NER)
// ============================================================

/// C42: IT运维报告中 IP 地址识别 — 至少 14 个（IPv4 内网/公网/DNS）
#[test]
fn test_txt_it_ops_detect_ip() {
    let path = fixture_path("scenarios/txt/IT运维事件报告.txt");
    let content = parse_txt(&path).expect("IT运维 TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::IpAddress);
    assert!(
        count >= 14,
        "应识别出至少 14 个 IP 地址（IPv4），实际: {}",
        count
    );
}

/// C42: IT运维报告中手机号识别 — 至少 6 个
#[test]
fn test_txt_it_ops_detect_phone() {
    let path = fixture_path("scenarios/txt/IT运维事件报告.txt");
    let content = parse_txt(&path).expect("IT运维 TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::Phone);
    assert!(
        count >= 6,
        "应识别出至少 6 个手机号，实际: {}",
        count
    );
}

/// C42: IT运维报告中邮箱识别 — 至少 3 个
#[test]
fn test_txt_it_ops_detect_email() {
    let path = fixture_path("scenarios/txt/IT运维事件报告.txt");
    let content = parse_txt(&path).expect("IT运维 TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    let count = count_by_type(&items, &SensitiveType::Email);
    assert!(
        count >= 3,
        "应识别出至少 3 个邮箱，实际: {}",
        count
    );
}

/// C42: 基线覆盖验证 — IPv6 格式暂未支持
#[test]
fn test_txt_it_ops_baseline_coverage() {
    let path = fixture_path("scenarios/txt/IT运维事件报告.txt");
    let content = parse_txt(&path).expect("IT运维 TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert_baseline_from_sidecar(&items, &path);
}
