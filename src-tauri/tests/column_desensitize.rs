mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::models::task::StrategyType;
use dimkey_lib::parser::excel::parse_csv;

use common::*;

/// 测试列级类型推断（使用 RegexEngine 对每列采样检测）
#[test]
fn test_column_inference() {
    let path = test_data_path("员工信息表.csv");
    let content = parse_csv(&path).expect("CSV 导入失败");

    let (headers, rows) = match &content {
        FileContent::Spreadsheet {
            sheets, ..
        } => (&sheets[0].headers, &sheets[0].rows),
        _ => panic!("期望 Spreadsheet 类型"),
    };

    let engine = RegexEngine::new();

    // 对每列做类型推断
    for col_idx in 0..headers.len() {
        let mut type_hits = std::collections::HashMap::new();
        let mut hit_count = 0usize;
        let sample_total = rows.len();

        for (row_idx, row) in rows.iter().enumerate() {
            let cell = match row.get(col_idx) {
                Some(cv) if !cv.text.is_empty() => &cv.text,
                _ => continue,
            };

            let items = engine.detect_text(cell, row_idx + 1, col_idx);
            if items.is_empty() {
                continue;
            }

            let cell_chars = cell.chars().count();
            let covered_chars: usize = items.iter().map(|i| i.end - i.start).sum();
            if covered_chars * 2 >= cell_chars {
                hit_count += 1;
                // 取覆盖最多字符的类型
                let mut type_coverage = std::collections::HashMap::new();
                for item in &items {
                    use dimkey_lib::commands::desensitize::sensitive_type_to_key;
                    let key = sensitive_type_to_key(&item.sensitive_type);
                    *type_coverage.entry(key).or_insert(0usize) += item.end - item.start;
                }
                if let Some((best_type, _)) = type_coverage.into_iter().max_by_key(|(_, v)| *v) {
                    *type_hits.entry(best_type).or_insert(0usize) += 1;
                }
            }
        }

        let confidence = if sample_total > 0 {
            hit_count as f64 / sample_total as f64
        } else {
            0.0
        };

        let header = &headers[col_idx];
        match header.as_str() {
            "手机号" => {
                assert!(
                    confidence >= 0.8,
                    "手机号列置信度应 >= 0.8，实际: {:.2}",
                    confidence
                );
                assert!(
                    type_hits.get("Phone").copied().unwrap_or(0) > 0,
                    "手机号列应推断为 Phone"
                );
            }
            "身份证号" => {
                assert!(
                    confidence >= 0.8,
                    "身份证号列置信度应 >= 0.8，实际: {:.2}",
                    confidence
                );
                assert!(
                    type_hits.get("IdCard").copied().unwrap_or(0) > 0,
                    "身份证号列应推断为 IdCard"
                );
            }
            "邮箱" => {
                assert!(
                    confidence >= 0.8,
                    "邮箱列置信度应 >= 0.8，实际: {:.2}",
                    confidence
                );
                assert!(
                    type_hits.get("Email").copied().unwrap_or(0) > 0,
                    "邮箱列应推断为 Email"
                );
            }
            "银行卡号" => {
                assert!(
                    confidence >= 0.8,
                    "银行卡号列置信度应 >= 0.8，实际: {:.2}",
                    confidence
                );
                assert!(
                    type_hits.get("BankCard").copied().unwrap_or(0) > 0,
                    "银行卡号列应推断为 BankCard"
                );
            }
            "姓名" | "所属公司" | "家庭住址" => {
                // 这些列正则引擎识别不了（需要 NER），不做强制断言
            }
            _ => {}
        }
    }
}

