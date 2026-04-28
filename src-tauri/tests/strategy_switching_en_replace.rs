//! 英文 Replace 风格输出形态测试 (S23-S31)
//!
//! 直接调用 apply_replace 验证 Fake/Mou/Ordinal 三种风格在 4 类 NER 实体
//! (PersonName/OrgName/Address/Title) 上的输出契约。
//! 不依赖 NER 模型识别，输入取自 attorney_engagement_letter.docx baseline 中已确认的实体。

mod common;

use std::collections::HashMap;

use dimkey_lib::desensitizer::replace::{apply_replace, ReplaceState};
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;

#[allow(unused_imports)]
use common::*;

const EN_ORG_SUFFIXES: &[&str] = &[
    "Inc.", "Corp.", "LLC", "LLP", "Ltd.", "Group", "Holdings",
    "Partners", "Associates", "International", "Co.",
];

const EN_TITLES_POOL: &[&str] = &[
    "Software Engineer", "Product Manager", "Data Analyst", "Project Manager",
    "Marketing Director", "Sales Representative", "Financial Analyst", "HR Manager",
    "Operations Manager", "Business Analyst", "UX Designer", "DevOps Engineer",
    "Account Manager", "Quality Assurance Engineer", "Research Scientist",
    "Technical Lead", "Customer Success Manager", "Content Strategist",
    "Supply Chain Manager", "Legal Counsel", "Executive Assistant",
    "Chief Technology Officer", "Vice President", "Senior Consultant",
    "Program Director", "Systems Administrator", "Database Administrator",
    "Network Engineer", "Security Analyst", "Compliance Officer",
];

fn has_han(s: &str) -> bool {
    s.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
}

fn fresh_state() -> ReplaceState {
    ReplaceState::new(42, HashMap::new())
}

// ============================================================
// S23: EN Replace Fake — PersonName 输出含空格、纯 ASCII、不含汉字
// ============================================================

#[test]
fn test_s23_en_replace_fake_personname_format() {
    let mut state = fresh_state();
    let names = ["James Anderson", "Sarah Mitchell", "David Park", "Rebecca Harrison"];

    for name in &names {
        let r = apply_replace(name, &SensitiveType::PersonName, &mut state, &ReplaceStyle::Fake);
        assert!(!has_han(&r), "PersonName Fake 不应含汉字: {} → {}", name, r);
        assert!(r.contains(' '), "PersonName Fake 应含空格分隔: {} → {}", name, r);
        assert!(r.is_ascii(), "PersonName Fake 应为纯 ASCII: {} → {}", name, r);
        assert_ne!(r.as_str(), *name, "替换值不应等于原文");
        assert_ne!(r, "John Doe", "Fake 不应输出 John Doe（占位符属于 Mou）");
        assert_ne!(r, "Jane Doe", "Fake 不应输出 Jane Doe（占位符属于 Mou）");
    }
}

// ============================================================
// S24: EN Replace Fake — OrgName 输出以 EN 字典 suffix 结尾
// ============================================================

#[test]
fn test_s24_en_replace_fake_orgname_suffix() {
    let mut state = fresh_state();
    let orgs = [
        "Mitchell, Chen & Park LLP",
        "Pacific Coast Medical Center",
        "Harrison & Associates LLP",
        "TechVenture Capital Partners",
    ];

    for org in &orgs {
        let r = apply_replace(org, &SensitiveType::OrgName, &mut state, &ReplaceStyle::Fake);
        assert!(!has_han(&r), "OrgName Fake 不应含汉字: {} → {}", org, r);
        assert_ne!(r.as_str(), *org, "替换值不应等于原文");
        // 末尾应在 EN_ORG_SUFFIXES 表内
        let matched = EN_ORG_SUFFIXES.iter().any(|s| r.ends_with(s));
        assert!(
            matched,
            "OrgName Fake 末尾应在 EN_ORG_SUFFIXES 表内: {} → {}",
            org, r
        );
    }
}

// ============================================================
// S25: EN Replace Fake — Address 输出含逗号、不含汉字
// ============================================================

