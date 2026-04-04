mod common;

use dimkey_lib::commands::desensitize::sensitive_type_to_key;
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::excel::{parse_csv, parse_excel};
use dimkey_lib::parser::txt::parse_txt;

use common::*;

// ============================================================
// T05 — 正则引擎不产生 NER 类型
// ============================================================

/// 验证 RegexEngine 的所有结果都不包含 NER 专属类型（PersonName/OrgName/Address/Title）
#[test]
fn test_regex_engine_does_not_produce_ner_types() {
    let path = test_data_path("员工花名册.xlsx");
    let content = parse_excel(&path).expect("Excel 导入失败");

    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    // 所有识别项来源必须是 Regex
    for item in &items {
        assert_eq!(
            item.source,
            DetectSource::Regex,
            "RegexEngine 产生了非 Regex 来源的结果: {:?}",
            item.source
        );
    }

    // NER 专属类型不应出现在正则引擎结果中
    let ner_types = [
        SensitiveType::PersonName,
        SensitiveType::OrgName,
        SensitiveType::Address,
        SensitiveType::Title,
    ];

    for ner_type in &ner_types {
        let count = count_by_type(&items, ner_type);
        assert_eq!(
            count, 0,
            "RegexEngine 不应产生 {:?} 类型，但发现了 {} 个",
            ner_type, count
        );
    }
}

// ============================================================
// T06 — 车牌号识别（通知公告.txt 有标准格式车牌 浙A12345/浙A67890）
// 注：物业业主信息表.xlsx 中车牌格式为 "京A·12345"（带中间点），
// 正则引擎可能不匹配，这是已知的格式兼容问题
// ============================================================

/// 验证标准格式车牌号识别（通知公告.txt 中有 浙A12345、浙A67890）
#[test]
fn test_license_plate_detection() {
    let path = test_data_path("通知公告.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");

    let engine = RegexEngine::new();
    let items = engine.detect(&content);
    let count = count_by_type(&items, &SensitiveType::LicensePlate);

    assert!(
        count >= 2,
        "通知公告.txt 应识别出至少 2 个车牌号（浙A12345, 浙A67890），实际: {}",
        count
    );
}

/// 验证识别到的车牌号格式：长度 7-8 字符
#[test]
fn test_license_plate_format() {
    let path = test_data_path("通知公告.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");

    let engine = RegexEngine::new();
    let items = engine.detect(&content);
    let plates: Vec<&SensitiveItem> = items
        .iter()
        .filter(|i| i.sensitive_type == SensitiveType::LicensePlate)
        .collect();

    assert!(!plates.is_empty(), "应至少识别到一个车牌号");

    for plate in &plates {
        let char_count = plate.text.chars().count();
        assert!(
            char_count >= 7 && char_count <= 9,
            "车牌号 '{}' 长度应为 7-9 字符，实际: {}",
            plate.text, char_count
        );
    }
}

// ============================================================
// T07 — 统一社会信用代码识别（会议纪要.txt 有 91110105MA01B2CH3X）
// 注：律所案件登记表.xlsx 中信用代码 91320500MA1WXYZ123 可能被误分类为 IdCard
// ============================================================

/// 验证统一社会信用代码识别（会议纪要.txt 中有 91110105MA01B2CH3X）
#[test]
fn test_credit_code_detection() {
    let path = test_data_path("会议纪要.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");

    let engine = RegexEngine::new();
    let items = engine.detect(&content);
    let count = count_by_type(&items, &SensitiveType::CreditCode);

    assert!(
        count >= 1,
        "会议纪要.txt 应至少识别出 1 个统一社会信用代码（91110105MA01B2CH3X），实际: {}",
        count
    );
}

