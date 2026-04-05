# NER 模型快速切换设计

## 背景

Dimkey 的 NER 引擎通过 ONNX Runtime 加载模型，识别机构名、人名、地址。目前有两个候选模型：

- **中文 BERT** — `shibing624/bert4ner-base-chinese`（`scripts/export_ner_model.py`）
- **多语言 XLM-R** — `Davlan/xlm-roberta-base-ner-hrl`（`scripts/prepare_ner_model.py`）

v0.5.1 发版用的是多语言模型。开发者在本地评估/对比两个模型效果时，需要反复切换，但目前每次切换都要重新跑导出脚本（下载 + INT8 量化），单次耗时 2-5 分钟，体验差。

此外，中文 BERT 的导出脚本只产出 `vocab.txt`，而 Rust 后端 `onnx_backend.rs::try_load` 硬编码检查 `tokenizer.json`，导致中文模型目前无法在 Rust 侧加载。

## 目标

- 开发者本地一条命令切换模型，切换耗时 <1 秒
- 首次使用某个模型时按需下载，之后从本地缓存恢复
- Rust 代码、Tauri 打包配置、发版流程零改动
- 发版时仍只打包一个模型（dev-only 切换，不涉及运行期切换）

## 非目标

- 运行期切换模型（需要同时打包多个模型，包体积翻倍）
- 发版期按平台选不同模型
- 支持任意第三方模型（只聚焦当前两个）

## 设计

### 目录结构

```
.ner_cache/                        # 新增，gitignored
├── chinese/
│   ├── model.onnx
│   ├── tokenizer.json
│   └── id2label.json
└── multilingual/
    ├── model.onnx
    ├── tokenizer.json
    └── id2label.json

src-tauri/resources/ner/           # 激活模型（Rust 加载 + Tauri 打包）
├── model.onnx
├── tokenizer.json
└── id2label.json
```

`.ner_cache/` 存放所有已下载的模型，`src-tauri/resources/ner/` 是当前激活的一份副本。Rust 和 Tauri 打包逻辑只看 `src-tauri/resources/ner/`，无需感知缓存机制。

### 用户接口

```bash
./scripts/use_ner_model.sh chinese
./scripts/use_ner_model.sh multilingual
```

行为：
1. 检查 `.ner_cache/<name>/` 是否存在且包含三个必需文件
2. 不存在则调用 `scripts/export_ner_model.py <name>` 下载并导出到 `.ner_cache/<name>/`
3. 用 `rsync -a --delete .ner_cache/<name>/ src-tauri/resources/ner/` 同步到激活目录
4. 打印当前激活的模型名和下次构建时间戳提示

### 统一的导出脚本

将现有的两个 Python 脚本合并为 `scripts/export_ner_model.py`，接受一个位置参数：

```bash
python3.11 scripts/export_ner_model.py chinese       # 导出到 .ner_cache/chinese/
python3.11 scripts/export_ner_model.py multilingual  # 导出到 .ner_cache/multilingual/
```

脚本内部维护模型配置字典：

```python
MODELS = {
    "chinese": {
        "hf_id": "shibing624/bert4ner-base-chinese",
        "tokenizer_source": "self",  # 从模型自带 tokenizer 转 tokenizer.json
    },
    "multilingual": {
        "hf_id": "Davlan/xlm-roberta-base-ner-hrl",
        "tokenizer_source": "xlm-roberta-base",  # 从基础模型借用（规避 transformers bug）
    },
}
```

两个模型共用导出 + 量化 + 文件复制逻辑，差异只在 `hf_id` 和 tokenizer 来源。

### 中文 BERT 的 tokenizer.json 兼容性

现有 `export_ner_model.py` 只输出 `vocab.txt`，这是 BERT 的词表格式，但 Rust 用 HuggingFace `tokenizers` crate 的 `Tokenizer::from_file` 加载，需要 `tokenizer.json`。

BERT 模型可以用 `AutoTokenizer.from_pretrained(..., use_fast=True).save_pretrained()` 直接保存为 `tokenizer.json`（不会触发 xlm-roberta 的 tiktoken 转换 bug，因为 BertTokenizer 不走那条路径）。

验证点：导出后跑一次 Rust 测试 `cargo test engine::backends::onnx_backend` 确认模型能加载。

### 文件管理

- `.ner_cache/` 加入 `.gitignore`
- `src-tauri/resources/ner/` 里的模型文件**继续 gitignore**（与现状一致）
- `.ner_cache/` 首次占用约 500MB（中文 50-100MB + 多语言 265MB），开发机可接受

### 废弃项

- `scripts/prepare_ner_model.py` 删除，功能并入统一脚本

## 影响面

| 文件 | 改动 |
|------|------|
| `scripts/export_ner_model.py` | 重写：参数化支持两个模型，统一输出到 `.ner_cache/<name>/` |
| `scripts/prepare_ner_model.py` | 删除 |
| `scripts/use_ner_model.sh` | 新增：切换脚本 |
| `.gitignore` | 新增 `.ner_cache/` |
| `.github/workflows/release.yml` | cache key 参数化，`prepare-model` job 调用 `use_ner_model.sh multilingual` |
| Rust 代码 | 零改动 |
| `tauri.conf.json` | 零改动 |

## 测试策略

- 手工验证：依次 `./scripts/use_ner_model.sh chinese && cargo tauri dev` 和 `./scripts/use_ner_model.sh multilingual && cargo tauri dev`，确认两个模型都能正确加载并识别
- 单元测试：`cargo test engine::backends::onnx_backend` 在激活任一模型后能跑通
- CI 测试：`prepare-model` job 沿用现有逻辑（下载 → 缓存 → build job 恢复），改动仅限 cache key 命名

## 风险

- **中文 BERT 的 `tokenizer.json` 生成可能失败** — transformers 库在 xlm-roberta 上有兼容问题，BERT 路径虽然理论上不受影响，但仍需实测验证。若失败，fallback 方案：改 Rust 后端同时支持 `vocab.txt`（BERT WordPiece）和 `tokenizer.json`（HF 格式），让 Python 脚本输出哪种都行。
- **`.ner_cache/` 占用磁盘** — 开发者首次切换两个模型后会占约 500MB，可接受。
