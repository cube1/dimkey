# NER 模型抽象层 — distilbert-NER 集成 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 NER 模型层抽象为配置驱动，集成 dslim/distilbert-NER 替代现有中文/多语言模型，为未来模型扩展做准备。

**Architecture:** 新增 `model_config.json` 描述模型元信息（标注方案、标签映射），`OnnxBackend` 重命名为 `OnnxTokenClassifier` 并根据 config 驱动后处理逻辑。`NerBackend` trait 新增 `label_map()` 方法，`NerEngine` 新增 `from_backend()` 简化初始化。

**Tech Stack:** Rust (ort, tokenizers, serde_json), Python (optimum, transformers), Bash

---

## 文件结构

| 文件 | 职责 |
|------|------|
| `src-tauri/src/engine/backends/onnx_token_classifier.rs` | 新文件，从 `onnx_backend.rs` 重构而来。通用 ONNX token 分类推理 + 可配置后处理 |
| `src-tauri/src/engine/backends/mod.rs` | 模块声明更新 |
| `src-tauri/src/engine/ner_engine.rs` | trait 扩展 + 构造方法简化 |
| `src-tauri/src/lib.rs` | 初始化代码简化 |
| `scripts/export_ner_model.py` | 新增 distilbert-ner 模型定义 + 自动生成 model_config.json |
| `scripts/use_ner_model.sh` | REQUIRED_FILES 加 model_config.json |

---

### Task 1: NerBackend trait 扩展 + NerEngine 简化

**Files:**
- Modify: `src-tauri/src/engine/ner_engine.rs`

- [ ] **Step 1: 为 NerBackend trait 新增 label_map() 方法**

在 `src-tauri/src/engine/ner_engine.rs` 中修改 `NerBackend` trait，新增 `label_map()` 方法：

```rust
/// NER 推理后端 trait
pub trait NerBackend: Send {
    /// 对单段文本执行实体识别
    fn detect_text(&mut self, text: &str) -> Result<Vec<RawEntity>, String>;
    /// 模型是否已加载
    fn is_loaded(&self) -> bool;
    /// 返回标签映射表（后端原始标签 → SensitiveType）
    fn label_map(&self) -> &HashMap<String, SensitiveType>;
}
```

- [ ] **Step 2: 为 NerEngine 新增 from_backend() 构造方法**

在 `NerEngine` 的 `impl` 块中，在 `new()` 方法之后新增 `from_backend()`：

```rust
    /// 从后端创建引擎实例（label_map 从后端获取）
    pub fn from_backend(backend: Box<dyn NerBackend>) -> Self {
        let label_map = backend.label_map().clone();
        Self {
            backend: Some(backend),
            label_map,
        }
    }
```

- [ ] **Step 3: 更新 MockBackend 以实现新的 trait 方法**

在测试模块中，为 `MockBackend` 添加 `label_map` 字段和实现：

```rust
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
```

更新测试中 `MockBackend` 的构造，添加 `label_map` 字段：

`test_mock_backend_mapping` 中：
```rust
        let mut label_map = HashMap::new();
        label_map.insert("PER".to_string(), SensitiveType::PersonName);
        label_map.insert("LOC".to_string(), SensitiveType::Address);

        let mock = MockBackend {
            entities: vec![
                RawEntity {
                    text: "张三".to_string(),
                    label: "PER".to_string(),
                    start: 0,
                    end: 2,
                    confidence: 0.9,
                },
                RawEntity {
                    text: "北京".to_string(),
                    label: "LOC".to_string(),
                    start: 3,
                    end: 5,
                    confidence: 0.85,
                },
            ],
            label_map: label_map.clone(),
        };

        let mut engine = NerEngine::from_backend(Box::new(mock));
```

`test_unknown_label_skipped` 中：
```rust
        let mock = MockBackend {
            entities: vec![
                RawEntity {
                    text: "某某".to_string(),
                    label: "UNKNOWN".to_string(),
                    start: 0,
                    end: 2,
                    confidence: 0.5,
                },
            ],
            label_map: HashMap::new(),
        };

        let mut engine = NerEngine::from_backend(Box::new(mock));
```

- [ ] **Step 4: 运行测试验证**

Run: `cd /Users/tanzeshun/workpath/github/dimkey/src-tauri && cargo test engine::ner_engine -- --nocapture`
Expected: 全部 PASS

- [ ] **Step 5: 提交**

```bash
cd /Users/tanzeshun/workpath/github/dimkey
git add src-tauri/src/engine/ner_engine.rs
git commit -m "refactor: NerBackend trait 新增 label_map()，NerEngine 新增 from_backend()"
```

---

### Task 2: OnnxBackend 重构为 OnnxTokenClassifier

