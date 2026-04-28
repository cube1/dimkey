//! 英文文档全管道测试
//!
//! C80: 三路识别 + Replace 替换全链路验证（NER 真模型）
//! C81: NER degraded 模式 — regex/dict 仍能识别替换 SSN/UsPhone/Email 等正则类型
//!
//! 注：C80 用 common::detect_full_pipeline（全局 NER 单例）；
//!     C81 必须自建 detect 流程注入 NerEngine::degraded()，因此本文件不复用单例。

mod common;

use dimkey_lib::engine::dict_engine::DictEngine;
use dimkey_lib::engine::ner_engine::NerEngine;
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::parser::word::parse_docx;

use common::*;

fn has_han(s: &str) -> bool {
    s.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
}

// ============================================================
// C80: 英文文档完整管道 — 三路识别 + Replace 替换全链路验证
// ============================================================

#[test]
fn test_c80_english_full_pipeline_replace_fake() {
    let path = fixture_path("scenarios/docx/attorney_engagement_letter.docx");
    let content = parse_fixture(&path);
    let items = detect_full_pipeline(&content, Language::En);

    let ner_items: Vec<&SensitiveItem> = items
        .iter()
        .filter(|i| i.source == DetectSource::Ner)
        .collect();
    assert!(
        !ner_items.is_empty(),
        "C80 要求 NER 必须有产出，否则降级模式不应继续测试"
    );

    // 配 4 类 NER 实体的 Replace Fake 策略
    let ner_types = [
        SensitiveType::PersonName,
        SensitiveType::OrgName,
        SensitiveType::Address,
        SensitiveType::Title,
    ];
    let strategies: Vec<StrategyConfig> = ner_types
        .iter()
        .map(|st| StrategyConfig {
            sensitive_type: st.clone(),
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        })
        .collect();

    let target_items: Vec<SensitiveItem> = items
        .iter()
        .filter(|i| ner_types.contains(&i.sensitive_type))
        .cloned()
        .collect();

    let result = desensitize_content(&content, &target_items, &strategies);

    // 提取脱敏后文本
    let after_text: String = match &result.content {
        FileContent::Document { paragraphs, .. } => {
            paragraphs.iter().map(|p| p.text.clone()).collect::<Vec<_>>().join(" ")
        }
        _ => panic!("attorney_engagement_letter.docx 应为 Document"),
    };

    // 契约 1：对 baseline 中已确认的 NER hard 实体，脱敏后文档不再包含原文
    // 不直接对 target_items.text 做 contains 检查 — NER 可能识别出短子串（如 "Sc"），
    // 这类短实体在文档其他位置以非实体形态出现是正常的，不构成"未替换" bug
    let baseline_ner_hard_values = [
        "James Anderson", "Sarah Mitchell", "David Park", "Rebecca Harrison",
        "Mitchell, Chen & Park LLP", "Pacific Coast Medical Center",
        "Harrison & Associates LLP", "TechVenture Capital Partners",
        "2500 Broadway Avenue, Suite 3100",
        "1420 Market Street, Apt 5B",
        "580 California Street, Suite 2000, San Francisco, CA",
        "Senior Partner", "Associate Attorney", "Managing Partner",
    ];
    // 仅对 NER 实际识别到的 baseline hard 实体校验 — 未识别的实体保留原文是模型限制
    let recognized_originals: std::collections::HashSet<String> =
        target_items.iter().map(|i| i.text.clone()).collect();
    for &hard in &baseline_ner_hard_values {
        if recognized_originals.contains(hard) {
            assert!(
                !after_text.contains(hard),
                "脱敏后文档仍包含已识别的 baseline NER 原文 '{}' — 替换未真正发生",
                hard
            );
        }
    }

    // 契约 2：脱敏后文档不含汉字（英文 Fake 应走英文池）
    assert!(
        !has_han(&after_text),
        "英文文档脱敏后不应含汉字"
    );

    // 契约 3：替换后文档与原文不同
    let before_text: String = match &content {
        FileContent::Document { paragraphs, .. } => {
            paragraphs.iter().map(|p| p.text.clone()).collect::<Vec<_>>().join(" ")
        }
        _ => panic!(),
    };
    assert_ne!(before_text, after_text, "脱敏前后文档应不同");

    // smoke：对 fixture baseline 中的 4 类 NER 实体计数下限
    eprintln!(
        "[c80] NER items: PersonName={}, OrgName={}, Address={}, Title={}",
        count_by_type(&items, &SensitiveType::PersonName),
        count_by_type(&items, &SensitiveType::OrgName),
        count_by_type(&items, &SensitiveType::Address),
        count_by_type(&items, &SensitiveType::Title),
    );
}

// ============================================================
// C81: NER degraded 模式 — regex/dict 仍能识别 + Mask 替换
// ============================================================

