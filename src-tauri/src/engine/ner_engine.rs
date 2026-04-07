use std::collections::HashMap;
use uuid::Uuid;
use crate::models::sensitive::{SensitiveItem, SensitiveType, DetectSource, FileContent};

/// 后端输出的原始实体（统一中间格式）
pub struct RawEntity {
    /// 实体文本
    pub text: String,
    /// 原始标签（如 "PER"、"LOC"、"ORG"），不含 B-/I- 前缀
    pub label: String,
    /// 字符偏移起始
    pub start: usize,
    /// 字符偏移结束
    pub end: usize,
    /// 置信度
    pub confidence: f32,
}

/// NER 推理后端 trait
pub trait NerBackend: Send {
    /// 对单段文本执行实体识别
    fn detect_text(&mut self, text: &str) -> Result<Vec<RawEntity>, String>;
    /// 模型是否已加载
    fn is_loaded(&self) -> bool;
    /// 返回标签映射表（后端原始标签 → SensitiveType）
    fn label_map(&self) -> &HashMap<String, SensitiveType>;
}

/// NER 引擎统一入口
/// 持有可插拔的推理后端，负责 FileContent 拆分和 RawEntity → SensitiveItem 映射
pub struct NerEngine {
    /// 推理后端，None 表示降级模式
    backend: Option<Box<dyn NerBackend>>,
    /// 标签映射表：后端原始标签 → SensitiveType
    label_map: HashMap<String, SensitiveType>,
}

impl NerEngine {
    /// 创建带后端的引擎实例
    pub fn new(backend: Box<dyn NerBackend>, label_map: HashMap<String, SensitiveType>) -> Self {
        Self {
            backend: Some(backend),
            label_map,
        }
    }

    /// 从后端创建引擎实例（label_map 从后端获取）
    pub fn from_backend(backend: Box<dyn NerBackend>) -> Self {
        let label_map = backend.label_map().clone();
        Self {
            backend: Some(backend),
            label_map,
        }
    }

    /// 返回降级实例（无后端，detect 返回空结果）
    pub fn degraded() -> Self {
        Self {
            backend: None,
            label_map: HashMap::new(),
        }
    }

    /// 模型是否已加载
    pub fn is_loaded(&self) -> bool {
        self.backend.as_ref().map_or(false, |b| b.is_loaded())
    }

