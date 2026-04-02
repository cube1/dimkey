# Phase 4 实施计划：NER 异步识别 + 词典管理 + 策略配置

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 完成三层识别引擎（词典引擎 + NER 架构），并实现词典管理、策略配置、敏感项详情三个前端交互面板。

**Architecture:** Rust 后端实现词典匹配逻辑和 NER ONNX 架构（可选降级），前端用 headlessui 实现 DictDrawer 抽屉、StrategyPanel 滑出面板、SensitivePopover 浮层。所有前端组件通过 Zustand store 驱动数据流。

**Tech Stack:** Rust + ort (ONNX Runtime) + Tauri Managed State / React + @headlessui/react + Zustand + TailwindCSS

---

## Task 1: 词典引擎 — 实现匹配逻辑 + 测试

**Files:**
- Modify: `src-tauri/src/engine/dict_engine.rs`

**Step 1: 在 dict_engine.rs 底部写测试模块**

在 `src-tauri/src/engine/dict_engine.rs` 底部添加测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::sensitive::{FileContent, FileType};
    use crate::models::strategy::{DictEntry, MatchMode};

    fn make_spreadsheet(rows: Vec<Vec<&str>>) -> FileContent {
        let headers = vec!["A".to_string(), "B".to_string()];
        let rows: Vec<Vec<String>> = rows.iter().map(|r| r.iter().map(|s| s.to_string()).collect()).collect();
        let row_count = rows.len();
        let col_count = if rows.is_empty() { 0 } else { rows[0].len() };
        FileContent::Spreadsheet {
            file_name: "test.csv".to_string(),
            file_type: FileType::Csv,
            headers,
            rows,
            row_count,
            col_count,
        }
    }

    #[test]
    fn test_exact_match_single_entry() {
        let entries = vec![DictEntry {
            text: "机密项目".to_string(),
            sensitive_type: SensitiveType::Custom("机密项目".to_string()),
            match_mode: MatchMode::Exact,
        }];
        let engine = DictEngine::new(entries);
        let content = make_spreadsheet(vec![vec!["这是机密项目的文档", "普通内容"]]);
        let items = engine.detect(&content);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, "机密项目");
        assert_eq!(items[0].start, 6); // "这是" = 6 bytes in slice, but we use char offset
        assert_eq!(items[0].row, 0);
        assert_eq!(items[0].col, 0);
    }

    #[test]
    fn test_exact_match_case_sensitive() {
        let entries = vec![DictEntry {
            text: "ABC".to_string(),
            sensitive_type: SensitiveType::Custom("ABC".to_string()),
            match_mode: MatchMode::Exact,
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
        }];
        let engine = DictEngine::new(entries);
        let content = FileContent::Document {
            file_name: "test.docx".to_string(),
            file_type: FileType::Docx,
            paragraphs: vec![
                crate::models::sensitive::Paragraph {
                    index: 0,
                    text: "这是秘密文件".to_string(),
                    style: "normal".to_string(),
                },
            ],
        };
        let items = engine.detect(&content);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].row, 0);
        assert_eq!(items[0].col, 0);
    }
}
```

**Step 2: 运行测试确认失败**

```bash
cd src-tauri && cargo test dict_engine -- --nocapture
```

预期：编译通过但测试失败（detect 返回空 vec）。

**Step 3: 实现匹配逻辑**

将 `src-tauri/src/engine/dict_engine.rs` 的完整内容替换为：

```rust
use uuid::Uuid;
use crate::models::sensitive::{SensitiveItem, SensitiveType, DetectSource, FileContent};
use crate::models::strategy::{DictEntry, MatchMode};

/// 词典引擎：基于自定义词典进行字符串匹配
pub struct DictEngine {
    entries: Vec<DictEntry>,
}

impl DictEngine {
    pub fn new(entries: Vec<DictEntry>) -> Self {
        Self { entries }
    }

    pub fn update_entries(&mut self, entries: Vec<DictEntry>) {
        self.entries = entries;
    }

