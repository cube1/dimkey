mod common;

use dimkey_lib::commands::file::{export_content, import_file_internal};
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::models::task::MappingEntry;

use common::*;

// ============================================================
// B01: 批量导入 — 多个不同格式的文件应能逐个导入
// ============================================================

/// B01: 批量导入 xlsx/csv/docx 三种格式文件
#[test]
fn test_batch_import_all_formats() {
    let files = vec![
        ("batch/batch_01.xlsx", "Excel"),
        ("batch/batch_02.csv", "CSV"),
        ("batch/batch_03.docx", "Word"),
    ];

    for (file, label) in &files {
        let path = fixture_path(file);
        let content = import_file_internal(&path)
            .unwrap_or_else(|e| panic!("{} ({}) 导入失败: {}", label, file, e));

        // 每个文件都应能被正则引擎识别出敏感信息
        let engine = RegexEngine::new();
        let items = engine.detect(&content);
        assert!(
            !items.is_empty(),
            "{} 应识别出敏感信息，实际: 0",
            label
        );

        // 基线覆盖验证
        assert_baseline_from_sidecar(&items, &path);
    }
}

// ============================================================
// B02: 逐个导出 — 每个文件脱敏后能正常导出
// ============================================================

/// B02: 每个 batch 文件导入 → 识别 → 脱敏 → 导出均正常
#[test]
fn test_batch_export_individually() {
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

    let files = vec![
        ("batch/batch_01.xlsx", ".xlsx"),
        ("batch/batch_02.csv", ".csv"),
    ];

    for (file, suffix) in &files {
        let path = fixture_path(file);
        let content = import_file_internal(&path)
            .unwrap_or_else(|e| panic!("{} 导入失败: {}", file, e));
        let engine = RegexEngine::new();
        let items = engine.detect(&content);
        let result = desensitize_content(&content, &items, &strategies);

        // 导出到临时文件
        let tmp = tempfile::Builder::new()
            .suffix(suffix)
            .tempfile()
            .unwrap_or_else(|e| panic!("创建临时文件失败: {}", e));
        let tmp_path = tmp.path().to_str().unwrap();

        export_content(&result.content, tmp_path, None)
            .unwrap_or_else(|e| panic!("{} 导出失败: {}", file, e));

        // 重新导入验证
        let reimported = import_file_internal(tmp_path)
            .unwrap_or_else(|e| panic!("{} 重新导入失败: {}", file, e));

        match &reimported {
            FileContent::Spreadsheet { sheets, .. } => {
                assert!(sheets[0].row_count > 0, "{} 导出后应有数据行", file);
            }
            _ => {}
        }
    }
}

// ============================================================
// B04: 混合格式 — xlsx/csv/docx 三种格式文件全部处理
// ============================================================

/// B04: 混合格式批量处理 — 不同格式的识别结果互不影响
#[test]
fn test_batch_mixed_formats() {
    let files = vec![
        fixture_path("batch/batch_01.xlsx"),
        fixture_path("batch/batch_02.csv"),
        fixture_path("batch/batch_03.docx"),
    ];

    let engine = RegexEngine::new();
    let mut total_items = 0;

    for path in &files {
        let content = import_file_internal(path)
            .unwrap_or_else(|e| panic!("{} 导入失败: {}", path, e));
        let items = engine.detect(&content);
        total_items += items.len();
    }

    assert!(
        total_items > 0,
        "三个文件合计应识别出敏感信息，实际: 0"
    );
}

// ============================================================
// B03: 中途跳过 — 处理第 1 个文件后回到 dropzone，剩余文件保持 pending
//
// 该用例的语义聚焦前端 batch session 状态机（fileQueue + batchAutoStore），
// 后端 API 无对应"中断恢复"概念。此处用一个最小契约测试占位：
// 批量处理时若仅消费前 N 个文件，剩余文件不应被 desensitize（无 mappings 产生）。
// ============================================================

