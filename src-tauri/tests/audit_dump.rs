//! 人工审查辅助测试：打印脱敏前后的完整对比，供肉眼抽查
//! 运行方式：cd src-tauri && cargo test --test audit_dump -- --nocapture

mod common;

use dimkey_lib::commands::file::{export_content, import_file_internal};
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;

use common::*;

/// 打印表格的前 N 行（原始 vs 脱敏后 vs 还原后）
fn print_spreadsheet_comparison(
    label: &str,
    original: &FileContent,
    desensitized: &FileContent,
    restored: &FileContent,
    max_rows: usize,
) {
    let orig_headers = get_headers(original);
    let orig_rows = get_rows(original);
    let desen_rows = get_rows(desensitized);
    let rest_rows = get_rows(restored);

    println!("\n======== {} ========", label);
    println!("表头: {:?}", orig_headers);
    println!("总行数: {}", orig_rows.len());
    println!();

    let display_rows = max_rows.min(orig_rows.len());
    for row_idx in 0..display_rows {
        println!("--- 第 {} 行 ---", row_idx + 1);
        for col_idx in 0..orig_headers.len() {
            let orig = orig_rows[row_idx].get(col_idx).map(|cv| cv.text.as_str()).unwrap_or("");
            let desen = desen_rows[row_idx].get(col_idx).map(|cv| cv.text.as_str()).unwrap_or("");
            let rest = rest_rows[row_idx].get(col_idx).map(|cv| cv.text.as_str()).unwrap_or("");

            let changed = if orig != desen { " [已脱敏]" } else { "" };
            let restored_ok = if orig == rest { "OK" } else { "MISMATCH!" };

            println!(
                "  [{}]{}\n    原始:   {}\n    脱敏后: {}\n    还原后: {} [{}]",
                orig_headers[col_idx], changed, orig, desen, rest, restored_ok
            );
        }
        println!();
    }
    if orig_rows.len() > display_rows {
        println!("  ... 省略 {} 行", orig_rows.len() - display_rows);
    }
}

/// 打印映射表
fn print_mappings(mappings: &[dimkey_lib::models::task::MappingEntry]) {
    println!("\n映射表（共 {} 条）:", mappings.len());
    println!(
        "  {:<20} {:<20} {:<15} {:<10} {}",
        "原文", "替换为", "类型", "策略", "次数"
    );
    println!("  {}", "-".repeat(80));
    for m in mappings {
        println!(
            "  {:<20} {:<20} {:<15?} {:<10?} {}",
            m.original_text, m.replaced_text, m.sensitive_type, m.strategy, m.occurrences
        );
    }
}