    /// 对文件内容进行词典匹配
    pub fn detect(&self, content: &FileContent) -> Vec<SensitiveItem> {
        if self.entries.is_empty() {
            return vec![];
        }

        let mut items = Vec::new();

        match content {
            FileContent::Spreadsheet { rows, .. } => {
                for (row_idx, row) in rows.iter().enumerate() {
                    for (col_idx, cell) in row.iter().enumerate() {
                        self.match_text(cell, row_idx, col_idx, &mut items);
                    }
                }
            }
            FileContent::Document { paragraphs, .. } => {
                for para in paragraphs {
                    self.match_text(&para.text, para.index, 0, &mut items);
                }
            }
        }

        items
    }

    /// 在单个文本片段中查找所有词典匹配
    fn match_text(&self, text: &str, row: usize, col: usize, items: &mut Vec<SensitiveItem>) {
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
                        });

                        search_start = abs_byte_pos + pattern_lower.len();
                    }
                }
            }
        }
    }
}
```

**Step 4: 运行测试确认通过**

```bash
cd src-tauri && cargo test dict_engine -- --nocapture
```

预期：全部 PASS。

**Step 5: 提交**

```bash
git add src-tauri/src/engine/dict_engine.rs
git commit -m "feat(phase4): 实现词典引擎匹配逻辑，支持 Exact/Fuzzy 模式"
```

---

## Task 2: 词典引擎 — 接入 detect_by_dict 命令

**Files:**
- Modify: `src-tauri/src/commands/detect.rs`

**Step 1: 实现 detect_by_dict 命令**

将 `detect_by_dict` 函数替换为：

```rust
/// 运行词典匹配
#[tauri::command]
pub async fn detect_by_dict(content: FileContent, app_handle: tauri::AppHandle) -> Result<Vec<SensitiveItem>, String> {
    use tauri::Manager;
    use crate::models::strategy::DictEntry;
    use crate::engine::dict_engine::DictEngine;

    // 读取词典文件
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录: {}", e))?;
    let dict_path = app_dir.join("dict.json");

    let entries: Vec<DictEntry> = if dict_path.exists() {
        let content_str = std::fs::read_to_string(&dict_path)
            .map_err(|e| format!("读取词典文件失败: {}", e))?;
        serde_json::from_str(&content_str)
            .map_err(|e| format!("解析词典文件失败: {}", e))?
    } else {
        vec![]
    };

    if entries.is_empty() {
        return Ok(vec![]);
    }

    let engine = DictEngine::new(entries);
    Ok(engine.detect(&content))
}
```

注意：函数签名多了 `app_handle: tauri::AppHandle` 参数，Tauri 会自动注入。同时在文件顶部添加 `use tauri::Manager;` import。

**Step 2: 运行 cargo check 确认编译通过**

```bash
cd src-tauri && cargo check
```

预期：编译成功，无错误。

**Step 3: 提交**

```bash
git add src-tauri/src/commands/detect.rs
git commit -m "feat(phase4): 接入 detect_by_dict 命令，读取 dict.json 并执行匹配"
```

---

## Task 3: NER 引擎 — 添加 ort 依赖 + 重构 NerEngine

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/engine/ner_engine.rs`

**Step 1: 添加 ort 依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 下添加：

```toml
ort = "2"
```

**Step 2: 重构 NerEngine**

将 `src-tauri/src/engine/ner_engine.rs` 完整替换为：

