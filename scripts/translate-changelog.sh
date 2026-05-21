#!/bin/bash
# 从 CHANGELOG.md 抽取指定 tag 的 section，调用 claude -p 翻译为英文。
# 输出：
#   /tmp/dimkey-release-notes-<TAG>.zh.md  中文原文
#   /tmp/dimkey-release-notes-<TAG>.en.md  英文翻译（或 fallback 文本）
#
# 用法: ./scripts/translate-changelog.sh v0.8.0

set -euo pipefail

TAG="${1:-}"
[ -n "$TAG" ] || { echo "用法: $0 <TAG>" >&2; exit 1; }

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHANGELOG="$REPO_ROOT/CHANGELOG.md"
ZH_OUT="/tmp/dimkey-release-notes-${TAG}.zh.md"
EN_OUT="/tmp/dimkey-release-notes-${TAG}.en.md"

[ -f "$CHANGELOG" ] || { echo "错误: 找不到 $CHANGELOG" >&2; exit 1; }

# ── 抽取 zh section（与 release-macos.sh awk 同思路）──
ZH_BODY=$(awk "
  /^## \\[${TAG}\\]/ { found=1 }
  /^## \\[v/ && found && !/\\[${TAG}\\]/ { exit }
  found { print }
" "$CHANGELOG")

if [ -z "$ZH_BODY" ]; then
  echo "错误: 在 $CHANGELOG 中未找到 ${TAG} 的 section" >&2
  exit 1
fi

echo "$ZH_BODY" > "$ZH_OUT"
echo "已写入中文 release notes: $ZH_OUT"

# ── 调用 claude -p 翻译 ──
FALLBACK_EN="See https://dimkey.com/releases/${TAG} for changelog."

if command -v claude >/dev/null 2>&1; then
  echo "调用 claude -p 翻译为英文..."
  PROMPT="Translate the following Chinese release notes for Dimkey (a local document
desensitization tool) into idiomatic English. Preserve Markdown structure (headings,
bullets, bold). Do not add commentary, just output the translation directly.

---

$ZH_BODY"

  # claude -p 非交互模式
  if claude -p "$PROMPT" --bare --allow-dangerously-skip-permissions > "$EN_OUT" 2>/tmp/claude-translate.log; then
    if [ -s "$EN_OUT" ]; then
      echo "已写入英文 release notes: $EN_OUT"
      exit 0
    fi
    echo "[WARN] claude 输出为空，使用 fallback" >&2
  else
    echo "[WARN] claude 调用失败，使用 fallback:" >&2
    tail -5 /tmp/claude-translate.log >&2 || true
  fi
fi

echo "[WARN] LLM CLI not found or failed, using fallback English notes"
echo "$FALLBACK_EN" > "$EN_OUT"
echo "已写入 fallback 英文 release notes: $EN_OUT"
