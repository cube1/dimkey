# 多语言 NER 模型适配实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 NER 引擎从字符级中文 BERT 分词替换为 HuggingFace `tokenizers` crate + XLM-R 多语言模型，支持中英文实体识别。

**Architecture:** 改造 `OnnxBackend`，用 `tokenizers::Tokenizer` 替代手动 vocab 解析和字符级分词。加载 `tokenizer.json` 替代 `vocab.txt`。BIO 解码适配子词 offset mapping。`NerBackend` trait 接口不变，上层代码无需改动。

**Tech Stack:** Rust, `tokenizers` crate (0.21), `ort` crate (2.0.0-rc.11), ONNX Runtime, Davlan/xlm-roberta-base-ner-hrl (INT8)

---

### Task 1: 准备模型文件

**Files:**
- Create: `scripts/prepare_ner_model.py`
- Modify: `src-tauri/resources/ner/id2label.json`
- Create: `src-tauri/resources/ner/tokenizer.json` (由脚本生成)
- Create: `src-tauri/resources/ner/model.onnx` (由脚本生成)

- [ ] **Step 1: 创建模型导出脚本**

```python
#!/usr/bin/env python3
"""导出 Davlan/xlm-roberta-base-ner-hrl 为 ONNX INT8 格式"""
import json
import shutil
from pathlib import Path

def main():
    from optimum.onnxruntime import ORTModelForTokenClassification, ORTQuantizer
    from optimum.onnxruntime.configuration import AutoQuantizationConfig
    from transformers import AutoTokenizer

    model_name = "Davlan/xlm-roberta-base-ner-hrl"
    export_dir = Path("onnx_export")
    int8_dir = Path("onnx_export_int8")
    target_dir = Path("src-tauri/resources/ner")

    # 1. 导出 ONNX
    print(f"导出 {model_name} → ONNX...")
    model = ORTModelForTokenClassification.from_pretrained(model_name, export=True)
    tokenizer = AutoTokenizer.from_pretrained(model_name)
    model.save_pretrained(export_dir)
    tokenizer.save_pretrained(export_dir)

    # 2. INT8 动态量化
    print("INT8 动态量化...")
    quantizer = ORTQuantizer.from_pretrained(export_dir)
    qconfig = AutoQuantizationConfig.avx512_vnni(is_static=False, per_channel=False)
    quantizer.quantize(save_dir=int8_dir, quantization_config=qconfig)

    # 3. 复制到项目目录
    target_dir.mkdir(parents=True, exist_ok=True)

    # model.onnx
    quantized = int8_dir / "model_quantized.onnx"
    if not quantized.exists():
        quantized = int8_dir / "model.onnx"
    shutil.copy2(quantized, target_dir / "model.onnx")
    print(f"模型已复制: {target_dir / 'model.onnx'}")

    # tokenizer.json
    shutil.copy2(export_dir / "tokenizer.json", target_dir / "tokenizer.json")
    print(f"分词器已复制: {target_dir / 'tokenizer.json'}")

    # id2label.json — 从 config.json 提取
    config = json.loads((export_dir / "config.json").read_text())
    id2label = config.get("id2label", {})
    (target_dir / "id2label.json").write_text(json.dumps(id2label, indent=2, ensure_ascii=False))
    print(f"标签映射已写入: {target_dir / 'id2label.json'}")

    # 清理
    print("清理临时目录...")
    shutil.rmtree(export_dir, ignore_errors=True)
    shutil.rmtree(int8_dir, ignore_errors=True)
    print("完成!")

if __name__ == "__main__":
    main()
```

- [ ] **Step 2: 运行脚本导出模型**

Run:
```bash
cd /Users/tanzs-mac-mini/workpath/personal/desensitize-tool
pip install optimum[onnxruntime] transformers torch
python scripts/prepare_ner_model.py
```

Expected: `src-tauri/resources/ner/` 下生成 `model.onnx`、`tokenizer.json`、`id2label.json` 三个文件。

- [ ] **Step 3: 验证生成的文件**