**Files:**
- Delete: `src-tauri/src/engine/backends/onnx_backend.rs`
- Create: `src-tauri/src/engine/backends/onnx_token_classifier.rs`
- Modify: `src-tauri/src/engine/backends/mod.rs`

- [ ] **Step 1: 创建 onnx_token_classifier.rs**

将 `onnx_backend.rs` 的内容复制到新文件 `onnx_token_classifier.rs`，做以下修改：

**1a. 结构体重命名 + 新增字段：**

```rust
use std::collections::HashMap;
use std::path::Path;
use std::fs;
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;
use crate::models::sensitive::SensitiveType;
use super::super::ner_engine::{NerBackend, RawEntity};

/// 标注方案
#[derive(Debug, Clone, PartialEq)]
pub enum TaggingScheme {
    /// BIO 标注（B-PER, I-PER, O）
    Bio,
    /// I-only 标注（I-PER, O，无 B- 前缀）
    IOnly,
}

/// 模型配置（从 model_config.json 加载）
#[derive(Debug, Clone, serde::Deserialize)]
struct ModelConfig {
    #[allow(dead_code)]
    name: String,
    tagging_scheme: String,
    label_map: HashMap<String, Option<String>>,
}

/// 基于 ONNX Runtime 的通用 token 分类推理后端
pub struct OnnxTokenClassifier {
    session: Session,
    tokenizer: Tokenizer,
    id2label: Vec<String>,
    needs_token_type_ids: bool,
    tagging_scheme: TaggingScheme,
    /// 标签映射表：后端原始标签 → SensitiveType
    label_map: HashMap<String, SensitiveType>,
}
```

**1b. try_load() 方法重写** — 加载 model_config.json，向后兼容无 config 场景：

```rust
impl OnnxTokenClassifier {
    pub fn try_load(model_dir: &Path) -> Result<Option<Self>, String> {
        let model_path = model_dir.join("model.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");
        let label_path = model_dir.join("id2label.json");

        if !model_path.exists() || !tokenizer_path.exists() || !label_path.exists() {
            if model_path.exists() && model_dir.join("vocab.txt").exists() && !tokenizer_path.exists() {
                eprintln!("警告: 检测到旧格式 vocab.txt，请使用 ./scripts/use_ner_model.sh distilbert-ner 下载新格式模型");
            }
            return Ok(None);
        }

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| format!("加载 tokenizer.json 失败: {}", e))?;

        // 加载 id2label
        let label_content = fs::read_to_string(&label_path)
            .map_err(|e| format!("读取标签映射失败: {}", e))?;
        let raw_label_map: HashMap<String, String> = serde_json::from_str(&label_content)
            .map_err(|e| format!("解析标签映射失败: {}", e))?;

        let max_id = raw_label_map.keys()
            .filter_map(|k| k.parse::<usize>().ok())
            .max()
            .unwrap_or(0);
        let mut id2label = vec!["O".to_string(); max_id + 1];
        for (k, v) in &raw_label_map {
            if let Ok(idx) = k.parse::<usize>() {
                id2label[idx] = v.clone();
            }
        }

        let session = Session::builder()
            .map_err(|e| format!("创建 ONNX Session Builder 失败: {}", e))?
            .commit_from_file(&model_path)
            .map_err(|e| format!("加载 ONNX 模型失败: {}", e))?;

        let needs_token_type_ids = session.inputs().iter()
            .any(|input| input.name() == "token_type_ids");

        // 加载 model_config.json（可选，向后兼容）
        let config_path = model_dir.join("model_config.json");
        let (tagging_scheme, label_map) = if config_path.exists() {
            let config_content = fs::read_to_string(&config_path)
                .map_err(|e| format!("读取 model_config.json 失败: {}", e))?;
            let config: ModelConfig = serde_json::from_str(&config_content)
                .map_err(|e| format!("解析 model_config.json 失败: {}", e))?;

            let scheme = match config.tagging_scheme.as_str() {
                "bio" => TaggingScheme::Bio,
                "ionly" => TaggingScheme::IOnly,
                other => return Err(format!("未知标注方案: {}", other)),
            };

            let mut map = HashMap::new();
            for (label, sensitive_name) in &config.label_map {
                if let Some(name) = sensitive_name {
                    if let Some(st) = Self::parse_sensitive_type(name) {
                        map.insert(label.clone(), st);
                    } else {
                        eprintln!("警告: 未知 SensitiveType: {}", name);
                    }
                }
                // None → 跳过该标签
            }
            (scheme, map)
        } else {
            // 向后兼容：无 config 时自动推断
            let scheme = Self::infer_tagging_scheme(&id2label);
            let map = Self::build_default_label_map(&id2label);
            (scheme, map)
        };

        Ok(Some(Self { session, tokenizer, id2label, needs_token_type_ids, tagging_scheme, label_map }))
    }

    /// 从字符串解析 SensitiveType
    fn parse_sensitive_type(name: &str) -> Option<SensitiveType> {
        match name {
            "PersonName" => Some(SensitiveType::PersonName),
            "OrgName" => Some(SensitiveType::OrgName),
            "Address" => Some(SensitiveType::Address),
            "Title" => Some(SensitiveType::Title),
            "Email" => Some(SensitiveType::Email),
            "Phone" => Some(SensitiveType::Phone),
            "IdCard" => Some(SensitiveType::IdCard),
            "BankCard" => Some(SensitiveType::BankCard),
            "Ssn" => Some(SensitiveType::Ssn),
            "CreditCard" => Some(SensitiveType::CreditCard),
            "UsPhone" => Some(SensitiveType::UsPhone),
            "UkPhone" => Some(SensitiveType::UkPhone),
            "Passport" => Some(SensitiveType::Passport),
            "Iban" => Some(SensitiveType::Iban),
            "ZipCode" => Some(SensitiveType::ZipCode),
            "UkPostcode" => Some(SensitiveType::UkPostcode),
            "DriversLicense" => Some(SensitiveType::DriversLicense),
            "IpAddress" => Some(SensitiveType::IpAddress),
            _ => None,
        }
    }

    /// 从 id2label 自动推断标注方案
    fn infer_tagging_scheme(id2label: &[String]) -> TaggingScheme {
        let has_b_prefix = id2label.iter().any(|l| l.starts_with("B-"));
        if has_b_prefix {
            TaggingScheme::Bio
        } else {
            TaggingScheme::IOnly
        }
    }

    /// 向后兼容：从 id2label 构建默认标签映射（旧逻辑）
    fn build_default_label_map(id2label: &[String]) -> HashMap<String, SensitiveType> {
        let mut map = HashMap::new();
        for label in id2label {
            let entity = if label.starts_with("B-") || label.starts_with("I-") {
                &label[2..]
            } else {
                continue;
            };
            if map.contains_key(entity) {
                continue;
            }
            let sensitive_type = match entity {
                "PER" | "PERSON" => SensitiveType::PersonName,
                "ORG" | "ORGANIZATION" => SensitiveType::OrgName,
                "LOC" | "LOCATION" | "GPE" => SensitiveType::Address,
                "TITLE" => SensitiveType::Title,
                _ => continue,
            };
            map.insert(entity.to_string(), sensitive_type);
        }
        map
    }

    fn byte_offset_to_char_offset(text: &str, byte_offset: usize) -> usize {
        text[..byte_offset.min(text.len())].chars().count()
    }
}
```

