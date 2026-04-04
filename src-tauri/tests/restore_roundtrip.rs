mod common;

use dimkey_lib::commands::file::{export_content, import_file_internal};
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::models::task::*;

use common::*;

/// 构建全 Replace 策略（用于往返测试，因为 Replace 可逆）
fn all_replace_strategies() -> Vec<StrategyConfig> {
    vec![
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
        StrategyConfig {
            sensitive_type: SensitiveType::BankCard,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::IpAddress,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::LandlinePhone,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::LicensePlate,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::CreditCode,
            strategy: Strategy::Replace { style: ReplaceStyle::Fake },
            consistent: true,
        },
    ]
}

/// CSV 往返测试：导入 → 识别 → 脱敏(Replace) → 导出 → 重新导入 → 还原 → 与原文逐格对比
#[test]
fn test_roundtrip_csv() {
    let path = test_data_path("员工信息表.csv");
    let original = import_file_internal(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&original);

    let strategies = all_replace_strategies();
    let result = desensitize_content(&original, &items, &strategies);

    // 导出到临时文件
    let tmp = tempfile::Builder::new()
        .suffix(".csv")
        .tempfile()
        .expect("创建临时文件失败");
    let tmp_path = tmp.path().to_str().unwrap();
    export_content(&result.content, tmp_path, None).expect("CSV 导出失败");

    // 重新导入
    let reimported = import_file_internal(tmp_path).expect("重新导入失败");
    let mut restored = reimported;

    // 使用映射还原
    let restored_count = restore_from_mappings(&mut restored, &result.mappings);
    assert!(restored_count > 0, "应有还原操作发生");

    // 逐格对比
    let original_rows = get_rows(&original);
    let restored_rows = get_rows(&restored);

    assert_eq!(original_rows.len(), restored_rows.len(), "行数应一致");
    for (row_idx, (orig_row, rest_row)) in
        original_rows.iter().zip(restored_rows.iter()).enumerate()
    {
        for (col_idx, (orig_cell, rest_cell)) in
            orig_row.iter().zip(rest_row.iter()).enumerate()
        {
            assert_eq!(
                orig_cell, rest_cell,
                "第 {} 行第 {} 列不一致: 原文='{}', 还原='{}'",
                row_idx + 1,
                col_idx + 1,
                orig_cell,
                rest_cell
            );
        }
    }
}