```rust
use std::collections::HashMap;
use std::path::Path;
use std::fs;
use uuid::Uuid;
use crate::models::sensitive::{SensitiveItem, SensitiveType, DetectSource, FileContent};

/// BIO 标签前缀到 SensitiveType 的映射
fn label_to_sensitive_type(label: &str) -> Option<SensitiveType> {
    // 去掉 B-/I- 前缀取实体类型
    let entity = if label.starts_with("B-") || label.starts_with("I-") {
        &label[2..]
    } else {
        return None; // O 标签或未知标签
    };

    match entity {
        "PER" | "PERSON" => Some(SensitiveType::PersonName),
        "ORG" | "ORGANIZATION" => Some(SensitiveType::OrgName),
        "LOC" | "LOCATION" | "GPE" => Some(SensitiveType::Address),
        "TITLE" => Some(SensitiveType::Title),
        _ => None,
    }
}

/// NER 引擎：基于 ONNX 模型进行命名实体识别（可选降级）
pub struct NerEngine {
    /// ONNX 推理会话，模型文件不存在时为 None
    session: Option<ort::Session>,
    /// 分词词表：token → id
    vocab: HashMap<String, i64>,
    /// 模型输出 ID → BIO 标签
    id2label: Vec<String>,
}

impl NerEngine {
    /// 尝试从指定目录加载 NER 模型
    /// 文件不存在时返回 Ok(降级实例)，不报错
    pub fn try_load(model_dir: &Path) -> Result<Self, String> {
        let model_path = model_dir.join("model.onnx");
        let vocab_path = model_dir.join("vocab.txt");
        let label_path = model_dir.join("id2label.json");

        // 任一文件不存在则降级
        if !model_path.exists() || !vocab_path.exists() || !label_path.exists() {
            return Ok(Self {
                session: None,
                vocab: HashMap::new(),
                id2label: Vec::new(),
            });
        }

        // 加载词表
        let vocab_content = fs::read_to_string(&vocab_path)
            .map_err(|e| format!("读取词表失败: {}", e))?;
        let vocab: HashMap<String, i64> = vocab_content
            .lines()
            .enumerate()
            .map(|(i, line)| (line.to_string(), i as i64))
            .collect();

        // 加载标签映射
        let label_content = fs::read_to_string(&label_path)
            .map_err(|e| format!("读取标签映射失败: {}", e))?;
        let label_map: HashMap<String, String> = serde_json::from_str(&label_content)
            .map_err(|e| format!("解析标签映射失败: {}", e))?;

        // 将 HashMap<"0": "O", "1": "B-PER"> 转为 Vec，按 key 排序
        let max_id = label_map.keys()
            .filter_map(|k| k.parse::<usize>().ok())
            .max()
            .unwrap_or(0);
        let mut id2label = vec!["O".to_string(); max_id + 1];
        for (k, v) in &label_map {
            if let Ok(idx) = k.parse::<usize>() {
                id2label[idx] = v.clone();
            }
        }

        // 加载 ONNX 模型
        let session = ort::Session::builder()
            .map_err(|e| format!("创建 ONNX Session Builder 失败: {}", e))?
            .commit_from_file(&model_path)
            .map_err(|e| format!("加载 ONNX 模型失败: {}", e))?;

        Ok(Self {
            session: Some(session),
            vocab,
            id2label,
        })
    }

    /// 模型是否已加载
    pub fn is_loaded(&self) -> bool {
        self.session.is_some()
    }

    /// 对文件内容进行 NER 推理
    /// 模型未加载时直接返回空结果
    pub fn detect(&self, content: &FileContent) -> Result<Vec<SensitiveItem>, String> {
        let session = match &self.session {
            Some(s) => s,
            None => return Ok(vec![]),
        };

        let mut items = Vec::new();

        // 收集所有文本片段及其位置信息
        let text_segments: Vec<(String, usize, usize)> = match content {
            FileContent::Spreadsheet { rows, .. } => {
                rows.iter().enumerate().flat_map(|(row_idx, row)| {
                    row.iter().enumerate().map(move |(col_idx, cell)| {
                        (cell.clone(), row_idx, col_idx)
                    })
                }).collect()
            }
            FileContent::Document { paragraphs, .. } => {
                paragraphs.iter().map(|p| {
                    (p.text.clone(), p.index, 0)
                }).collect()
            }
        };

        for (text, row, col) in &text_segments {
            if text.is_empty() {
                continue;
            }

            let segment_items = self.detect_text(session, text, *row, *col)?;
            items.extend(segment_items);
        }

        Ok(items)
    }

    /// 对单段文本执行 NER 推理
    fn detect_text(
        &self,
        session: &ort::Session,
        text: &str,
        row: usize,
        col: usize,
    ) -> Result<Vec<SensitiveItem>, String> {
        // 字符级分词
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() {
            return Ok(vec![]);
        }

        // 构建 input_ids: [CLS] + char tokens + [SEP]
        let cls_id = self.vocab.get("[CLS]").copied().unwrap_or(101);
        let sep_id = self.vocab.get("[SEP]").copied().unwrap_or(102);
        let unk_id = self.vocab.get("[UNK]").copied().unwrap_or(100);

        let max_len = 510; // 留 2 个位置给 CLS/SEP
        let truncated_len = chars.len().min(max_len);

        let mut input_ids: Vec<i64> = Vec::with_capacity(truncated_len + 2);
        input_ids.push(cls_id);
        for ch in &chars[..truncated_len] {
            let token = ch.to_string();
            let id = self.vocab.get(&token).copied().unwrap_or(unk_id);
            input_ids.push(id);
        }
        input_ids.push(sep_id);

        let seq_len = input_ids.len();
        let attention_mask: Vec<i64> = vec![1; seq_len];
        let token_type_ids: Vec<i64> = vec![0; seq_len];

        // 构建 ONNX 输入张量
        let input_ids_array = ndarray::Array2::from_shape_vec((1, seq_len), input_ids)
            .map_err(|e| format!("构建 input_ids 张量失败: {}", e))?;
        let attention_mask_array = ndarray::Array2::from_shape_vec((1, seq_len), attention_mask)
            .map_err(|e| format!("构建 attention_mask 张量失败: {}", e))?;
        let token_type_ids_array = ndarray::Array2::from_shape_vec((1, seq_len), token_type_ids)
            .map_err(|e| format!("构建 token_type_ids 张量失败: {}", e))?;

        // 运行推理
        let outputs = session.run(ort::inputs! {
            "input_ids" => input_ids_array,
            "attention_mask" => attention_mask_array,
            "token_type_ids" => token_type_ids_array,
        }.map_err(|e| format!("构建 ONNX 输入失败: {}", e))?)
            .map_err(|e| format!("ONNX 推理失败: {}", e))?;

        // 解析输出：shape [1, seq_len, num_labels]
        let logits = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("提取输出张量失败: {}", e))?;

        let logits_view = logits.view();

        // 对每个 token 取 argmax 得到标签 ID
        let mut label_ids: Vec<usize> = Vec::with_capacity(seq_len);
        for i in 0..seq_len {
            let mut max_idx = 0usize;
            let mut max_val = f32::NEG_INFINITY;
            for j in 0..self.id2label.len() {
                let val = logits_view[[0, i, j]];
                if val > max_val {
                    max_val = val;
                    max_idx = j;
                }
            }
            label_ids.push(max_idx);
        }

        // BIO 后处理：合并连续实体，跳过 [CLS]/[SEP] 位置
        let mut items = Vec::new();
        let mut entity_start: Option<usize> = None;
        let mut entity_type: Option<SensitiveType> = None;

        // label_ids[0] 是 [CLS]，从 1 开始，到 truncated_len（不含 [SEP]）
        for i in 1..=truncated_len {
            let label = &self.id2label[label_ids[i]];
            let char_idx = i - 1; // 对应 chars 中的索引

            if label.starts_with("B-") {
                // 先保存之前的实体
                if let (Some(start), Some(st)) = (entity_start, entity_type.take()) {
                    let entity_text: String = chars[start..char_idx].iter().collect();
                    if !entity_text.trim().is_empty() {
                        items.push(SensitiveItem {
                            id: Uuid::new_v4().to_string(),
                            text: entity_text,
                            sensitive_type: st,
                            source: DetectSource::Ner,
                            confidence: 0.8,
                            start,
                            end: char_idx,
                            row,
                            col,
                        });
                    }
                }
                // 开始新实体
                entity_start = Some(char_idx);
                entity_type = label_to_sensitive_type(label);
            } else if label.starts_with("I-") {
                // 如果当前没有活跃实体，或类型不匹配，忽略
                if entity_start.is_none() {
                    continue;
                }
                let current_type = label_to_sensitive_type(label);
                if current_type != entity_type {
                    // 类型不匹配，结束之前的实体
                    if let (Some(start), Some(st)) = (entity_start, entity_type.take()) {
                        let entity_text: String = chars[start..char_idx].iter().collect();
                        if !entity_text.trim().is_empty() {
                            items.push(SensitiveItem {
                                id: Uuid::new_v4().to_string(),
                                text: entity_text,
                                sensitive_type: st,
                                source: DetectSource::Ner,
                                confidence: 0.8,
                                start,
                                end: char_idx,
                                row,
                                col,
                            });
                        }
                    }
                    entity_start = None;
                    entity_type = None;
                }
                // 否则继续延伸当前实体
            } else {
                // O 标签，结束当前实体
                if let (Some(start), Some(st)) = (entity_start, entity_type.take()) {
                    let entity_text: String = chars[start..char_idx].iter().collect();
                    if !entity_text.trim().is_empty() {
                        items.push(SensitiveItem {
                            id: Uuid::new_v4().to_string(),
                            text: entity_text,
                            sensitive_type: st,
                            source: DetectSource::Ner,
                            confidence: 0.8,
                            start,
                            end: char_idx,
                            row,
                            col,
                        });
                    }
                }
                entity_start = None;
                entity_type = None;
            }
        }

        // 处理尾部未关闭的实体
        if let (Some(start), Some(st)) = (entity_start, entity_type) {
            let entity_text: String = chars[start..truncated_len].iter().collect();
            if !entity_text.trim().is_empty() {
                items.push(SensitiveItem {
                    id: Uuid::new_v4().to_string(),
                    text: entity_text,
                    sensitive_type: st,
                    source: DetectSource::Ner,
                    confidence: 0.8,
                    start,
                    end: truncated_len,
                    row,
                    col,
                });
            }
        }

        Ok(items)
    }
}
```

