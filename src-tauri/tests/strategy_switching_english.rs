//! 英文场景策略切换测试（对标中文 S01-S06）
//! 覆盖 Mask、Replace（Fake/Mou/Ordinal）、Generalize 三类策略的效果与切换

mod common;

use dimkey_lib::desensitizer::generalize::apply_generalize_for_language;
use dimkey_lib::desensitizer::mask::apply_mask;
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::models::task::StrategyType;
use dimkey_lib::parser::excel::parse_csv;

use common::*;

// ============================================================
// SE01: Mask 策略验证 — english_employee.csv
// ============================================================

#[test]
fn test_en_mask_strategy_ssn_phone_email() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);
    assert!(!items.is_empty(), "应识别到敏感信息");

    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::Ssn,
            strategy: Strategy::Mask { keep_prefix: 3, keep_suffix: 0 },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::UsPhone,
            strategy: Strategy::Mask { keep_prefix: 3, keep_suffix: 4 },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::Email,
            strategy: Strategy::Mask { keep_prefix: 1, keep_suffix: 0 },
            consistent: true,
        },
    ];

    let result = desensitize_content(&content, &items, &strategies);

    for mapping in &result.mappings {
        assert_eq!(mapping.strategy, StrategyType::Mask, "策略应为 Mask");
        assert!(
            mapping.replaced_text.contains('*'),
            "Mask 后应包含 *: {}",
            mapping.replaced_text
        );
    }

    // 脱敏后整体应产生差异
    let orig_rows = get_rows(&content);
    let new_rows = get_rows(&result.content);
    let has_change = orig_rows.iter().zip(new_rows.iter()).any(|(o, n)| {
        o.iter().zip(n.iter()).any(|(a, b)| a.text != b.text)
    });
    assert!(has_change, "Mask 后应有单元格发生变化");
}

// ============================================================
// SE02: Replace 策略验证 — 假数据格式合法
// ============================================================

#[test]
fn test_en_replace_strategy_format_valid() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let configured_types = [
        SensitiveType::Ssn,
        SensitiveType::UsPhone,
        SensitiveType::Email,
        SensitiveType::Passport,
    ];
    let items: Vec<_> = items.into_iter()
        .filter(|i| configured_types.contains(&i.sensitive_type))
        .collect();

    let strategies: Vec<_> = configured_types.iter().map(|st| StrategyConfig {
        sensitive_type: st.clone(),
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }).collect();

    let result = desensitize_content(&content, &items, &strategies);

    for mapping in &result.mappings {
        assert_eq!(mapping.strategy, StrategyType::Replace);
        assert_ne!(
            mapping.replaced_text, mapping.original_text,
            "替换后应与原文不同: {}",
            mapping.original_text
        );
    }

    // Email 假数据应包含 @
    let email_mappings: Vec<_> = result.mappings.iter()
        .filter(|m| m.sensitive_type == SensitiveType::Email)
        .collect();
    assert!(!email_mappings.is_empty(), "应有 Email 映射");
    for m in &email_mappings {
        assert!(m.replaced_text.contains('@'), "假 Email 应包含 @: {}", m.replaced_text);
    }
}

// ============================================================
// SE03: Generalize 策略验证（英文语言分发）
// ============================================================

#[test]
fn test_en_generalize_address() {
    // 完整地址：保留城市 + 州 + 邮编
    let result = apply_generalize_for_language(
        "742 Elm Street, San Francisco, CA 94102",
        &SensitiveType::Address,
        Language::En,
    );
    assert_eq!(result, "San Francisco, CA 94102", "应保留最后两级");
}

#[test]
fn test_en_generalize_ssn() {
    let result = apply_generalize_for_language(
        "123-45-6789",
        &SensitiveType::Ssn,
        Language::En,
    );
    assert_eq!(result, "***-**-6789", "SSN 应保留后 4 位");
}

#[test]
fn test_en_generalize_zip() {
    let result = apply_generalize_for_language("94102", &SensitiveType::ZipCode, Language::En);
    assert_eq!(result, "941**", "ZIP 应保留前 3 位");

    let result2 = apply_generalize_for_language("94102-1234", &SensitiveType::ZipCode, Language::En);
    assert_eq!(result2, "941**", "ZIP+4 也应保留前 3 位");
}

#[test]
fn test_en_generalize_uk_postcode() {
    let result = apply_generalize_for_language(
        "SW1A 1AA",
        &SensitiveType::UkPostcode,
        Language::En,
    );
    assert_eq!(result, "SW1A ***", "UK Postcode 应保留外码");
}