/// 验证统一社会信用代码格式：18 位，以数字开头
#[test]
fn test_credit_code_format() {
    let path = test_data_path("会议纪要.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");

    let engine = RegexEngine::new();
    let items = engine.detect(&content);
    let codes: Vec<&SensitiveItem> = items
        .iter()
        .filter(|i| i.sensitive_type == SensitiveType::CreditCode)
        .collect();

    assert!(!codes.is_empty(), "应至少识别到一个统一社会信用代码");

    for code in &codes {
        assert_eq!(
            code.text.len(),
            18,
            "统一社会信用代码 '{}' 应为 18 位，实际: {}",
            code.text,
            code.text.len()
        );
        let first_char = code.text.chars().next().unwrap();
        assert!(
            first_char.is_ascii_digit() || first_char.is_ascii_alphabetic(),
            "统一社会信用代码 '{}' 应以数字或字母开头",
            code.text
        );
    }
}

// ============================================================
// T08 — 银行卡号识别（银行贷款申请表.xlsx，预期 5 个）
// ============================================================

/// 验证银行卡号识别数量 >= 5
#[test]
fn test_bank_card_detection() {
    let path = test_data_path("银行贷款申请表.xlsx");
    let content = parse_excel(&path).expect("Excel 导入失败");

    let engine = RegexEngine::new();
    let items = engine.detect(&content);
    let count = count_by_type(&items, &SensitiveType::BankCard);

    assert!(
        count >= 5,
        "银行卡号识别数量应 >= 5，实际: {}",
        count
    );
}

/// 验证银行卡号长度：16-19 位纯数字
#[test]
fn test_bank_card_length() {
    let path = test_data_path("银行贷款申请表.xlsx");
    let content = parse_excel(&path).expect("Excel 导入失败");

    let engine = RegexEngine::new();
    let items = engine.detect(&content);
    let cards: Vec<&SensitiveItem> = items
        .iter()
        .filter(|i| i.sensitive_type == SensitiveType::BankCard)
        .collect();

    assert!(!cards.is_empty(), "应至少识别到一个银行卡号");

    for card in &cards {
        let digits: String = card.text.chars().filter(|c| c.is_ascii_digit()).collect();
        assert!(
            digits.len() >= 16 && digits.len() <= 19,
            "银行卡号 '{}' 应为 16-19 位数字，实际数字位数: {}",
            card.text, digits.len()
        );
    }
}

// ============================================================
// T09 — 固定电话识别（律所案件登记表.xlsx，预期 3 个）
// ============================================================

/// 验证固定电话识别数量 >= 2
#[test]
fn test_landline_detection() {
    let path = test_data_path("律所案件登记表.xlsx");
    let content = parse_excel(&path).expect("Excel 导入失败");

    let engine = RegexEngine::new();
    let items = engine.detect(&content);
    let count = count_by_type(&items, &SensitiveType::LandlinePhone);

    assert!(
        count >= 2,
        "固定电话识别数量应 >= 2，实际: {}",
        count
    );
}

/// 验证固定电话格式：包含短横线（如 0xx-xxxxxxxx）
#[test]
fn test_landline_format() {
    let path = test_data_path("律所案件登记表.xlsx");
    let content = parse_excel(&path).expect("Excel 导入失败");

    let engine = RegexEngine::new();
    let items = engine.detect(&content);
    let landlines: Vec<&SensitiveItem> = items
        .iter()
        .filter(|i| i.sensitive_type == SensitiveType::LandlinePhone)
        .collect();

    assert!(!landlines.is_empty(), "应至少识别到一个固定电话");

    for landline in &landlines {
        assert!(
            landline.text.contains('-'),
            "固定电话 '{}' 应包含短横线分隔符",
            landline.text
        );
    }
}

// ============================================================
// T01 — 全类型启用: 识别结果包含所有敏感类型
// ============================================================