/// 测试列级脱敏只影响目标列，不影响其他列
#[test]
fn test_column_desensitize_only_target_col() {
    let content = FileContent::Spreadsheet {
        file_name: "test.csv".to_string(),
        file_type: FileType::Csv,
        sheets: vec![SheetData {
            name: String::new(),
            headers: vec!["工号".into(), "手机号".into(), "备注".into()],
            rows: vec![
                vec!["EMP001".into(), "13800138001".into(), "无".into()],
                vec!["EMP002".into(), "15912345678".into(), "VIP".into()],
                vec!["EMP003".into(), "18676543210".into(), "无".into()],
            ],
            row_count: 3,
            col_count: 3,
        }],
    };

    // 只对 col=1（手机号列）做脱敏
    let items: Vec<SensitiveItem> = (0..3)
        .map(|i| {
            let phone = match i {
                0 => "13800138001",
                1 => "15912345678",
                _ => "18676543210",
            };
            SensitiveItem {
                id: format!("{}", i + 1),
                text: phone.to_string(),
                sensitive_type: SensitiveType::Phone,
                source: DetectSource::Regex,
            pdf_bboxes: None,
                confidence: 0.95,
                start: 0,
                end: 11,
                row: i + 1,
                col: 1,
                sheet_index: 0,
            }
        })
        .collect();

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::Phone,
        strategy: Strategy::Mask {
            keep_prefix: 3,
            keep_suffix: 4,
        },
        consistent: true,
    }];

    let result = desensitize_content(&content, &items, &strategies);
    let rows = get_rows(&result.content);

    // col=0（工号）不变
    assert_eq!(rows[0][0], "EMP001");
    assert_eq!(rows[1][0], "EMP002");
    assert_eq!(rows[2][0], "EMP003");

    // col=1（手机号）已脱敏
    assert_eq!(rows[0][1], "138****8001");
    assert_eq!(rows[1][1], "159****5678");
    assert_eq!(rows[2][1], "186****3210");

    // col=2（备注）不变
    assert_eq!(rows[0][2], "无");
    assert_eq!(rows[1][2], "VIP");
    assert_eq!(rows[2][2], "无");
}

/// 测试列级 Replace 脱敏生成正确的映射记录
#[test]
fn test_column_desensitize_generates_mappings() {
    let content = FileContent::Spreadsheet {
        file_name: "test.csv".to_string(),
        file_type: FileType::Csv,
        sheets: vec![SheetData {
            name: String::new(),
            headers: vec!["姓名".into()],
            rows: vec![
                vec!["张三".into()],
                vec!["李四".into()],
                vec!["张三".into()], // 重复
            ],
            row_count: 3,
            col_count: 1,
        }],
    };

    let items = vec![
        SensitiveItem {
            id: "1".into(),
            text: "张三".into(),
            sensitive_type: SensitiveType::PersonName,
            source: DetectSource::Regex,
            pdf_bboxes: None,
            confidence: 0.95,
            start: 0,
            end: 2,
            row: 1,
            col: 0,
            sheet_index: 0,
        },
        SensitiveItem {
            id: "2".into(),
            text: "李四".into(),
            sensitive_type: SensitiveType::PersonName,
            source: DetectSource::Regex,
            pdf_bboxes: None,
            confidence: 0.95,
            start: 0,
            end: 2,
            row: 2,
            col: 0,
            sheet_index: 0,
        },
        SensitiveItem {
            id: "3".into(),
            text: "张三".into(),
            sensitive_type: SensitiveType::PersonName,
            source: DetectSource::Regex,
            pdf_bboxes: None,
            confidence: 0.95,
            start: 0,
            end: 2,
            row: 3,
            col: 0,
            sheet_index: 0,
        },
    ];

    let strategies = vec![StrategyConfig {
        sensitive_type: SensitiveType::PersonName,
        strategy: Strategy::Replace { style: ReplaceStyle::Fake },
        consistent: true,
    }];

    let result = desensitize_content(&content, &items, &strategies);

    // 应有 2 条映射（张三、李四去重）
    assert_eq!(result.mappings.len(), 2, "应有 2 条去重映射");

    // "张三" 的映射 occurrences 应为 2
    let zhangsan_mapping = result
        .mappings
        .iter()
        .find(|m| m.original_text == "张三")
        .expect("应有 '张三' 的映射");
    assert_eq!(zhangsan_mapping.occurrences, 2, "张三出现 2 次");
    assert_eq!(zhangsan_mapping.strategy, StrategyType::Replace);
    assert_ne!(zhangsan_mapping.replaced_text, "张三");

    // "李四" 的映射 occurrences 应为 1
    let lisi_mapping = result
        .mappings
        .iter()
        .find(|m| m.original_text == "李四")
        .expect("应有 '李四' 的映射");
    assert_eq!(lisi_mapping.occurrences, 1, "李四出现 1 次");
}
