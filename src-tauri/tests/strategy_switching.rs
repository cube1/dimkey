mod common;

use dimkey_lib::desensitizer::mask::apply_mask;
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::models::task::StrategyType;
use dimkey_lib::parser::txt::parse_txt;

use common::*;

// ============================================================
// S01: Mask 策略验证 — sample.txt
// ============================================================

/// S01: Mask 策略应将敏感值替换为部分掩码格式
#[test]
fn test_mask_strategy_on_txt() {
    let path = fixture_path("sample.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(!items.is_empty(), "应识别到敏感信息");

    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::Phone,
            strategy: Strategy::Mask { keep_prefix: 3, keep_suffix: 4 },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::IdCard,
            strategy: Strategy::Mask { keep_prefix: 6, keep_suffix: 4 },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::Email,
            strategy: Strategy::Mask { keep_prefix: 1, keep_suffix: 0 },
            consistent: true,
        },
    ];

    let result = desensitize_content(&content, &items, &strategies);

    // 验证所有映射都是 Mask 策略
    for mapping in &result.mappings {
        assert_eq!(mapping.strategy, StrategyType::Mask, "策略应为 Mask");
    }

    // 验证手机号掩码格式：前 3 后 4，中间 *
    let phone_mappings: Vec<_> = result.mappings.iter()
        .filter(|m| m.sensitive_type == SensitiveType::Phone)
        .collect();
    for m in &phone_mappings {
        assert!(
            m.replaced_text.contains('*'),
            "Mask 后应包含 *: {}",
            m.replaced_text
        );
        assert_eq!(
            &m.replaced_text[..3], &m.original_text[..3],
            "前 3 位应保留"
        );
    }

    // 脱敏后文本应与原文不同
    let orig_paras = get_paragraphs(&content);
    let new_paras = get_paragraphs(&result.content);
    let has_change = orig_paras.iter().zip(new_paras.iter()).any(|(o, n)| o.text != n.text);
    assert!(has_change, "Mask 后应有段落发生变化");
}

// ============================================================
// S02: Replace 策略验证 — sample.txt
// ============================================================

/// S02: Replace 策略应将敏感值替换为格式合法的假数据
#[test]
fn test_replace_strategy_on_txt() {
    let path = fixture_path("sample.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::Phone,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::IdCard,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::Email,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
    ];

    // 只保留配置了策略的类型，避免未配置类型 fallback 为 Mask
    let configured_types = [SensitiveType::Phone, SensitiveType::IdCard, SensitiveType::Email];
    let items: Vec<_> = items.into_iter()
        .filter(|i| configured_types.contains(&i.sensitive_type))
        .collect();

    let result = desensitize_content(&content, &items, &strategies);

    // 所有映射都是 Replace
    for mapping in &result.mappings {
        assert_eq!(mapping.strategy, StrategyType::Replace, "策略应为 Replace");
        assert_ne!(mapping.replaced_text, mapping.original_text, "替换后应与原文不同");
    }

    // 假手机号格式验证
    let phone_mappings: Vec<_> = result.mappings.iter()
        .filter(|m| m.sensitive_type == SensitiveType::Phone)
        .collect();
    for m in &phone_mappings {
        assert_eq!(m.replaced_text.len(), 11, "假手机号应为 11 位");
        assert!(m.replaced_text.starts_with('1'), "假手机号应以 1 开头");
    }

    // 假邮箱格式验证
    let email_mappings: Vec<_> = result.mappings.iter()
        .filter(|m| m.sensitive_type == SensitiveType::Email)
        .collect();
    for m in &email_mappings {
        assert!(m.replaced_text.contains('@'), "假邮箱应包含 @");
    }
}

// ============================================================
// S03: Generalize 策略验证 — sample.txt
// ============================================================

