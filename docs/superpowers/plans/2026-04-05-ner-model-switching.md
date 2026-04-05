# NER 模型快速切换 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 开发者本地一条命令切换中文 BERT 或多语言 XLM-R NER 模型，切换耗时 <1 秒，首次使用时自动下载并缓存。

**Architecture:** `.ner_cache/<name>/` 缓存全部已下载模型，`src-tauri/resources/ner/` 是当前激活副本。统一的 Python 导出脚本按参数下载对应模型并写入缓存，shell 切换脚本只负责检查缓存 + rsync 同步。Rust 代码和 Tauri 打包零改动。

**Tech Stack:** Python 3.11 (optimum + transformers + huggingface_hub) · Bash · Rust（只是验证端，不修改）

---

## File Structure

| 文件 | 角色 |
|------|------|
| `scripts/export_ner_model.py` | 统一的 Python 导出脚本（重写），参数化支持 `chinese` / `multilingual`，下载 HF 模型 → INT8 量化 → 输出到 `.ner_cache/<name>/` |
| `scripts/use_ner_model.sh` | 新增 shell 切换脚本：检查 `.ner_cache/<name>/` 是否存在，不存在则调用 export 脚本，之后 rsync 到 `src-tauri/resources/ner/` |
| `scripts/prepare_ner_model.py` | 删除（功能并入 `export_ner_model.py`） |
| `.gitignore` | 新增 `.ner_cache/` 条目 |
| `.github/workflows/release.yml` | `prepare-model` job 改为调用 `use_ner_model.sh multilingual`，移除对已删除脚本的引用 |

每个文件职责单一：Python 负责 HF 模型下载 + ONNX 导出 + 量化；Shell 负责本地缓存管理 + 激活切换；CI 配置复用 shell 脚本避免逻辑重复。

---

## Task 1：重写统一导出脚本（支持参数选择模型）

**Files:**
- Modify/rewrite: `scripts/export_ner_model.py`

- [ ] **Step 1: 重写 `scripts/export_ner_model.py` 为参数化版本**

完整覆盖现有文件，内容如下：