/// Excel 往返测试
#[test]
fn test_roundtrip_xlsx() {
    let path = test_data_path("员工花名册.xlsx");
    let original = import_file_internal(&path).expect("Excel 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&original);

    let strategies = all_replace_strategies();
    let result = desensitize_content(&original, &items, &strategies);

    // 导出到临时 xlsx 文件
    let tmp = tempfile::Builder::new()
        .suffix(".xlsx")
        .tempfile()
        .expect("创建临时文件失败");
    let tmp_path = tmp.path().to_str().unwrap();
    export_content(&result.content, tmp_path, None).expect("Excel 导出失败");

    // 重新导入
    let reimported = import_file_internal(tmp_path).expect("重新导入失败");
    let mut restored = reimported;

    let restored_count = restore_from_mappings(&mut restored, &result.mappings);
    assert!(restored_count > 0, "应有还原操作发生");

    // 逐格对比
    let original_rows = get_rows(&original);
    let restored_rows = get_rows(&restored);

    assert_eq!(original_rows.len(), restored_rows.len(), "行数应一致");
    for (row_idx, (orig_row, rest_row)) in
        original_rows.iter().zip(restored_rows.iter()).enumerate()
    {
        for (col_idx, (orig_cell, rest_cell)) in
            orig_row.iter().zip(rest_row.iter()).enumerate()
        {
            assert_eq!(
                orig_cell, rest_cell,
                "第 {} 行第 {} 列不一致: 原文='{}', 还原='{}'",
                row_idx + 1,
                col_idx + 1,
                orig_cell,
                rest_cell
            );
        }
    }
}

/// Word 往返测试
#[test]
fn test_roundtrip_docx() {
    let path = test_data_path("客户调研报告.docx");
    let original = import_file_internal(&path).expect("Word 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&original);

    if items.is_empty() {
        return; // 没有识别到敏感项则跳过
    }

    let strategies = all_replace_strategies();
    let result = desensitize_content(&original, &items, &strategies);

    // Word 导出需要 original_path 作为模板
    let tmp = tempfile::Builder::new()
        .suffix(".docx")
        .tempfile()
        .expect("创建临时文件失败");
    let tmp_path = tmp.path().to_str().unwrap();
    export_content(&result.content, tmp_path, Some(&path)).expect("Word 导出失败");

    // 重新导入
    let reimported = import_file_internal(tmp_path).expect("重新导入失败");
    let mut restored = reimported;

    let restored_count = restore_from_mappings(&mut restored, &result.mappings);
    assert!(restored_count > 0, "应有还原操作发生");

    // 逐段落对比
    let original_paragraphs = get_paragraphs(&original);
    let restored_paragraphs = get_paragraphs(&restored);

    assert_eq!(
        original_paragraphs.len(),
        restored_paragraphs.len(),
        "段落数量应一致"
    );
    for (i, (orig, rest)) in original_paragraphs
        .iter()
        .zip(restored_paragraphs.iter())
        .enumerate()
    {
        assert_eq!(
            orig.text, rest.text,
            "第 {} 段落不一致: 原文='{}', 还原='{}'",
            i, orig.text, rest.text
        );
    }
}

/// 第二个 CSV 文件的往返测试（验证通用性）
#[test]
fn test_roundtrip_csv_customer() {
    let path = test_data_path("客户通讯录.csv");
    let original = import_file_internal(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&original);

    let strategies = all_replace_strategies();
    let result = desensitize_content(&original, &items, &strategies);

    let tmp = tempfile::Builder::new()
        .suffix(".csv")
        .tempfile()
        .expect("创建临时文件失败");
    let tmp_path = tmp.path().to_str().unwrap();
    export_content(&result.content, tmp_path, None).expect("CSV 导出失败");

    let reimported = import_file_internal(tmp_path).expect("重新导入失败");
    let mut restored = reimported;

    let restored_count = restore_from_mappings(&mut restored, &result.mappings);
    assert!(restored_count > 0, "应有还原操作发生");

    let original_rows = get_rows(&original);
    let restored_rows = get_rows(&restored);

    assert_eq!(original_rows.len(), restored_rows.len(), "行数应一致");
    for (row_idx, (orig_row, rest_row)) in
        original_rows.iter().zip(restored_rows.iter()).enumerate()
    {
        for (col_idx, (orig_cell, rest_cell)) in
            orig_row.iter().zip(rest_row.iter()).enumerate()
        {
            assert_eq!(
                orig_cell, rest_cell,
                "第 {} 行第 {} 列不一致: 原文='{}', 还原='{}'",
                row_idx + 1,
                col_idx + 1,
                orig_cell,
                rest_cell
            );
        }
    }
}

/// 行业场景 Excel 的往返测试
#[test]
fn test_roundtrip_xlsx_law() {
    let path = test_data_path("律所案件登记表.xlsx");
    let original = import_file_internal(&path).expect("Excel 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&original);

    let strategies = all_replace_strategies();
    let result = desensitize_content(&original, &items, &strategies);

    let tmp = tempfile::Builder::new()
        .suffix(".xlsx")
        .tempfile()
        .expect("创建临时文件失败");
    let tmp_path = tmp.path().to_str().unwrap();
    export_content(&result.content, tmp_path, None).expect("Excel 导出失败");

    let reimported = import_file_internal(tmp_path).expect("重新导入失败");
    let mut restored = reimported;

    let restored_count = restore_from_mappings(&mut restored, &result.mappings);
    assert!(restored_count > 0, "应有还原操作发生");

    let original_rows = get_rows(&original);
    let restored_rows = get_rows(&restored);

    assert_eq!(original_rows.len(), restored_rows.len(), "行数应一致");
    for (row_idx, (orig_row, rest_row)) in
        original_rows.iter().zip(restored_rows.iter()).enumerate()
    {
        for (col_idx, (orig_cell, rest_cell)) in
            orig_row.iter().zip(rest_row.iter()).enumerate()
        {
            assert_eq!(
                orig_cell, rest_cell,
                "第 {} 行第 {} 列不一致: 原文='{}', 还原='{}'",
                row_idx + 1,
                col_idx + 1,
                orig_cell,
                rest_cell
            );
        }
    }
}

/// TXT 文件 Replace 策略往返测试（R05）
#[test]
fn test_roundtrip_txt() {
    let path = test_data_path("会议纪要.txt");
    let original = import_file_internal(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&original);

    assert!(!items.is_empty(), "TXT 应检测到敏感信息");

    let strategies = all_replace_strategies();
    let result = desensitize_content(&original, &items, &strategies);

    // 验证脱敏后内容与原文不同
    let orig_paras = get_paragraphs(&original);
    let new_paras = get_paragraphs(&result.content);
    let has_change = orig_paras.iter().zip(new_paras.iter())
        .any(|(o, n)| o.text != n.text);
    assert!(has_change, "脱敏后应有段落发生变化");

    // 使用映射还原
    let mut restored = result.content.clone();
    let restored_count = restore_from_mappings(&mut restored, &result.mappings);
    assert!(restored_count > 0, "应有还原操作发生");

    // 逐段落对比
    let restored_paras = get_paragraphs(&restored);
    for (i, (orig, rest)) in orig_paras.iter().zip(restored_paras.iter()).enumerate() {
        assert_eq!(
            orig.text, rest.text,
            "第 {} 段落不一致: 原文='{}', 还原='{}'",
            i, orig.text, rest.text
        );
    }
}

/// Generalize 策略不可逆验证（R06）
#[test]
fn test_generalize_not_reversible() {
    let path = test_data_path("员工信息表.csv");
    let original = import_file_internal(&path).expect("CSV 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&original);

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

    let phone_items: Vec<_> = items.into_iter()
        .filter(|i| matches!(i.sensitive_type, SensitiveType::Phone | SensitiveType::IdCard))
        .collect();
    let result = desensitize_content(&original, &phone_items, &strategies);

    // Generalize 映射的 strategy 应为 Generalize
    for mapping in &result.mappings {
        assert_eq!(mapping.strategy, StrategyType::Generalize);
    }

    // 尝试还原 — Generalize 不是 Replace，restore_from_mappings 应不还原
    let mut content = result.content.clone();
    let restored_count = restore_from_mappings(&mut content, &result.mappings);
    assert_eq!(restored_count, 0, "Generalize 策略不应可还原");
}

/// R03: Mask 策略不可逆验证 — Mask 后 restore_from_mappings 应不还原
#[test]
fn test_mask_not_reversible() {
    let path = fixture_path("sample.txt");
    let original = import_file_internal(&path).expect("TXT 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&original);

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
    ];

    let phone_and_id: Vec<_> = items.into_iter()
        .filter(|i| matches!(i.sensitive_type, SensitiveType::Phone | SensitiveType::IdCard))
        .collect();

    let result = desensitize_content(&original, &phone_and_id, &strategies);

    // 验证所有映射都是 Mask
    for mapping in &result.mappings {
        assert_eq!(mapping.strategy, StrategyType::Mask);
    }

    // 尝试还原 — Mask 不是 Replace，不应被还原
    let mut content = result.content.clone();
    let restored_count = restore_from_mappings(&mut content, &result.mappings);
    assert_eq!(restored_count, 0, "Mask 策略不应可还原");
}