    /// 对文件内容执行 NER 识别
    pub fn detect(&mut self, content: &FileContent) -> Result<Vec<SensitiveItem>, String> {
        let backend = match self.backend.as_mut() {
            Some(b) => b,
            None => return Ok(vec![]),
        };

        let mut items = Vec::new();

        // FileContent → 文本片段拆分：(text, row, col, sheet_index)
        let text_segments: Vec<(String, usize, usize, usize)> = match content {
            FileContent::Spreadsheet { sheets, .. } => {
                let mut segments = Vec::new();
                for (sheet_idx, sheet) in sheets.iter().enumerate() {
                    for (col_idx, cell) in sheet.headers.iter().enumerate() {
                        segments.push((cell.clone(), 0usize, col_idx, sheet_idx));
                    }
                    for (row_idx, row) in sheet.rows.iter().enumerate() {
                        for (col_idx, cell) in row.iter().enumerate() {
                            segments.push((cell.text.clone(), row_idx + 1, col_idx, sheet_idx));
                        }
                    }
                }
                segments
            }
            FileContent::Document { paragraphs, .. } => {
                paragraphs.iter().map(|p| {
                    (p.text.clone(), p.index, 0, 0)
                }).collect()
            }
        };

        for (text, row, col, sheet_index) in &text_segments {
            if text.is_empty() {
                continue;
            }

            // 调用后端执行推理
            let raw_entities = backend.detect_text(text)?;

            // RawEntity → SensitiveItem 映射
            for entity in raw_entities {
                if let Some(sensitive_type) = self.label_map.get(&entity.label) {
                    items.push(SensitiveItem {
                        id: Uuid::new_v4().to_string(),
                        text: entity.text,
                        sensitive_type: sensitive_type.clone(),
                        source: DetectSource::Ner,
                        confidence: entity.confidence as f64,
                        start: entity.start,
                        end: entity.end,
                        row: *row,
                        col: *col,
                        sheet_index: *sheet_index,
                        pdf_bbox: None,
                    });
                }
            }
        }

        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用的 Mock 后端
    struct MockBackend {
        entities: Vec<RawEntity>,
        label_map: HashMap<String, SensitiveType>,
    }

    impl NerBackend for MockBackend {
        fn detect_text(&mut self, _text: &str) -> Result<Vec<RawEntity>, String> {
            Ok(self.entities.clone())
        }
        fn is_loaded(&self) -> bool { true }
        fn label_map(&self) -> &HashMap<String, SensitiveType> {
            &self.label_map
        }
    }

    // RawEntity 需要 Clone 用于测试
    impl Clone for RawEntity {
        fn clone(&self) -> Self {
            Self {
                text: self.text.clone(),
                label: self.label.clone(),
                start: self.start,
                end: self.end,
                confidence: self.confidence,
            }
        }
    }

    #[test]
    fn test_degraded_returns_empty() {
        let mut engine = NerEngine::degraded();
        assert!(!engine.is_loaded());
        let content = FileContent::Spreadsheet {
            file_name: "test.csv".to_string(),
            file_type: crate::models::sensitive::FileType::Csv,
            sheets: vec![crate::models::sensitive::SheetData {
                name: String::new(),
                headers: vec!["A".to_string()],
                rows: vec![vec![crate::models::sensitive::CellValue::text("张三在北京工作".to_string())]],
                row_count: 1,
                col_count: 1,
            }],
        };
        let items = engine.detect(&content).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_mock_backend_mapping() {
        let mut label_map = HashMap::new();
        label_map.insert("PER".to_string(), SensitiveType::PersonName);
        label_map.insert("LOC".to_string(), SensitiveType::Address);

        let mock = MockBackend {
            entities: vec![
                RawEntity { text: "张三".to_string(), label: "PER".to_string(), start: 0, end: 2, confidence: 0.9 },
                RawEntity { text: "北京".to_string(), label: "LOC".to_string(), start: 3, end: 5, confidence: 0.85 },
            ],
            label_map: label_map.clone(),
        };

        let mut engine = NerEngine::from_backend(Box::new(mock));

        let content = FileContent::Spreadsheet {
            file_name: "test.csv".to_string(),
            file_type: crate::models::sensitive::FileType::Csv,
            sheets: vec![crate::models::sensitive::SheetData {
                name: String::new(),
                headers: vec![],
                rows: vec![vec![crate::models::sensitive::CellValue::text("张三在北京工作".to_string())]],
                row_count: 1,
                col_count: 1,
            }],
        };

        let items = engine.detect(&content).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text, "张三");
        assert_eq!(items[0].sensitive_type, SensitiveType::PersonName);
        assert_eq!(items[0].row, 1);
        assert_eq!(items[1].text, "北京");
        assert_eq!(items[1].sensitive_type, SensitiveType::Address);
    }

    #[test]
    fn test_unknown_label_skipped() {
        let mock = MockBackend {
            entities: vec![
                RawEntity { text: "某某".to_string(), label: "UNKNOWN".to_string(), start: 0, end: 2, confidence: 0.5 },
            ],
            label_map: HashMap::new(),
        };

        let mut engine = NerEngine::from_backend(Box::new(mock));
        let content = FileContent::Document {
            file_name: "test.docx".to_string(),
            file_type: crate::models::sensitive::FileType::Docx,
            paragraphs: vec![crate::models::sensitive::Paragraph {
                index: 0,
                text: "某某测试".to_string(),
                style: "Normal".to_string(),
                table_position: None,
                pdf_position: None,
            }],
            encoding: None,
        };

        let items = engine.detect(&content).unwrap();
        assert!(items.is_empty());
    }
}