**1c. detect_text() — 根据 tagging_scheme 分支后处理：**

```rust
impl NerBackend for OnnxTokenClassifier {
    fn detect_text(&mut self, text: &str) -> Result<Vec<RawEntity>, String> {
        if text.trim().is_empty() {
            return Ok(vec![]);
        }

        // 分词 + 推理部分与现有代码完全一致（从 encoding 到 label_ids + confidences）
        let encoding = self.tokenizer.encode(text, true)
            .map_err(|e| format!("分词失败: {}", e))?;

        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let attention_mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&m| m as i64).collect();
        let offsets = encoding.get_offsets();
        let seq_len = input_ids.len();

        if seq_len == 0 {
            return Ok(vec![]);
        }

        let shape = vec![1i64, seq_len as i64];

        let input_ids_tensor = Tensor::from_array((shape.clone(), input_ids.into_boxed_slice()))
            .map_err(|e| format!("构建 input_ids 张量失败: {}", e))?;
        let attention_mask_tensor = Tensor::from_array((shape.clone(), attention_mask.into_boxed_slice()))
            .map_err(|e| format!("构建 attention_mask 张量失败: {}", e))?;

        let outputs = if self.needs_token_type_ids {
            let token_type_ids: Vec<i64> = vec![0; seq_len];
            let token_type_ids_tensor = Tensor::from_array((shape, token_type_ids.into_boxed_slice()))
                .map_err(|e| format!("构建 token_type_ids 张量失败: {}", e))?;
            self.session.run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor,
                "token_type_ids" => token_type_ids_tensor,
            ]).map_err(|e| format!("ONNX 推理失败: {}", e))?
        } else {
            self.session.run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor,
            ]).map_err(|e| format!("ONNX 推理失败: {}", e))?
        };

        let (output_shape, logits_data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("提取输出张量失败: {}", e))?;

        let num_labels = if output_shape.len() == 3 {
            output_shape[2] as usize
        } else {
            self.id2label.len()
        };

        let mut label_ids: Vec<usize> = Vec::with_capacity(seq_len);
        let mut confidences: Vec<f32> = Vec::with_capacity(seq_len);
        for i in 0..seq_len {
            let offset = i * num_labels;
            let mut max_idx = 0usize;
            let mut max_val = f32::NEG_INFINITY;
            for j in 0..num_labels {
                let val = logits_data[offset + j];
                if val > max_val {
                    max_val = val;
                    max_idx = j;
                }
            }
            label_ids.push(max_idx);

            let sum_exp: f32 = (0..num_labels)
                .map(|j| (logits_data[offset + j] - max_val).exp())
                .sum();
            confidences.push(1.0 / sum_exp);
        }

        // 根据标注方案执行后处理
        match self.tagging_scheme {
            TaggingScheme::Bio => self.post_process_bio(&label_ids, &confidences, offsets, text),
            TaggingScheme::IOnly => self.post_process_ionly(&label_ids, &confidences, offsets, text),
        }
    }

    fn is_loaded(&self) -> bool {
        true
    }

    fn label_map(&self) -> &HashMap<String, SensitiveType> {
        &self.label_map
    }
}
```

