#![allow(dead_code)]

use std::collections::HashMap;

use dimkey_lib::commands::desensitize::sensitive_type_to_key;
use dimkey_lib::desensitizer::{generalize, mask, replace};
use dimkey_lib::desensitizer::replace::ReplaceState;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;
use dimkey_lib::models::task::*;

/// 获取 test-data 目录下的文件路径
pub fn test_data_path(filename: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/../test-data/{}", manifest_dir, filename)
}

/// 统计识别结果中某种类型的数量
pub fn count_by_type(items: &[SensitiveItem], st: &SensitiveType) -> usize {
    items.iter().filter(|i| &i.sensitive_type == st).count()
}

/// 在文本中替换敏感项（从后往前替换，避免偏移问题）
fn replace_in_text(
    text: &str,
    items: &[&SensitiveItem],
    consistency_map: &HashMap<(String, SensitiveType), (String, StrategyType)>,
) -> String {
    let mut sorted: Vec<&SensitiveItem> = items.to_vec();
    sorted.sort_by_key(|i| i.start);

    let mut non_overlapping: Vec<&SensitiveItem> = Vec::new();
    let mut last_end = 0usize;
    for item in &sorted {
        if item.start >= last_end {
            non_overlapping.push(item);
            last_end = item.end;
        }
    }

    non_overlapping.sort_by(|a, b| b.start.cmp(&a.start));

    let mut chars: Vec<char> = text.chars().collect();
    for item in non_overlapping {
        let key = (item.text.clone(), item.sensitive_type.clone());
        if let Some((replacement, _)) = consistency_map.get(&key) {
            let end = item.end.min(chars.len());
            let start = item.start.min(end);
            let replacement_chars: Vec<char> = replacement.chars().collect();
            chars.splice(start..end, replacement_chars);
        }
    }

    chars.into_iter().collect()
}

/// 对 FileContent 执行脱敏（不依赖 app_handle，纯内存操作）
/// 复制自 commands/desensitize.rs 核心逻辑，去除 workspace 持久化部分
pub fn desensitize_content(
    content: &FileContent,
    items: &[SensitiveItem],
    strategies: &[StrategyConfig],
) -> DesensitizeResult {
    // 1. 构建策略查找表
    let strategy_map: HashMap<SensitiveType, (Strategy, bool)> = strategies
        .iter()
        .map(|s| (s.sensitive_type.clone(), (s.strategy.clone(), s.consistent)))
        .collect();

    // 2. 构建一致性映射
    let mut consistency_map: HashMap<(String, SensitiveType), (String, StrategyType)> =
        HashMap::new();
    let mut replace_state = ReplaceState::new(42, HashMap::new());

    for item in items {
        let key = (item.text.clone(), item.sensitive_type.clone());
        let (strategy, consistent) = strategy_map
            .get(&item.sensitive_type)
            .cloned()
            .unwrap_or((
                Strategy::Mask {
                    keep_prefix: 1,
                    keep_suffix: 1,
                },
                true,
            ));

        if consistent && consistency_map.contains_key(&key) {
            continue;
        }

        let (replaced, st_type) = match &strategy {
            Strategy::Mask {
                keep_prefix,
                keep_suffix,
            } => {
                let r =
                    mask::apply_mask(&item.text, &item.sensitive_type, *keep_prefix, *keep_suffix);
                (r, StrategyType::Mask)
            }
            Strategy::Replace { ref style } => {
                let r = replace::apply_replace(&item.text, &item.sensitive_type, &mut replace_state, style);
                (r, StrategyType::Replace)
            }
            Strategy::Generalize => {
                let r = generalize::apply_generalize(&item.text, &item.sensitive_type);
                (r, StrategyType::Generalize)
            }
        };

        consistency_map.insert(key, (replaced, st_type));
    }

    // 3. 克隆内容并替换
    let mut new_content = content.clone();
    match &mut new_content {
        FileContent::Spreadsheet {
            sheets, ..
        } => {
            let mut cell_items: HashMap<(usize, usize, usize), Vec<&SensitiveItem>> = HashMap::new();
            for item in items {
                cell_items
                    .entry((item.sheet_index, item.row, item.col))
                    .or_default()
                    .push(item);
            }

            for ((sheet_idx, row, col), ref cell_items) in &cell_items {
                if let Some(sheet) = sheets.get_mut(*sheet_idx) {
                    if *row == 0 {
                        if let Some(header) = sheet.headers.get_mut(*col) {
                            *header = replace_in_text(header, cell_items, &consistency_map);
                        }
                    } else {
                        if let Some(cell_value) = sheet.rows.get_mut(row - 1).and_then(|r| r.get_mut(*col)) {
                            cell_value.text = replace_in_text(&cell_value.text, cell_items, &consistency_map);
                            cell_value.cell_type = CellType::Text;
                        }
                    }
                }
            }
        }
        FileContent::Document { paragraphs, .. } => {
            let mut para_items: HashMap<usize, Vec<&SensitiveItem>> = HashMap::new();
            for item in items {
                para_items.entry(item.row).or_default().push(item);
            }

            for (para_idx, ref p_items) in &para_items {
                if let Some(para) = paragraphs.iter_mut().find(|p| p.index == *para_idx) {
                    para.text = replace_in_text(&para.text, p_items, &consistency_map);
                }
            }
        }
    }

    // 4. 构建映射记录
    let mut mapping_map: HashMap<(String, SensitiveType), MappingEntry> = HashMap::new();
    for item in items {
        let key = (item.text.clone(), item.sensitive_type.clone());
        if let Some((replaced, st_type)) = consistency_map.get(&key) {
            let entry = mapping_map.entry(key).or_insert(MappingEntry {
                original_text: item.text.clone(),
                replaced_text: replaced.clone(),
                sensitive_type: item.sensitive_type.clone(),
                strategy: st_type.clone(),
                occurrences: 0,
            });
            entry.occurrences += 1;
        }
    }
    let mappings: Vec<MappingEntry> = mapping_map.into_values().collect();

    // 5. 构建统计摘要
    let mut by_type: HashMap<String, usize> = HashMap::new();
    for item in items {
        let key = sensitive_type_to_key(&item.sensitive_type);
        *by_type.entry(key).or_default() += 1;
    }
    let total = items.len();
    let summary = DesensitizeSummary { total, by_type };

    DesensitizeResult {
        content: new_content,
        mappings,
        summary,
    }
}

