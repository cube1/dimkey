#!/bin/bash
# macOS 本地构建 + 签名 + 上传到 GitHub Release
# 用法: ./scripts/release-macos.sh <tag> <lang>
# 示例: ./scripts/release-macos.sh v0.7.1 zh
#       ./scripts/release-macos.sh v0.7.1 en
#
# lang: zh | en，决定打包语言（NER 模型 / Cargo feature / Vite env / bundle id / updater endpoint / 产物文件名）
#
# 前置要求:
#   1. ~/.tauri/dimkey.key 存在（Tauri updater 签名私钥）
#   2. TAURI_SIGNING_PRIVATE_KEY_PASSWORD 环境变量已设置（私钥密码）
#   3. APPLE_ID 环境变量已设置（Apple ID 邮箱，用于公证）
#   4. APPLE_PASSWORD 环境变量已设置（App 专用密码，用于公证）
#   5. "Developer ID Application" 证书已安装在 Keychain 中
#   6. gh CLI 已登录
#   7. cargo tauri 已安装
#   8. jq 已安装（用于改写 tauri.conf.json）
#   9. python3.11 + ML 依赖（仅 .ner_cache 缓存未命中时需要）：
#      pip install optimum[onnxruntime] transformers torch huggingface_hub
#
# 执行顺序: 必须先 zh 后 en
#   ./scripts/release-macos.sh vX.Y.Z zh   # 第一次：上传中文版 + 更新 Release 描述
#   ./scripts/release-macos.sh vX.Y.Z en   # 第二次：上传英文版 + 触发 latest-*.json 生成

set -euo pipefail

# ── 参数 ──
TAG="${1:-}"
LANG_CODE="${2:-zh}"

if [ -z "$TAG" ]; then
  VERSION=$(grep '^version' src-tauri/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
  TAG="v${VERSION}"
  echo "未指定 tag，自动使用: $TAG"
else
  VERSION="${TAG#v}"
fi

case "$LANG_CODE" in
  zh)
    MODEL_NAME="chinese"
    BUNDLE_IDENT="com.dimkey.cn"
    UPDATER_ENDPOINT="https://dimkey.com/latest-zh.json"
    DEFAULT_FEATURES_LINE='default = ["lang-zh"]'
    ;;
  en)
    MODEL_NAME="distilbert-ner"
    BUNDLE_IDENT="com.dimkey.en"
    UPDATER_ENDPOINT="https://dimkey.com/latest-en.json"
    DEFAULT_FEATURES_LINE='default = ["lang-en"]'
    ;;
  *)
    echo "错误: 第二个参数必须是 zh 或 en（不是 $LANG_CODE）"
    exit 1
    ;;
esac

PRODUCT_VARIANT="Dimkey-${LANG_CODE}"
DMG_NAME="${PRODUCT_VARIANT}_${VERSION}_aarch64.dmg"
TAR_GZ_NAME="${PRODUCT_VARIANT}_${VERSION}_aarch64.app.tar.gz"

echo "========================================="
echo "  Dimkey macOS 本地发布: $TAG ($LANG_CODE)"
echo "========================================="