**1d. 后处理方法 — BIO（现有逻辑提取为方法）+ IOnly（新增）：**

```rust
impl OnnxTokenClassifier {
    /// BIO 标注后处理（现有逻辑）
    fn post_process_bio(
        &self,
        label_ids: &[usize],
        confidences: &[f32],
        offsets: &[(usize, usize)],
        text: &str,
    ) -> Result<Vec<RawEntity>, String> {
        let mut items = Vec::new();
        let mut entity_start_byte: Option<usize> = None;
        let mut entity_end_byte: Option<usize> = None;
        let mut entity_label: Option<String> = None;
        let mut entity_confidence_sum: f32 = 0.0;
        let mut entity_token_count: usize = 0;
        let seq_len = label_ids.len();

        for i in 0..seq_len {
            let (tok_start, tok_end) = offsets[i];
            if tok_start == 0 && tok_end == 0 {
                if let (Some(sb), Some(eb), Some(lbl)) = (entity_start_byte, entity_end_byte, entity_label.take()) {
                    Self::flush_entity(sb, eb, &lbl, entity_confidence_sum, entity_token_count, text, &mut items);
                }
                entity_start_byte = None;
                entity_end_byte = None;
                entity_confidence_sum = 0.0;
                entity_token_count = 0;
                continue;
            }

            let label = &self.id2label[label_ids[i]];

            if label.starts_with("B-") {
                if let (Some(sb), Some(eb), Some(lbl)) = (entity_start_byte, entity_end_byte, entity_label.take()) {
                    Self::flush_entity(sb, eb, &lbl, entity_confidence_sum, entity_token_count, text, &mut items);
                }
                entity_start_byte = Some(tok_start);
                entity_end_byte = Some(tok_end);
                entity_label = Some(label[2..].to_string());
                entity_confidence_sum = confidences[i];
                entity_token_count = 1;
            } else if label.starts_with("I-") {
                let current_entity = &label[2..];
                if entity_label.as_deref() == Some(current_entity) {
                    entity_end_byte = Some(tok_end);
                    entity_confidence_sum += confidences[i];
                    entity_token_count += 1;
                } else {
                    if let (Some(sb), Some(eb), Some(lbl)) = (entity_start_byte, entity_end_byte, entity_label.take()) {
                        Self::flush_entity(sb, eb, &lbl, entity_confidence_sum, entity_token_count, text, &mut items);
                    }
                    entity_start_byte = None;
                    entity_end_byte = None;
                    entity_label = None;
                    entity_confidence_sum = 0.0;
                    entity_token_count = 0;
                }
            } else {
                if let (Some(sb), Some(eb), Some(lbl)) = (entity_start_byte, entity_end_byte, entity_label.take()) {
                    Self::flush_entity(sb, eb, &lbl, entity_confidence_sum, entity_token_count, text, &mut items);
                }
                entity_start_byte = None;
                entity_end_byte = None;
                entity_label = None;
                entity_confidence_sum = 0.0;
                entity_token_count = 0;
            }
        }

        if let (Some(sb), Some(eb), Some(lbl)) = (entity_start_byte, entity_end_byte, entity_label) {
            Self::flush_entity(sb, eb, &lbl, entity_confidence_sum, entity_token_count, text, &mut items);
        }

        Ok(items)
    }

    /// I-only 标注后处理
    /// 规则：I-X 后跟 O 或不同 I-Y → 结束实体；I-X 后跟同类 I-X → 续接
    fn post_process_ionly(
        &self,
        label_ids: &[usize],
        confidences: &[f32],
        offsets: &[(usize, usize)],
        text: &str,
    ) -> Result<Vec<RawEntity>, String> {
        let mut items = Vec::new();
        let mut entity_start_byte: Option<usize> = None;
        let mut entity_end_byte: Option<usize> = None;
        let mut entity_label: Option<String> = None;
        let mut entity_confidence_sum: f32 = 0.0;
        let mut entity_token_count: usize = 0;
        let seq_len = label_ids.len();

        for i in 0..seq_len {
            let (tok_start, tok_end) = offsets[i];
            if tok_start == 0 && tok_end == 0 {
                if let (Some(sb), Some(eb), Some(lbl)) = (entity_start_byte, entity_end_byte, entity_label.take()) {
                    Self::flush_entity(sb, eb, &lbl, entity_confidence_sum, entity_token_count, text, &mut items);
                }
                entity_start_byte = None;
                entity_end_byte = None;
                entity_confidence_sum = 0.0;
                entity_token_count = 0;
                continue;
            }

            let label = &self.id2label[label_ids[i]];

            if label.starts_with("I-") {
                let current_entity = &label[2..];
                if entity_label.as_deref() == Some(current_entity) {
                    // 续接当前实体
                    entity_end_byte = Some(tok_end);
                    entity_confidence_sum += confidences[i];
                    entity_token_count += 1;
                } else {
                    // 不同类型的 I- → 结束旧实体，开始新实体
                    if let (Some(sb), Some(eb), Some(lbl)) = (entity_start_byte, entity_end_byte, entity_label.take()) {
                        Self::flush_entity(sb, eb, &lbl, entity_confidence_sum, entity_token_count, text, &mut items);
                    }
                    entity_start_byte = Some(tok_start);
                    entity_end_byte = Some(tok_end);
                    entity_label = Some(current_entity.to_string());
                    entity_confidence_sum = confidences[i];
                    entity_token_count = 1;
                }
            } else {
                // O 标签 → 结束当前实体
                if let (Some(sb), Some(eb), Some(lbl)) = (entity_start_byte, entity_end_byte, entity_label.take()) {
                    Self::flush_entity(sb, eb, &lbl, entity_confidence_sum, entity_token_count, text, &mut items);
                }
                entity_start_byte = None;
                entity_end_byte = None;
                entity_label = None;
                entity_confidence_sum = 0.0;
                entity_token_count = 0;
            }
        }

        if let (Some(sb), Some(eb), Some(lbl)) = (entity_start_byte, entity_end_byte, entity_label) {
            Self::flush_entity(sb, eb, &lbl, entity_confidence_sum, entity_token_count, text, &mut items);
        }

        Ok(items)
    }

    fn flush_entity(
        start_byte: usize,
        end_byte: usize,
        label: &str,
        conf_sum: f32,
        count: usize,
        text: &str,
        items: &mut Vec<RawEntity>,
    ) {
        let entity_text = &text[start_byte..end_byte];
        if !entity_text.trim().is_empty() {
            let start_char = Self::byte_offset_to_char_offset(text, start_byte);
            let end_char = Self::byte_offset_to_char_offset(text, end_byte);
            items.push(RawEntity {
                text: entity_text.to_string(),
                label: label.to_string(),
                start: start_char,
                end: end_char,
                confidence: conf_sum / count as f32,
            });
        }
    }
}
```

