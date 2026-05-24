#!/usr/bin/env bash
#
# 下载并签好 PDFium 动态库到 src-tauri/resources/pdfium/
#
# 设计：仅 macOS 本地使用（Windows CI 由 GitHub Actions 自己拉）。
# 幂等：如果目标文件已存在、签名带 timestamp 且 TeamID 匹配 SIGNING_ID 的 team，直接跳过。
#
# 用法:
#   ./scripts/fetch_pdfium.sh                              # 默认 PINNED_VERSION，自动识别架构
#   PDFIUM_VERSION=chromium/7843 ./scripts/...             # 指定 pdfium-binaries release tag
#   PDFIUM_SIGN_ID="Developer ID Application: ..." ...     # 指定签名身份
#
# 依赖: curl, tar, codesign, security（macOS 自带）
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="$REPO_ROOT/src-tauri/resources/pdfium"
TARGET_LIB="$TARGET_DIR/libpdfium.dylib"

# 已知可用的 pinned 版本，作为 GitHub API 限速或未指定 PDFIUM_VERSION 时的 fallback
PINNED_PDFIUM_VERSION="chromium/7843"

SIGNING_ID="${PDFIUM_SIGN_ID:-Developer ID Application: zeshun tan (2GDQYR464F)}"

if [[ "$OSTYPE" != darwin* ]]; then
  echo "本脚本仅 macOS。Windows 请在 GitHub Actions 中处理。"
  exit 0
fi

# 从 SIGNING_ID 提取期望的 TeamID（括号内）用于幂等验证
# 形如 "Developer ID Application: zeshun tan (2GDQYR464F)" → "2GDQYR464F"
EXPECTED_TEAM_ID=$(echo "$SIGNING_ID" | sed -E 's/.*\(([A-Z0-9]{10})\).*/\1/')

# 幂等：已就位且签名带 timestamp 且 TeamID 匹配 → skip
# （所有变量引用用 ${VAR:-} 保护，避免 set -u 与 codesign 输出缺字段时炸）
if [ -f "$TARGET_LIB" ]; then
  SIG_INFO="$(codesign -dvv "$TARGET_LIB" 2>&1 || true)"
  HAS_TS="$(printf '%s\n' "${SIG_INFO:-}" | grep -c '^Timestamp=' || true)"
  CURRENT_TEAM="$(printf '%s\n' "${SIG_INFO:-}" | awk -F= '/^TeamIdentifier=/ {print $2; exit}')"
  CURRENT_TEAM="${CURRENT_TEAM:-}"
  HAS_TS="${HAS_TS:-0}"
  if [ "$HAS_TS" -ge 1 ] && [ -n "$CURRENT_TEAM" ] && [ "$CURRENT_TEAM" = "$EXPECTED_TEAM_ID" ]; then
    echo "PDFium 已就位且签名带 timestamp + TeamID 匹配（${CURRENT_TEAM}），跳过下载"
    exit 0
  fi
  if [ -n "$CURRENT_TEAM" ] && [ "$CURRENT_TEAM" != "$EXPECTED_TEAM_ID" ]; then
    echo "现有 libpdfium.dylib TeamID=$CURRENT_TEAM 与 SIGNING_ID 期望的 $EXPECTED_TEAM_ID 不符，将重新下载并签名"
  fi
fi

# 签名前校验：keychain 中存在该 Developer ID Application 证书才进入下载
# 避免下载完成后才在 codesign 阶段失败、留下未签名的 dylib
if ! security find-identity -v -p codesigning 2>/dev/null | grep -qF "$SIGNING_ID"; then
  echo "错误: keychain 中未找到签名身份: $SIGNING_ID"
  echo "  请用 'security find-identity -v -p codesigning' 检查可用证书"
  echo "  或通过 PDFIUM_SIGN_ID 环境变量覆盖默认身份"
  exit 1
fi

# 版本：env 覆盖 > GitHub latest > PINNED fallback
if [ -z "${PDFIUM_VERSION:-}" ]; then
  echo "查询 pdfium-binaries latest release ..."
  PDFIUM_VERSION=$(curl -fsSL https://api.github.com/repos/bblanchon/pdfium-binaries/releases/latest 2>/dev/null \
    | grep '"tag_name"' | head -1 | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/' || true)
  if [ -z "$PDFIUM_VERSION" ]; then
    echo "  查询 latest 失败（可能是 API 限速或网络问题），回退到 PINNED 版本 $PINNED_PDFIUM_VERSION"
    PDFIUM_VERSION="$PINNED_PDFIUM_VERSION"
  fi
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
trap 'rm -rf "$TMP"' EXIT

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
# 验证签名信息（管道末尾失败也不让脚本退出 — 校验输出是诊断信息，
# codesign 之前的 set -e 已经保证主流程成功）
{ codesign -dvv "$TARGET_LIB" 2>&1 | grep -E "^(Authority=|Timestamp=|TeamIdentifier=)" | head -4; } || true
