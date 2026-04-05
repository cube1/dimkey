#!/bin/bash
# macOS 本地构建 + 签名 + 上传到 GitHub Release
# 用法: ./scripts/release-macos.sh [tag]
# 示例: ./scripts/release-macos.sh v0.3.2
#
# 前置要求:
#   1. ~/.tauri/dimkey.key 存在（Tauri updater 签名私钥）
#   2. TAURI_SIGNING_PRIVATE_KEY_PASSWORD 环境变量已设置（私钥密码）
#   3. APPLE_ID 环境变量已设置（Apple ID 邮箱，用于公证）
#   4. APPLE_PASSWORD 环境变量已设置（App 专用密码，用于公证）
#   5. "Developer ID Application" 证书已安装在 Keychain 中
#   6. gh CLI 已登录
#   7. cargo tauri 已安装

set -euo pipefail

# ── 参数 ──
TAG="${1:-}"
if [ -z "$TAG" ]; then
  # 自动从 Cargo.toml 读取版本号
  VERSION=$(grep '^version' src-tauri/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
  TAG="v${VERSION}"
  echo "未指定 tag，自动使用: $TAG"
else
  VERSION="${TAG#v}"
fi

echo "========================================="
echo "  Dimkey macOS 本地发布: $TAG"
echo "========================================="

# ── 检查前置条件 ──
KEY_FILE="$HOME/.tauri/dimkey.key"
if [ ! -f "$KEY_FILE" ]; then
  echo "错误: 未找到签名私钥: $KEY_FILE"
  exit 1
fi

if [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]; then
  echo "错误: 未设置 TAURI_SIGNING_PRIVATE_KEY_PASSWORD 环境变量"
  exit 1
fi

if [ -z "${APPLE_ID:-}" ]; then
  echo "错误: 未设置 APPLE_ID 环境变量（Apple ID 邮箱，用于公证）"
  echo "   请先运行: export APPLE_ID='your@email.com'"
  exit 1
fi

if [ -z "${APPLE_PASSWORD:-}" ]; then
  echo "错误: 未设置 APPLE_PASSWORD 环境变量（App 专用密码，用于公证）"
  echo "   请先运行: export APPLE_PASSWORD='xxxx-xxxx-xxxx-xxxx'"
  exit 1
fi

if ! command -v gh &>/dev/null; then
  echo "错误: 未安装 gh CLI"
  exit 1
fi

if ! gh auth status &>/dev/null; then
  echo "错误: gh CLI 未登录，请先运行: gh auth login"
  exit 1
fi

# ── 检查 tag 是否已推送到远端 ──
if ! git ls-remote --tags origin | grep -q "refs/tags/${TAG}$"; then
  echo ""
  echo "警告: Tag $TAG 尚未推送到远端，Windows CI 不会自动触发"
  echo "   继续构建 macOS 产物..."
fi

# ── 清理旧构建产物（避免误上传上次的文件） ──
BUNDLE_DIR="src-tauri/target/release/bundle"
if [ -d "$BUNDLE_DIR" ]; then
  echo ""
  echo "清理旧构建产物..."
  rm -rf "$BUNDLE_DIR"
fi

# ── 构建（仅签名，不走 Tauri 内置公证）──
echo ""
echo "开始构建..."
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_FILE")"
# Apple 代码签名（不设置 APPLE_ID/APPLE_PASSWORD 环境变量给 Tauri，跳过其内置公证）
export APPLE_SIGNING_IDENTITY="Developer ID Application: zeshun tan (2GDQYR464F)"
export APPLE_TEAM_ID="2GDQYR464F"
# 临时清除公证相关变量，避免 Tauri 内置公证（其 S3 上传在部分地区超时）
_SAVED_APPLE_ID="$APPLE_ID"
_SAVED_APPLE_PASSWORD="$APPLE_PASSWORD"
unset APPLE_ID APPLE_PASSWORD
cargo tauri build
# 恢复公证变量
export APPLE_ID="$_SAVED_APPLE_ID"
export APPLE_PASSWORD="$_SAVED_APPLE_PASSWORD"

# ── 使用 xcrun notarytool 公证（比 Tauri 内置更稳定）──
echo ""
echo "使用 xcrun notarytool 公证..."
APP_PATH="$BUNDLE_DIR/macos/Dimkey.app"
NOTARIZE_ZIP="$BUNDLE_DIR/macos/Dimkey_notarize.zip"
/usr/bin/ditto -c -k --keepParent "$APP_PATH" "$NOTARIZE_ZIP"

MAX_RETRIES=3
for i in $(seq 1 $MAX_RETRIES); do
  echo "  公证提交（第 ${i}/${MAX_RETRIES} 次）..."
  if xcrun notarytool submit "$NOTARIZE_ZIP" \
    --apple-id "$APPLE_ID" \
    --password "$APPLE_PASSWORD" \
    --team-id "$APPLE_TEAM_ID" \
    --wait 2>&1 | tee /tmp/notarytool.log; then
    echo "  公证通过"
    break
  fi
  if [ $i -eq $MAX_RETRIES ]; then
    echo "错误: 公证提交 ${MAX_RETRIES} 次均失败"
    cat /tmp/notarytool.log
    exit 1
  fi
  echo "  上传超时，${i}s 后重试..."
  sleep $i
done

echo "  Staple 公证票据..."
xcrun stapler staple "$APP_PATH"
echo "  验证 Gatekeeper..."
spctl -a -vv "$APP_PATH" 2>&1

# ── 重新生成 DMG 和 updater 产物（基于已公证的 app）──
echo ""
echo "重新生成 DMG..."
DMG_DIR="$BUNDLE_DIR/dmg"
mkdir -p "$DMG_DIR"
rm -f "$DMG_DIR"/*.dmg
DMG_PATH="$DMG_DIR/Dimkey_${VERSION}_aarch64.dmg"

# 创建临时目录，放入 app 和 Applications 快捷方式
DMG_STAGE=$(mktemp -d)
cp -R "$APP_PATH" "$DMG_STAGE/"
ln -s /Applications "$DMG_STAGE/Applications"
hdiutil create -volname "Dimkey" -srcfolder "$DMG_STAGE" -ov -format UDZO "$DMG_PATH"
rm -rf "$DMG_STAGE"

echo "生成 updater 包..."
MACOS_DIR="$BUNDLE_DIR/macos"
TAR_GZ_PATH="$MACOS_DIR/Dimkey.app.tar.gz"
rm -f "$TAR_GZ_PATH" "$TAR_GZ_PATH.sig"
tar -czf "$TAR_GZ_PATH" -C "$MACOS_DIR" Dimkey.app
# 用 Tauri 签名密钥签署 updater 包
if command -v cargo-tauri &>/dev/null || command -v cargo &>/dev/null; then
  cargo tauri signer sign "$TAR_GZ_PATH" --private-key "$(cat "$KEY_FILE")" 2>/dev/null || true
fi

# ── 检查产物 ──
DMG=$(find "$DMG_DIR" -name "*.dmg" 2>/dev/null | head -1)
TAR_GZ=$(find "$MACOS_DIR" -name "*.app.tar.gz" 2>/dev/null | head -1)
SIG=$(find "$MACOS_DIR" -name "*.app.tar.gz.sig" 2>/dev/null | head -1)

echo ""
echo "构建产物:"
[ -n "$DMG" ] && echo "  DMG（安装包）: $(basename "$DMG")" || echo "  [缺失] DMG 未找到"
[ -n "$TAR_GZ" ] && echo "  Updater（更新包）: $(basename "$TAR_GZ")" || echo "  [缺失] .app.tar.gz 未找到"
[ -n "$SIG" ] && echo "  签名: $(basename "$SIG")" || echo "  [缺失] .app.tar.gz.sig 未找到（updater 将不含 macOS）"

if [ -z "$DMG" ] && [ -z "$TAR_GZ" ]; then
  echo "错误: 未找到任何构建产物"
  exit 1
fi

# ── 上传到 GitHub Release ──
echo ""
echo "上传到 GitHub Release ($TAG)..."

# 确保 Release 存在（CI 可能还没创建）
if ! gh release view "$TAG" &>/dev/null; then
  echo "  Release 不存在，先创建..."
  gh release create "$TAG" \
    --title "Dimkey $TAG" \
    --notes "Release 准备中..."
fi

UPLOAD_FILES=()
[ -n "$DMG" ] && UPLOAD_FILES+=("$DMG")
[ -n "$TAR_GZ" ] && UPLOAD_FILES+=("$TAR_GZ")
[ -n "$SIG" ] && UPLOAD_FILES+=("$SIG")

for f in "${UPLOAD_FILES[@]}"; do
  echo "  上传 $(basename "$f")..."
  gh release upload "$TAG" "$f" --clobber
done

echo ""
echo "macOS 产物已上传到 GitHub Release (私有仓库)"

# ── 从 CHANGELOG.md 提取当前版本日志，更新 Release 描述 ──
echo ""
echo "更新 Release 描述..."
RELEASE_NOTES=$(awk "
  /^## \\[${TAG}\\]/ { found=1 }
  /^## \\[v/ && found && !/\\[${TAG}\\]/ { exit }
  found { print }
" CHANGELOG.md)

if [ -n "$RELEASE_NOTES" ]; then
  gh release edit "$TAG" --notes "$RELEASE_NOTES"
  echo "  已从 CHANGELOG.md 更新 Release 描述"
else
  echo "  警告: 未在 CHANGELOG.md 中找到 ${TAG} 的条目，Release 描述未更新"
fi

# ── 上传到公开仓库 dimkey-site Releases ──
SITE_REPO="cube1/dimkey-site"
echo ""
echo "上传到公开仓库 ($SITE_REPO) Release..."

# 确保 dimkey-site Release 存在
if ! gh release view "$TAG" --repo "$SITE_REPO" &>/dev/null; then
  echo "  Release 不存在，先创建..."
  SITE_RELEASE_NOTES="${RELEASE_NOTES:-Release $TAG}"
  gh release create "$TAG" \
    --repo "$SITE_REPO" \
    --title "Dimkey $TAG" \
    --notes "$SITE_RELEASE_NOTES"
fi

for f in "${UPLOAD_FILES[@]}"; do
  echo "  上传 $(basename "$f") → $SITE_REPO..."
  gh release upload "$TAG" "$f" --clobber --repo "$SITE_REPO"
done

echo "  macOS 产物已上传到 $SITE_REPO Release"

# ── 检查 Windows CI 构建结果 ──
echo ""
echo "检查 Windows CI 构建状态..."
TAG_RUN=$(gh run list --workflow=release.yml --branch="$TAG" --event=push --limit 1 \
  --json databaseId,status,conclusion --jq '.[0]' 2>/dev/null || true)

if [ -n "$TAG_RUN" ] && [ "$TAG_RUN" != "null" ]; then
  RUN_ID=$(echo "$TAG_RUN" | jq -r '.databaseId')
  RUN_STATUS=$(echo "$TAG_RUN" | jq -r '.status')
  RUN_CONCLUSION=$(echo "$TAG_RUN" | jq -r '.conclusion')

  if [ "$RUN_STATUS" != "completed" ]; then
    echo "  CI (Run #$RUN_ID) 运行中，等待完成..."
    echo "  （按 Ctrl+C 可跳过等待，之后手动运行: gh workflow run release.yml -f tag=$TAG)"
    gh run watch "$RUN_ID" --exit-status || true
    RUN_CONCLUSION=$(gh run view "$RUN_ID" --json conclusion --jq '.conclusion' 2>/dev/null || echo "unknown")
  fi

  if [ "$RUN_CONCLUSION" != "success" ]; then
    echo ""
    echo "警告: CI 运行 #$RUN_ID 结果为 ${RUN_CONCLUSION}（非 success）"
    echo "   Windows 构建可能失败，latest.json 将只包含 macOS 平台"
    echo "   请检查: gh run view $RUN_ID --log-failed"
    echo "   继续触发 latest.json 生成..."
  else
    echo "  CI 运行成功，Windows 产物已上传"
  fi
else
  echo "  未找到本次 tag 的 CI 运行记录"
  echo "  latest.json 将只包含 macOS 平台"
fi

# ── 触发 workflow_dispatch 生成 latest.json 并同步到 dimkey-site ──
echo ""
echo "触发 workflow_dispatch 生成 latest.json 并同步到 dimkey-site..."
if gh workflow run release.yml -f tag="$TAG"; then
  echo "  已触发，等待运行开始..."
  sleep 5
  DISPATCH_RUN=$(gh run list --workflow=release.yml --event=workflow_dispatch --limit 1 \
    --json databaseId,status --jq '.[0].databaseId' 2>/dev/null || true)
  if [ -n "$DISPATCH_RUN" ] && [ "$DISPATCH_RUN" != "null" ]; then
    echo "  等待 workflow_dispatch (Run #$DISPATCH_RUN) 完成..."
    gh run watch "$DISPATCH_RUN" --exit-status || echo "  警告: workflow_dispatch 运行失败，请检查 Actions 日志"
  fi
else
  echo "错误: 触发 workflow_dispatch 失败"
  echo "  请手动运行: gh workflow run release.yml -f tag=$TAG"
fi

echo ""
echo "========================================="
echo "  macOS 发布完成: $TAG"
echo "========================================="
echo ""
echo "验证:"
echo "  公开下载页: https://dimkey.com"
echo "  自动更新:   curl -s https://dimkey.com/latest.json | jq .version"
echo "  Release:    gh release view ${TAG} --repo $SITE_REPO"