```python
#!/usr/bin/env python3
"""
导出 NER 模型为 ONNX INT8 格式，输出到 .ner_cache/<name>/

使用方法:
  pip install optimum[onnxruntime] transformers torch huggingface_hub
  python scripts/export_ner_model.py chinese
  python scripts/export_ner_model.py multilingual
"""

import json
import shutil
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
CACHE_ROOT = REPO_ROOT / ".ner_cache"

MODELS = {
    "chinese": {
        "hf_id": "shibing624/bert4ner-base-chinese",
        # BERT 模型：tokenizer 从模型自身保存
        "tokenizer_source": "self",
    },
    "multilingual": {
        "hf_id": "Davlan/xlm-roberta-base-ner-hrl",
        # XLM-R：transformers 4.49+ 转 fast tokenizer 有 tiktoken bug，
        # 直接从 xlm-roberta-base 借用 tokenizer.json（共享词表）
        "tokenizer_source": "xlm-roberta-base",
    },
}


def export_model(name: str):
    if name not in MODELS:
        raise SystemExit(f"未知模型: {name}，可选: {list(MODELS.keys())}")

    from optimum.onnxruntime import ORTModelForTokenClassification, ORTQuantizer
    from optimum.onnxruntime.configuration import AutoQuantizationConfig
    from huggingface_hub import hf_hub_download

    cfg = MODELS[name]
    hf_id = cfg["hf_id"]
    target_dir = CACHE_ROOT / name
    export_tmp = CACHE_ROOT / f"_tmp_{name}_fp32"
    int8_tmp = CACHE_ROOT / f"_tmp_{name}_int8"

    print(f"[{name}] 导出 {hf_id} → ONNX...")
    model = ORTModelForTokenClassification.from_pretrained(hf_id, export=True)
    model.save_pretrained(export_tmp)

    print(f"[{name}] INT8 动态量化...")
    quantizer = ORTQuantizer.from_pretrained(export_tmp)
    qconfig = AutoQuantizationConfig.avx512_vnni(is_static=False, per_channel=False)
    quantizer.quantize(save_dir=int8_tmp, quantization_config=qconfig)

    target_dir.mkdir(parents=True, exist_ok=True)

    quantized = int8_tmp / "model_quantized.onnx"
    if not quantized.exists():
        quantized = int8_tmp / "model.onnx"
    shutil.copy2(quantized, target_dir / "model.onnx")
    print(f"[{name}] 模型已复制: {target_dir / 'model.onnx'}")

    if cfg["tokenizer_source"] == "self":
        # BERT：用 AutoTokenizer 保存 fast tokenizer（会生成 tokenizer.json）
        from transformers import AutoTokenizer
        tok = AutoTokenizer.from_pretrained(hf_id, use_fast=True)
        tok.save_pretrained(target_dir)
        # 清理 BERT 保存的多余文件，只保留 tokenizer.json
        for f in target_dir.iterdir():
            if f.name not in {"model.onnx", "tokenizer.json", "id2label.json"}:
                f.unlink()
        if not (target_dir / "tokenizer.json").exists():
            raise SystemExit(f"[{name}] tokenizer.json 生成失败")
    else:
        # XLM-R：从基础模型下载 tokenizer.json
        src = cfg["tokenizer_source"]
        tok_path = hf_hub_download(src, "tokenizer.json")
        shutil.copy2(tok_path, target_dir / "tokenizer.json")
    print(f"[{name}] 分词器已复制: {target_dir / 'tokenizer.json'}")

    # id2label.json — 从 config.json 提取
    config = json.loads((export_tmp / "config.json").read_text())
    id2label = config.get("id2label", {})
    (target_dir / "id2label.json").write_text(
        json.dumps(id2label, indent=2, ensure_ascii=False)
    )
    print(f"[{name}] 标签映射已写入: {target_dir / 'id2label.json'}")

    print(f"[{name}] 清理临时目录...")
    shutil.rmtree(export_tmp, ignore_errors=True)
    shutil.rmtree(int8_tmp, ignore_errors=True)
    print(f"[{name}] 完成！")


def main():
    if len(sys.argv) != 2:
        raise SystemExit(f"用法: python {sys.argv[0]} <{'|'.join(MODELS.keys())}>")
    export_model(sys.argv[1])


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: 运行多语言模型导出，验证脚本工作**

Run:
```bash
rm -rf .ner_cache
/opt/homebrew/opt/python@3.11/bin/python3.11 scripts/export_ner_model.py multilingual
ls -lh .ner_cache/multilingual/
```

Expected output 包含：
- `model.onnx` 约 265MB
- `tokenizer.json` 约 8.7MB
- `id2label.json` 约 150B（内容含 PER/ORG/LOC 标签）

- [ ] **Step 3: 运行中文模型导出，验证 BERT 分支**

Run:
```bash
/opt/homebrew/opt/python@3.11/bin/python3.11 scripts/export_ner_model.py chinese
ls -lh .ner_cache/chinese/
cat .ner_cache/chinese/id2label.json
```

Expected output：
- `model.onnx` 约 50-100MB
- `tokenizer.json` 存在（关键验证：BERT 分支能生成这个文件）
- `id2label.json` 包含 `B-PER` / `B-ORG` / `B-LOC` 等标签

**若 tokenizer.json 生成失败**（transformers 兼容问题），立即停止并报告错误，后续任务需要调整方案（fallback 到修改 Rust 支持 vocab.txt）。

- [ ] **Step 4: Commit**

```bash
git add scripts/export_ner_model.py
git commit -m "$(cat <<'EOF'
refactor: 统一 NER 模型导出脚本支持中文/多语言双模型

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2：删除旧脚本 `prepare_ner_model.py`

**Files:**
- Delete: `scripts/prepare_ner_model.py`

- [ ] **Step 1: 删除文件**

```bash
rm scripts/prepare_ner_model.py
```

- [ ] **Step 2: 确认没有其他地方引用它**

Run:
```bash
grep -rn "prepare_ner_model" . --exclude-dir=node_modules --exclude-dir=target --exclude-dir=.git --exclude-dir=.ner_cache
```

Expected: 无输出（所有引用已在 Task 4 移除）。如有残留引用，说明 CI workflow 未同步更新。

- [ ] **Step 3: Commit**

```bash
git add scripts/prepare_ner_model.py
git commit -m "$(cat <<'EOF'
chore: 删除冗余的 prepare_ner_model.py（已并入 export_ner_model.py）

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3：新增切换脚本 `use_ner_model.sh`

**Files:**
- Create: `scripts/use_ner_model.sh`

- [ ] **Step 1: 创建切换脚本**

文件内容：

```bash
#!/usr/bin/env bash
#
# 切换当前激活的 NER 模型
# 用法: ./scripts/use_ner_model.sh <chinese|multilingual>
#
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "用法: $0 <chinese|multilingual>"
  exit 1