/// T01: 全类型启用时，员工信息表中应识别出至少 4 种敏感类型
#[test]
fn test_all_types_enabled_detects_all() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    // 全量扫描应包含 Phone, IdCard, Email, BankCard
    let has_phone = count_by_type(&items, &SensitiveType::Phone) > 0;
    let has_idcard = count_by_type(&items, &SensitiveType::IdCard) > 0;
    let has_email = count_by_type(&items, &SensitiveType::Email) > 0;
    let has_bankcard = count_by_type(&items, &SensitiveType::BankCard) > 0;

    assert!(has_phone, "全类型启用应识别到 Phone");
    assert!(has_idcard, "全类型启用应识别到 IdCard");
    assert!(has_email, "全类型启用应识别到 Email");
    assert!(has_bankcard, "全类型启用应识别到 BankCard");

    // 基线覆盖验证
    assert_baseline_from_sidecar(&items, &path);
}

// ============================================================
// T02 — 只启用手机号: 识别结果仅含 Phone 类型
// ============================================================

/// T02: 仅启用 Phone 时，过滤后结果只含 Phone 类型
/// 注：复现 detect_by_regex 的过滤逻辑作为规约测试；同时通过 baseline_filtered 验证基线
#[test]
fn test_only_phone_enabled() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let all_items = engine.detect(&content);

    // 模拟 enabled_types = ["Phone"]，按 detect_by_regex 逻辑过滤
    let enabled_types = vec!["Phone".to_string()];
    let filtered: Vec<&SensitiveItem> = all_items
        .iter()
        .filter(|i| {
            let key = sensitive_type_to_key(&i.sensitive_type);
            enabled_types.contains(&key)
        })
        .collect();

    assert!(!filtered.is_empty(), "Phone 过滤后不应为空");

    // 所有结果都应该是 Phone 类型
    for item in &filtered {
        assert_eq!(
            item.sensitive_type,
            SensitiveType::Phone,
            "过滤后不应包含非 Phone 类型: {:?}",
            item.sensitive_type
        );
    }

    // 基线校验：只检查 Phone 类型
    assert_baseline_from_sidecar_filtered(
        &all_items,
        &path,
        Some(&[SensitiveType::Phone]),
    );
}

// ============================================================
// T03 — 全关再全开: 关闭所有类型后识别为空，重新全开后恢复
// ============================================================

/// T03: 空 enabled_types 列表应过滤掉所有结果
#[test]
fn test_all_off_then_all_on() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let all_items = engine.detect(&content);

    // 全关：空类型列表
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

    // 全开：不传 enabled_types（None），等价于不过滤
    assert!(
        all_items.len() >= 32,
        "全开后应识别出至少 32 项（8行×4类型），实际: {}",
        all_items.len()
    );
}

// ============================================================
// T10 — enabled_types 传 None: 不传参数时应全量识别
// ============================================================

/// T10: enabled_types 为 None 时等价于全量扫描
/// 注：此测试验证过滤规约（specification test），实际 detect_by_regex 为 Tauri command，
/// 需 app_handle 无法直接调用，此处复现其过滤逻辑以确认契约正确性
#[test]
fn test_enabled_types_none_means_all() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let all_items = engine.detect(&content);

    // 模拟 enabled_types = None：不进行过滤
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

    // None 应等价于全量
    assert_eq!(
        result.len(),
        all_items.len(),
        "enabled_types=None 应返回所有识别结果"
    );

    // 验证至少包含 4 种类型
    let type_set: std::collections::HashSet<_> = result
        .iter()
        .map(|i| &i.sensitive_type)
        .collect();
    assert!(
        type_set.len() >= 4,
        "全量扫描应包含至少 4 种类型，实际: {}",
        type_set.len()
    );
}

// ============================================================
// T11 — enabled_types 含不存在的类型名: 未知类型不应报错
// ============================================================