/// 【CSV 审查】员工信息表 - 全量 Replace 策略的往返对比
#[test]
fn audit_csv_roundtrip() {
    let path = test_data_path("员工信息表.csv");
    let original = import_file_internal(&path).expect("导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&original);

    // 打印识别结果统计
    println!("\n===== 员工信息表.csv 识别结果 =====");
    println!("总识别数: {}", items.len());
    let mut type_counts = std::collections::HashMap::new();
    for item in &items {
        *type_counts
            .entry(format!("{:?}", item.sensitive_type))
            .or_insert(0usize) += 1;
    }
    for (t, c) in &type_counts {
        println!("  {}: {} 个", t, c);
    }

    // 打印前 3 行的识别详情
    println!("\n前 3 行识别详情:");
    for row in 1..=3 {
        let row_items: Vec<_> = items.iter().filter(|i| i.row == row).collect();
        println!("  第 {} 行: {} 个识别项", row, row_items.len());
        for item in &row_items {
            println!(
                "    col={} '{}' -> {:?} (start={}, end={})",
                item.col, item.text, item.sensitive_type, item.start, item.end
            );
        }
    }

    // 脱敏
    let strategies: Vec<StrategyConfig> = vec![
        SensitiveType::Phone,
        SensitiveType::IdCard,
        SensitiveType::Email,
        SensitiveType::BankCard,
    ]
    .into_iter()
    .map(|st| StrategyConfig {
        sensitive_type: st,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    })
    .collect();

    let result = desensitize_content(&original, &items, &strategies);
    print_mappings(&result.mappings);

    // 导出 → 重新导入 → 还原
    let tmp = tempfile::Builder::new()
        .suffix(".csv")
        .tempfile()
        .expect("创建临时文件失败");
    let tmp_path = tmp.path().to_str().unwrap();
    export_content(&result.content, tmp_path, None, None).expect("导出失败");

    let reimported = import_file_internal(tmp_path).expect("重新导入失败");
    let mut restored = reimported.clone();
    let restored_count = restore_from_mappings(&mut restored, &result.mappings);
    println!("\n还原替换次数: {}", restored_count);

    // 打印对比（前 5 行）
    print_spreadsheet_comparison("员工信息表.csv 往返对比", &original, &result.content, &restored, 5);

    // 统计不一致的单元格
    let orig_rows = get_rows(&original);
    let rest_rows = get_rows(&restored);
    let mut mismatch_count = 0;
    for (r, (orig_row, rest_row)) in orig_rows.iter().zip(rest_rows.iter()).enumerate() {
        for (c, (orig_cell, rest_cell)) in orig_row.iter().zip(rest_row.iter()).enumerate() {
            if orig_cell.text != rest_cell.text {
                mismatch_count += 1;
                println!(
                    "!! 不一致: 行{} 列{}: 原文='{}' 还原='{}'",
                    r + 1,
                    c + 1,
                    orig_cell.text,
                    rest_cell.text
                );
            }
        }
    }
    println!("\n总不一致单元格数: {}", mismatch_count);
    assert_eq!(mismatch_count, 0, "存在未正确还原的单元格");
}

/// 【Excel 审查】员工花名册 - Mask 策略的掩码效果
#[test]
fn audit_xlsx_mask_effect() {
    let path = test_data_path("员工花名册.xlsx");
    let original = import_file_internal(&path).expect("导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&original);

    println!("\n===== 员工花名册.xlsx Mask 效果审查 =====");
    println!("总识别数: {}", items.len());

    let strategies = vec![
        StrategyConfig {
            sensitive_type: SensitiveType::Phone,
            strategy: Strategy::Mask {
                keep_prefix: 3,
                keep_suffix: 4,
            },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::IdCard,
            strategy: Strategy::Mask {
                keep_prefix: 6,
                keep_suffix: 4,
            },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::Email,
            strategy: Strategy::Mask {
                keep_prefix: 1,
                keep_suffix: 0,
            },
            consistent: true,
        },
        StrategyConfig {
            sensitive_type: SensitiveType::BankCard,
            strategy: Strategy::Mask {
                keep_prefix: 4,
                keep_suffix: 4,
            },
            consistent: true,
        },
    ];

    let result = desensitize_content(&original, &items, &strategies);

    println!("\n掩码映射:");
    for m in &result.mappings {
        println!("  {} -> {} ({:?})", m.original_text, m.replaced_text, m.sensitive_type);
    }

    // 打印前 3 行逐列对比
    let orig_headers = get_headers(&original);
    let orig_rows = get_rows(&original);
    let desen_rows = get_rows(&result.content);

    println!("\n前 3 行掩码前后对比:");
    for row_idx in 0..3.min(orig_rows.len()) {
        println!("--- 第 {} 行 ---", row_idx + 1);
        for col_idx in 0..orig_headers.len() {
            let orig = &orig_rows[row_idx][col_idx].text;
            let desen = &desen_rows[row_idx][col_idx].text;
            if orig != desen {
                println!("  [{}] {} -> {}", orig_headers[col_idx], orig, desen);
            }
        }
    }
}

/// 【Word 审查】客户调研报告 - Replace 效果
#[test]
fn audit_docx_replace_effect() {
    let path = test_data_path("客户调研报告.docx");
    let original = import_file_internal(&path).expect("导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&original);

    println!("\n===== 客户调研报告.docx Replace 效果审查 =====");
    println!("段落数: {}", get_paragraphs(&original).len());
    println!("识别数: {}", items.len());

    for item in &items {
        println!(
            "  段落{}: '{}' -> {:?} (位置 {}..{})",
            item.row, item.text, item.sensitive_type, item.start, item.end
        );
    }

    if items.is_empty() {
        println!("无识别项，跳过脱敏");
        return;
    }

    let strategies: Vec<StrategyConfig> = vec![
        SensitiveType::Phone,
        SensitiveType::IdCard,
        SensitiveType::Email,
        SensitiveType::BankCard,
    ]
    .into_iter()
    .map(|st| StrategyConfig {
        sensitive_type: st,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    })
    .collect();

    let result = desensitize_content(&original, &items, &strategies);
    print_mappings(&result.mappings);

    // 打印有变化的段落
    let orig_paras = get_paragraphs(&original);
    let desen_paras = get_paragraphs(&result.content);

    println!("\n有变化的段落:");
    for (orig, desen) in orig_paras.iter().zip(desen_paras.iter()) {
        if orig.text != desen.text {
            println!("  段落 {} [{}]:", orig.index, orig.style);
            println!("    原始: {}", orig.text);
            println!("    脱敏: {}", desen.text);
        }
    }
}