Run:
```bash
ls -lh src-tauri/resources/ner/
cat src-tauri/resources/ner/id2label.json
```

Expected:
- `model.onnx` 存在（~200-400MB）
- `tokenizer.json` 存在
- `id2label.json` 包含 B-PER/I-PER/B-ORG/I-ORG/B-LOC/I-LOC/B-DATE/I-DATE/O 标签

- [ ] **Step 4: Commit**

```bash
git add scripts/prepare_ner_model.py
git commit -m "chore: 添加 NER 多语言模型导出脚本"
```

注意：模型文件（`model.onnx`, `tokenizer.json`）已在 `.gitignore` 中排除，不要提交二进制文件。如果 `.gitignore` 未排除，需要先添加规则。

---

### Task 2: 添加 `tokenizers` 依赖

**Files:**
- Modify: `src-tauri/Cargo.toml:35`

- [ ] **Step 1: 在 Cargo.toml 添加 tokenizers 依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 中，在 `ort` 行之后添加：

```toml
tokenizers = { version = "0.21", default-features = false, features = ["onig"] }
```

- [ ] **Step 2: 验证依赖能编译**

Run:
```bash
cd src-tauri && cargo check
```

Expected: 编译成功，无错误。如果 `tokenizers` 的 `onig` feature 在当前平台有问题，改用：
```toml
tokenizers = { version = "0.21", default-features = false }
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "chore: 添加 tokenizers crate 依赖"
```

---

### Task 3: 重写 OnnxBackend 分词和推理逻辑

**Files:**
- Modify: `src-tauri/src/engine/backends/onnx_backend.rs`

- [ ] **Step 1: 重写 OnnxBackend 结构体和 try_load**

将 `src-tauri/src/engine/backends/onnx_backend.rs` 的**全部内容**替换为：