**1e. 测试模块** — 从 `onnx_backend.rs` 迁移，更新类型名：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn model_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join("ner")
    }

    #[test]
    fn test_load_real_model() {
        let dir = model_dir();
        if !dir.join("model.onnx").exists() {
            println!("跳过：模型文件不存在");
            return;
        }
        let backend = OnnxTokenClassifier::try_load(&dir).unwrap();
        assert!(backend.is_some(), "模型加载失败");
        let backend = backend.unwrap();
        assert!(backend.is_loaded());
        let label_map = backend.label_map();
        assert!(!label_map.is_empty(), "标签映射不应为空");
    }

    #[test]
    fn test_detect_english_person_and_org() {
        let dir = model_dir();
        if !dir.join("model.onnx").exists() {
            println!("跳过：模型文件不存在");
            return;
        }
        let mut backend = OnnxTokenClassifier::try_load(&dir).unwrap().unwrap();
        let entities = backend.detect_text("John Smith works at Google in New York").unwrap();
        let labels: Vec<&str> = entities.iter().map(|e| e.label.as_str()).collect();
        let texts: Vec<&str> = entities.iter().map(|e| e.text.as_str()).collect();
        println!("英文识别结果: {:?}", entities.iter().map(|e| (&e.text, &e.label, e.confidence)).collect::<Vec<_>>());
        assert!(labels.contains(&"PER"), "应识别出人名，实际: {:?}", texts);
        assert!(labels.contains(&"ORG"), "应识别出组织名，实际: {:?}", texts);
        assert!(labels.contains(&"LOC"), "应识别出地名，实际: {:?}", texts);
    }

    #[test]
    fn test_detect_chinese_person_and_location() {
        let dir = model_dir();
        if !dir.join("model.onnx").exists() {
            println!("跳过：模型文件不存在");
            return;
        }
        let mut backend = OnnxTokenClassifier::try_load(&dir).unwrap().unwrap();
        let entities = backend.detect_text("张三在北京市海淀区工作").unwrap();
        let labels: Vec<&str> = entities.iter().map(|e| e.label.as_str()).collect();
        let texts: Vec<&str> = entities.iter().map(|e| e.text.as_str()).collect();
        println!("中文识别结果: {:?}", entities.iter().map(|e| (&e.text, &e.label, e.confidence)).collect::<Vec<_>>());
        assert!(labels.contains(&"PER"), "应识别出人名，实际: {:?}", texts);
        assert!(labels.contains(&"LOC"), "应识别出地名，实际: {:?}", texts);
    }

    #[test]
    fn test_detect_mixed_language() {
        let dir = model_dir();
        if !dir.join("model.onnx").exists() {
            println!("跳过：模型文件不存在");
            return;
        }
        let mut backend = OnnxTokenClassifier::try_load(&dir).unwrap().unwrap();
        let entities = backend.detect_text("李明在Google北京办公室工作").unwrap();
        println!("中英混排识别结果: {:?}", entities.iter().map(|e| (&e.text, &e.label, e.confidence)).collect::<Vec<_>>());
        assert!(!entities.is_empty(), "中英混排文本应识别出实体，实际为空");
    }

    #[test]
    fn test_detect_org() {
        let dir = model_dir();
        if !dir.join("model.onnx").exists() {
            println!("跳过：模型文件不存在");
            return;
        }
        let mut backend = OnnxTokenClassifier::try_load(&dir).unwrap().unwrap();
        let entities = backend.detect_text("李明在腾讯科技有限公司担任高级工程师").unwrap();
        let labels: Vec<&str> = entities.iter().map(|e| e.label.as_str()).collect();
        let texts: Vec<&str> = entities.iter().map(|e| e.text.as_str()).collect();
        println!("识别结果: {:?}", entities.iter().map(|e| (&e.text, &e.label)).collect::<Vec<_>>());
        assert!(labels.contains(&"PER"), "应识别出人名，实际: {:?}", texts);
        assert!(labels.contains(&"ORG"), "应识别出组织名，实际: {:?}", texts);
    }

    #[test]
    fn test_detect_isolated_cells() {
        let dir = model_dir();
        if !dir.join("model.onnx").exists() {
            return;
        }
        let mut backend = OnnxTokenClassifier::try_load(&dir).unwrap().unwrap();
        let sentence = "张三在北京市朝阳区的腾讯科技有限公司工作";
        let entities = backend.detect_text(sentence).unwrap();
        let labels: Vec<&str> = entities.iter().map(|e| e.label.as_str()).collect();
        assert!(labels.contains(&"PER"), "应识别出人名");
        assert!(labels.contains(&"LOC"), "应识别出地址");
        assert!(labels.contains(&"ORG"), "应识别出组织名");
    }

    #[test]
    fn test_char_offset_correctness() {
        let dir = model_dir();
        if !dir.join("model.onnx").exists() {
            return;
        }
        let mut backend = OnnxTokenClassifier::try_load(&dir).unwrap().unwrap();
        let text = "张三在北京工作";
        let entities = backend.detect_text(text).unwrap();
        for entity in &entities {
            let chars: Vec<char> = text.chars().collect();
            let extracted: String = chars[entity.start..entity.end].iter().collect();
            assert_eq!(extracted, entity.text,
                "字符偏移量不正确: start={}, end={}, expected='{}', got='{}'",
                entity.start, entity.end, entity.text, extracted);
        }
    }

    #[test]
    fn test_confidence_is_valid() {
        let dir = model_dir();
        if !dir.join("model.onnx").exists() {
            return;
        }
        let mut backend = OnnxTokenClassifier::try_load(&dir).unwrap().unwrap();
        let entities = backend.detect_text("张三在北京市海淀区工作").unwrap();
        for entity in &entities {
            assert!(entity.confidence > 0.0 && entity.confidence <= 1.0,
                "置信度应在 (0, 1] 范围内，实际: {}", entity.confidence);
        }
    }

    #[test]
    fn test_empty_and_whitespace() {
        let dir = model_dir();
        if !dir.join("model.onnx").exists() {
            return;
        }
        let mut backend = OnnxTokenClassifier::try_load(&dir).unwrap().unwrap();
        assert!(backend.detect_text("").unwrap().is_empty());
        assert!(backend.detect_text("   ").unwrap().is_empty());
    }

    #[test]
    fn test_infer_tagging_scheme_bio() {
        let labels = vec!["O".to_string(), "B-PER".to_string(), "I-PER".to_string()];
        assert_eq!(OnnxTokenClassifier::infer_tagging_scheme(&labels), TaggingScheme::Bio);
    }

    #[test]
    fn test_infer_tagging_scheme_ionly() {
        let labels = vec!["O".to_string(), "I-GIVENNAME".to_string(), "I-SURNAME".to_string()];
        assert_eq!(OnnxTokenClassifier::infer_tagging_scheme(&labels), TaggingScheme::IOnly);
    }

    #[test]
    fn test_parse_sensitive_type() {
        assert_eq!(OnnxTokenClassifier::parse_sensitive_type("PersonName"), Some(SensitiveType::PersonName));
        assert_eq!(OnnxTokenClassifier::parse_sensitive_type("OrgName"), Some(SensitiveType::OrgName));
        assert_eq!(OnnxTokenClassifier::parse_sensitive_type("Address"), Some(SensitiveType::Address));
        assert_eq!(OnnxTokenClassifier::parse_sensitive_type("Unknown"), None);
    }
}
```

- [ ] **Step 2: 更新 backends/mod.rs**

将 `src-tauri/src/engine/backends/mod.rs` 内容改为：

```rust
pub mod onnx_token_classifier;
```

- [ ] **Step 3: 删除旧文件 onnx_backend.rs**

```bash
rm /Users/tanzeshun/workpath/github/dimkey/src-tauri/src/engine/backends/onnx_backend.rs
```

- [ ] **Step 4: 运行测试验证编译和逻辑**

Run: `cd /Users/tanzeshun/workpath/github/dimkey/src-tauri && cargo test engine::backends -- --nocapture`
Expected: 全部 PASS（模型文件存在时运行集成测试，不存在时 skip）

再运行全量测试确认无破坏：
Run: `cd /Users/tanzeshun/workpath/github/dimkey/src-tauri && cargo test -- --nocapture`
Expected: 全部 PASS

- [ ] **Step 5: 提交**

```bash
cd /Users/tanzeshun/workpath/github/dimkey
git add src-tauri/src/engine/backends/
git commit -m "refactor: OnnxBackend 重构为 OnnxTokenClassifier，支持 Bio/IOnly 标注方案和配置驱动标签映射"
```

---

### Task 3: lib.rs 初始化代码更新

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 更新 import 路径和初始化逻辑**

将 `lib.rs` 第 28 行的 import 改为：

```rust
use engine::backends::onnx_token_classifier::OnnxTokenClassifier;
```

将 `lib.rs` 第 75-89 行的 NER 初始化代码改为：

```rust
            let ner_engine = match OnnxTokenClassifier::try_load(&ner_dir) {
                Ok(Some(backend)) => {
                    println!("NER 引擎已加载 (ONNX)");
                    NerEngine::from_backend(Box::new(backend))
                }
                Ok(None) => {
                    println!("NER 引擎未加载模型，降级运行");
                    NerEngine::degraded()
                }
                Err(e) => {
                    eprintln!("NER 引擎加载警告: {}", e);
                    NerEngine::degraded()
                }
            };
