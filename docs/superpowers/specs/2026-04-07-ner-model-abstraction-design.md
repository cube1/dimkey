# NER 模型抽象层设计 — distilbert-NER 集成

**日期**: 2026-04-07
**状态**: 已确认

## 背景

Dimkey 面向英文律师场景，需要将 NER 模型从当前的中文/多语言模型切换为 `dslim/distilbert-NER`（MIT 许可，~250MB，92% F1）。同时抽象模型层，为未来添加 GLiNER 等模型做准备。

当前架构问题：`OnnxBackend` 耦合了 ONNX 推理、BIO 后处理、标签映射（`build_label_map()` 硬编码 PER/ORG/LOC/TITLE）。无法支持不同标注方案（如 Piiranha 的 I-only）或不同架构（如 GLiNER）。

## 设计方案：模型配置外置 + 后处理策略解耦（方案 B）

### 1. 模型配置文件 `model_config.json`

每个模型目录（`resources/ner/` 或 `.ner_cache/<name>/`）新增 `model_config.json`：

```json
{
  "name": "distilbert-ner",
  "tagging_scheme": "bio",
  "label_map": {
    "PER": "PersonName",
    "ORG": "OrgName",
    "LOC": "Address",
    "MISC": null
  }
}
```

- `tagging_scheme`：`"bio"` 或 `"ionly"`，决定后处理逻辑
- `label_map`：模型原始标签 → `SensitiveType` 变体名。值为 `null` 表示跳过该标签
- 现有 `id2label.json` 保留（ONNX 输出 ID → 标签名），`model_config.json` 是上层业务映射
- 加载顺序：`id2label.json`（ID→标签）→ `model_config.json`（标签→SensitiveType）

### 2. OnnxBackend 重构

`OnnxBackend` 重命名为 `OnnxTokenClassifier`：

```
OnnxTokenClassifier
  ├── session: Session              // ONNX 推理（不变）
  ├── tokenizer: Tokenizer          // 分词（不变）
  ├── id2label: Vec<String>         // ID→标签（不变，从 id2label.json）
  ├── needs_token_type_ids: bool    // 动态检测（不变）
  ├── tagging_scheme: TaggingScheme // 新：Bio | IOnly
  └── label_map: HashMap<String, SensitiveType>  // 新：从 model_config.json 加载
```

变化点：

1. **`build_label_map()` 移除** — 改为从 `model_config.json` 读取
2. **后处理分支** — `detect_text()` 根据 `tagging_scheme` 走不同分支：
   - `Bio`：遇 `B-` 开始新实体，`I-` 续接（现有逻辑）
   - `IOnly`：遇 `I-X`（前一个是 `O` 或不同类型）开始新实体，同类型续接
3. **`try_load()` 内部完成所有加载** — 返回的 backend 已包含 label_map
4. **向后兼容**：`model_config.json` 不存在时回退到现有行为（自动推断 BIO + 硬编码映射）

### 3. NerBackend trait 与 NerEngine 变更

**NerBackend trait** 新增 `label_map()` 方法：

```rust
pub trait NerBackend: Send {
    fn detect_text(&mut self, text: &str) -> Result<Vec<RawEntity>, String>;
    fn is_loaded(&self) -> bool;
    fn label_map(&self) -> &HashMap<String, SensitiveType>;  // 新增
}
```

**NerEngine** 新增 `from_backend()` 构造方法：

```rust
pub fn from_backend(backend: Box<dyn NerBackend>) -> Self {
    let label_map = backend.label_map().clone();
    Self { backend: Some(backend), label_map }
}
```

`lib.rs` 初始化简化为：

```rust
NerEngine::from_backend(Box::new(backend))
```

### 4. 导出脚本与模型切换

**`export_ner_model.py`**：MODELS 新增 `distilbert-ner` 条目，导出时自动生成 `model_config.json`（分析 id2label 前缀判断 tagging_scheme，使用默认映射表）。

**`use_ner_model.sh`**：REQUIRED_FILES 新增 `model_config.json`。

### 5. 不变的部分

- `SensitiveType` 枚举 — 不新增值，MISC 映射为 null 跳过
- `DetectSource` 枚举 — 所有 token 分类模型统一标记为 `Ner`
- `RawEntity` 结构 — 后处理输出格式不变
- `NerEngine::detect()` — FileContent 拆分和映射逻辑不变

## 文件变更清单

| 文件 | 变更类型 | 内容 |
|------|---------|------|
| `src-tauri/src/engine/backends/onnx_backend.rs` | 重构 | 重命名为 `onnx_token_classifier.rs`，加载 model_config.json，后处理支持 Bio/IOnly，移除 build_label_map()，新增 label_map() |
| `src-tauri/src/engine/backends/mod.rs` | 改 | 模块名更新 |
| `src-tauri/src/engine/ner_engine.rs` | 改 | trait 新增 label_map()，新增 from_backend() |
| `src-tauri/src/lib.rs` | 改 | 初始化简化，import 路径更新 |
| `scripts/export_ner_model.py` | 改 | 新增 distilbert-ner 定义，生成 model_config.json |
| `scripts/use_ner_model.sh` | 改 | REQUIRED_FILES 加 model_config.json |
| `src-tauri/resources/ner/model_config.json` | 新增 | 当前激活模型的配置 |

## 模型信息

- **名称**: dslim/distilbert-NER
- **许可证**: MIT（可商用）
- **架构**: DistilBERT + token classification head
- **实体类型**: PER, LOC, ORG, MISC（CoNLL-2003）
- **F1**: ~92%（CoNLL-2003 test set）
- **大小**: 65.2M 参数，~250MB ONNX
- **标注方案**: BIO
- **与现有正则引擎互补**: NER 做人名/机构/地址，正则做 SSN/电话/邮箱/信用卡等结构化 PII