# ── 检查前置条件 ──
KEY_FILE="$HOME/.tauri/dimkey.key"
[ -f "$KEY_FILE" ] || { echo "错误: 未找到签名私钥: $KEY_FILE"; exit 1; }
[ -n "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ] || { echo "错误: 未设置 TAURI_SIGNING_PRIVATE_KEY_PASSWORD"; exit 1; }
[ -n "${APPLE_ID:-}" ] || { echo "错误: 未设置 APPLE_ID"; exit 1; }
[ -n "${APPLE_PASSWORD:-}" ] || { echo "错误: 未设置 APPLE_PASSWORD"; exit 1; }
command -v gh &>/dev/null || { echo "错误: 未安装 gh CLI"; exit 1; }
command -v jq &>/dev/null || { echo "错误: 未安装 jq"; exit 1; }
gh auth status &>/dev/null || { echo "错误: gh CLI 未登录"; exit 1; }

# ── License preflight 校验（lang=zh 阶段调用，避免 lang=en 重复跑）──
# 在临时改写 tauri.conf.json 和 Cargo.toml 之前跑，确保"工作区干净"判定准确。
if [ "$LANG_CODE" = "zh" ]; then
  echo ""
  echo "运行 License preflight 校验..."
  ./scripts/preflight-license.sh
fi

# ── 顺序检查：lang=en 时必须先有 zh 产物 ──
if [ "$LANG_CODE" = "en" ]; then
  echo ""
  echo "检查中文版产物是否已上传..."
  if ! gh release view "$TAG" --json assets --jq '.assets[].name' 2>/dev/null | grep -q "^Dimkey-zh_"; then
    echo "错误: 未在 Release $TAG 中找到中文版产物（Dimkey-zh_*）。"
    echo "       请先执行: ./scripts/release-macos.sh $TAG zh"
    exit 1
  fi
  echo "  中文版产物已存在 ✓"
fi

# ── 检查 tag 是否已推送到远端 ──
if ! git ls-remote --tags origin | grep -q "refs/tags/${TAG}$"; then
  echo ""
  echo "警告: Tag $TAG 尚未推送到远端，Windows CI 不会自动触发"
fi

# ── 确保 PDFium dylib 就位（缺失则从 bblanchon 下载 + 重签 timestamp）──
echo ""
echo "检查 PDFium dylib ..."
./scripts/fetch_pdfium.sh

# ── 切换 NER 模型 ──
echo ""
echo "切换 NER 模型 → $MODEL_NAME ..."
./scripts/use_ner_model.sh "$MODEL_NAME"

# ── 临时改写 tauri.conf.json 和 Cargo.toml（构建结束自动还原）──
# Cargo.lock 也备份：当前 lang-* feature 是空的，lock 不会变；但将来
# 给 feature 加依赖时（如 lang-zh = ["dep:zh-tokenizer"]）lock 会变，
# 不还原会污染工作区。
CONF_FILE="src-tauri/tauri.conf.json"
CARGO_FILE="src-tauri/Cargo.toml"
LOCK_FILE="src-tauri/Cargo.lock"
cp "$CONF_FILE" "$CONF_FILE.bak"
cp "$CARGO_FILE" "$CARGO_FILE.bak"
[ -f "$LOCK_FILE" ] && cp "$LOCK_FILE" "$LOCK_FILE.bak"

restore_files() {
  [ -f "$CONF_FILE.bak" ] && mv "$CONF_FILE.bak" "$CONF_FILE"
  [ -f "$CARGO_FILE.bak" ] && mv "$CARGO_FILE.bak" "$CARGO_FILE"
  [ -f "$LOCK_FILE.bak" ] && mv "$LOCK_FILE.bak" "$LOCK_FILE"
}
trap restore_files EXIT

echo "临时改写 $CONF_FILE: identifier=$BUNDLE_IDENT, updater=$UPDATER_ENDPOINT"
jq --arg id "$BUNDLE_IDENT" --arg endpoint "$UPDATER_ENDPOINT" \
  '.identifier = $id | .plugins.updater.endpoints = [$endpoint]' \
  "$CONF_FILE" > "$CONF_FILE.tmp"
mv "$CONF_FILE.tmp" "$CONF_FILE"

echo "临时改写 $CARGO_FILE: $DEFAULT_FEATURES_LINE"
# 替换 [features] 段下的 default = [...] 行（macOS sed 兼容）
sed -i '' "s/^default = \[\".*\"\]$/${DEFAULT_FEATURES_LINE}/" "$CARGO_FILE"

# ── 清理旧构建产物 ──
BUNDLE_DIR="src-tauri/target/release/bundle"
if [ -d "$BUNDLE_DIR" ]; then
  echo ""
  echo "清理旧构建产物..."
  rm -rf "$BUNDLE_DIR"
fi

# ── 构建（仅签名，不走 Tauri 内置公证）──
echo ""
echo "开始构建（lang=$LANG_CODE）..."
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_FILE")"
export APPLE_SIGNING_IDENTITY="Developer ID Application: zeshun tan (2GDQYR464F)"
export APPLE_TEAM_ID="2GDQYR464F"
export VITE_DIMKEY_LANG="$LANG_CODE"

# 临时清除公证相关变量，避免 Tauri 内置公证
_SAVED_APPLE_ID="$APPLE_ID"
_SAVED_APPLE_PASSWORD="$APPLE_PASSWORD"
unset APPLE_ID APPLE_PASSWORD
cargo tauri build
export APPLE_ID="$_SAVED_APPLE_ID"
export APPLE_PASSWORD="$_SAVED_APPLE_PASSWORD"

# ── xcrun notarytool 公证 ──
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
echo "重新生成 DMG → $DMG_NAME ..."
DMG_DIR="$BUNDLE_DIR/dmg"
mkdir -p "$DMG_DIR"
rm -f "$DMG_DIR"/*.dmg
DMG_PATH="$DMG_DIR/$DMG_NAME"

DMG_STAGE=$(mktemp -d)
cp -R "$APP_PATH" "$DMG_STAGE/"
ln -s /Applications "$DMG_STAGE/Applications"
hdiutil create -volname "Dimkey" -srcfolder "$DMG_STAGE" -ov -format UDZO "$DMG_PATH"
rm -rf "$DMG_STAGE"

echo "生成 updater 包 → $TAR_GZ_NAME ..."
MACOS_DIR="$BUNDLE_DIR/macos"
TAR_GZ_PATH="$MACOS_DIR/$TAR_GZ_NAME"
rm -f "$MACOS_DIR"/*.app.tar.gz "$MACOS_DIR"/*.app.tar.gz.sig
tar -czf "$TAR_GZ_PATH" -C "$MACOS_DIR" Dimkey.app
if command -v cargo-tauri &>/dev/null || command -v cargo &>/dev/null; then
  cargo tauri signer sign "$TAR_GZ_PATH" --private-key "$(cat "$KEY_FILE")" 2>/dev/null || true
fi

# ── 检查产物 ──
DMG=$(find "$DMG_DIR" -name "${PRODUCT_VARIANT}*.dmg" 2>/dev/null | head -1)
TAR_GZ=$(find "$MACOS_DIR" -name "${PRODUCT_VARIANT}*.app.tar.gz" 2>/dev/null | head -1)
SIG=$(find "$MACOS_DIR" -name "${PRODUCT_VARIANT}*.app.tar.gz.sig" 2>/dev/null | head -1)

echo ""
echo "构建产物:"
[ -n "$DMG" ] && echo "  DMG: $(basename "$DMG")" || echo "  [缺失] DMG 未找到"
[ -n "$TAR_GZ" ] && echo "  Updater: $(basename "$TAR_GZ")" || echo "  [缺失] tar.gz 未找到"
[ -n "$SIG" ] && echo "  签名: $(basename "$SIG")" || echo "  [缺失] tar.gz.sig 未找到"

[ -n "$DMG" ] || [ -n "$TAR_GZ" ] || { echo "错误: 未找到任何构建产物"; exit 1; }

# ── 上传到 GitHub Release（私有 + 公开仓库）──
upload_to_release() {
  local repo_arg="$1"
  echo ""
  echo "上传到 GitHub Release (${repo_arg:-私有仓库})..."

  if ! gh release view "$TAG" $repo_arg &>/dev/null; then
    echo "  Release 不存在，先创建..."
    gh release create "$TAG" $repo_arg \
      --title "Dimkey $TAG" \
      --notes "Release 准备中..."
  fi

  for f in "$DMG" "$TAR_GZ" "$SIG"; do
    [ -n "$f" ] || continue
    echo "  上传 $(basename "$f")..."
    gh release upload "$TAG" "$f" --clobber $repo_arg
  done
}

upload_to_release ""
upload_to_release "--repo cube1/dimkey-site"

# ── 从 CHANGELOG.md 提取当前版本日志，更新 Release 描述（lang=zh 时执行，避免重复）──
if [ "$LANG_CODE" = "zh" ]; then
  echo ""
  echo "生成双语 release notes（zh + en LLM 翻译）..."
  ./scripts/translate-changelog.sh "$TAG"

  ZH_NOTES_FILE="/tmp/dimkey-release-notes-${TAG}.zh.md"
  EN_NOTES_FILE="/tmp/dimkey-release-notes-${TAG}.en.md"

  if [ -s "$ZH_NOTES_FILE" ]; then
    echo "更新 Release 描述（中文）..."
    gh release edit "$TAG" --notes-file "$ZH_NOTES_FILE"
    gh release edit "$TAG" --notes-file "$ZH_NOTES_FILE" --repo cube1/dimkey-site
    echo "  已从 CHANGELOG.md 更新两个仓库的 Release 描述（中文）"

    # 上传双语 notes 为 release asset，供 workflow_dispatch 阶段复用
    for f in "$ZH_NOTES_FILE" "$EN_NOTES_FILE"; do
      [ -s "$f" ] || continue
      echo "  上传 $(basename "$f") 作为 release asset..."
      gh release upload "$TAG" "$f" --clobber
      gh release upload "$TAG" "$f" --clobber --repo cube1/dimkey-site
    done
  else
    echo "  警告: 未在 CHANGELOG.md 中找到 ${TAG} 的条目，跳过 Release 描述更新"
  fi
fi

# lang=en 完成时，把英文 notes 追加到 Release 描述
if [ "$LANG_CODE" = "en" ]; then
  EN_NOTES_FILE="/tmp/dimkey-release-notes-${TAG}.en.md"
  ZH_NOTES_FILE="/tmp/dimkey-release-notes-${TAG}.zh.md"
  if [ -s "$EN_NOTES_FILE" ] && [ -s "$ZH_NOTES_FILE" ]; then
    echo ""
    echo "把英文 notes 追加到 Release 描述..."
    COMBINED=$(mktemp)
    {
      cat "$ZH_NOTES_FILE"
      echo ""
      echo "---"
      echo ""
      echo "## English"
      echo ""
      cat "$EN_NOTES_FILE"
    } > "$COMBINED"
    gh release edit "$TAG" --notes-file "$COMBINED"
    gh release edit "$TAG" --notes-file "$COMBINED" --repo cube1/dimkey-site
    rm -f "$COMBINED"
    echo "  Release 描述已更新为中英双语"
  fi
fi

# ── 检查 Windows CI 构建状态 + 触发 latest.json 生成（lang=en 时执行，确保两个 lang 的 macOS 产物都已上传）──
if [ "$LANG_CODE" = "en" ]; then
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
      gh run watch "$RUN_ID" --exit-status || true
      RUN_CONCLUSION=$(gh run view "$RUN_ID" --json conclusion --jq '.conclusion' 2>/dev/null || echo "unknown")
    fi

    if [ "$RUN_CONCLUSION" != "success" ]; then
      echo "  警告: CI 运行 #$RUN_ID 结果为 ${RUN_CONCLUSION}"
    else
      echo "  CI 运行成功"
    fi
  fi

  echo ""
  echo "触发 workflow_dispatch 生成 latest-zh.json + latest-en.json ..."
  if gh workflow run release.yml -f tag="$TAG"; then
    echo "  已触发，等待运行开始..."
    sleep 5
    DISPATCH_RUN=$(gh run list --workflow=release.yml --event=workflow_dispatch --limit 1 \
      --json databaseId,status --jq '.[0].databaseId' 2>/dev/null || true)
    if [ -n "$DISPATCH_RUN" ] && [ "$DISPATCH_RUN" != "null" ]; then
      echo "  等待 workflow_dispatch (Run #$DISPATCH_RUN) 完成..."
      gh run watch "$DISPATCH_RUN" --exit-status || echo "  警告: workflow_dispatch 失败"
    fi
  else
    echo "错误: 触发 workflow_dispatch 失败"
    echo "  请手动运行: gh workflow run release.yml -f tag=$TAG"
  fi
fi

echo ""
echo "========================================="
echo "  macOS 发布完成: $TAG ($LANG_CODE)"
echo "========================================="
if [ "$LANG_CODE" = "zh" ]; then
  echo ""
  echo "提示: 接下来执行英文版："
  echo "  ./scripts/release-macos.sh $TAG en"
fi
