mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::parser::word::parse_docx;

use common::*;

/// 测试 Word 文档导入后的段落结构
#[test]
fn test_docx_import_paragraphs() {
    let path = test_data_path("客户调研报告.docx");
    let content = parse_docx(&path).expect("Word 导入失败");

    let paragraphs = get_paragraphs(&content);
    assert!(
        !paragraphs.is_empty(),
        "应解析出至少一个段落"
    );

    // 文档中应包含敏感信息
    let all_text: String = paragraphs.iter().map(|p| p.text.clone()).collect::<Vec<_>>().join(" ");
    assert!(
        all_text.contains("张三") || all_text.contains("1380") || all_text.contains("@"),
        "文档中应包含敏感信息（姓名/手机/邮箱）"
    );
}

/// 测试 Word 文档的正则识别
#[test]
fn test_docx_regex_detect() {
    let path = test_data_path("客户调研报告.docx");
    let content = parse_docx(&path).expect("Word 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    // 文档中至少有手机号和邮箱
    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 1,
        "应识别出至少 1 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 1,
        "应识别出至少 1 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
}

/// 测试脱敏前后段落数量和样式不变
#[test]
fn test_docx_replace_preserves_paragraph_count() {
    let path = test_data_path("客户调研报告.docx");
    let content = parse_docx(&path).expect("Word 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    let original_paragraphs = get_paragraphs(&content);
    let original_count = original_paragraphs.len();
    let original_styles: Vec<String> = original_paragraphs.iter().map(|p| p.style.clone()).collect();

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
            sensitive_type: SensitiveType::IdCard,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::BankCard,
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
fn test_docx_no_sensitive_leak_after_desensitize() {
    let path = test_data_path("客户调研报告.docx");
    let content = parse_docx(&path).expect("Word 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    if items.is_empty() {
        return; // 没有敏感项则跳过
    }

    // 收集所有原始敏感文本
    let original_texts: Vec<String> = items.iter().map(|i| i.text.clone()).collect();

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
            sensitive_type: SensitiveType::IdCard,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::BankCard,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
    ];

    let result = desensitize_content(&content, &items, &strategies);

    // 收集脱敏后的全文
    let new_paragraphs = get_paragraphs(&result.content);
    let all_new_text: String = new_paragraphs
        .iter()
        .map(|p| p.text.clone())
        .collect::<Vec<_>>()
        .join(" ");

    // 验证原始敏感文本不存在于脱敏后的文档中
    for text in &original_texts {
        assert!(
            !all_new_text.contains(text.as_str()),
            "脱敏后不应包含原始敏感信息: {}",
            text
        );
    }
}