注意：需要在 `Cargo.toml` 中额外添加 `ndarray = "0.16"` 依赖（ort 输入需要 ndarray 张量）。

同时需要为 `SensitiveType` 添加 `PartialEq` 比较能力（已有 `#[derive(PartialEq)]`，确认一下）。

**Step 3: 运行 cargo check 确认编译通过**

```bash
cd src-tauri && cargo check
```

预期：编译成功。由于 ort 首次下载可能较慢，等待 ONNX Runtime 预编译库下载。

**Step 4: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/src/engine/ner_engine.rs
git commit -m "feat(phase4): 添加 ort 依赖，重构 NerEngine 支持 ONNX 加载和可选降级"
```

---

## Task 4: NER 引擎 — 接入 Tauri Managed State + 命令

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/detect.rs`
- Create: `src-tauri/resources/ner/.gitkeep`

**Step 1: 在 lib.rs 中注册 NerEngine 为 Managed State**

修改 `src-tauri/src/lib.rs`，在 `run()` 中初始化 NerEngine 并注册：

```rust
mod commands;
mod engine;
mod parser;
mod desensitizer;
pub mod models;

use std::sync::Mutex;
use commands::file::{import_file, export_file};
use commands::detect::{detect_by_regex, detect_by_ner, detect_by_dict};
use commands::desensitize::apply_desensitize;
use commands::config::{load_config, save_config, load_dict, save_dict};
use commands::task::{save_task, list_tasks, delete_task, restore_file};
use engine::ner_engine::NerEngine;

/// NER 引擎全局状态（Mutex 包裹以满足 Send + Sync）
pub struct NerEngineState(pub Mutex<NerEngine>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 初始化 NER 引擎（从 resources/ner/ 加载，文件不存在则降级）
            let resource_dir = app.path().resource_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("resources"));
            let ner_dir = resource_dir.join("ner");
            let ner_engine = NerEngine::try_load(&ner_dir)
                .unwrap_or_else(|e| {
                    eprintln!("NER 引擎加载警告: {}", e);
                    // 降级：返回空引擎
                    NerEngine::try_load(std::path::Path::new("/nonexistent")).unwrap()
                });
            app.manage(NerEngineState(Mutex::new(ner_engine)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            import_file,
            export_file,
            detect_by_regex,
            detect_by_ner,
            detect_by_dict,
            apply_desensitize,
            load_config,
            save_config,
            load_dict,
            save_dict,
            save_task,
            list_tasks,
            delete_task,
            restore_file,
        ])
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
```

