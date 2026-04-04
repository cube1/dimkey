use uuid::Uuid;
use crate::models::sensitive::{SensitiveItem, DetectSource, FileContent};
use crate::models::strategy::{DictEntry, MatchMode};

/// 词典引擎：基于自定义词典进行字符串匹配
pub struct DictEngine {
    entries: Vec<DictEntry>,
}

impl DictEngine {
    pub fn new(entries: Vec<DictEntry>) -> Self {
        Self { entries }
    }

    /// 对文件内容进行词典匹配
    pub fn detect(&self, content: &FileContent) -> Vec<SensitiveItem> {
        if self.entries.is_empty() {
            return vec![];
        }

        let mut items = Vec::new();

        match content {
            FileContent::Spreadsheet { sheets, .. } => {
                for (sheet_idx, sheet) in sheets.iter().enumerate() {
                    // headers 作为 row=0，数据行从 row=1 开始（与正则引擎对齐）
                    for (col_idx, cell) in sheet.headers.iter().enumerate() {
                        self.match_text(cell, 0, col_idx, sheet_idx, &mut items);
                    }
                    for (row_idx, row) in sheet.rows.iter().enumerate() {
                        for (col_idx, cell) in row.iter().enumerate() {
                            self.match_text(&cell.text, row_idx + 1, col_idx, sheet_idx, &mut items);
                        }
                    }
                }
            }
            FileContent::Document { paragraphs, .. } => {
                for para in paragraphs {
                    self.match_text(&para.text, para.index, 0, 0, &mut items);
                }
            }
        }

        items
    }