/// 使用映射记录还原 FileContent（仅 Replace 策略可逆）
/// 返回还原的替换次数
pub fn restore_from_mappings(content: &mut FileContent, mappings: &[MappingEntry]) -> usize {
    let mut reverse_mappings: Vec<(String, String)> = mappings
        .iter()
        .filter(|m| m.strategy == StrategyType::Replace)
        .map(|m| (m.replaced_text.clone(), m.original_text.clone()))
        .collect();
    // 按 replaced_text 长度降序，优先匹配长文本
    reverse_mappings.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    let mut total = 0;
    match content {
        FileContent::Spreadsheet {
            sheets, ..
        } => {
            for sheet in sheets.iter_mut() {
                for header in sheet.headers.iter_mut() {
                    for (from, to) in &reverse_mappings {
                        let count = header.matches(from.as_str()).count();
                        if count > 0 {
                            *header = header.replace(from.as_str(), to.as_str());
                            total += count;
                        }
                    }
                }
                for row in sheet.rows.iter_mut() {
                    for cell in row.iter_mut() {
                        for (from, to) in &reverse_mappings {
                            let count = cell.text.matches(from.as_str()).count();
                            if count > 0 {
                                cell.text = cell.text.replace(from.as_str(), to.as_str());
                                total += count;
                            }
                        }
                    }
                }
            }
        }
        FileContent::Document { paragraphs, .. } => {
            for para in paragraphs.iter_mut() {
                for (from, to) in &reverse_mappings {
                    let count = para.text.matches(from.as_str()).count();
                    if count > 0 {
                        para.text = para.text.replace(from.as_str(), to.as_str());
                        total += count;
                    }
                }
            }
        }
    }

    total
}

/// 提取 Spreadsheet 第一个 Sheet 的行数据（便于断言）
pub fn get_rows(content: &FileContent) -> &Vec<Vec<CellValue>> {
    match content {
        FileContent::Spreadsheet { sheets, .. } => &sheets[0].rows,
        _ => panic!("期望 Spreadsheet 类型"),
    }
}

/// 提取 Spreadsheet 第一个 Sheet 的表头（便于断言）
pub fn get_headers(content: &FileContent) -> &Vec<String> {
    match content {
        FileContent::Spreadsheet { sheets, .. } => &sheets[0].headers,
        _ => panic!("期望 Spreadsheet 类型"),
    }
}

/// 提取 Document 的段落（便于断言）
pub fn get_paragraphs(content: &FileContent) -> &Vec<Paragraph> {
    match content {
        FileContent::Document { paragraphs, .. } => paragraphs,
        _ => panic!("期望 Document 类型"),
    }
}
