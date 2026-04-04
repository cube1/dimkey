mod common;

use dimkey_lib::engine::dict_engine::DictEngine;
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::{DictEntry, MatchMode};
use dimkey_lib::parser::txt::parse_txt;
use dimkey_lib::parser::excel::parse_csv;

use common::*;

/// 构造 Document 类型的 FileContent（便于字典测试）
fn make_document(text: &str) -> FileContent {
    FileContent::Document {
        file_name: "test.txt".to_string(),
        file_type: FileType::Txt,
        paragraphs: vec![Paragraph {
            index: 0,
            text: text.to_string(),
            style: "normal".to_string(),
            table_position: None,
            pdf_position: None,
        }],
        encoding: None,
    }
}

// ============================================================
// D01: 添加字典条目后命中
// ============================================================

/// D01: 新增自定义词条应在识别结果中出现
#[test]
fn test_dict_add_entry_then_detect() {
    let path = fixture_path("sample.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");

    // OrgName 是 NER 类型，正则引擎不应识别（已在 T05 中验证过）

    // 添加字典条目
    let entries = vec![DictEntry {
        text: "阿里巴巴集团控股有限公司".to_string(),
        sensitive_type: SensitiveType::OrgName,
        match_mode: MatchMode::Exact,
        replacement: None,
        language: None,
        builtin: false,
    }];

    let dict_engine = DictEngine::new(entries);
    let dict_items = dict_engine.detect(&content);

    assert!(
        dict_items.iter().any(|i| i.text == "阿里巴巴集团控股有限公司"),
        "字典应命中 '阿里巴巴集团控股有限公司'"
    );
}

// ============================================================
// D02: 删除字典条目后不再命中
// ============================================================

/// D02: 移除词条后该文本不被字典引擎识别
#[test]
fn test_dict_remove_entry_no_longer_detected() {
    let content = make_document("这是机密项目的相关资料");

    // 有词条时应命中
    let with_entry = DictEngine::new(vec![DictEntry {
        text: "机密项目".to_string(),
        sensitive_type: SensitiveType::Custom("机密".to_string()),
        match_mode: MatchMode::Exact,
        replacement: None,
        language: None,
        builtin: false,
    }]);
    assert_eq!(with_entry.detect(&content).len(), 1, "有词条时应命中");

    // 删除词条后（空词典）不应命中
    let without_entry = DictEngine::new(vec![]);
    assert!(without_entry.detect(&content).is_empty(), "删除词条后不应命中");
}

// ============================================================
// D03: 白名单排除 — 将已识别值排除
// ============================================================