#[test]
fn test_s25_en_replace_fake_address_format() {
    let mut state = fresh_state();
    let addrs = [
        "2500 Broadway Avenue, Suite 3100",
        "1420 Market Street, Apt 5B",
        "580 California Street, Suite 2000, San Francisco, CA",
    ];

    for addr in &addrs {
        let r = apply_replace(addr, &SensitiveType::Address, &mut state, &ReplaceStyle::Fake);
        assert!(!has_han(&r), "Address Fake 不应含汉字: {} → {}", addr, r);
        assert!(r.contains(','), "Address Fake 应含逗号分隔街道与城市: {} → {}", addr, r);
        assert_ne!(r.as_str(), *addr, "替换值不应等于原文");
    }
}

// ============================================================
// S26: EN Replace Fake — Title 输出在英文 titles 池内
// ============================================================

#[test]
fn test_s26_en_replace_fake_title_in_pool() {
    let mut state = fresh_state();
    let titles = ["Senior Partner", "Associate Attorney", "Managing Partner"];

    for t in &titles {
        let r = apply_replace(t, &SensitiveType::Title, &mut state, &ReplaceStyle::Fake);
        assert!(!has_han(&r), "Title Fake 不应含汉字: {} → {}", t, r);
        assert_ne!(r.as_str(), *t, "替换值不应等于原文");
        // 形态：在池内 或 "{title} {wrap}" 形式（>= 池大小后）
        let in_pool = EN_TITLES_POOL.contains(&r.as_str());
        let wrapped = EN_TITLES_POOL.iter().any(|p| {
            r.starts_with(p) && r.len() > p.len() + 1
                && r[p.len()..].starts_with(' ')
                && r[p.len() + 1..].chars().all(|c| c.is_ascii_digit())
        });
        assert!(
            in_pool || wrapped,
            "Title Fake 应在池内或附 wrap 后缀: {} → {}",
            t, r
        );
    }
}

// ============================================================
// S27: EN Replace Mou — PersonName 性别轮换 John Doe / Jane Doe / John Doe 2 / Jane Doe 2
// 4 unique 输入 → 集合 == {John Doe, Jane Doe, John Doe 2, Jane Doe 2}
// ============================================================

#[test]
fn test_s27_en_replace_mou_personname_gender_rotation() {
    let mut state = fresh_state();
    let names = ["James Anderson", "Sarah Mitchell", "David Park", "Rebecca Harrison"];

    let outputs: Vec<String> = names
        .iter()
        .map(|n| apply_replace(n, &SensitiveType::PersonName, &mut state, &ReplaceStyle::Mou))
        .collect();

    let actual: std::collections::HashSet<&str> =
        outputs.iter().map(|s| s.as_str()).collect();
    let expected: std::collections::HashSet<&str> =
        ["John Doe", "Jane Doe", "John Doe 2", "Jane Doe 2"]
            .iter()
            .copied()
            .collect();
    assert_eq!(
        actual, expected,
        "4 unique PersonName Mou 替换值集合应为 {{John Doe, Jane Doe, John Doe 2, Jane Doe 2}}，实际: {:?}",
        outputs
    );
}

// ============================================================
// S28: EN Replace Mou — OrgName 保留原文 suffix，无 suffix 兜底 Acme Co.
// ============================================================

#[test]
fn test_s28_en_replace_mou_orgname_suffix_buckets() {
    let mut state = fresh_state();

    // 用例期望顺序：
    // 1) TechVenture Capital Partners (Partners, count=1) → Acme Partners
    // 2) Mitchell, Chen & Park LLP   (LLP, count=1)      → Acme LLP
    // 3) Harrison & Associates LLP   (LLP, count=2)      → Acme LLP 2
    // 4) Pacific Coast Medical Center (Center 不在表，走兜底 Co., count=1) → Acme Co.
    let inputs_outputs = [
        ("TechVenture Capital Partners", "Acme Partners"),
        ("Mitchell, Chen & Park LLP", "Acme LLP"),
        ("Harrison & Associates LLP", "Acme LLP 2"),
        ("Pacific Coast Medical Center", "Acme Co."),
    ];

    let outputs: Vec<String> = inputs_outputs
        .iter()
        .map(|(orig, _)| {
            apply_replace(orig, &SensitiveType::OrgName, &mut state, &ReplaceStyle::Mou)
        })
        .collect();

    let actual: std::collections::HashSet<&str> =
        outputs.iter().map(|s| s.as_str()).collect();
    let expected: std::collections::HashSet<&str> =
        inputs_outputs.iter().map(|(_, e)| *e).collect();

    assert_eq!(
        actual, expected,
        "4 unique OrgName Mou 替换值集合应为 {{Acme Partners, Acme LLP, Acme LLP 2, Acme Co.}}，实际: {:?}",
        outputs
    );

    // 同时验证按调用顺序的精确映射（顺序敏感）
    for ((orig, expected_out), actual_out) in inputs_outputs.iter().zip(outputs.iter()) {
        assert_eq!(
            actual_out, expected_out,
            "OrgName Mou: '{}' 应替换为 '{}', 实际 '{}'",
            orig, expected_out, actual_out
        );
    }
}