**Step 2: 更新 detect_by_ner 命令**

在 `src-tauri/src/commands/detect.rs` 中替换 `detect_by_ner`：

```rust
/// 运行 NER 模型识别（慢速，异步补充）
#[tauri::command]
pub async fn detect_by_ner(
    content: FileContent,
    ner_state: tauri::State<'_, crate::NerEngineState>,
) -> Result<Vec<SensitiveItem>, String> {
    let engine = ner_state.0.lock().map_err(|e| format!("获取 NER 引擎失败: {}", e))?;
    engine.detect(&content)
}
```

**Step 3: 创建 resources/ner 占位目录**

```bash
mkdir -p src-tauri/resources/ner && touch src-tauri/resources/ner/.gitkeep
```

**Step 4: 运行 cargo check**

```bash
cd src-tauri && cargo check
```

预期：编译成功。NER 引擎降级运行，detect_by_ner 返回空结果。

**Step 5: 提交**

```bash
git add src-tauri/src/lib.rs src-tauri/src/commands/detect.rs src-tauri/resources/ner/.gitkeep
git commit -m "feat(phase4): NER 引擎接入 Tauri Managed State，detect_by_ner 命令就绪"
```

---

## Task 5: DictDrawer (M1) — 词典管理抽屉组件

**Files:**
- Modify: `src/components/DictManager/index.tsx`