#[test]
fn test_en_generalize_title() {
    assert_eq!(
        apply_generalize_for_language("Chief Executive Officer", &SensitiveType::Title, Language::En),
        "Senior Executive"
    );
    assert_eq!(
        apply_generalize_for_language("Senior Software Engineer", &SensitiveType::Title, Language::En),
        "Technical Staff"
    );
    // Director-Level 分支用 "Head of XXX" 格式验证（见 test_en_generalize_title_director_bug_substring_collision）
    assert_eq!(
        apply_generalize_for_language("Head of Engineering", &SensitiveType::Title, Language::En),
        "Director-Level"
    );
    assert_eq!(
        apply_generalize_for_language("Sales Manager", &SensitiveType::Title, Language::En),
        "Management"
    );
    assert_eq!(
        apply_generalize_for_language("Research Assistant", &SensitiveType::Title, Language::En),
        "Staff"
    );
}

/// BUG-032 修复回归测试：Director / Sector / Vector 等含 "cto" 子串的词不应被误归 Senior Executive
#[test]
fn test_en_generalize_title_director_not_executive() {
    assert_eq!(
        apply_generalize_for_language("Director of Marketing", &SensitiveType::Title, Language::En),
        "Director-Level",
        "Director 应归为 Director-Level，而非受 'cto' 子串污染归入 Senior Executive"
    );
    assert_eq!(
        apply_generalize_for_language("Marketing Director", &SensitiveType::Title, Language::En),
        "Director-Level"
    );
    // VP 缩写（短单词也要能匹配）
    assert_eq!(
        apply_generalize_for_language("VP of Engineering", &SensitiveType::Title, Language::En),
        "Director-Level"
    );
    // 短语匹配依然生效
    assert_eq!(
        apply_generalize_for_language("Vice President of Sales", &SensitiveType::Title, Language::En),
        "Director-Level"
    );
    assert_eq!(
        apply_generalize_for_language("Head of Product", &SensitiveType::Title, Language::En),
        "Director-Level"
    );
}

/// BUG-032 衍生回归：其他含 ceo/cfo/cto/coo 子串的无关词不应被污染
#[test]
fn test_en_generalize_title_no_substring_pollution() {
    // "Vector Analyst" 含 "cto" 子串，但应按 analyst 归为 Technical Staff
    assert_eq!(
        apply_generalize_for_language("Vector Analyst", &SensitiveType::Title, Language::En),
        "Technical Staff",
        "含 'cto' 子串的 'Vector' 不应污染分类"
    );
    // "Sector Manager" 含 "cto" 子串，应按 manager 归为 Management
    assert_eq!(
        apply_generalize_for_language("Sector Manager", &SensitiveType::Title, Language::En),
        "Management"
    );
}

#[test]
fn test_en_generalize_uspoe_phone_partial_mask() {
    let result = apply_generalize_for_language(
        "(415) 293-8847",
        &SensitiveType::UsPhone,
        Language::En,
    );
    // 英文 Phone 泛化：保留前半部分，后半部分用 * 替代
    assert!(
        result.contains('*'),
        "UsPhone 泛化应包含 *: {}",
        result
    );
    assert_ne!(result, "(415) 293-8847", "泛化后应与原文不同");
}

// ============================================================
// SE04: 策略切换 — Mask vs Replace 结果不同
// ============================================================

#[test]
fn test_en_strategy_switch_mask_then_replace() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let ssn_items: Vec<_> = items.iter()
        .filter(|i| i.sensitive_type == SensitiveType::Ssn)
        .cloned()
        .collect();

    // 第一次：Mask
    let mask_strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Ssn,
        strategy: Strategy::Mask { keep_prefix: 3, keep_suffix: 0 },
        consistent: true,
    }];
    let mask_result = desensitize_content(&content, &ssn_items, &mask_strategies);

    // 第二次：Replace
    let replace_strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Ssn,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];
    let replace_result = desensitize_content(&content, &ssn_items, &replace_strategies);

    assert!(!mask_result.mappings.is_empty());
    assert!(!replace_result.mappings.is_empty());
    assert_eq!(mask_result.mappings[0].strategy, StrategyType::Mask);
    assert_eq!(replace_result.mappings[0].strategy, StrategyType::Replace);

    // 同一 SSN 两种策略结果应不同
    let orig = &mask_result.mappings[0].original_text;
    let mask_find = mask_result.mappings.iter().find(|m| &m.original_text == orig).unwrap();
    let replace_find = replace_result.mappings.iter().find(|m| &m.original_text == orig).unwrap();
    assert_ne!(
        mask_find.replaced_text, replace_find.replaced_text,
        "Mask 和 Replace 对同一 SSN 的结果应不同"
    );
    // Mask 结果含 *，Replace 结果不含 *
    assert!(mask_find.replaced_text.contains('*'), "Mask 结果应含 *");
    assert!(!replace_find.replaced_text.contains('*'), "Replace 结果不应含 *");
}