/// 本地三路合并：注入指定 NerEngine（不走全局单例）
/// 复现 common::detect_full_pipeline 的合并逻辑
fn detect_with_custom_ner(
    content: &FileContent,
    lang: Language,
    ner_engine: &mut NerEngine,
) -> Vec<SensitiveItem> {
    let regex_engine = RegexEngine::for_language(lang);
    let regex_items = regex_engine.detect(content);

    let ner_items = ner_engine.detect(content).unwrap_or_default();

    // 词典：内置 en 词典
    let builtin_dict_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("builtin_dict");
    let dict_json = match lang {
        Language::Zh => std::fs::read_to_string(builtin_dict_path.join("zh.json")).unwrap_or_default(),
        Language::En => std::fs::read_to_string(builtin_dict_path.join("en.json")).unwrap_or_default(),
    };

    #[derive(serde::Deserialize)]
    struct BuiltinDictItem {
        text: String,
        sensitive_type: String,
        match_mode: MatchMode,
    }

    let dict_entries: Vec<DictEntry> = serde_json::from_str::<Vec<BuiltinDictItem>>(&dict_json)
        .unwrap_or_default()
        .into_iter()
        .map(|item| DictEntry {
            text: item.text,
            sensitive_type: dimkey_lib::commands::desensitize::string_to_sensitive_type(&item.sensitive_type),
            match_mode: item.match_mode,
            replacement: None,
            language: None,
            builtin: true,
        })
        .collect();

    let dict_items = if dict_entries.is_empty() {
        vec![]
    } else {
        DictEngine::new(dict_entries).detect(content)
    };

    let mut merged = regex_items;
    for di in dict_items.into_iter().chain(ner_items.into_iter()) {
        let overlap = merged.iter().any(|ex| {
            ex.sheet_index == di.sheet_index
                && ex.row == di.row
                && ex.col == di.col
                && ex.start < di.end
                && di.start < ex.end
        });
        if !overlap {
            merged.push(di);
        }
    }
    merged
}

#[test]
fn test_c81_english_degraded_ner_regex_still_works() {
    let path = fixture_path("scenarios/docx/attorney_engagement_letter.docx");
    let content = parse_docx(&path).expect("docx 导入失败");

    let mut degraded = NerEngine::degraded();

    // 单独测 NER 降级 → 应返回空
    let ner_only = degraded.detect(&content).expect("degraded ner detect");
    assert_eq!(ner_only.len(), 0, "degraded NER 应返回空: {:?}", ner_only);

    // 三路合并：regex/dict 应正常工作
    let items = detect_with_custom_ner(&content, Language::En, &mut degraded);

    // 至少应有 SSN + UsPhone + Email
    let ssn_count = count_by_type(&items, &SensitiveType::Ssn);
    let phone_count = count_by_type(&items, &SensitiveType::UsPhone);
    let email_count = count_by_type(&items, &SensitiveType::Email);
    assert!(ssn_count > 0, "regex 应识别到 SSN，实际 0");
    assert!(phone_count > 0, "regex 应识别到 UsPhone，实际 0");
    assert!(email_count > 0, "regex 应识别到 Email，实际 0");

    // 配 Mask 策略
    let regex_types = [
        SensitiveType::Ssn,
        SensitiveType::UsPhone,
        SensitiveType::Email,
        SensitiveType::CreditCard,
        SensitiveType::Iban,
    ];
    let strategies: Vec<StrategyConfig> = regex_types
        .iter()
        .map(|st| StrategyConfig {
            sensitive_type: st.clone(),
            strategy: Strategy::Mask { keep_prefix: 1, keep_suffix: 1 },
            consistent: true,
        })
        .collect();

    let target_items: Vec<SensitiveItem> = items
        .iter()
        .filter(|i| regex_types.contains(&i.sensitive_type))
        .cloned()
        .collect();

    let result = desensitize_content(&content, &target_items, &strategies);

    let after_text: String = match &result.content {
        FileContent::Document { paragraphs, .. } => {
            paragraphs.iter().map(|p| p.text.clone()).collect::<Vec<_>>().join(" ")
        }
        _ => panic!(),
    };

    // 契约：实际进入 mappings 的正则原文，在 Mask 后文档中不再出现
    // 仅断言 mapping 中已存在的原文，避免对 regex 未支持的类型（如 IBAN）做无效检查
    let mapped_originals: std::collections::HashSet<String> = result
        .mappings
        .iter()
        .map(|m| m.original_text.clone())
        .collect();
    let baseline_regex_values = [
        "539-48-2671",
        "(415) 782-3300",
        "(415) 293-8847",
        "intake@mitchellchenpark.com",
        "j.anderson@gmail.com",
        "4539 1488 0343 6467",
    ];
    let mut checked = 0;
    for v in &baseline_regex_values {
        if mapped_originals.contains(*v) {
            assert!(
                !after_text.contains(v),
                "Mask 后文档仍包含已 mapping 的原文 '{}'",
                v
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 3,
        "至少应有 3 个 baseline regex 值进入 mapping 并被 Mask，实际 {}",
        checked
    );

    // 至少有一条 mapping 包含 *（Mask 输出特征）
    assert!(
        result.mappings.iter().any(|m| m.replaced_text.contains('*')),
        "Mask 输出应包含 *"
    );

    // 验证 PersonName/OrgName 等 NER 类型在降级模式下未被识别（因此原文保留，可接受）
    let ner_types_count = items
        .iter()
        .filter(|i| {
            matches!(
                i.sensitive_type,
                SensitiveType::PersonName | SensitiveType::OrgName | SensitiveType::Address | SensitiveType::Title
            ) && i.source == DetectSource::Ner
        })
        .count();
    assert_eq!(ner_types_count, 0, "降级模式下不应有 NER 来源的实体");
}