**Step 1: 实现完整的 DictDrawer 组件**

将 `src/components/DictManager/index.tsx` 替换为完整实现。

组件接收 props: `{ open: boolean; onClose: () => void }`

功能要点：
- 用 headlessui `Dialog` + `Transition` 实现右侧滑出抽屉
- 词条列表：每条显示文本、类型标签（颜色）、匹配模式（Exact/Fuzzy badge）、删除按钮
- 底部添加表单：文本输入 + 类型选择下拉（13 种 + Custom）+ 匹配模式切换 + 添加按钮
- 空状态："暂无自定义词条"
- 打开时 `configStore.loadDict()`
- 关闭时 `configStore.saveDict()`

数据全部从 `useConfigStore` 读写。

**Step 2: 运行 `npm run dev` 确认编译无错**

```bash
npm run dev
```

预期：前端编译成功。

**Step 3: 提交**

```bash
git add src/components/DictManager/index.tsx
git commit -m "feat(phase4): 实现 DictDrawer 词典管理抽屉组件"
```

---

## Task 6: DictDrawer — 接入 TopBar + 关闭时重新检测

**Files:**
- Modify: `src/components/TopBar/index.tsx`
- Modify: `src/App.tsx`
- Modify: `src/stores/detectStore.ts`

**Step 1: 在 detectStore 中添加替换词典匹配项的方法**

在 `src/stores/detectStore.ts` 中添加 `replaceDictItems` 方法：

```typescript
// 在 interface DetectState 中添加：
replaceDictItems: (newDictItems: SensitiveItem[]) => void;

// 在 create 中添加实现：
replaceDictItems: (newDictItems) =>
    set((state) => ({
        items: [
            ...state.items.filter((i) => i.source !== "Dict"),
            ...newDictItems,
        ],
    })),
```

**Step 2: 在 App.tsx 中管理 DictDrawer 的 open 状态**

在 `src/App.tsx` 中：
- 添加 `const [dictOpen, setDictOpen] = useState(false)` 状态
- 引入 DictManager 组件，渲染 `<DictManager open={dictOpen} onClose={handleDictClose} />`
- `handleDictClose` 中：保存词典 → 如果当前在 preview 页面，重新调用 `detect_by_dict` → `replaceDictItems`
- 通过 Context 或 props 将 `setDictOpen` 传给 TopBar

考虑到简单性，在 `appStore` 中添加一个 `dictDrawerOpen` 状态字段：

```typescript
// appStore 中添加:
dictDrawerOpen: boolean;
setDictDrawerOpen: (open: boolean) => void;
```

**Step 3: TopBar 中连接"词典管理"按钮**

```typescript
// TopBar 中修改:
{
    label: "词典管理",
    onClick: () => useAppStore.getState().setDictDrawerOpen(true),
},
```

**Step 4: 运行确认编译通过**

```bash
npm run dev
```

**Step 5: 提交**

```bash
git add src/App.tsx src/components/TopBar/index.tsx src/stores/appStore.ts src/stores/detectStore.ts
git commit -m "feat(phase4): DictDrawer 接入 TopBar，关闭时重新触发词典匹配"
```

---

## Task 7: StrategyPanel (M2) — 策略配置面板组件

**Files:**
- Modify: `src/components/StrategyConfig/index.tsx`

**Step 1: 实现完整的 StrategyPanel 组件**

组件接收 props: `{ open: boolean; onClose: () => void }`

功能要点：
- 右侧滑出面板，宽度 360px，headlessui `Dialog` + `Transition`
- 列出 13 种 SensitiveType（用 `SENSITIVE_TYPE_CONFIG` 遍历），每种一行
- 每行：类型颜色标签 + 策略下拉选择
- 策略下拉约束逻辑（用一个 `getAllowedStrategies(typeKey: string)` 函数）：
  - PersonName / OrgName / Title → ["Replace", "Mask"]
  - Address → ["Mask", "Replace", "Generalize"]
  - 其他 → ["Mask", "Replace"]