// ============================================================
// S29: EN Replace Mou — Address 输出 [REDACTED CITY] 序号化
// ============================================================

#[test]
fn test_s29_en_replace_mou_address_sequential() {
    let mut state = fresh_state();
    let addrs = [
        "2500 Broadway Avenue, Suite 3100",
        "1420 Market Street, Apt 5B",
        "580 California Street, Suite 2000, San Francisco, CA",
    ];
    let expected = ["[REDACTED CITY]", "[REDACTED CITY] 2", "[REDACTED CITY] 3"];

    for (i, addr) in addrs.iter().enumerate() {
        let r = apply_replace(addr, &SensitiveType::Address, &mut state, &ReplaceStyle::Mou);
        assert_eq!(r, expected[i], "Address Mou 第 {} 次应为 '{}'", i + 1, expected[i]);
    }
}

// ============================================================
// S30: EN Replace Mou — Title 输出 [REDACTED TITLE] 序号化
// ============================================================

#[test]
fn test_s30_en_replace_mou_title_sequential() {
    let mut state = fresh_state();
    let titles = ["Senior Partner", "Associate Attorney", "Managing Partner"];
    let expected = ["[REDACTED TITLE]", "[REDACTED TITLE] 2", "[REDACTED TITLE] 3"];

    for (i, t) in titles.iter().enumerate() {
        let r = apply_replace(t, &SensitiveType::Title, &mut state, &ReplaceStyle::Mou);
        assert_eq!(r, expected[i], "Title Mou 第 {} 次应为 '{}'", i + 1, expected[i]);
    }
}

// ============================================================
// S31: EN Replace Ordinal — 静默降级为 Fake，不输出 'Person A' / 'Company A' / 'Address 1' / 'Title 1'
// ============================================================

#[test]
fn test_s31_en_replace_ordinal_degrades_to_fake() {
    let mut state = fresh_state();

    // PersonName
    let pname = apply_replace(
        "James Anderson",
        &SensitiveType::PersonName,
        &mut state,
        &ReplaceStyle::Ordinal,
    );
    assert!(!pname.starts_with("Person "), "Ordinal En PersonName 不应输出 Person A: {}", pname);
    assert!(pname.contains(' '), "Ordinal En PersonName 应含空格: {}", pname);
    assert!(!has_han(&pname), "Ordinal En PersonName 不应含汉字: {}", pname);

    // OrgName
    let oname = apply_replace(
        "Mitchell, Chen & Park LLP",
        &SensitiveType::OrgName,
        &mut state,
        &ReplaceStyle::Ordinal,
    );
    assert!(!oname.starts_with("Company "), "Ordinal En OrgName 不应输出 Company A: {}", oname);
    let oname_suffix_ok = EN_ORG_SUFFIXES.iter().any(|s| oname.ends_with(s));
    assert!(oname_suffix_ok, "Ordinal En OrgName 末尾应在 EN_ORG_SUFFIXES: {}", oname);
    assert!(!has_han(&oname), "Ordinal En OrgName 不应含汉字: {}", oname);

    // Address
    let addr = apply_replace(
        "2500 Broadway Avenue, Suite 3100",
        &SensitiveType::Address,
        &mut state,
        &ReplaceStyle::Ordinal,
    );
    assert!(!addr.starts_with("Address "), "Ordinal En Address 不应输出 Address 1: {}", addr);
    assert!(addr.contains(','), "Ordinal En Address 应含逗号: {}", addr);
    assert!(!has_han(&addr), "Ordinal En Address 不应含汉字: {}", addr);

    // Title
    let title = apply_replace(
        "Senior Partner",
        &SensitiveType::Title,
        &mut state,
        &ReplaceStyle::Ordinal,
    );
    assert!(!title.starts_with("Title "), "Ordinal En Title 不应输出 Title 1: {}", title);
    assert!(!has_han(&title), "Ordinal En Title 不应含汉字: {}", title);
}