```

- [ ] **Step 2: 运行编译验证**

Run: `cd /Users/tanzeshun/workpath/github/dimkey/src-tauri && cargo check`
Expected: 编译成功，无错误

- [ ] **Step 3: 提交**

```bash
cd /Users/tanzeshun/workpath/github/dimkey
git add src-tauri/src/lib.rs
git commit -m "refactor: lib.rs 使用 OnnxTokenClassifier 和 from_backend() 简化 NER 初始化"
```

---

### Task 4: 导出脚本更新 + model_config.json 自动生成

**Files:**
- Modify: `scripts/export_ner_model.py`
- Modify: `scripts/use_ner_model.sh`

- [ ] **Step 1: 更新 export_ner_model.py**

在 MODELS 字典中新增 distilbert-ner 条目（第 19-31 行区域）：

```python
MODELS = {
    "chinese": {
        "hf_id": "shibing624/bert4ner-base-chinese",
        "tokenizer_source": "self",
    },
    "multilingual": {
        "hf_id": "Davlan/xlm-roberta-base-ner-hrl",
        "tokenizer_source": "xlm-roberta-base",
    },
    "distilbert-ner": {
        "hf_id": "dslim/distilbert-NER",
        "tokenizer_source": "self",
    },
}
```

在 `export_model()` 函数末尾（第 88 行 `id2label` 写入之后），添加 `model_config.json` 自动生成：

```python
    # model_config.json — 自动生成模型配置
    # 检测标注方案
    labels = list(id2label.values())
    has_b_prefix = any(l.startswith("B-") for l in labels)
    tagging_scheme = "bio" if has_b_prefix else "ionly"

    # 提取实体标签（去掉 B-/I- 前缀，去重）
    entity_labels = set()
    for l in labels:
        if l.startswith("B-") or l.startswith("I-"):
            entity_labels.add(l[2:])

    # 默认标签映射
    default_map = {
        "PER": "PersonName", "PERSON": "PersonName",
        "ORG": "OrgName", "ORGANIZATION": "OrgName",
        "LOC": "Address", "LOCATION": "Address", "GPE": "Address",
        "TITLE": "Title",
    }

    label_map = {}
    for entity in sorted(entity_labels):
        label_map[entity] = default_map.get(entity)

    model_config = {
        "name": name,
        "tagging_scheme": tagging_scheme,
        "label_map": label_map,
    }
    (target_dir / "model_config.json").write_text(
        json.dumps(model_config, indent=2, ensure_ascii=False)
    )
    print(f"[{name}] 模型配置已写入: {target_dir / 'model_config.json'}")
