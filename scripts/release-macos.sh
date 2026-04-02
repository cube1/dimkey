#!/bin/bash
# macOS 本地构建 + 签名 + 上传到 GitHub Release
# 用法: ./scripts/release-macos.sh [tag]
# 示例: ./scripts/release-macos.sh v0.3.2
#
# 前置要求:
#   1. ~/.tauri/desensitize-tool.key 存在（Tauri updater 签名私钥）
#   2. TAURI_SIGNING_PRIVATE_KEY_PASSWORD 环境变量已设置（私钥密码）
#   3. gh CLI 已登录
#   4. cargo tauri 已安装

set -euo pipefail

# ── 参数 ──
TAG="${1:-}"
if [ -z "$TAG" ]; then
  # 自动从 Cargo.toml 读取版本号
  VERSION=$(grep '^version' src-tauri/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
  TAG="v${VERSION}"
  echo "未指定 tag，自动使用: $TAG"
fi

echo "========================================="
echo "  Dimkey macOS 本地发布: $TAG"
echo "========================================="

# ── 检查前置条件 ──
KEY_FILE="$HOME/.tauri/desensitize-tool.key"
if [ ! -f "$KEY_FILE" ]; then
  echo "错误: 未找到签名私钥: $KEY_FILE"
  exit 1
fi

if [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]; then
  echo "错误: 未设置 TAURI_SIGNING_PRIVATE_KEY_PASSWORD 环境变量"
  echo "   请先运行: export TAURI_SIGNING_PRIVATE_KEY_PASSWORD='你的密码'"
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
  echo "警告: Tag $TAG 尚未推送到远端"
  echo "   推荐先运行: git tag $TAG && git push origin $TAG"
  echo "   否则 Windows CI 不会自动触发构建"
  read -p "是否继续？(y/N) " -n 1 -r
  echo
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    exit 1
  fi
fi

# ── 清理旧构建产物（避免误上传上次的文件） ──
BUNDLE_DIR="src-tauri/target/release/bundle"
if [ -d "$BUNDLE_DIR" ]; then
  echo ""
  echo "清理旧构建产物..."
  rm -rf "$BUNDLE_DIR"
fi

# ── 构建 ──
echo ""
echo "开始构建..."
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_FILE")"
# 跳过 Apple 代码签名（签名后未公证会被 macOS 提示"移入废纸篓"）
# 保留 Tauri updater 签名（.sig 文件用于自动更新校验）
export APPLE_SIGNING_IDENTITY="-"
BUILD_EXIT=0
cargo tauri build || BUILD_EXIT=$?

if [ $BUILD_EXIT -ne 0 ]; then
  echo ""
  echo "警告: cargo tauri build 退出码 ${BUILD_EXIT}（可能是公证失败），检查产物是否已生成..."
fi

# ── 检查产物 ──
DMG=$(find "$BUNDLE_DIR/dmg" -name "*.dmg" 2>/dev/null | head -1)
TAR_GZ=$(find "$BUNDLE_DIR/macos" -name "*.app.tar.gz" 2>/dev/null | head -1)
SIG=$(find "$BUNDLE_DIR/macos" -name "*.app.tar.gz.sig" 2>/dev/null | head -1)

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
    --title "Dimkey Dimkey $TAG" \
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
echo "macOS 产物已上传到 GitHub Release"

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
    echo "  （按 Ctrl+C 可跳过等待，之后手动运行: gh workflow run release.yml -f tag=$TAG）"
    gh run watch "$RUN_ID" --exit-status || true
    RUN_CONCLUSION=$(gh run view "$RUN_ID" --json conclusion --jq '.conclusion' 2>/dev/null || echo "unknown")
  fi

  if [ "$RUN_CONCLUSION" != "success" ]; then
    echo ""
    echo "警告: CI 运行 #$RUN_ID 结果为 ${RUN_CONCLUSION}（非 success）"
    echo "   Windows 构建可能失败，latest.json 将只包含 macOS 平台"
    echo "   请检查: gh run view $RUN_ID --log-failed"
    echo ""
    read -p "是否继续触发 latest.json 生成？(y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
      echo "已取消。修复 CI 后手动运行: gh workflow run release.yml -f tag=$TAG"
      exit 1
    fi
  else
    echo "  CI 运行成功，Windows 产物已上传"
  fi
else
  echo "  未找到本次 tag 的 CI 运行记录"
  echo "  latest.json 将只包含 macOS 平台"
fi

# ── 触发 workflow_dispatch 重新生成 latest.json + 同步 Gitee ──
echo ""
echo "触发 workflow_dispatch 重新生成 latest.json..."
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
echo "后续手动步骤（Gitee 同步）:"
echo "  1. 从 GitHub Release 下载全部产物（含 Windows）"
echo "  2. 上传到 Gitee Release"
echo "  3. 验证更新配置:"
echo "     Gitee: curl -s https://gitee.com/qiubye/dimkey/raw/main/latest.json | jq .version"
echo "     GitHub: gh release download ${TAG} --pattern latest.json --dir /tmp && cat /tmp/latest.json | jq .version"
