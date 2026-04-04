mod common;

use dimkey_lib::commands::file::{export_content, import_file_internal};
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;

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