/// S03: Generalize 策略应降低信息精度
#[test]
fn test_generalize_strategy_on_txt() {
    let path = fixture_path("sample.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::Phone,
            strategy: Strategy::Generalize,
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::IdCard,
            strategy: Strategy::Generalize,
            consistent: true,
        },
    ];

    let phone_and_id: Vec<_> = items.into_iter()
        .filter(|i| matches!(i.sensitive_type, SensitiveType::Phone | SensitiveType::IdCard))
        .collect();

    let result = desensitize_content(&content, &phone_and_id, &strategies);

    for mapping in &result.mappings {
        assert_eq!(mapping.strategy, StrategyType::Generalize);
        assert_ne!(mapping.replaced_text, mapping.original_text, "泛化后应与原文不同");
    }

    // 身份证泛化应包含出生年份
    let id_mappings: Vec<_> = result.mappings.iter()
        .filter(|m| m.sensitive_type == SensitiveType::IdCard)
        .collect();
    for m in &id_mappings {
        assert!(
            m.replaced_text.contains("年出生"),
            "身份证泛化应包含'年出生': {}",
            m.replaced_text
        );
    }
}

// ============================================================
// S04: 策略来回切换 — 先 Mask 再 Replace，结果应不同
// ============================================================

/// S04: 同一数据先用 Mask 再用 Replace，两次结果应完全不同
#[test]
fn test_strategy_switch_mask_then_replace() {
    let path = fixture_path("sample.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    // 第一次：Mask
    let mask_strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Phone,
        strategy: Strategy::Mask { keep_prefix: 3, keep_suffix: 4 },
        consistent: true,
    }];
    let phone_items: Vec<_> = items.iter()
        .filter(|i| i.sensitive_type == SensitiveType::Phone)
        .cloned()
        .collect();
    let mask_result = desensitize_content(&content, &phone_items, &mask_strategies);

    // 第二次：Replace（对原始内容重新脱敏）
    let replace_strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Phone,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];
    let replace_result = desensitize_content(&content, &phone_items, &replace_strategies);

    // 两次脱敏的映射应使用不同策略
    assert!(!mask_result.mappings.is_empty(), "Mask 应有映射");
    assert!(!replace_result.mappings.is_empty(), "Replace 应有映射");
    assert_eq!(mask_result.mappings[0].strategy, StrategyType::Mask);
    assert_eq!(replace_result.mappings[0].strategy, StrategyType::Replace);

    // 同一原文的替换结果应不同
    let mask_replaced = &mask_result.mappings[0].replaced_text;
    let replace_replaced = &replace_result.mappings[0].replaced_text;
    assert_ne!(mask_replaced, replace_replaced, "Mask 和 Replace 结果应不同");
}

// ============================================================
// S05: Replace 风格 Fake/Mou/Ordinal
// ============================================================

/// S05: Replace 三种风格都能正常替换
#[test]
fn test_replace_styles_fake_mou_ordinal() {
    let path = fixture_path("sample.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    let phone_items: Vec<_> = items.into_iter()
        .filter(|i| i.sensitive_type == SensitiveType::Phone)
        .collect();
    assert!(!phone_items.is_empty(), "应有手机号");

    for style in [ReplaceStyle::Fake, ReplaceStyle::Mou, ReplaceStyle::Ordinal] {
        let strategies = vec![StrategyConfig {
            sensitive_type: SensitiveType::Phone,
            strategy: Strategy::Replace { style: style.clone() },
            consistent: true,
        }];
        let result = desensitize_content(&content, &phone_items, &strategies);

        for mapping in &result.mappings {
            assert_ne!(
                mapping.replaced_text, mapping.original_text,
                "{:?} 风格替换后应与原文不同",
                style
            );
        }
    }
}

// ============================================================
// S06: Mask 前后缀参数 — 不同参数产生不同掩码
// ============================================================

/// S06: Mask 不同前后缀参数产生不同结果
#[test]
fn test_mask_prefix_suffix_params() {
    let phone = "13800138000";

    // 前 3 后 4
    let r1 = apply_mask(phone, &SensitiveType::Phone, 3, 4);
    assert_eq!(&r1[..3], "138");
    assert_eq!(&r1[7..], "8000");

    // 前 0 后 0 — 全掩码
    let r2 = apply_mask(phone, &SensitiveType::Phone, 0, 0);
    assert!(r2.chars().all(|c| c == '*'), "全掩码应全为 *: {}", r2);

    // 前 6 后 2
    let r3 = apply_mask(phone, &SensitiveType::Phone, 6, 2);
    assert_eq!(&r3[..6], "138001");
    assert_eq!(&r3[9..], "00");

    // 三种参数结果应各不相同
    assert_ne!(r1, r2, "不同参数结果应不同");
    assert_ne!(r2, r3, "不同参数结果应不同");
    assert_ne!(r1, r3, "不同参数结果应不同");
}