// ============================================================
// SE05: Mask 前后缀参数 — 针对英文类型
// ============================================================

#[test]
fn test_en_mask_params_ssn() {
    // SSN 格式: 123-45-6789（11 字符）
    let ssn = "123-45-6789";

    // 前 3 后 0
    let r1 = apply_mask(ssn, &SensitiveType::Ssn, 3, 0);
    assert_eq!(&r1[..3], "123");
    assert!(r1[3..].chars().all(|c| c == '*'), "其余应全为 *: {}", r1);

    // 前 0 后 4
    let r2 = apply_mask(ssn, &SensitiveType::Ssn, 0, 4);
    assert_eq!(&r2[r2.len() - 4..], "6789");

    // 前 0 后 0：全掩码
    let r3 = apply_mask(ssn, &SensitiveType::Ssn, 0, 0);
    assert!(r3.chars().all(|c| c == '*'), "全掩码应全为 *: {}", r3);

    // 三种参数结果应各不相同
    assert_ne!(r1, r2);
    assert_ne!(r2, r3);
    assert_ne!(r1, r3);
}

#[test]
fn test_en_mask_params_email() {
    let email = "john@test.com";
    let r1 = apply_mask(email, &SensitiveType::Email, 1, 0);
    assert_eq!(&r1[..1], "j");
    assert!(r1[1..].chars().all(|c| c == '*'), "首字母后应全为 *: {}", r1);

    let r2 = apply_mask(email, &SensitiveType::Email, 2, 4);
    assert_eq!(&r2[..2], "jo");
    assert_eq!(&r2[r2.len() - 4..], ".com");
}

// ============================================================
// SE06: Replace 风格对 NER 类型有效（PersonName/OrgName/Title/Address）
// ============================================================

#[test]
fn test_en_replace_styles_on_ner_types() {
    use dimkey_lib::desensitizer::replace::{apply_replace, ReplaceState};
    use std::collections::HashMap;

    let mut state = ReplaceState::new(42, HashMap::new());

    // 对同一 PersonName 使用 Fake / Mou / Ordinal 三种风格
    let name = "James Anderson";
    let fake = apply_replace(name, &SensitiveType::PersonName, &mut state, &ReplaceStyle::Fake);

    let mut state2 = ReplaceState::new(42, HashMap::new());
    let mou = apply_replace(name, &SensitiveType::PersonName, &mut state2, &ReplaceStyle::Mou);

    let mut state3 = ReplaceState::new(42, HashMap::new());
    let ordinal = apply_replace(name, &SensitiveType::PersonName, &mut state3, &ReplaceStyle::Ordinal);

    // 三种风格都应替换原文
    assert_ne!(fake, name, "Fake 应替换原文");
    assert_ne!(mou, name, "Mou 应替换原文");
    assert_ne!(ordinal, name, "Ordinal 应替换原文");
}

// ============================================================
// SE07: Replace 对 SSN/Email/Phone 产生有效替换（风格差异不保证）
// ============================================================

#[test]
fn test_en_replace_regex_types_produce_valid_output() {
    use dimkey_lib::desensitizer::replace::{apply_replace, ReplaceState};
    use std::collections::HashMap;

    let mut state = ReplaceState::new(42, HashMap::new());

    let ssn = apply_replace("123-45-6789", &SensitiveType::Ssn, &mut state, &ReplaceStyle::Fake);
    assert_ne!(ssn, "123-45-6789", "SSN 应被替换");

    let email = apply_replace("john@test.com", &SensitiveType::Email, &mut state, &ReplaceStyle::Fake);
    assert_ne!(email, "john@test.com", "Email 应被替换");
    assert!(email.contains('@'), "假 Email 应包含 @: {}", email);

    // UsPhone 替换
    let phone = apply_replace("(415) 293-8847", &SensitiveType::UsPhone, &mut state, &ReplaceStyle::Fake);
    assert_ne!(phone, "(415) 293-8847", "UsPhone 应被替换");
}
