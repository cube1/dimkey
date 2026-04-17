//! 英文 DOCX 端到端脱敏测试
//! 导入 → 正则识别 → 脱敏 → 验证段落保持 + 无泄漏

mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::language::Language;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::parser::word::parse_docx;

use common::*;

/// 测试英文 DOCX 导入后的段落结构
#[test]
fn test_en_docx_import_paragraphs() {
    let path = test_data_path("attorney_engagement_letter.docx");
    let content = parse_docx(&path).expect("Word 导入失败");

    let paragraphs = get_paragraphs(&content);
    assert!(
        !paragraphs.is_empty(),
        "应解析出至少一个段落"
    );

    // 英文文档中应有一些文本
    let all_text: String = paragraphs.iter().map(|p| p.text.clone()).collect::<Vec<_>>().join(" ");
    assert!(
        all_text.len() > 100,
        "英文文档应有足够的文本内容"
    );
}

/// 测试英文 DOCX 正则识别
#[test]
fn test_en_docx_regex_detect() {
    let path = test_data_path("attorney_engagement_letter.docx");
    let content = parse_docx(&path).expect("Word 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert!(!items.is_empty(), "英文法律文档应识别出敏感信息");

    // 法律文档中应至少有邮箱或电话
    let has_contact_info = items.iter().any(|i| {
        matches!(
            i.sensitive_type,
            SensitiveType::Email | SensitiveType::UsPhone | SensitiveType::Ssn
        )
    });
    assert!(
        has_contact_info,
        "法律文档应包含联系方式（Email/Phone/SSN）"
    );
}

/// 测试脱敏前后段落数量和样式不变
#[test]
fn test_en_docx_replace_preserves_paragraphs() {
    let path = test_data_path("attorney_engagement_letter.docx");
    let content = parse_docx(&path).expect("Word 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    let original_paragraphs = get_paragraphs(&content);
    let original_count = original_paragraphs.len();
    let original_styles: Vec<String> = original_paragraphs.iter().map(|p| p.style.clone()).collect();

    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::Ssn,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
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
        StrategyConfig {
            sensitive_type: SensitiveType::Iban,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::ZipCode,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
    ];

    let result = desensitize_content(&content, &items, &strategies);

    let new_paragraphs = get_paragraphs(&result.content);
    assert_eq!(
        new_paragraphs.len(),
        original_count,
        "脱敏前后段落数量应一致"
    );

    for (i, para) in new_paragraphs.iter().enumerate() {
        assert_eq!(
            para.style, original_styles[i],
            "第 {} 段落的样式不应改变",
            i
        );
    }
}

/// 测试脱敏后不存在原始敏感信息泄漏
#[test]
fn test_en_docx_no_sensitive_leak() {
    let path = test_data_path("attorney_engagement_letter.docx");
    let content = parse_docx(&path).expect("Word 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    if items.is_empty() {
        return;
    }

    let original_texts: Vec<String> = items.iter().map(|i| i.text.clone()).collect();

    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::Ssn,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
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
        StrategyConfig {
            sensitive_type: SensitiveType::Iban,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
    ];

    let result = desensitize_content(&content, &items, &strategies);

    let new_paragraphs = get_paragraphs(&result.content);
    let all_new_text: String = new_paragraphs
        .iter()
        .map(|p| p.text.clone())
        .collect::<Vec<_>>()
        .join(" ");

    for text in &original_texts {
        if result.mappings.iter().any(|m| m.original_text == *text) {
            assert!(
                !all_new_text.contains(text.as_str()),
                "脱敏后不应包含原始敏感信息: {}",
                text
            );
        }
    }
}

/// 测试 litigation_discovery_memo.docx 的识别和脱敏
#[test]
fn test_en_docx_litigation_memo() {
    let path = test_data_path("litigation_discovery_memo.docx");
    let content = parse_docx(&path).expect("Word 导入失败");
    let engine = RegexEngine::for_language(Language::En);
    let items = engine.detect(&content);

    assert!(!items.is_empty(), "诉讼发现备忘录应识别出敏感信息");

    // 全 Replace 脱敏
    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::Ssn,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
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
        StrategyConfig {
            sensitive_type: SensitiveType::ZipCode,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
    ];

    let result = desensitize_content(&content, &items, &strategies);
    assert!(
        !result.mappings.is_empty(),
        "应有脱敏映射记录"
    );

    // 验证脱敏后段落有变化
    let orig_paras = get_paragraphs(&content);
    let new_paras = get_paragraphs(&result.content);
    let has_change = orig_paras
        .iter()
        .zip(new_paras.iter())
        .any(|(o, n)| o.text != n.text);
    assert!(has_change, "脱敏后应有段落发生变化");
}