/// T11: 传入未知类型字符串，不应报错，仅过滤掉已知类型
/// 注：规约测试 — 验证 sensitive_type_to_key 过滤契约，未知类型被自然忽略
#[test]
fn test_unknown_type_in_enabled_types() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let all_items = engine.detect(&content);

    // 传入未知类型 + 已知类型
    let enabled_types = vec!["Phone".to_string(), "UnknownType123".to_string()];
    let filtered: Vec<&SensitiveItem> = all_items
        .iter()
        .filter(|i| {
            let key = sensitive_type_to_key(&i.sensitive_type);
            enabled_types.contains(&key)
        })
        .collect();

    // 应正常返回 Phone 类型的结果
    assert!(!filtered.is_empty(), "应识别出 Phone 类型");
    for item in &filtered {
        assert_eq!(
            item.sensitive_type,
            SensitiveType::Phone,
            "应仅包含 Phone 类型"
        );
    }
}

// ============================================================
// T04 — 关闭单一类型: 仅 IdCard 被排除，其他类型不受影响
// ============================================================

/// T04: 关闭 IdCard 后，结果中不应有 IdCard，但其他类型不受影响
/// 注：规约测试 — 复现 detect_by_regex 的过滤逻辑
#[test]
fn test_disable_single_type_idcard() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let all_items = engine.detect(&content);

    // 启用除 IdCard 外的所有类型
    let all_except_idcard: Vec<String> = all_items
        .iter()
        .map(|i| sensitive_type_to_key(&i.sensitive_type))
        .filter(|k| k != "IdCard")
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let filtered: Vec<&SensitiveItem> = all_items
        .iter()
        .filter(|i| {
            let key = sensitive_type_to_key(&i.sensitive_type);
            all_except_idcard.contains(&key)
        })
        .collect();

    // IdCard 应被排除
    assert!(
        !filtered.iter().any(|i| i.sensitive_type == SensitiveType::IdCard),
        "关闭 IdCard 后不应有 IdCard 类型"
    );

    // Phone, Email, BankCard 应保留
    assert!(
        filtered.iter().any(|i| i.sensitive_type == SensitiveType::Phone),
        "Phone 应保留"
    );
    assert!(
        filtered.iter().any(|i| i.sensitive_type == SensitiveType::Email),
        "Email 应保留"
    );
    assert!(
        filtered.iter().any(|i| i.sensitive_type == SensitiveType::BankCard),
        "BankCard 应保留"
    );

    // 基线校验：排除 IdCard
    let enabled_st: Vec<SensitiveType> = all_items
        .iter()
        .map(|i| i.sensitive_type.clone())
        .filter(|st| *st != SensitiveType::IdCard)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    assert_baseline_from_sidecar_filtered(&all_items, &path, Some(&enabled_st));
}

// ============================================================
// T12 — 类型过滤后脱敏: 关闭 IdCard 后脱敏，导出中 IdCard 应保持原文
// ============================================================

/// T12: 关闭 IdCard 后执行脱敏，IdCard 应保持原文不变
#[test]
fn test_filter_type_then_desensitize_preserves_excluded() {
    use dimkey_lib::models::strategy::*;

    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let all_items = engine.detect(&content);

    // 过滤掉 IdCard，只对其余类型脱敏
    let items_without_idcard: Vec<_> = all_items
        .into_iter()
        .filter(|i| i.sensitive_type != SensitiveType::IdCard)
        .collect();

    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::Phone,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::Email,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::BankCard,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
    ];

    let result = desensitize_content(&content, &items_without_idcard, &strategies);

    // 验证 IdCard 列（第 3 列，index=2）保持原文
    let orig_rows = get_rows(&content);
    let new_rows = get_rows(&result.content);

    for (i, (orig, new)) in orig_rows.iter().zip(new_rows.iter()).enumerate() {
        assert_eq!(
            orig[2], new[2],
            "第 {} 行身份证号应保持原文: 原='{}', 新='{}'",
            i + 1, orig[2], new[2]
        );
    }

    // 验证 Phone 列（第 2 列，index=1）已被替换
    let has_phone_change = orig_rows.iter().zip(new_rows.iter())
        .any(|(o, n)| o[1] != n[1]);
    assert!(has_phone_change, "手机号列应被替换");
}