- 选择 Mask 时：展开两个数字输入框（keep_prefix, keep_suffix）
- 底部："恢复默认"按钮 + "保存"按钮
- 数据从 `useConfigStore` 读写
- "保存"调用 `configStore.saveConfig()` 后关闭面板
- "恢复默认"调用 `configStore.resetToDefault()`

策略下拉用 `<select>` + TailwindCSS 样式即可，不需要 headlessui Listbox。

**Step 2: 运行确认**

```bash
npm run dev
```

**Step 3: 提交**

```bash
git add src/components/StrategyConfig/index.tsx
git commit -m "feat(phase4): 实现 StrategyPanel 策略配置面板组件"
```

---

## Task 8: StrategyPanel — 接入 PreviewPage 底部栏

**Files:**
- Modify: `src/pages/PreviewPage/index.tsx`
- Modify: `src/stores/appStore.ts`

**Step 1: 在 appStore 中添加 strategyPanelOpen 状态**

```typescript
// appStore 中添加:
strategyPanelOpen: boolean;
setStrategyPanelOpen: (open: boolean) => void;
```

**Step 2: 在 PreviewPage 底部栏添加"脱敏策略"按钮**

在 `src/pages/PreviewPage/index.tsx` 的底部栏 `<button>` 左侧添加：

```tsx
<button
    onClick={() => useAppStore.getState().setStrategyPanelOpen(true)}
    className="px-4 py-2 text-sm text-gray-600 border border-gray-300 rounded-lg hover:bg-gray-50 transition-colors"
>
    脱敏策略
</button>
```

**Step 3: 在 App.tsx 中渲染 StrategyPanel**

```tsx
import { StrategyConfig } from "./components/StrategyConfig";

// 在 App 组件中:
const strategyPanelOpen = useAppStore((s) => s.strategyPanelOpen);

// 在 JSX 中:
<StrategyConfig
    open={strategyPanelOpen}
    onClose={() => useAppStore.getState().setStrategyPanelOpen(false)}
/>
```

**Step 4: 运行确认**

```bash
npm run dev
```

**Step 5: 提交**

```bash
git add src/pages/PreviewPage/index.tsx src/stores/appStore.ts src/App.tsx
git commit -m "feat(phase4): StrategyPanel 接入 PreviewPage 底部栏脱敏策略按钮"
```

---

## Task 9: SensitivePopover (M3) — 敏感项详情浮层组件

**Files:**
- Create: `src/components/SensitivePopover/index.tsx`

**Step 1: 创建 SensitivePopover 组件**

组件接收 props:
```typescript
interface SensitivePopoverProps {
    item: SensitiveItem | null;      // 当前选中的敏感项，null 时不显示
    anchorRect: DOMRect | null;      // 被点击元素的位置
    onClose: () => void;
}
```

功能要点：
- 绝对定位浮层，位于 `anchorRect` 正下方
- 内容区：
  1. 原始文本（加粗，字号大一号）
  2. 类型标签（带颜色 badge，从 `SENSITIVE_TYPE_CONFIG` 取）
  3. 策略下拉（使用与 StrategyPanel 相同的约束逻辑 `getAllowedStrategies`）
  4. 脱敏预览区：
     - Mask → 前端计算：`text.slice(0, keep_prefix) + '*'.repeat(mid) + text.slice(-keep_suffix)`
     - Replace → 灰色文字 "（将替换为假数据）"
     - Generalize → 灰色文字 "（将泛化处理）"
  5. "取消标记"按钮 → 调用 `detectStore.removeItem(item.id)`，然后 `onClose()`
- 策略变更时调用 `detectStore.overrideStrategy(item.id, newStrategy)`
- 点击浮层外部关闭：用 `useEffect` 监听 `mousedown` 事件

将 `getAllowedStrategies` 函数抽取到 `src/types/index.ts` 或一个 `src/utils/strategy.ts` 中，让 StrategyPanel 和 SensitivePopover 共用。

**Step 2: 抽取共用的策略工具函数**

创建或在 `src/types/index.ts` 底部添加：

