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

REQUIRED_FILES=("model.onnx" "tokenizer.json" "id2label.json" "model_config.json")

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
  --include "model_config.json" \
  --include ".gitkeep" \
  --exclude "*" \
  "$CACHE_DIR/" "$ACTIVE_DIR/"

# 保留 .gitkeep（如果原来有）
touch "$ACTIVE_DIR/.gitkeep"

echo "完成！当前激活模型: $NAME"
echo "  目录: $ACTIVE_DIR"
ls -lh "$ACTIVE_DIR"