#[test]
#[ignore = "B03 中途跳过为前端 batch session 状态机行为，后端 API 无对应概念。完整 e2e 验证应在 pytest 中实现"]
fn test_b03_batch_partial_skip_remaining_pending() {
    let files = vec![
        fixture_path("batch/batch_01.xlsx"),
        fixture_path("batch/batch_02.csv"),
        fixture_path("batch/batch_03.docx"),
    ];

    let engine = RegexEngine::new();
    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Phone,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];

    // 仅处理第 1 个文件
    let path0 = &files[0];
    let content0 = import_file_internal(path0).expect("导入失败");
    let items0 = engine.detect(&content0);
    let result0 = desensitize_content(&content0, &items0, &strategies);
    assert!(!result0.mappings.is_empty(), "第 1 个文件应有 mappings");

    // 剩余文件未走 desensitize → 这里只验证它们仍可独立 import（仍处于 pending 等价状态）
    for path in &files[1..] {
        let content = import_file_internal(path).expect("导入失败");
        match content {
            FileContent::Spreadsheet { sheets, .. } => {
                assert!(!sheets.is_empty(), "{} 应有 sheets", path);
            }
            FileContent::Document { paragraphs, .. } => {
                assert!(!paragraphs.is_empty(), "{} 应有段落", path);
            }
        }
    }
}

// ============================================================
// B06: 批量脱敏 — 多个英文文档批处理后 NER 实体应被替换
// 覆盖用户报告的"批量没替换"bug — 验证 NER 替换在批量管道中真实发生
// ============================================================

#[test]
fn test_b06_batch_english_ner_replace_actually_happens() {
    use dimkey_lib::models::language::Language;

    let files = vec![
        fixture_path("scenarios/docx/attorney_engagement_letter.docx"),
        fixture_path("scenarios/xlsx/law_firm_client_intake.xlsx"),
    ];

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

    for path in &files {
        let content = parse_fixture(path);
        let items = detect_full_pipeline(&content, Language::En);

        let target_items: Vec<SensitiveItem> = items
            .iter()
            .filter(|i| ner_types.contains(&i.sensitive_type))
            .cloned()
            .collect();
        assert!(
            !target_items.is_empty(),
            "{} 应识别到至少一个 NER 实体（PersonName/OrgName/Address/Title）",
            path
        );

        let result = desensitize_content(&content, &target_items, &strategies);

        // 契约 1：mappings 至少含一条 PersonName
        let person_mappings: Vec<&MappingEntry> = result
            .mappings
            .iter()
            .filter(|m| m.sensitive_type == SensitiveType::PersonName)
            .collect();
        assert!(
            !person_mappings.is_empty(),
            "{} 批量脱敏后 mappings 应至少含一条 PersonName 类型映射 — 否则即批量未替换 bug",
            path
        );

        // 契约 2：每个 PersonName 替换值含空格、纯 ASCII、不含汉字（英文 Fake 形态）
        for m in &person_mappings {
            assert!(
                !m.replaced_text.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c)),
                "{} 中 PersonName 替换值不应含汉字: '{}' → '{}'",
                path, m.original_text, m.replaced_text
            );
            assert!(
                m.replaced_text.contains(' '),
                "{} 中 PersonName 替换值应含空格: '{}' → '{}'",
                path, m.original_text, m.replaced_text
            );
            assert_ne!(
                m.replaced_text, m.original_text,
                "{} 中 PersonName 替换值不应等于原文",
                path
            );
        }

        // 契约 3：脱敏后文档不再包含 mappings 中任一 PersonName 原文
        // 仅检查实际进入 mappings 的实体（即 NER 识别且发生了替换）—
        // 避免 NER 识别短子串（如 "Sc"）造成的 contains 误报
        let after_text: String = match &result.content {
            FileContent::Document { paragraphs, .. } => {
                paragraphs.iter().map(|p| p.text.clone()).collect::<Vec<_>>().join(" ")
            }
            FileContent::Spreadsheet { sheets, .. } => sheets
                .iter()
                .flat_map(|s| s.rows.iter().flat_map(|r| r.iter().map(|c| c.text.clone())))
                .collect::<Vec<_>>()
                .join(" "),
        };
        // 只检查含空格的 full-name PersonName（过滤 NER 单 token 噪声实体）
        // 单 token 名（如 "Anderson"）可能是 NER 在不同位置独立识别的子实体，
        // 与其他 full-name 实体的 token 重合，contains 检查无法区分
        for m in &person_mappings {
            if !m.original_text.contains(' ') {
                continue;
            }
            assert!(
                !after_text.contains(m.original_text.as_str()),
                "{} 批量脱敏后文档仍包含已 mapping 的 full-name PersonName '{}' — 替换未真正发生",
                path, m.original_text
            );
        }
    }
}
