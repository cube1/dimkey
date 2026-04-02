# 多语言 NER 模型适配设计

## 背景

Dimkey当前 NER 引擎使用字符级分词（针对中文 BERT 设计），无法支持英文实体识别。需要替换为多语言模型以同时覆盖中英文 NER。

## 模型选择

**Davlan/xlm-roberta-base-ner-hrl**，INT8 量化。

| 属性 | 值 |
|---|---|
| 参数量 | 278M |
| ONNX INT8 体积 | ~278MB |
| 英文 F1 (WikiANN) | ~88% |
| 中文 F1 (WikiANN) | ~81% |
| 实体类型 | PER, ORG, LOC, DATE |
| 分词器 | SentencePiece BPE (250k vocab) |

选择理由：
- 一个模型覆盖中英文，架构简单
- XLM-R 训练语料大（2.5TB CommonCrawl），250k 词表对中文覆盖优于 mBERT
- 中文 F1 ~81% 配合正则引擎 + 自定义词典，综合识别率足够
- SentencePiece 对中英混排文档（如中文文档中的英文公司名）表现好
- INT8 量化后 ~278MB，桌面应用可接受

## 改动范围

### 需要改的文件

1. **`src-tauri/Cargo.toml`** — 新增 `tokenizers` 依赖
2. **`src-tauri/src/engine/backends/onnx_backend.rs`** — 重写分词逻辑和 BIO 解码
3. **`src-tauri/resources/ner/`** — 替换模型文件

### 不需要改的文件

- `ner_engine.rs` — `NerBackend` trait 接口和 `RawEntity` 格式不变
- `models/sensitive.rs` — `SensitiveType` 枚举已包含所需类型
- `commands/detect.rs` — 调用方式不变
- 前端代码 — 完全不受影响

## 详细设计

### 1. 模型文件结构变化

```
resources/ner/
├── model.onnx         # xlm-roberta-base-ner-hrl INT8 量化版
├── tokenizer.json     # HuggingFace tokenizer 配置（替代 vocab.txt）
└── id2label.json      # 标签映射（更新内容）
```

`vocab.txt` 不再需要，由 `tokenizer.json` 替代。`tokenizer.json` 是 HuggingFace tokenizers 库的标准格式，包含完整的 BPE 词表和 merge 规则。

### 2. `onnx_backend.rs` 改造

#### 2.1 结构体变更

```rust
// 之前
pub struct OnnxBackend {
    session: Session,
    vocab: HashMap<String, i64>,      // 移除
    id2label: Vec<String>,
}

// 之后
use tokenizers::Tokenizer;

pub struct OnnxBackend {
    session: Session,
    tokenizer: Tokenizer,             // 新增
    id2label: Vec<String>,
}
```

#### 2.2 加载逻辑变更

`try_load()` 中：
- 移除 `vocab.txt` 的手动解析
- 改为 `Tokenizer::from_file(tokenizer_path)` 加载 `tokenizer.json`
- 检查文件从 `vocab.txt` 改为 `tokenizer.json`

#### 2.3 分词逻辑变更

`detect_text()` 中：

**之前**（字符级）：
```
text.chars() → 逐字符查 vocab → [CLS] + char_ids + [SEP]
```

**之后**（tokenizers crate）：
```
tokenizer.encode(text, true) → 自动添加特殊 token → input_ids + attention_mask + offsets
```

关键点：
- `tokenizer.encode()` 自动处理 `<s>` / `</s>` 特殊 token（XLM-R 用这些而非 `[CLS]`/`[SEP]`）
- 返回的 `encoding.get_offsets()` 提供每个 token 对应的原文 `(start_byte, end_byte)` 偏移量
- XLM-R 不需要 `token_type_ids`，ONNX 模型输入只需 `input_ids` + `attention_mask`

#### 2.4 BIO 解码适配子词

字符级模型中 1 token = 1 字符，偏移量直接对应。子词模型中需要用 offset mapping 还原：

