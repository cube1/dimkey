mod common;

use dimkey_lib::engine::dict_engine::DictEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::{DictEntry, MatchMode};

/// D06: 字典条目含正则特殊字符时应精确匹配文本
#[test]
fn test_dict_regex_special_chars() {
    let entries = vec![
        DictEntry {
            text: "张.三".to_string(),
            sensitive_type: SensitiveType::Custom("特殊字符测试".to_string()),
            match_mode: MatchMode::Exact,
            replacement: None,
            language: None,
            builtin: false,
        },
    ];

    let engine = DictEngine::new(entries);

    // 构造含 "张.三" 文本的 Document
    let content = FileContent::Document {
        file_name: "test.txt".to_string(),
        file_type: FileType::Txt,
        paragraphs: vec![
            Paragraph {
                index: 0,
                text: "联系人是张.三，请注意。".to_string(),
                style: "normal".to_string(),
                table_position: None,
                pdf_position: None,
            },
            Paragraph {
                index: 1,
                text: "另外张伟三也在场。".to_string(),
                style: "normal".to_string(),
                table_position: None,
                pdf_position: None,
            },
        ],
        encoding: None,
    };

    let items = engine.detect(&content);

    // 应精确匹配 "张.三"
    assert!(
        items.iter().any(|i| i.text == "张.三"),
        "应匹配到 '张.三': {:?}",
        items.iter().map(|i| &i.text).collect::<Vec<_>>()
    );

    // "张伟三" 不应被 "张.三" 的正则模式匹配到（如果引擎错误地用正则）
    // 注：精确匹配模式下不应有此问题
    let false_match = items.iter().any(|i| i.text.contains("张伟三"));
    assert!(!false_match, "'张伟三' 不应被匹配");
}

/// D06 补充: 字典含其他正则元字符
#[test]
fn test_dict_more_special_chars() {
    let entries = vec![
        DictEntry {
            text: "test@[special]".to_string(),
            sensitive_type: SensitiveType::Custom("括号测试".to_string()),
            match_mode: MatchMode::Exact,
            replacement: None,
            language: None,
            builtin: false,
        },
        DictEntry {
            text: "价格$100+".to_string(),
            sensitive_type: SensitiveType::Custom("美元测试".to_string()),
            match_mode: MatchMode::Exact,
            replacement: None,
            language: None,
            builtin: false,
        },
    ];

    let engine = DictEngine::new(entries);

    let content = FileContent::Document {
        file_name: "test.txt".to_string(),
        file_type: FileType::Txt,
        paragraphs: vec![
            Paragraph {
                index: 0,
                text: "请发送至test@[special]邮箱".to_string(),
                style: "normal".to_string(),
                table_position: None,
                pdf_position: None,
            },
            Paragraph {
                index: 1,
                text: "该商品价格$100+，较贵".to_string(),
                style: "normal".to_string(),
                table_position: None,
                pdf_position: None,
            },
        ],
        encoding: None,
    };

    let items = engine.detect(&content);
    assert!(
        items.iter().any(|i| i.text == "test@[special]"),
        "应匹配含方括号的文本"
    );
    assert!(
        items.iter().any(|i| i.text == "价格$100+"),
        "应匹配含美元符和加号的文本"
    );
}
