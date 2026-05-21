#!/bin/bash
# 发版前置校验：PUBKEY 真公钥已烧入 / dimkey.app API 通畅 / 版本一致 / 工作区干净
# 任一不通过即 exit 1，阻断打包。
#
# 调用点：
#   - scripts/release-macos.sh 开头（参数解析后，临时改写 tauri.conf.json 之前）
#   - .github/workflows/release.yml Windows build job 第一步（CI 工作区天然干净）

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

fail() {
  echo "preflight: ❌ $1" >&2
  exit 1
}

ok() {
  echo "preflight: ✅ $1"
}

# ── 1. PUBKEY 不能是 [0; 32] 占位 ──
CERT_FILE="src-tauri/src/license/certificate.rs"
[ -f "$CERT_FILE" ] || fail "找不到 $CERT_FILE"

# 抽取 PUBKEY_V1 数组（多行），grep 所有非零字节是否存在
PUBKEY_BLOCK=$(awk '/pub const PUBKEY_V1: \[u8; 32\] = \[/,/\];/' "$CERT_FILE")
if [ -z "$PUBKEY_BLOCK" ]; then
  fail "无法在 $CERT_FILE 中定位 PUBKEY_V1 数组"
fi
NONZERO=$(echo "$PUBKEY_BLOCK" | grep -oE '[1-9][0-9]*' || true)
if [ -z "$NONZERO" ]; then
  fail "PUBKEY_V1 仍为 [0; 32] 占位 — 需要先把 dimkey-web 生成的真公钥填进 $CERT_FILE"
fi
ok "PUBKEY_V1 已烧入真公钥（非全零）"

# ── 2. API health endpoint 通畅 ──
API_BASE="${DIMKEY_API_BASE:-https://dimkey.app/api/v1}"
HEALTH_URL="${API_BASE}/health"
HTTP_CODE=$(curl -sS -o /tmp/dimkey-health.json -w "%{http_code}" --max-time 10 "$HEALTH_URL" 2>/dev/null || echo "000")
# 若 curl 失败（exit non-zero），-w 仍会输出 000，|| echo "000" 再追加一次，去掉后6位取最后3位
HTTP_CODE="${HTTP_CODE: -3}"
if [ "${HTTP_CODE}" != "200" ]; then
  fail "${HEALTH_URL} 返回 HTTP ${HTTP_CODE}（期望 200）— dimkey-web 后端可能未部署"
fi

# 检查 health response 中的 pubkey fingerprint 是否匹配本地 PUBKEY_V1 sha256[:16]
if command -v jq >/dev/null 2>&1; then
  BACKEND_FP=$(jq -r '.ed25519_pubkey_fingerprint // empty' /tmp/dimkey-health.json)
  if [ -n "$BACKEND_FP" ]; then
    # 本地 PUBKEY_V1 转字节 → sha256 → 取前 16 字符
    LOCAL_FP=$(echo "$PUBKEY_BLOCK" | grep -oE '[0-9]+' \
      | head -32 | awk '{printf "%02x", $1}' \
      | xxd -r -p | shasum -a 256 | cut -c1-16)
    if [ "$LOCAL_FP" != "$BACKEND_FP" ]; then
      fail "客户端 PUBKEY 指纹 ($LOCAL_FP) 与后端 ($BACKEND_FP) 不一致 — keypair 配错了"
    fi
    ok "PUBKEY 指纹与后端匹配: $LOCAL_FP"
  else
    echo "preflight: ⚠️  health response 未包含 ed25519_pubkey_fingerprint，跳过指纹校验"
  fi
fi
ok "API health endpoint 通畅: $HEALTH_URL"

# ── 3. Cargo.toml 与 tauri.conf.json 版本一致 ──
CARGO_VER=$(grep -E '^version = "' src-tauri/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
TAURI_VER=$(jq -r '.version' src-tauri/tauri.conf.json)
if [ "$CARGO_VER" != "$TAURI_VER" ]; then
  fail "版本不一致: Cargo.toml=$CARGO_VER, tauri.conf.json=$TAURI_VER"
fi
ok "版本号一致: $CARGO_VER"

# ── 4. 工作区干净（无未提交改动）──
# 排除 untracked 的临时文件，只检查 tracked 文件的改动
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "preflight: ⚠️  工作区有未提交改动:"
  git status --short
  fail "提交所有改动后再发版"
fi
ok "工作区干净"

echo "preflight: 全部通过 ✓"