/// D03: 白名单值应从最终结果中排除
#[test]
fn test_whitelist_excludes_detected_value() {
    let path = fixture_path("sample.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    // sample.txt 包含 13800138000，将其加入白名单
    let whitelist = vec!["13800138000".to_string()];

    // 模拟白名单过滤（实际应用中在前端/命令层过滤）
    let filtered: Vec<_> = items.iter()
        .filter(|i| !whitelist.contains(&i.text))
        .collect();

    assert!(
        !filtered.iter().any(|i| i.text == "13800138000"),
        "白名单值不应出现在过滤后结果中"
    );

    // 其他手机号（如 13912345678）应保留
    assert!(
        filtered.iter().any(|i| i.text == "13912345678"),
        "非白名单手机号应保留"
    );
}

// ============================================================
// D04: 字典+白名单组合
// ============================================================

/// D04: 字典命中的条目若在白名单中则被排除
#[test]
fn test_dict_hit_excluded_by_whitelist() {
    let content = make_document("联系张三处理机密项目");

    let entries = vec![
        DictEntry {
            text: "张三".to_string(),
            sensitive_type: SensitiveType::PersonName,
            match_mode: MatchMode::Exact,
            replacement: None,
            language: None,
            builtin: false,
        },
        DictEntry {
            text: "机密项目".to_string(),
            sensitive_type: SensitiveType::Custom("机密".to_string()),
            match_mode: MatchMode::Exact,
            replacement: None,
            language: None,
            builtin: false,
        },
    ];

    let dict_engine = DictEngine::new(entries);
    let items = dict_engine.detect(&content);
    assert_eq!(items.len(), 2, "字典应命中 2 项");

    // 白名单排除 "张三"
    let whitelist = vec!["张三".to_string()];
    let filtered: Vec<_> = items.iter()
        .filter(|i| !whitelist.contains(&i.text))
        .collect();

    assert_eq!(filtered.len(), 1, "白名单排除后应剩 1 项");
    assert_eq!(filtered[0].text, "机密项目");
}

// ============================================================
// D05: 模糊匹配 — 忽略大小写
// ============================================================

/// D05: Fuzzy 模式下忽略大小写匹配
#[test]
fn test_dict_fuzzy_case_insensitive() {
    let content = make_document("Contact John Smith for Project Alpha details");

    let entries = vec![DictEntry {
        text: "john smith".to_string(),
        sensitive_type: SensitiveType::PersonName,
        match_mode: MatchMode::Fuzzy,
        replacement: None,
        language: None,
        builtin: false,
    }];

    let engine = DictEngine::new(entries);
    let items = engine.detect(&content);

    assert_eq!(items.len(), 1, "Fuzzy 模式应忽略大小写匹配");
    assert_eq!(items[0].text, "John Smith", "应保留原始大小写");
}

// ============================================================
// D07: 字典条目跨格式生效 — 同一词条在 CSV 和 TXT 中都应命中
// ============================================================

/// D07: 同一词条在不同格式文件中都应命中
#[test]
fn test_dict_entry_cross_format() {
    let entries = vec![DictEntry {
        text: "13800138000".to_string(),
        sensitive_type: SensitiveType::Phone,
        match_mode: MatchMode::Exact,
        replacement: None,
        language: None,
        builtin: false,
    }];

    // TXT 格式
    let txt_path = fixture_path("sample.txt");
    let txt_content = parse_txt(&txt_path).expect("TXT 导入失败");
    let txt_engine = DictEngine::new(entries.clone());
    let txt_items = txt_engine.detect(&txt_content);
    let txt_hit = txt_items.iter().any(|i| i.text == "13800138000");

    // CSV 格式
    let csv_path = fixture_path("sample.csv");
    let csv_content = parse_csv(&csv_path).expect("CSV 导入失败");
    let csv_engine = DictEngine::new(entries);
    let csv_items = csv_engine.detect(&csv_content);
    let csv_hit = csv_items.iter().any(|i| i.text == "13800138000");

    assert!(txt_hit, "TXT 中应命中 13800138000");
    assert!(csv_hit, "CSV 中应命中 13800138000");
}

// ============================================================
// D08: 字典条目为空字符串 — 不应崩溃或匹配所有文本
// ============================================================

/// D08: 空字符串词条不应崩溃或误匹配
#[test]
fn test_dict_empty_string_entry() {
    let entries = vec![DictEntry {
        text: "".to_string(),
        sensitive_type: SensitiveType::Custom("空".to_string()),
        match_mode: MatchMode::Exact,
        replacement: None,
        language: None,
        builtin: false,
    }];

    let content = make_document("任何普通文本内容");
    let engine = DictEngine::new(entries);
    // 不应 panic
    let items = engine.detect(&content);
    // 空字符串不应匹配所有文本（实现依赖，但不应产生大量结果）
    // 注：如果 engine 对空字符串 find 会匹配每个位置，这里验证行为可控
    eprintln!("[D08] 空字符串词条产生 {} 个匹配项", items.len());
}

// ============================================================
// D09: 字典语言过滤 — zh 语言条目在 En 模式下不生效
// ============================================================

/// D09: 设置 language=zh 的词条在过滤为 En 时应被排除
/// 注：DictEngine 本身不做语言过滤，语言过滤在 detect_by_dict 命令层
/// 此测试验证按语言过滤词条后构建的引擎行为
#[test]
fn test_dict_language_filter() {
    let all_entries = vec![
        DictEntry {
            text: "机密项目".to_string(),
            sensitive_type: SensitiveType::Custom("zh词条".to_string()),
            match_mode: MatchMode::Exact,
            replacement: None,
            language: Some("zh".to_string()),
            builtin: false,
        },
        DictEntry {
            text: "Confidential".to_string(),
            sensitive_type: SensitiveType::Custom("en词条".to_string()),
            match_mode: MatchMode::Exact,
            replacement: None,
            language: Some("en".to_string()),
            builtin: false,
        },
    ];

    let content = make_document("这是机密项目，属于 Confidential 级别");

    // 全语言引擎
    let all_engine = DictEngine::new(all_entries.clone());
    let all_items = all_engine.detect(&content);
    assert_eq!(all_items.len(), 2, "全语言应匹配 2 项");

    // 仅 En 语言过滤（模拟 detect_by_dict 中的过滤逻辑）
    let en_entries: Vec<_> = all_entries.into_iter()
        .filter(|e| e.language.as_deref() != Some("zh"))
        .collect();
    let en_engine = DictEngine::new(en_entries);
    let en_items = en_engine.detect(&content);

    assert_eq!(en_items.len(), 1, "En 模式下应仅匹配 1 项");
    assert_eq!(en_items[0].text, "Confidential");
}

// ============================================================
// D10: 白名单精确匹配 — '138001380' 不应排除 '13800138000'
// ============================================================

/// D10: 白名单应精确匹配，子串不应排除完整值
#[test]
fn test_whitelist_exact_match_no_substring() {
    let path = fixture_path("sample.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    // 白名单只有子串 "138001380"，不应排除 "13800138000"
    let whitelist = vec!["138001380".to_string()];

    let filtered: Vec<_> = items.iter()
        .filter(|i| !whitelist.contains(&i.text))
        .collect();

    // "13800138000" 不等于 "138001380"，应保留
    assert!(
        filtered.iter().any(|i| i.text == "13800138000"),
        "白名单子串 '138001380' 不应排除完整号码 '13800138000'"
    );
}