```rust
use std::collections::HashMap;
use std::path::Path;
use std::fs;
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;
use crate::models::sensitive::SensitiveType;
use super::super::ner_engine::{NerBackend, RawEntity};

/// 基于 ONNX Runtime 的 NER 推理后端（支持 HuggingFace tokenizers 分词）
pub struct OnnxBackend {
    session: Session,
    /// HuggingFace tokenizer（加载 tokenizer.json）
    tokenizer: Tokenizer,
    /// 模型输出 ID → BIO 标签
    id2label: Vec<String>,
    /// ONNX 模型是否需要 token_type_ids 输入
    needs_token_type_ids: bool,
}

impl OnnxBackend {
    /// 尝试从指定目录加载 ONNX 模型
    /// 文件不存在返回 Ok(None)，加载出错返回 Err
    pub fn try_load(model_dir: &Path) -> Result<Option<Self>, String> {
        let model_path = model_dir.join("model.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");
        let label_path = model_dir.join("id2label.json");

        // 任一文件不存在则返回 None（优雅降级）
        if !model_path.exists() || !tokenizer_path.exists() || !label_path.exists() {
            // 兼容提示：如果有旧格式 vocab.txt 但没有 tokenizer.json
            if model_path.exists() && model_dir.join("vocab.txt").exists() && !tokenizer_path.exists() {
                eprintln!("警告: 检测到旧格式 vocab.txt，请使用 scripts/prepare_ner_model.py 重新导出模型");
            }
            return Ok(None);
        }

        // 加载 tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| format!("加载 tokenizer.json 失败: {}", e))?;

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
        let session = Session::builder()
            .map_err(|e| format!("创建 ONNX Session Builder 失败: {}", e))?
            .commit_from_file(&model_path)
            .map_err(|e| format!("加载 ONNX 模型失败: {}", e))?;

        // 检查模型是否需要 token_type_ids（BERT 需要，XLM-R 不需要）
        let needs_token_type_ids = session.inputs.iter()
            .any(|input| input.name == "token_type_ids");

        Ok(Some(Self { session, tokenizer, id2label, needs_token_type_ids }))
    }

    /// 从 id2label 自动构建标签映射表
    pub fn build_label_map(&self) -> HashMap<String, SensitiveType> {
        let mut map = HashMap::new();
        for label in &self.id2label {
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

    /// 将字节偏移量转换为字符偏移量
    fn byte_offset_to_char_offset(text: &str, byte_offset: usize) -> usize {
        text[..byte_offset.min(text.len())].chars().count()
    }
}

impl NerBackend for OnnxBackend {
    fn detect_text(&mut self, text: &str) -> Result<Vec<RawEntity>, String> {
        if text.trim().is_empty() {
            return Ok(vec![]);
        }

        // 使用 tokenizer 编码（add_special_tokens = true，自动添加 <s>...</s>）
        let encoding = self.tokenizer.encode(text, true)
            .map_err(|e| format!("分词失败: {}", e))?;

        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let attention_mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&m| m as i64).collect();
        let offsets = encoding.get_offsets();
        let seq_len = input_ids.len();

        if seq_len == 0 {
            return Ok(vec![]);
        }

        // 构建 ort Tensor
        let shape = vec![1i64, seq_len as i64];

        let input_ids_tensor = Tensor::from_array((shape.clone(), input_ids.into_boxed_slice()))
            .map_err(|e| format!("构建 input_ids 张量失败: {}", e))?;
        let attention_mask_tensor = Tensor::from_array((shape.clone(), attention_mask.into_boxed_slice()))
            .map_err(|e| format!("构建 attention_mask 张量失败: {}", e))?;

        // 运行推理（根据模型需求动态构建输入）
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

        // 解析输出：shape [1, seq_len, num_labels]
        let (output_shape, logits_data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("提取输出张量失败: {}", e))?;

        let num_labels = if output_shape.len() == 3 {
            output_shape[2] as usize
        } else {
            self.id2label.len()
        };

        // 对每个 token 取 argmax 得到标签 ID，同时计算 softmax confidence
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

            // softmax 近似：exp(max) / sum(exp(all))
            let sum_exp: f32 = (0..num_labels)
                .map(|j| (logits_data[offset + j] - max_val).exp())
                .sum();
            confidences.push(1.0 / sum_exp);
        }

        // BIO 后处理：利用 offset mapping 将子词标签还原为原文实体
        let mut items = Vec::new();
        let mut entity_start_byte: Option<usize> = None;
        let mut entity_end_byte: Option<usize> = None;
        let mut entity_label: Option<String> = None;
        let mut entity_confidence_sum: f32 = 0.0;
        let mut entity_token_count: usize = 0;

        let flush_entity = |start_byte: usize, end_byte: usize, label: &str, conf_sum: f32, count: usize, text: &str, items: &mut Vec<RawEntity>| {
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
        };

        for i in 0..seq_len {
            let (tok_start, tok_end) = offsets[i];
            // 跳过特殊 token（offset 为 (0, 0) 且不是第一个有效字符）
            if tok_start == 0 && tok_end == 0 {
                // 结束当前实体
                if let (Some(sb), Some(eb), Some(lbl)) = (entity_start_byte, entity_end_byte, entity_label.take()) {
                    flush_entity(sb, eb, &lbl, entity_confidence_sum, entity_token_count, text, &mut items);
                }
                entity_start_byte = None;
                entity_end_byte = None;
                entity_confidence_sum = 0.0;
                entity_token_count = 0;
                continue;
            }

            let label = &self.id2label[label_ids[i]];

            if label.starts_with("B-") {
                // 先结束上一个实体
                if let (Some(sb), Some(eb), Some(lbl)) = (entity_start_byte, entity_end_byte, entity_label.take()) {
                    flush_entity(sb, eb, &lbl, entity_confidence_sum, entity_token_count, text, &mut items);
                }
                entity_start_byte = Some(tok_start);
                entity_end_byte = Some(tok_end);
                entity_label = Some(label[2..].to_string());
                entity_confidence_sum = confidences[i];
                entity_token_count = 1;
            } else if label.starts_with("I-") {
                let current_entity = &label[2..];
                if entity_label.as_deref() == Some(current_entity) {
                    // 继续当前实体，扩展 end
                    entity_end_byte = Some(tok_end);
                    entity_confidence_sum += confidences[i];
                    entity_token_count += 1;
                } else {
                    // I- 与当前实体不匹配，结束当前实体
                    if let (Some(sb), Some(eb), Some(lbl)) = (entity_start_byte, entity_end_byte, entity_label.take()) {
                        flush_entity(sb, eb, &lbl, entity_confidence_sum, entity_token_count, text, &mut items);
                    }
                    entity_start_byte = None;
                    entity_end_byte = None;
                    entity_label = None;
                    entity_confidence_sum = 0.0;
                    entity_token_count = 0;
                }
            } else {
                // O 标签，结束当前实体
                if let (Some(sb), Some(eb), Some(lbl)) = (entity_start_byte, entity_end_byte, entity_label.take()) {
                    flush_entity(sb, eb, &lbl, entity_confidence_sum, entity_token_count, text, &mut items);
                }
                entity_start_byte = None;
                entity_end_byte = None;
                entity_label = None;
                entity_confidence_sum = 0.0;
                entity_token_count = 0;
            }
        }

        // 处理末尾实体
        if let (Some(sb), Some(eb), Some(lbl)) = (entity_start_byte, entity_end_byte, entity_label) {
            flush_entity(sb, eb, &lbl, entity_confidence_sum, entity_token_count, text, &mut items);
        }

        Ok(items)
    }

    fn is_loaded(&self) -> bool {
        true
    }
}

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
        let backend = OnnxBackend::try_load(&dir).unwrap();
        assert!(backend.is_some(), "模型加载失败");
        let backend = backend.unwrap();
        assert!(backend.is_loaded());
        let label_map = backend.build_label_map();
        assert!(label_map.contains_key("PER"), "缺少 PER 标签");
        assert!(label_map.contains_key("LOC"), "缺少 LOC 标签");
        assert!(label_map.contains_key("ORG"), "缺少 ORG 标签");
    }

    #[test]
    fn test_detect_chinese_person_and_location() {
        let dir = model_dir();
        if !dir.join("model.onnx").exists() {
            println!("跳过：模型文件不存在");
            return;
        }
        let mut backend = OnnxBackend::try_load(&dir).unwrap().unwrap();
        let entities = backend.detect_text("张三在北京市海淀区工作").unwrap();
        let labels: Vec<&str> = entities.iter().map(|e| e.label.as_str()).collect();
        let texts: Vec<&str> = entities.iter().map(|e| e.text.as_str()).collect();
        println!("中文识别结果: {:?}", entities.iter().map(|e| (&e.text, &e.label, e.confidence)).collect::<Vec<_>>());
        assert!(labels.contains(&"PER"), "应识别出人名，实际: {:?}", texts);
        assert!(labels.contains(&"LOC"), "应识别出地名，实际: {:?}", texts);
    }

    #[test]
    fn test_detect_english_person_and_org() {
        let dir = model_dir();
        if !dir.join("model.onnx").exists() {
            println!("跳过：模型文件不存在");
            return;
        }
        let mut backend = OnnxBackend::try_load(&dir).unwrap().unwrap();
        let entities = backend.detect_text("John Smith works at Google in New York").unwrap();
        let labels: Vec<&str> = entities.iter().map(|e| e.label.as_str()).collect();
        let texts: Vec<&str> = entities.iter().map(|e| e.text.as_str()).collect();
        println!("英文识别结果: {:?}", entities.iter().map(|e| (&e.text, &e.label, e.confidence)).collect::<Vec<_>>());
        assert!(labels.contains(&"PER"), "应识别出人名，实际: {:?}", texts);
        assert!(labels.contains(&"ORG"), "应识别出组织名，实际: {:?}", texts);
        assert!(labels.contains(&"LOC"), "应识别出地名，实际: {:?}", texts);
    }

    #[test]
    fn test_detect_mixed_language() {
        let dir = model_dir();
        if !dir.join("model.onnx").exists() {
            println!("跳过：模型文件不存在");
            return;
        }
        let mut backend = OnnxBackend::try_load(&dir).unwrap().unwrap();
        let entities = backend.detect_text("李明在Google北京办公室工作").unwrap();
        let labels: Vec<&str> = entities.iter().map(|e| e.label.as_str()).collect();
        let texts: Vec<&str> = entities.iter().map(|e| e.text.as_str()).collect();
        println!("中英混排识别结果: {:?}", entities.iter().map(|e| (&e.text, &e.label, e.confidence)).collect::<Vec<_>>());
        // 至少应识别出人名或组织名
        assert!(!entities.is_empty(), "中英混排文本应识别出实体，实际为空");
    }

    #[test]
    fn test_detect_org() {
        let dir = model_dir();
        if !dir.join("model.onnx").exists() {
            println!("跳过：模型文件不存在");
            return;
        }
        let mut backend = OnnxBackend::try_load(&dir).unwrap().unwrap();
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
        let mut backend = OnnxBackend::try_load(&dir).unwrap().unwrap();
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
        let mut backend = OnnxBackend::try_load(&dir).unwrap().unwrap();

        // 验证中文文本的字符偏移量正确性
        let text = "张三在北京工作";
        let entities = backend.detect_text(text).unwrap();
        for entity in &entities {
            // 用字符偏移量从原文提取的文本应与 entity.text 一致
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
        let mut backend = OnnxBackend::try_load(&dir).unwrap().unwrap();
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
        let mut backend = OnnxBackend::try_load(&dir).unwrap().unwrap();
        assert!(backend.detect_text("").unwrap().is_empty());
        assert!(backend.detect_text("   ").unwrap().is_empty());
    }
}
```