fi

NAME="$1"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CACHE_DIR="$REPO_ROOT/.ner_cache/$NAME"
ACTIVE_DIR="$REPO_ROOT/src-tauri/resources/ner"

REQUIRED_FILES=("model.onnx" "tokenizer.json" "id2label.json")

cache_valid() {
  [ -d "$CACHE_DIR" ] || return 1
  for f in "${REQUIRED_FILES[@]}"; do
    [ -s "$CACHE_DIR/$f" ] || return 1
  done
  return 0
}

if ! cache_valid; then
  echo "缓存不存在或不完整: $CACHE_DIR"
  echo "开始下载并导出模型..."
  PYTHON_BIN="${PYTHON_BIN:-python3.11}"
  "$PYTHON_BIN" "$REPO_ROOT/scripts/export_ner_model.py" "$NAME"
fi

if ! cache_valid; then
  echo "ERROR: 导出后缓存仍然不完整: $CACHE_DIR"
  exit 1
fi

echo "激活模型 → $NAME"
mkdir -p "$ACTIVE_DIR"
rsync -a --delete \
  --include "model.onnx" \
  --include "tokenizer.json" \
  --include "id2label.json" \
  --include ".gitkeep" \
  --exclude "*" \
  "$CACHE_DIR/" "$ACTIVE_DIR/"

# 保留 .gitkeep（如果原来有）
touch "$ACTIVE_DIR/.gitkeep"

echo "完成！当前激活模型: $NAME"
echo "  目录: $ACTIVE_DIR"
ls -lh "$ACTIVE_DIR"
```

- [ ] **Step 2: 赋予执行权限**

```bash
chmod +x scripts/use_ner_model.sh
```

- [ ] **Step 3: 切到中文模型并验证文件被正确同步**

Run:
```bash
./scripts/use_ner_model.sh chinese
ls -lh src-tauri/resources/ner/
```

Expected: 输出中 `model.onnx` 大小接近 `.ner_cache/chinese/model.onnx`（约 50-100MB）。

- [ ] **Step 4: 切回多语言模型并验证**

Run:
```bash
./scripts/use_ner_model.sh multilingual
ls -lh src-tauri/resources/ner/
```

Expected: `model.onnx` 约 265MB。

- [ ] **Step 5: 再次切到中文，确认无需重新下载（验证缓存命中）**

Run:
```bash
time ./scripts/use_ner_model.sh chinese
```

Expected: 命令在 2 秒内完成（仅 rsync 复制，不再调用 Python 脚本）。

- [ ] **Step 6: Commit**

```bash
git add scripts/use_ner_model.sh
git commit -m "$(cat <<'EOF'
feat: 新增 use_ner_model.sh NER 模型切换脚本

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4：更新 `.gitignore` 和 CI workflow

**Files:**
- Modify: `.gitignore:51-53`
- Modify: `.github/workflows/release.yml:41-45`

- [ ] **Step 1: 在 `.gitignore` 中增加 `.ner_cache/`，移除已删除脚本相关的临时目录**

编辑 `.gitignore`，找到以下内容：

```
# 模型导出临时文件
scripts/_ner_export_tmp/
onnx_export/
onnx_export_int8/
```

替换为：

```
# 模型缓存（本地，可多个模型共存）
.ner_cache/

# 模型导出临时文件（旧脚本遗留，保留以清理历史 checkout）
scripts/_ner_export_tmp/
onnx_export/
onnx_export_int8/
```

- [ ] **Step 2: 更新 CI workflow 的模型准备步骤**

编辑 `.github/workflows/release.yml` 第 41-45 行，找到：

```yaml
      - name: 下载并导出 NER 模型
        if: steps.cache-model.outputs.cache-hit != 'true'
        run: |
          pip install optimum[onnxruntime] transformers torch huggingface_hub
          python scripts/prepare_ner_model.py
```

替换为：

```yaml
      - name: 下载并导出 NER 模型
        if: steps.cache-model.outputs.cache-hit != 'true'
        run: |
          pip install optimum[onnxruntime] transformers torch huggingface_hub
          PYTHON_BIN=python bash scripts/use_ner_model.sh multilingual
```

- [ ] **Step 3: 验证 CI 配置语法无误**

