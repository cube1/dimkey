#!/bin/bash
# 校验 Tauri build 产物的关键资源是否齐全
# 用法: ./scripts/verify-bundle.sh [path-to-Dimkey.app]
# 默认路径: src-tauri/target/release/bundle/macos/Dimkey.app
#
# 解决用户痛点: "打包后连模型文件都没带，都不能识别"
# 校验项:
#   1. NER ONNX 模型 (model.onnx 应 >= 30 MB)
#   2. NER 配置文件 (tokenizer.json / id2label.json / model_config.json)
#   3. PDFium 动态库 (libpdfium.dylib / pdfium.dll, 应 >= 1 MB)
#   4. 主可执行文件 (Contents/MacOS/Dimkey)
#
# 任何一项缺失/异常立即 exit 1，阻止发布

set -euo pipefail

APP_PATH="${1:-src-tauri/target/release/bundle/macos/Dimkey.app}"
PLATFORM="${2:-macos}"  # macos | windows

echo "========================================="
echo "  Bundle Smoke Check: $APP_PATH"
echo "========================================="

if [ ! -d "$APP_PATH" ]; then
  echo "错误: bundle 目录不存在: $APP_PATH"
  exit 1
fi

# macOS: 资源在 Contents/Resources，可执行文件在 Contents/MacOS/Dimkey
# Windows: 资源在 resources/，可执行文件在 dimkey.exe
if [ "$PLATFORM" = "macos" ]; then
  RESOURCES_DIR="$APP_PATH/Contents/Resources"
  BINARY_PATH="$APP_PATH/Contents/MacOS/Dimkey"
  PDFIUM_LIB="libpdfium.dylib"
elif [ "$PLATFORM" = "windows" ]; then
  RESOURCES_DIR="$APP_PATH/resources"
  BINARY_PATH="$APP_PATH/dimkey.exe"
  PDFIUM_LIB="pdfium.dll"
else
  echo "错误: 不支持的平台: $PLATFORM (期望 macos 或 windows)"
  exit 1
fi

ERRORS=0

check() {
  local desc="$1"
  local cond="$2"
  if eval "$cond"; then
    echo "  [OK] $desc"
  else
    echo "  [FAIL] $desc"
    ERRORS=$((ERRORS + 1))
  fi
}

check_file_min_size() {
  local desc="$1"
  local path="$2"
  local min_kb="$3"

  if [ ! -f "$path" ]; then
    echo "  [FAIL] $desc — 文件不存在: $path"
    ERRORS=$((ERRORS + 1))
    return
  fi

  # macOS stat 用 -f%z，Linux 用 -c%s
  local size
  if stat -f%z "$path" >/dev/null 2>&1; then
    size=$(stat -f%z "$path")
  else
    size=$(stat -c%s "$path")
  fi
  local size_kb=$((size / 1024))

  if [ "$size_kb" -lt "$min_kb" ]; then
    echo "  [FAIL] $desc — 文件大小 ${size_kb}KB < 期望 ${min_kb}KB ($path)"
    ERRORS=$((ERRORS + 1))
  else
    echo "  [OK] $desc (${size_kb}KB)"
  fi
}

echo ""
echo "[1/4] 检查 NER ONNX 模型..."
check_file_min_size "ner/model.onnx (NER 模型本体)" "$RESOURCES_DIR/ner/model.onnx" 30000

echo ""
echo "[2/4] 检查 NER 配置文件..."
check "ner/tokenizer.json" "[ -f '$RESOURCES_DIR/ner/tokenizer.json' ]"
check "ner/id2label.json" "[ -f '$RESOURCES_DIR/ner/id2label.json' ]"
check "ner/model_config.json" "[ -f '$RESOURCES_DIR/ner/model_config.json' ]"

echo ""
echo "[3/4] 检查 PDFium 动态库..."
# pdfium 至少 1MB（实际 macOS arm64 约 5-10MB）
check_file_min_size "pdfium/$PDFIUM_LIB (PDF 解析依赖)" "$RESOURCES_DIR/pdfium/$PDFIUM_LIB" 1000

echo ""
echo "[4/4] 检查主可执行文件..."
check "主程序可执行 ($BINARY_PATH)" "[ -x '$BINARY_PATH' ]"

echo ""
echo "========================================="
if [ "$ERRORS" -eq 0 ]; then
  echo "  Bundle Smoke Check: 全部通过"
  echo "========================================="
  exit 0
else
  echo "  Bundle Smoke Check: $ERRORS 项失败"
  echo ""
  echo "  这些问题会导致用户打开 app 后:"
  echo "  - 模型缺失 → NER 识别完全不工作 (姓名/地址/机构等漏识别)"
  echo "  - PDFium 缺失 → PDF 文件解析直接报错或静默失败"
  echo ""
  echo "  请勿发布此构建产物。"
  echo "========================================="
  exit 1
fi
