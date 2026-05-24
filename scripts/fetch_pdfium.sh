#!/usr/bin/env bash
#
# 下载并签好 PDFium 动态库到 src-tauri/resources/pdfium/
#
# 设计：仅 macOS 本地使用（Windows CI 由 GitHub Actions 自己拉）。
# 幂等：如果目标文件已存在且签名带 timestamp，直接跳过。
#
# 用法:
#   ./scripts/fetch_pdfium.sh                        # 默认版本，自动识别架构
#   PDFIUM_VERSION=chromium/7515 ./scripts/...       # 指定 pdfium-binaries release tag
#   PDFIUM_SIGN_ID="..." ./scripts/...               # 指定签名身份
#
# 依赖: curl, tar, codesign（macOS 自带）
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="$REPO_ROOT/src-tauri/resources/pdfium"
TARGET_LIB="$TARGET_DIR/libpdfium.dylib"

SIGNING_ID="${PDFIUM_SIGN_ID:-Developer ID Application: zeshun tan (2GDQYR464F)}"

if [[ "$OSTYPE" != darwin* ]]; then
  echo "本脚本仅 macOS。Windows 请在 GitHub Actions 中处理。"
  exit 0
fi

# 幂等：已就位且签名带 timestamp → skip
if [ -f "$TARGET_LIB" ]; then
  SIG_INFO=$(codesign -dvv "$TARGET_LIB" 2>&1 || true)
  if echo "$SIG_INFO" | grep -q "^Timestamp="; then
    echo "PDFium 已就位且签名带 timestamp，跳过下载"
    exit 0
  fi
fi

# 默认版本：从 GitHub API 取 latest tag
if [ -z "${PDFIUM_VERSION:-}" ]; then
  echo "查询 pdfium-binaries latest release ..."
  PDFIUM_VERSION=$(curl -fsSL https://api.github.com/repos/bblanchon/pdfium-binaries/releases/latest \
    | grep '"tag_name"' | head -1 | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')
  [ -n "$PDFIUM_VERSION" ] || { echo "错误: 查询 latest tag 失败"; exit 1; }
fi

ARCH=$(uname -m)
case "$ARCH" in
  arm64)   PDFIUM_ARCH="mac-arm64" ;;
  x86_64)  PDFIUM_ARCH="mac-x64" ;;
  *) echo "错误: 不支持的架构 $ARCH"; exit 1 ;;
esac

ASSET="pdfium-${PDFIUM_ARCH}.tgz"
URL="https://github.com/bblanchon/pdfium-binaries/releases/download/${PDFIUM_VERSION}/${ASSET}"

TMP=$(mktemp -d)
trap "rm -rf $TMP" EXIT

echo "下载 PDFium ($PDFIUM_VERSION, $PDFIUM_ARCH) ..."
echo "  URL: $URL"
curl -fL "$URL" -o "$TMP/pdfium.tgz"

mkdir -p "$TARGET_DIR"
tar -xzf "$TMP/pdfium.tgz" -C "$TMP"
[ -f "$TMP/lib/libpdfium.dylib" ] || { echo "错误: 解压后未找到 lib/libpdfium.dylib"; exit 1; }
cp "$TMP/lib/libpdfium.dylib" "$TARGET_LIB"

echo "重签（Developer ID + timestamp + runtime hardening）..."
codesign --sign "$SIGNING_ID" --timestamp --options runtime --force "$TARGET_LIB"

echo ""
echo "完成: $TARGET_LIB"
codesign -dvv "$TARGET_LIB" 2>&1 | grep -E "^(Authority=|Timestamp=)" | head -3