```
输入: "John works at Google"
tokens: ["<s>", "▁John", "▁works", "▁at", "▁Google", "</s>"]
offsets: [(0,0), (0,4), (5,10), (11,13), (14,20), (0,0)]
labels: [O, B-PER, O, O, B-ORG, O]

→ RawEntity { text: "John", label: "PER", start: 0, end: 4 }
→ RawEntity { text: "Google", label: "ORG", start: 14, end: 20 }
```

注意事项：
- 特殊 token（`<s>`, `</s>`）的 offset 为 `(0, 0)`，跳过
- 多个子词 token 属于同一实体时（如 `["▁un", "happi", "ness"]` 都标为 `I-PER`），取第一个 token 的 start 和最后一个 token 的 end
- offset 返回的是**字节偏移量**，需转换为**字符偏移量**（对中文尤其重要，一个中文字符 = 3 字节 UTF-8）
- confidence 从 logits 中提取 softmax 后的最大值，替代当前硬编码的 0.8

#### 2.5 模型输入张量

XLM-R 与 BERT 的输入差异：

| | BERT (当前) | XLM-R (改后) |
|---|---|---|
| 输入 | input_ids, attention_mask, token_type_ids | input_ids, attention_mask |
| 特殊 token | [CLS]=101, [SEP]=102 | \<s\>=0, \</s\>=2 |
| token_type_ids | 全 0 | 不需要 |

推理调用时移除 `token_type_ids` 输入（或根据 ONNX 模型实际导出的输入名动态适配）。

### 3. 标签映射更新

`id2label.json` 更新为 xlm-roberta-base-ner-hrl 的标签：

```json
{
  "0": "O",
  "1": "B-PER",
  "2": "I-PER",
  "3": "B-ORG",
  "4": "I-ORG",
  "5": "B-LOC",
  "6": "I-LOC",
  "7": "B-DATE",
  "8": "I-DATE"
}
```

`build_label_map()` 已支持 PER/ORG/LOC 映射。新增 DATE 类型的映射需要决定：
- 映射到现有某个 `SensitiveType`（无直接对应）
- 或暂时跳过（当前 `build_label_map` 的 match 分支会自动 `continue` 跳过未识别的标签）

**建议**：暂时跳过 DATE，不新增枚举值。日期信息通常不属于敏感个人信息，regex 引擎已覆盖需要的日期格式。

### 4. 依赖变更

```toml
# Cargo.toml
tokenizers = { version = "0.21", default-features = false, features = ["onig"] }
```

使用 `default-features = false` 避免引入不必要的 Python 绑定等功能。`onig` feature 提供 unicode 正则支持。

## 模型准备流程（开发时手动执行）

```bash
pip install optimum[onnxruntime]

# 导出 ONNX
optimum-cli export onnx \
  --model Davlan/xlm-roberta-base-ner-hrl \
  --task token-classification \
  onnx_export/

# INT8 动态量化
python -c "
from optimum.onnxruntime import ORTQuantizer
from optimum.onnxruntime.configuration import AutoQuantizationConfig
q = ORTQuantizer.from_pretrained('onnx_export/')
qconfig = AutoQuantizationConfig.avx512_vnni(is_static=False, per_channel=False)
q.quantize(save_dir='onnx_export_int8/', quantization_config=qconfig)
"

# 复制到项目
cp onnx_export_int8/model_quantized.onnx src-tauri/resources/ner/model.onnx
cp onnx_export/tokenizer.json src-tauri/resources/ner/tokenizer.json
# id2label.json 从模型 config.json 中提取
```

模型文件通过 `.gitignore` 排除，不入 git。构建/发布时通过脚本或 CI 下载。

## 向后兼容

- 保持 `try_load()` 的优雅降级：文件不存在返回 `Ok(None)`
- 如果用户仍然放了旧格式的 `vocab.txt`（而非 `tokenizer.json`），需要给出明确错误提示
- `NerBackend` trait 接口不变，`NerEngine` 无需修改