```

- [ ] **Step 2: 更新 use_ner_model.sh**

将第 18 行的 REQUIRED_FILES 改为：

```bash
REQUIRED_FILES=("model.onnx" "tokenizer.json" "id2label.json" "model_config.json")
```

将第 42-48 行的 rsync 改为：

```bash
rsync -a --delete \
  --include "model.onnx" \
  --include "tokenizer.json" \
  --include "id2label.json" \
  --include "model_config.json" \
  --include ".gitkeep" \
  --exclude "*" \
  "$CACHE_DIR/" "$ACTIVE_DIR/"
```

- [ ] **Step 3: 提交**

```bash
cd /Users/tanzeshun/workpath/github/dimkey
git add scripts/export_ner_model.py scripts/use_ner_model.sh
git commit -m "feat: 导出脚本新增 distilbert-ner 模型定义，自动生成 model_config.json"
```

---

### Task 5: 为现有模型补充 model_config.json

**Files:**
- 无代码变更，仅生成配置文件

- [ ] **Step 1: 为中文模型生成 model_config.json**

如果 `.ner_cache/chinese/` 存在，在其中创建 `model_config.json`：

```json
{
  "name": "chinese",
  "tagging_scheme": "bio",
  "label_map": {
    "PER": "PersonName",
    "ORG": "OrgName",
    "LOC": "Address"
  }
}
```

- [ ] **Step 2: 为多语言模型生成 model_config.json**

如果 `.ner_cache/multilingual/` 存在，在其中创建 `model_config.json`：

```json
{
  "name": "multilingual",
  "tagging_scheme": "bio",
  "label_map": {
    "PER": "PersonName",
    "ORG": "OrgName",
    "LOC": "Address",
    "DATE": null
  }
}
```

- [ ] **Step 3: 如果 resources/ner/ 有激活的模型，同步 model_config.json**

检查 `src-tauri/resources/ner/id2label.json` 是否存在，若存在则根据其内容创建对应的 `model_config.json`。

- [ ] **Step 4: 运行全量测试**

Run: `cd /Users/tanzeshun/workpath/github/dimkey/src-tauri && cargo test -- --nocapture`
Expected: 全部 PASS

- [ ] **Step 5: 提交**

```bash
cd /Users/tanzeshun/workpath/github/dimkey
git add -A .ner_cache/*/model_config.json src-tauri/resources/ner/model_config.json 2>/dev/null; true
git commit -m "chore: 为现有 NER 模型补充 model_config.json"
```

---

### Task 6: 导出 distilbert-NER 模型并验证

**Files:**
- 无代码变更，执行脚本导出模型

- [ ] **Step 1: 导出 distilbert-NER 模型**

Run: `cd /Users/tanzeshun/workpath/github/dimkey && python3.11 scripts/export_ner_model.py distilbert-ner`
Expected: 输出模型到 `.ner_cache/distilbert-ner/`，包含 `model.onnx`、`tokenizer.json`、`id2label.json`、`model_config.json`

- [ ] **Step 2: 验证导出文件**

```bash
ls -lh .ner_cache/distilbert-ner/
cat .ner_cache/distilbert-ner/id2label.json
cat .ner_cache/distilbert-ner/model_config.json
```

Expected: `model.onnx` ~250MB，`id2label.json` 包含 B-PER/I-PER/B-LOC/I-LOC/B-ORG/I-ORG/B-MISC/I-MISC/O，`model_config.json` 的 `tagging_scheme` 为 `"bio"`

- [ ] **Step 3: 激活 distilbert-NER 模型**

Run: `cd /Users/tanzeshun/workpath/github/dimkey && ./scripts/use_ner_model.sh distilbert-ner`
Expected: 模型文件同步到 `src-tauri/resources/ner/`

- [ ] **Step 4: 运行集成测试验证模型加载和推理**

Run: `cd /Users/tanzeshun/workpath/github/dimkey/src-tauri && cargo test engine::backends -- --nocapture`
Expected: 英文测试用例 PASS（`test_detect_english_person_and_org` 识别出 PER/ORG/LOC）

Run: `cd /Users/tanzeshun/workpath/github/dimkey/src-tauri && cargo test -- --nocapture`
Expected: 全部 PASS