Run:
```bash
grep -n "prepare_ner_model" .github/workflows/release.yml || echo "无引用"
grep -n "use_ner_model" .github/workflows/release.yml
```

Expected:
- 第一条输出 `无引用`
- 第二条输出匹配行包含 `bash scripts/use_ner_model.sh multilingual`

- [ ] **Step 4: Commit**

```bash
git add .gitignore .github/workflows/release.yml
git commit -m "$(cat <<'EOF'
chore: gitignore 加 .ner_cache，CI 改用 use_ner_model.sh 切换脚本

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5：验证 Rust 后端在两个模型下均能加载

**Files:**
- Test only: `src-tauri/src/engine/backends/onnx_backend.rs`（只运行测试，不修改）

- [ ] **Step 1: 激活中文模型后运行后端测试**

Run:
```bash
./scripts/use_ner_model.sh chinese
cd src-tauri && cargo test engine::backends::onnx_backend -- --nocapture
```

Expected: 所有测试通过。重点看 `test_detect_chinese_person_and_location` 和 `test_char_offset_correctness`，这些依赖真实模型推理。

**若失败**：检查是否是 BERT tokenizer.json 与 Rust `tokenizers` crate 不兼容，阅读 stderr 定位具体错误。常见原因是 BERT 的 tokenizer.json 缺少 `offset_mapping` 能力，需要回退到修改 Rust 代码支持 vocab.txt（超出本计划范围，停下来告诉用户）。

- [ ] **Step 2: 激活多语言模型后运行同样的测试**

Run:
```bash
cd /Users/tanzeshun/workpath/github/dimkey
./scripts/use_ner_model.sh multilingual
cd src-tauri && cargo test engine::backends::onnx_backend -- --nocapture
```

Expected: 所有测试通过。

- [ ] **Step 3: 验证 tauri dev 能启动并看到模型加载日志**

Run（后台启动，5 秒后停）:
```bash
cd /Users/tanzeshun/workpath/github/dimkey
timeout 60 cargo tauri dev 2>&1 | grep -i "ner\|onnx\|label" | head -20 || true
```

Expected: 日志中出现 `NER 引擎已加载 (ONNX)`（证明模型文件就绪）。若出现 `NER 引擎未加载模型，降级运行`，说明激活失败，检查 `src-tauri/resources/ner/` 内容。

- [ ] **Step 4: 最终回到多语言模型作为默认激活状态**

Run:
```bash
./scripts/use_ner_model.sh multilingual
```

这一步是为了下次发版默认使用多语言模型。无需 commit（模型文件已 gitignored）。

---

## Self-Review

1. **Spec coverage**
   - ✓ `.ner_cache/<name>/` 缓存结构（Task 1 输出目录 + Task 4 gitignore）
   - ✓ `scripts/use_ner_model.sh <name>` 一条命令切换（Task 3）
   - ✓ 统一 `scripts/export_ner_model.py` 支持两个模型参数（Task 1）
   - ✓ 删除 `scripts/prepare_ner_model.py`（Task 2）
   - ✓ 中文 BERT 生成 `tokenizer.json`（Task 1 Step 3 + Task 5 Rust 加载验证）
   - ✓ Rust 代码零改动（Task 5 验证而非修改）
   - ✓ Tauri 打包零改动（未列入文件修改清单）
   - ✓ 发版流程零改动（CI workflow 只是替换脚本调用，不改结构）
   - ✓ CI cache key 参数化 — 已在 v0.5.1 改为 `ner-model-davlan-xlm-roberta-base-ner-hrl-v1`，本计划只改脚本调用不动 key，因为默认激活仍是 multilingual

2. **Placeholder scan**
   - 无 TBD/TODO
   - 所有代码块都是完整内容
   - 所有命令都有具体路径

3. **Type consistency**
   - `MODELS` 字典 key `chinese` / `multilingual` 在 Task 1/3/5 保持一致
   - `.ner_cache/<name>/` 路径约定一致
   - `tokenizer_source` 字段语义清晰（`"self"` vs 外部模型 ID）

4. **Ambiguity check**
   - Task 1 Step 3 明确了失败时的处理路径（停下来而非猜）
   - Task 5 Step 1 同样明确了 Rust 测试失败的处置方式
   - Shell 脚本的 `PYTHON_BIN` 可覆盖，默认 `python3.11`（与本机环境一致，CI 环境通过显式传 `PYTHON_BIN=python` 覆盖）

---