    /// 在单个文本片段中查找所有词典匹配
    fn match_text(&self, text: &str, row: usize, col: usize, sheet_index: usize, items: &mut Vec<SensitiveItem>) {
        if text.is_empty() {
            return;
        }

        for entry in &self.entries {
            match entry.match_mode {
                MatchMode::Exact => {
                    // 区分大小写的子串匹配
                    let pattern = &entry.text;
                    let mut search_start = 0;
                    while let Some(byte_pos) = text[search_start..].find(pattern.as_str()) {
                        let abs_byte_pos = search_start + byte_pos;
                        // 将字节偏移转换为字符偏移
                        let char_start = text[..abs_byte_pos].chars().count();
                        let char_end = char_start + pattern.chars().count();

                        items.push(SensitiveItem {
                            id: Uuid::new_v4().to_string(),
                            text: pattern.clone(),
                            sensitive_type: entry.sensitive_type.clone(),
                            source: DetectSource::Dict,
                            confidence: 1.0,
                            start: char_start,
                            end: char_end,
                            row,
                            col,
                            sheet_index,
                            pdf_bbox: None,
                        });

                        search_start = abs_byte_pos + pattern.len();
                    }
                }
                MatchMode::Fuzzy => {
                    // 忽略大小写的子串匹配
                    let text_lower = text.to_lowercase();
                    let pattern_lower = entry.text.to_lowercase();
                    let mut search_start = 0;
                    while let Some(byte_pos) = text_lower[search_start..].find(pattern_lower.as_str()) {
                        let abs_byte_pos = search_start + byte_pos;
                        let char_start = text_lower[..abs_byte_pos].chars().count();
                        let char_end = char_start + pattern_lower.chars().count();
                        // 从原文中取匹配到的文本（保持原始大小写）
                        let matched_text: String = text.chars().skip(char_start).take(char_end - char_start).collect();

                        items.push(SensitiveItem {
                            id: Uuid::new_v4().to_string(),
                            text: matched_text,
                            sensitive_type: entry.sensitive_type.clone(),
                            source: DetectSource::Dict,
                            confidence: 1.0,
                            start: char_start,
                            end: char_end,
                            row,
                            col,
                            sheet_index,
                            pdf_bbox: None,
                        });

                        search_start = abs_byte_pos + pattern_lower.len();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::sensitive::{FileContent, FileType, Paragraph, SensitiveType, SheetData, CellValue};

    fn make_spreadsheet(rows: Vec<Vec<&str>>) -> FileContent {
        let headers = vec!["A".to_string(), "B".to_string()];
        let rows: Vec<Vec<CellValue>> = rows.iter().map(|r| r.iter().map(|s| CellValue::text(s.to_string())).collect()).collect();
        let row_count = rows.len();
        let col_count = if rows.is_empty() { 0 } else { rows[0].len() };
        FileContent::Spreadsheet {
            file_name: "test.csv".to_string(),
            file_type: FileType::Csv,
            sheets: vec![SheetData {
                name: String::new(),
                headers,
                rows,
                row_count,
                col_count,
            }],
        }
    }

    #[test]
    fn test_exact_match_single_entry() {
        let entries = vec![DictEntry {
            text: "机密项目".to_string(),
            sensitive_type: SensitiveType::Custom("机密项目".to_string()),
            match_mode: MatchMode::Exact,
            replacement: None,
            language: None,
            builtin: false,
        }];
        let engine = DictEngine::new(entries);
        let content = make_spreadsheet(vec![vec!["这是机密项目的文档", "普通内容"]]);
        let items = engine.detect(&content);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, "机密项目");
        assert_eq!(items[0].start, 2); // "这是" = 2 个字符
        assert_eq!(items[0].end, 6);   // "机密项目" = 4 个字符
        assert_eq!(items[0].row, 1); // 数据行从 row=1 开始
        assert_eq!(items[0].col, 0);
    }

    #[test]
    fn test_exact_match_case_sensitive() {
        let entries = vec![DictEntry {
            text: "ABC".to_string(),
            sensitive_type: SensitiveType::Custom("ABC".to_string()),
            match_mode: MatchMode::Exact,
            replacement: None,
            language: None,
            builtin: false,
        }];
        let engine = DictEngine::new(entries);
        let content = make_spreadsheet(vec![vec!["abc ABC Abc"]]);
        let items = engine.detect(&content);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, "ABC");
    }

    #[test]
    fn test_fuzzy_match_case_insensitive() {
        let entries = vec![DictEntry {
            text: "ABC".to_string(),
            sensitive_type: SensitiveType::Custom("ABC".to_string()),
            match_mode: MatchMode::Fuzzy,
            replacement: None,
            language: None,
            builtin: false,
        }];
        let engine = DictEngine::new(entries);
        let content = make_spreadsheet(vec![vec!["abc ABC Abc"]]);
        let items = engine.detect(&content);
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn test_multiple_occurrences_in_one_cell() {
        let entries = vec![DictEntry {
            text: "敏感".to_string(),
            sensitive_type: SensitiveType::Custom("敏感".to_string()),
            match_mode: MatchMode::Exact,
            replacement: None,
            language: None,
            builtin: false,
        }];
        let engine = DictEngine::new(entries);
        let content = make_spreadsheet(vec![vec!["敏感数据和敏感信息"]]);
        let items = engine.detect(&content);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_empty_dict_returns_empty() {
        let engine = DictEngine::new(vec![]);
        let content = make_spreadsheet(vec![vec!["任何内容"]]);
        let items = engine.detect(&content);
        assert!(items.is_empty());
    }

    #[test]
    fn test_document_content() {
        let entries = vec![DictEntry {
            text: "秘密".to_string(),
            sensitive_type: SensitiveType::Custom("秘密".to_string()),
            match_mode: MatchMode::Exact,
            replacement: None,
            language: None,
            builtin: false,
        }];
        let engine = DictEngine::new(entries);
        let content = FileContent::Document {
            file_name: "test.docx".to_string(),
            file_type: FileType::Docx,
            paragraphs: vec![
                Paragraph {
                    index: 0,
                    text: "这是秘密文件".to_string(),
                    style: "normal".to_string(),
                    table_position: None,
                    pdf_position: None,
                },
            ],
            encoding: None,
        };
        let items = engine.detect(&content);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, "秘密");
        assert_eq!(items[0].start, 2);
        assert_eq!(items[0].end, 4);
        assert_eq!(items[0].row, 0);
        assert_eq!(items[0].col, 0);
    }
}