```typescript
/** 获取某类型允许的策略列表 */
export function getAllowedStrategies(typeKey: string): StrategyType[] {
    switch (typeKey) {
        case "PersonName":
        case "OrgName":
        case "Title":
            return ["Replace", "Mask"];
        case "Address":
            return ["Mask", "Replace", "Generalize"];
        default:
            return ["Mask", "Replace"];
    }
}

/** 前端本地计算掩码预览 */
export function previewMask(text: string, keepPrefix: number, keepSuffix: number): string {
    const chars = [...text];
    if (chars.length <= keepPrefix + keepSuffix) {
        return '*'.repeat(chars.length);
    }
    const prefix = chars.slice(0, keepPrefix).join('');
    const suffix = keepSuffix > 0 ? chars.slice(-keepSuffix).join('') : '';
    const mid = '*'.repeat(chars.length - keepPrefix - keepSuffix);
    return prefix + mid + suffix;
}
```

**Step 3: 运行确认**

```bash
npm run dev
```

**Step 4: 提交**

```bash
git add src/components/SensitivePopover/index.tsx src/types/index.ts
git commit -m "feat(phase4): 实现 SensitivePopover 敏感项详情浮层组件"
```

---

## Task 10: SensitivePopover — 接入 PreviewPage 高亮点击

**Files:**
- Modify: `src/pages/PreviewPage/index.tsx`

**Step 1: 在 PreviewPage 中管理 Popover 状态**

在 `PreviewPage` 组件中添加：

```typescript
const [popoverItem, setPopoverItem] = useState<SensitiveItem | null>(null);
const [popoverAnchor, setPopoverAnchor] = useState<DOMRect | null>(null);

const handleClickItem = (item: SensitiveItem, event: React.MouseEvent) => {
    const rect = (event.target as HTMLElement).getBoundingClientRect();
    setPopoverItem(item);
    setPopoverAnchor(rect);
};

const handleClosePopover = () => {
    setPopoverItem(null);
    setPopoverAnchor(null);
};
```

**Step 2: 将 handleClickItem 传递给 ContentRenderer → HighlightedText**

`ContentRenderer` 组件需要接收 `onClickItem` prop 并传递给内部的 `HighlightedText`。

`HighlightedText` 已有 `onClickItem?: (item: SensitiveItem) => void` prop，但需要扩展为同时传递 event：

```typescript
// HighlightedText 修改 onClickItem 签名:
onClickItem?: (item: SensitiveItem, event: React.MouseEvent) => void;

// onClick handler 修改:
onClick={(e) => {
    e.stopPropagation();
    onClickItem?.(seg.item!, e);
}}
```

**Step 3: 在 PreviewPage 中渲染 SensitivePopover**

```tsx
<SensitivePopover
    item={popoverItem}
    anchorRect={popoverAnchor}
    onClose={handleClosePopover}
/>
```

**Step 4: 运行 `npm run dev` 确认**

```bash
npm run dev
```

**Step 5: 提交**

```bash
git add src/pages/PreviewPage/index.tsx src/components/HighlightedText/index.tsx src/components/ContentRenderer/index.tsx
git commit -m "feat(phase4): SensitivePopover 接入 PreviewPage 高亮点击交互"
```

---

## Task 11: 集成验证 — cargo tauri dev 完整测试

**Step 1: 运行 Rust 全量测试**

```bash
cd src-tauri && cargo test
```

预期：所有测试通过（含新增的 dict_engine 测试）。

**Step 2: 启动完整应用**

```bash
cargo tauri dev
```

**Step 3: 手动验证清单**

- [ ] 打开应用 → 无报错
- [ ] 导入一个 CSV/Excel 文件 → P2 正则识别正常
- [ ] P2 顶部 NER 状态显示"NER识别中..."后消失（NER 降级返回空）
- [ ] TopBar "词典管理" → 打开抽屉 → 添加词条 → 关闭 → 词典项出现在高亮中
- [ ] 重新打开词典管理 → 词条已持久化
- [ ] P2 底部"脱敏策略" → 打开面板 → 切换策略 → 保存
- [ ] 点击高亮项 → Popover 弹出 → 显示详情 → 切换策略看到预览变化 → "取消标记"能移除
- [ ] 整个正向流程：识别 → 脱敏 → 对比 → 导出 仍然正常

**Step 4: 提交集成测试通过的状态**

```bash
git add -A
git commit -m "feat: Phase 4 — NER 架构 + 词典引擎 + 策略配置 + 敏感项浮层"
```
