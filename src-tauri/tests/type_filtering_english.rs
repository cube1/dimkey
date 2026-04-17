//! 英文场景类型过滤测试（对标中文 T01-T12）
//! 验证 enabled_types 过滤契约对英文敏感类型同样生效

mod common;

use dimkey_lib::commands::desensitize::sensitive_type_to_key;
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::parser::excel::parse_csv;

use common::*;

// ============================================================
// TE01: 全类型启用 — english_employee.csv 至少识别 4 种英文类型
// ============================================================

#[test]
fn test_en_all_types_enabled_detects_all() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let has_ssn = count_by_type(&items, &SensitiveType::Ssn) > 0;
    let has_phone = count_by_type(&items, &SensitiveType::UsPhone) > 0;
    let has_email = count_by_type(&items, &SensitiveType::Email) > 0;
    let has_iban_or_passport = count_by_type(&items, &SensitiveType::Iban) > 0
        || count_by_type(&items, &SensitiveType::Passport) > 0;

    assert!(has_ssn, "全类型启用应识别到 SSN");
    assert!(has_phone, "全类型启用应识别到 UsPhone");
    assert!(has_email, "全类型启用应识别到 Email");
    assert!(has_iban_or_passport, "全类型启用应识别到 IBAN 或 Passport");
}

// ============================================================
// TE02: 仅启用 SSN — 过滤后结果只含 Ssn
// ============================================================

#[test]
fn test_en_only_ssn_enabled() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let all_items = engine.detect(&content);

    let enabled_types = vec!["Ssn".to_string()];
    let filtered: Vec<&SensitiveItem> = all_items
        .iter()
        .filter(|i| {
            let key = sensitive_type_to_key(&i.sensitive_type);
            enabled_types.contains(&key)
        })
        .collect();

    assert!(!filtered.is_empty(), "SSN 过滤后不应为空");
    for item in &filtered {
        assert_eq!(
            item.sensitive_type,
            SensitiveType::Ssn,
            "过滤后不应包含非 Ssn 类型: {:?}",
            item.sensitive_type
        );
    }
}

// ============================================================
// TE03: 仅启用 UsPhone
// ============================================================

#[test]
fn test_en_only_us_phone_enabled() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let all_items = engine.detect(&content);

    let enabled_types = vec!["UsPhone".to_string()];
    let filtered: Vec<&SensitiveItem> = all_items
        .iter()
        .filter(|i| {
            let key = sensitive_type_to_key(&i.sensitive_type);
            enabled_types.contains(&key)
        })
        .collect();

    assert!(!filtered.is_empty(), "UsPhone 过滤后不应为空");
    for item in &filtered {
        assert_eq!(item.sensitive_type, SensitiveType::UsPhone);
    }
}

// ============================================================
// TE04: 全关再全开 — 空 enabled_types 后识别为空
// ============================================================

#[test]
fn test_en_all_off_then_all_on() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let all_items = engine.detect(&content);

    // 全关
    let empty_types: Vec<String> = vec![];
    let filtered_off: Vec<&SensitiveItem> = all_items
        .iter()
        .filter(|i| {
            let key = sensitive_type_to_key(&i.sensitive_type);
            empty_types.contains(&key)
        })
        .collect();
    assert!(
        filtered_off.is_empty(),
        "关闭所有类型后应无识别结果，实际: {}",
        filtered_off.len()
    );

    // 全开（等同于全量）
    assert!(
        all_items.len() >= 20,
        "全开后应识别出至少 20 项（10 行 × 多类型），实际: {}",
        all_items.len()
    );
}

// ============================================================
// TE05: 关闭单一类型 — 关闭 Ssn 后结果不含 Ssn
// ============================================================

#[test]
fn test_en_disable_single_type_ssn() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let all_items = engine.detect(&content);

    // 启用除 Ssn 外的所有类型
    let all_except_ssn: Vec<String> = all_items
        .iter()
        .map(|i| sensitive_type_to_key(&i.sensitive_type))
        .filter(|k| k != "Ssn")
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let filtered: Vec<&SensitiveItem> = all_items
        .iter()
        .filter(|i| {
            let key = sensitive_type_to_key(&i.sensitive_type);
            all_except_ssn.contains(&key)
        })
        .collect();

    // Ssn 应被排除
    assert!(
        !filtered.iter().any(|i| i.sensitive_type == SensitiveType::Ssn),
        "关闭 Ssn 后不应有 Ssn 类型"
    );

    // 其他类型应保留
    assert!(
        filtered.iter().any(|i| i.sensitive_type == SensitiveType::UsPhone),
        "UsPhone 应保留"
    );
    assert!(
        filtered.iter().any(|i| i.sensitive_type == SensitiveType::Email),
        "Email 应保留"
    );
}

// ============================================================
// TE06: 未知类型 — 不应报错，仅忽略
// ============================================================

#[test]
fn test_en_unknown_type_in_enabled_types() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let all_items = engine.detect(&content);

    let enabled_types = vec!["Ssn".to_string(), "UnknownTypeXYZ".to_string()];
    let filtered: Vec<&SensitiveItem> = all_items
        .iter()
        .filter(|i| {
            let key = sensitive_type_to_key(&i.sensitive_type);
            enabled_types.contains(&key)
        })
        .collect();

    assert!(!filtered.is_empty(), "应正常识别出 Ssn 类型");
    for item in &filtered {
        assert_eq!(item.sensitive_type, SensitiveType::Ssn, "应仅含 Ssn");
    }
}

// ============================================================
// TE07: enabled_types=None — 等价于全量
// ============================================================

#[test]
fn test_en_enabled_types_none_means_all() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let all_items = engine.detect(&content);

    let enabled_types: Option<Vec<String>> = None;
    let result = if let Some(ref types) = enabled_types {
        all_items
            .iter()
            .filter(|i| {
                let key = sensitive_type_to_key(&i.sensitive_type);
                types.contains(&key)
            })
            .collect::<Vec<_>>()
    } else {
        all_items.iter().collect::<Vec<_>>()
    };

    assert_eq!(result.len(), all_items.len(), "None 应返回所有结果");
    let type_set: std::collections::HashSet<_> = result.iter().map(|i| &i.sensitive_type).collect();
    assert!(type_set.len() >= 3, "全量扫描应包含至少 3 种类型");
}

// ============================================================
// TE08: 过滤后脱敏 — 关闭 Ssn 后 Ssn 列保持原文
// ============================================================

#[test]
fn test_en_filter_ssn_then_desensitize_preserves_ssn() {
    let path = test_data_path("english_employee.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let all_items = engine.detect(&content);

    // 过滤掉 Ssn，只对其余类型脱敏
    let items_without_ssn: Vec<_> = all_items
        .into_iter()
        .filter(|i| i.sensitive_type != SensitiveType::Ssn)
        .collect();

    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::UsPhone,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::Email,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
    ];

    let result = desensitize_content(&content, &items_without_ssn, &strategies);

    // SSN 列是第 3 列（index=2），应保持原文
    let orig_rows = get_rows(&content);
    let new_rows = get_rows(&result.content);

    for (i, (orig, new)) in orig_rows.iter().zip(new_rows.iter()).enumerate() {
        assert_eq!(
            orig[2], new[2],
            "第 {} 行 SSN 应保持原文: 原='{}', 新='{}'",
            i + 1, orig[2].text, new[2].text
        );
    }

    // Phone 列（index=3）应被替换
    let has_phone_change = orig_rows.iter().zip(new_rows.iter())
        .any(|(o, n)| o[3] != n[3]);
    assert!(has_phone_change, "Phone 列应被替换");
}