- [ ] **Step 2: 验证编译通过**

Run:
```bash
cd src-tauri && cargo check
```

Expected: 编译成功，无错误。

- [ ] **Step 3: 运行测试（需要模型文件）**

Run:
```bash
cd src-tauri && cargo test engine::backends::onnx_backend -- --nocapture
```

Expected: 如果模型文件存在，所有测试通过，包括新增的英文和中英混排测试。如果模型文件不存在，测试自动跳过。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/engine/backends/onnx_backend.rs
git commit -m "feat(ner): 重写 OnnxBackend，使用 tokenizers crate 支持多语言分词"
```

---

### Task 4: 确保 .gitignore 排除模型二进制文件

**Files:**
- Modify: `.gitignore` 或 `src-tauri/.gitignore`

- [ ] **Step 1: 检查 .gitignore 是否已排除模型文件**

Run:
```bash
cat .gitignore | grep -i ner
cat .gitignore | grep -i onnx
```

如果没有相关规则，在 `.gitignore` 中添加：

```
# NER 模型二进制文件（通过脚本下载，不入 git）
src-tauri/resources/ner/model.onnx
src-tauri/resources/ner/tokenizer.json
src-tauri/resources/ner/id2label.json
```

- [ ] **Step 2: Commit**

```bash
git add .gitignore
git commit -m "chore: gitignore 排除 NER 模型二进制文件"
```

---

### Task 5: 端到端验证

**Files:** 无新增修改，纯验证

- [ ] **Step 1: 运行全量 Rust 测试**

Run:
```bash
cd src-tauri && cargo test
```

Expected: 所有测试通过。`ner_engine.rs` 中的 mock 测试不受影响（`NerBackend` trait 接口未变）。

- [ ] **Step 2: 启动 dev 模式验证集成**

Run:
```bash
cargo tauri dev
```

Expected:
- 控制台输出 `NER 引擎已加载 (ONNX)` 或 `NER 引擎未加载模型，降级运行`（取决于模型文件是否存在）
- 应用正常启动，拖入文件后识别功能正常
- 如果模型已加载，英文文档中的人名/机构名/地名应能被识别

- [ ] **Step 3: 手动测试中英文识别**

在应用中分别测试：
1. 纯中文文档：人名、机构名、地址应被识别
2. 纯英文文档：Person names、Organization names、Locations 应被识别
3. 中英混排文档：两种语言的实体都应被识别

- [ ] **Step 4: 最终 Commit（如有调整）**

```bash
git add -A
git commit -m "test: 验证多语言 NER 模型集成"
```
