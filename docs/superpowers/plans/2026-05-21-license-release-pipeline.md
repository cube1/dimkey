# License 集成进发版流水线 — 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 license 控制模块集成到现有 zh/en 双语发版流水线中，发出 v0.8.0。

**Architecture:** license 模块跨 lang 共享存储路径（`~/Library/Application Support/com.dimkey/`，不带 cn/en 后缀），PUBKEY 真公钥编译期写死代码，preflight 脚本在打包前卡占位 PUBKEY / API 不通 / 版本不一致，CHANGELOG 通过 `claude -p` 自动翻译为英文注入 latest-en.json。

**Tech Stack:** Rust (Tauri v2) / ed25519-dalek / mockito / bash / GitHub Actions / Cloudflare Pages

**关联 spec:** `docs/superpowers/specs/2026-05-21-license-release-pipeline-design.md`

**阶段划分：**
- **阶段一（T1–T5）**：客户端代码改动，不依赖外部
- **阶段二（T6–T9）**：流水线脚本，不依赖外部
- **阶段三（T10）**：CHANGELOG v0.8.0 条目
- **阶段四（T11）**：发版前置——dimkey-web 部署完成 + 拿到真公钥（**外部依赖**）
- **阶段五（T12）**：本地实机发版执行

---

## Task 1: license 存储路径跨 lang 共享

**Files:**
- Modify: `src-tauri/src/lib.rs:155-159`
- Test: `src-tauri/src/lib.rs`（无 test，集成测试随 e2e 一起验证；本 task 只改代码 + cargo check）

**目的：** 让 `com.dimkey.cn` 和 `com.dimkey.en` 两个 bundle 共享同一份 `license.lic` 和 `trial.json`，使用户买一份 license 能在同设备 zh+en 之间复用。

- [ ] **Step 1: 修改 lib.rs 的 license 初始化段**

打开 `src-tauri/src/lib.rs`，定位到第 150-161 行（`// === License 系统初始化（Phase 9 集成层）===` 这段）。

替换：

```rust
            // === License 系统初始化（Phase 9 集成层）===
            // 1) 解析 app_config_dir 作为证书/试用记录目录
            // 2) 计算本机指纹（一次性，缓存在 manager）
            // 3) 装配 3 处 TrialStore + manager + boot 决定状态
            // 4) 注册全局 State + 启动后台 heartbeat 任务
            let config_dir = app.path().app_config_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let machine_fp = compute_fingerprint();
            let trial_stores = build_default_stores(config_dir.clone());
            let manager = Arc::new(LicenseManager::new(trial_stores, config_dir, machine_fp));
```

为：

```rust
            // === License 系统初始化（Phase 9 集成层）===
            // license 数据跨 zh/en 包共享：不依赖 Tauri app_config_dir（那个路径附加
            // bundle identifier 会让 com.dimkey.cn 和 com.dimkey.en 分裂）
            let license_config_dir = dirs::config_dir()
                .map(|d| d.join("com.dimkey"))
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            if let Err(e) = std::fs::create_dir_all(&license_config_dir) {
                eprintln!("[license] 创建共享配置目录失败: {} ({})", license_config_dir.display(), e);
            }
            let machine_fp = compute_fingerprint();
            let trial_stores = build_default_stores(license_config_dir.clone());
            let manager = Arc::new(LicenseManager::new(trial_stores, license_config_dir, machine_fp));
```

- [ ] **Step 2: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过，无 warning（dirs crate 已在 Cargo.toml 中）

- [ ] **Step 3: 全量单测确认未引入回归**

Run: `cd src-tauri && cargo test --lib license`
Expected: 所有 license 模块测试 PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "$(cat <<'EOF'
refactor(license): 存储路径改用跨 lang 共享目录 com.dimkey/

不再依赖 app_config_dir（其路径带 bundle identifier 会让 com.dimkey.cn
和 com.dimkey.en 分裂为两份 .lic 和 trial.json）。改用 dirs::config_dir
下的 com.dimkey 目录，让一份 license 能同时激活中英文版。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: api_client 加 mockito 测试

**Files:**
- Modify: `src-tauri/Cargo.toml`（加 dev-dep `mockito = "1"`）
- Modify: `src-tauri/src/license/api_client.rs`（test 模块扩展）

**目的：** CI 上能跑 license API 的业务逻辑测试，覆盖 5 个 endpoint 的 happy/error path，不依赖真实后端。

**重要：** 由于 `api_base()` 通过 `DIMKEY_API_BASE` 环境变量配置 + `reqwest::Client` 是 `OnceLock` 缓存的，多个 `#[tokio::test]` 并发改 env 会 race。**所有 mockito 测试必须合并到一个 `#[tokio::test]` 内串行跑**（参考 commit `c17958c` 的 race 处理思路）。

- [ ] **Step 1: 加 mockito 依赖**

打开 `src-tauri/Cargo.toml`，在 `[dev-dependencies]` 段（当前只有 `tempfile = "3"`）末尾追加：

```toml
mockito = "1"
```

Run: `cd src-tauri && cargo fetch`
Expected: 下载 mockito v1.x

- [ ] **Step 2: 写 activate happy 路径的 mockito 测试**

打开 `src-tauri/src/license/api_client.rs`，在 `#[cfg(test)] mod tests` 块内、文件末尾的 `}` 前追加：

```rust
    /// Mockito 串行测试：5 endpoint × happy/error path。
    ///
    /// 必须单 #[tokio::test]，因为：
    /// 1) api_base() 读 DIMKEY_API_BASE 环境变量；多测试并发改 env 会 race
    /// 2) reqwest::Client 是 OnceLock 缓存的，多 server URL 在同 process 内
    ///    通过 env 切换是唯一方式（不改 api_client 接口的前提下）
    #[tokio::test]
    async fn mockito_all_endpoints_happy_and_error_paths() {
        use mockito::Server;
        let mut server = Server::new_async().await;
        let url = server.url();

        // 保存原 env，结束时恢复
        let original = std::env::var("DIMKEY_API_BASE").ok();
        std::env::set_var("DIMKEY_API_BASE", format!("{}/api/v1", url));

        // ── /activate happy ──
        let m = server.mock("POST", "/api/v1/activate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({
                "ok": true,
                "data": {
                    "license_certificate": {
                        "v": 1,
                        "payload_b64": "cGF5bG9hZA==",
                        "sig_b64": "c2ln"
                    },
                    "device_summary": {
                        "current_device_id": "d1",
                        "active_count": 1,
                        "max_devices": 3
                    }
                }
            }).to_string())
            .create_async().await;
        let body = ActivateBody {
            license_key: "DK-A-B-C-D-E", email: "u@x.com", fingerprint: "fp",
            machine_label: "mac", os: "macos", flavor: "zh", app_version: "0.8.0",
        };
        let r = activate(&body).await.expect("activate ok");
        assert_eq!(r.device_summary.active_count, 1);
        m.assert_async().await;

        // ── /activate error INVALID_LICENSE ──
        let m = server.mock("POST", "/api/v1/activate")
            .with_status(200)
            .with_body(json!({"ok": false, "code": "INVALID_LICENSE", "message": "bad key"}).to_string())
            .create_async().await;
        let r = activate(&body).await;
        assert!(matches!(r, Err(LicenseError::InvalidLicense)));
        m.assert_async().await;

        // ── /activate error DEVICE_LIMIT_REACHED ──
        let m = server.mock("POST", "/api/v1/activate")
            .with_status(200)
            .with_body(json!({
                "ok": false,
                "code": "DEVICE_LIMIT_REACHED",
                "message": "limit",
                "data": {"max_devices": 3, "devices": []}
            }).to_string())
            .create_async().await;
        let r = activate(&body).await;
        assert!(matches!(r, Err(LicenseError::DeviceLimitReached { max: 3, .. })));
        m.assert_async().await;

        // ── /heartbeat happy ──
        let m = server.mock("POST", "/api/v1/heartbeat")
            .with_status(200)
            .with_body(json!({
                "ok": true,
                "data": {"status": "active", "next_check_at": 1234567890}
            }).to_string())
            .create_async().await;
        let r = heartbeat(&HeartbeatBody {
            license_id: "id", device_id: "d", fingerprint: "fp"
        }).await.expect("heartbeat ok");
        assert_eq!(r.status, "active");
        m.assert_async().await;

        // ── /heartbeat error LICENSE_REVOKED ──
        let m = server.mock("POST", "/api/v1/heartbeat")
            .with_status(200)
            .with_body(json!({
                "ok": false, "code": "LICENSE_REVOKED",
                "message": "revoked by admin"
            }).to_string())
            .create_async().await;
        let r = heartbeat(&HeartbeatBody {
            license_id: "id", device_id: "d", fingerprint: "fp"
        }).await;
        assert!(matches!(r, Err(LicenseError::LicenseRevoked { .. })));
        m.assert_async().await;

        // ── /deactivate happy ──
        let m = server.mock("POST", "/api/v1/deactivate")
            .with_status(200)
            .with_body(json!({"ok": true, "data": null}).to_string())
            .create_async().await;
        let r = deactivate(&DeactivateBody {
            license_key: "k", email: "u@x.com", device_id: Some("d"), fingerprint: Some("fp")
        }).await;
        assert!(r.is_ok());
        m.assert_async().await;

        // ── /devices/list happy ──
        let m = server.mock("POST", "/api/v1/devices/list")
            .with_status(200)
            .with_body(json!({
                "ok": true,
                "data": {"devices": [], "max_devices": 3}
            }).to_string())
            .create_async().await;
        let r = list_devices(&DevicesListBody {
            license_key: "k", email: "u@x.com", fingerprint: None
        }).await.expect("list ok");
        assert_eq!(r.max_devices, 3);
        m.assert_async().await;

        // ── /recover happy ──
        let m = server.mock("POST", "/api/v1/recover")
            .with_status(200)
            .with_body(json!({"ok": true}).to_string())
            .create_async().await;
        let r = recover(&RecoverBody { email: "u@x.com" }).await;
        assert!(r.is_ok());
        m.assert_async().await;

        // ── /recover error EMAIL_FORMAT_INVALID ──
        let m = server.mock("POST", "/api/v1/recover")
            .with_status(200)
            .with_body(json!({
                "ok": false, "code": "EMAIL_FORMAT_INVALID", "message": "bad email"
            }).to_string())
            .create_async().await;
        let r = recover(&RecoverBody { email: "bad" }).await;
        assert!(matches!(r, Err(LicenseError::EmailFormatInvalid)));
        m.assert_async().await;

        // ── 5xx → NetworkUnavailable ──
        let m = server.mock("POST", "/api/v1/heartbeat")
            .with_status(500)
            .with_body("internal error")
            .create_async().await;
        let r = heartbeat(&HeartbeatBody {
            license_id: "id", device_id: "d", fingerprint: "fp"
        }).await;
        assert!(matches!(r, Err(LicenseError::NetworkUnavailable)));
        m.assert_async().await;

        // 恢复 env
        match original {
            Some(v) => std::env::set_var("DIMKEY_API_BASE", v),
            None => std::env::remove_var("DIMKEY_API_BASE"),
        }
    }
```

- [ ] **Step 3: 跑测试验证**

Run: `cd src-tauri && cargo test --lib license::api_client::tests::mockito_all_endpoints -- --nocapture`
Expected: PASS（所有 mock assertion 通过）

- [ ] **Step 4: 跑 api_client 全量测试确认不影响既有 env race fix**

Run: `cd src-tauri && cargo test --lib license::api_client`
Expected: 全部 PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/license/api_client.rs
git commit -m "$(cat <<'EOF'
test(license): mockito 测试覆盖 5 个 API endpoint 的 happy/error path

CI 上不依赖真实后端即可验证 api_client 的业务逻辑：错误码映射、
data 字段解析、网络错误降级。所有 mock 测试串行跑在单 #[tokio::test]
中（与 c17958c 同样思路，绕开 env race + reqwest OnceLock 缓存约束）。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: 版本号升 0.8.0

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `package.json`（若有 version 字段则同步；当前应是 `"version": "0.0.0"` 或不存在，按实际情况）

- [ ] **Step 1: bump Cargo.toml**

修改 `src-tauri/Cargo.toml` 第 3 行附近：

```toml
version = "0.7.0"
```

为：

```toml
version = "0.8.0"
```

- [ ] **Step 2: bump tauri.conf.json**

修改 `src-tauri/tauri.conf.json` 中的 `"version": "0.7.0"`：

```json
"version": "0.8.0",
```

- [ ] **Step 3: 检查 package.json**

Run: `grep '"version"' package.json`
- 如果输出 `"version": "0.7.0"` 之类，把它改成 `"0.8.0"`
- 如果没有 version 字段，跳过

- [ ] **Step 4: cargo check 验证两处版本一致**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: 编译成功，无版本相关 warning

Run: `grep -E '"version"|^version' src-tauri/Cargo.toml src-tauri/tauri.conf.json | head`
Expected: 两个文件都显示 `0.8.0`

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/tauri.conf.json package.json
git commit -m "$(cat <<'EOF'
chore(release): bump version 0.7.0 → 0.8.0

License 控制是 minor 级 new feature（含 trial 过期注水印的 breaking
行为变化），按惯例升 minor。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: 写 preflight 校验脚本

**Files:**
- Create: `scripts/preflight-license.sh`

**目的：** 在打包之前卡掉：PUBKEY 占位、API 不通、版本不一致、工作区脏。

- [ ] **Step 1: 创建脚本**

新建 `scripts/preflight-license.sh`：

```bash
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
HTTP_CODE=$(curl -sS -o /tmp/dimkey-health.json -w "%{http_code}" --max-time 10 "$HEALTH_URL" || echo "000")
if [ "$HTTP_CODE" != "200" ]; then
  fail "$HEALTH_URL 返回 HTTP $HTTP_CODE（期望 200）— dimkey-web 后端可能未部署"
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
```

- [ ] **Step 2: 加可执行权限**

Run: `chmod +x scripts/preflight-license.sh`

- [ ] **Step 3: 本地试跑（预期 PUBKEY 占位 fail）**

Run: `./scripts/preflight-license.sh`
Expected: `❌ PUBKEY_V1 仍为 [0; 32] 占位` 退出码 1（在 T11 真公钥填入之前，这是预期行为）

- [ ] **Step 4: Commit**

```bash
git add scripts/preflight-license.sh
git commit -m "$(cat <<'EOF'
feat(release): preflight 校验脚本 — 卡占位 PUBKEY / API 不通 / 版本不一致

发版前置硬约束：PUBKEY 非全零、dimkey.app /health 200、Cargo.toml 与
tauri.conf.json 版本一致、git 工作区干净。任一不通过 exit 1 阻断打包。

附带 PUBKEY 指纹校验：health endpoint 返回 sha256(pubkey)[:16]，本地计算
对比，防止"keypair 配错"事故。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: 写 CHANGELOG 翻译脚本

**Files:**
- Create: `scripts/translate-changelog.sh`

**目的：** 发版时把 CHANGELOG 当前版本的中文 section 自动翻译为英文，供 latest-en.json + 英文 Release 描述使用。

**依赖：** `claude` CLI（已确认本机 `/Users/tanzs-mac-mini/.local/bin/claude` 可用，支持 `-p` 非交互模式）

- [ ] **Step 1: 创建脚本**

新建 `scripts/translate-changelog.sh`：

```bash
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

  # claude -p 非交互模式，stdin 不读，--print 走 prompt 参数
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
```

- [ ] **Step 2: 加可执行权限**

Run: `chmod +x scripts/translate-changelog.sh`

- [ ] **Step 3: 本地试跑（用现有 v0.6.0 测试）**

Run: `./scripts/translate-changelog.sh v0.6.0`
Expected:
- `/tmp/dimkey-release-notes-v0.6.0.zh.md` 包含 v0.6.0 的 markdown
- `/tmp/dimkey-release-notes-v0.6.0.en.md` 包含 LLM 翻译的英文（或 fallback 文本）

如果 claude 调用要求交互授权失败，先用 fallback 路径走通即可，T6 集成时再处理 claude 授权。

Run: `head -10 /tmp/dimkey-release-notes-v0.6.0.zh.md`
Expected: 中文 markdown 内容

- [ ] **Step 4: Commit**

```bash
git add scripts/translate-changelog.sh
git commit -m "$(cat <<'EOF'
feat(release): CHANGELOG 中英文翻译脚本 — 调 claude -p 自动翻译

发版时为 latest-en.json + 英文 Release 描述提供英文版 release notes。
claude CLI 不可用或调用失败时降级为固定 fallback 文本，不阻断发版。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: release-macos.sh 集成 preflight + 双语 notes

**Files:**
- Modify: `scripts/release-macos.sh`

**目的：** 在 mac 本地构建脚本中接入 preflight 卡占位 PUBKEY，并在 lang=zh 时生成双语 notes、lang=en 时把英文 notes 追加到 Release 描述、并把双语 notes 上传为 release asset 供 release.yml 复用。

- [ ] **Step 1: 在脚本中插入 preflight 调用**

打开 `scripts/release-macos.sh`，定位到 "── 检查前置条件 ──" 段（约 70-80 行）之后、"── 顺序检查：lang=en 时必须先有 zh 产物 ──" 之前，插入：

```bash
# ── License preflight 校验（lang=zh 阶段调用，避免 lang=en 重复跑）──
# 在临时改写 tauri.conf.json 和 Cargo.toml 之前跑，确保"工作区干净"判定准确。
if [ "$LANG_CODE" = "zh" ]; then
  echo ""
  echo "运行 License preflight 校验..."
  ./scripts/preflight-license.sh
fi
```

- [ ] **Step 2: 在 lang=zh 抽完中文 notes 后调用翻译脚本**

定位到 "── 从 CHANGELOG.md 提取当前版本日志，更新 Release 描述（lang=zh 时执行，避免重复）──" 这段（脚本接近末尾）。

把当前的：

```bash
if [ "$LANG_CODE" = "zh" ]; then
  echo ""
  echo "更新 Release 描述..."
  RELEASE_NOTES=$(awk "
    /^## \\[${TAG}\\]/ { found=1 }
    /^## \\[v/ && found && !/\\[${TAG}\\]/ { exit }
    found { print }
  " CHANGELOG.md)

  if [ -n "$RELEASE_NOTES" ]; then
    gh release edit "$TAG" --notes "$RELEASE_NOTES"
    gh release edit "$TAG" --notes "$RELEASE_NOTES" --repo cube1/dimkey-site
    echo "  已从 CHANGELOG.md 更新两个仓库的 Release 描述"
  else
    echo "  警告: 未在 CHANGELOG.md 中找到 ${TAG} 的条目"
  fi
fi
```

替换为：

```bash
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
```

- [ ] **Step 3: 干跑（不要真发版）检查语法**

Run: `bash -n scripts/release-macos.sh`
Expected: 无 syntax error 输出

- [ ] **Step 4: Commit**

```bash
git add scripts/release-macos.sh
git commit -m "$(cat <<'EOF'
feat(release): mac 发版脚本接入 preflight + 双语 release notes

- 在 lang=zh 阶段跑 preflight-license.sh（卡占位 PUBKEY/API 不通/版本不一致）
- 在 lang=zh 阶段调 translate-changelog.sh 生成中英文 notes 文件
- 双语 notes 作为 release asset 上传，供 release.yml workflow_dispatch 复用
- lang=en 完成时把英文 notes 追加到 Release 描述（中英并列）

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: release.yml 集成 preflight + latest-en.json 英文 notes

**Files:**
- Modify: `.github/workflows/release.yml`

**目的：** Windows CI build job 跑 preflight；workflow_dispatch 阶段下载 release asset 中的英文 notes，写入 `latest-en.json`。

- [ ] **Step 1: 在 build job 中插入 preflight 步骤**

打开 `.github/workflows/release.yml`，定位到 build job 的步骤序列。在 "恢复 NER 模型缓存（${{ matrix.lang }}）" 之前、"- uses: actions/checkout@v4" 之后，插入：

```yaml
      - name: License preflight 校验（lang=zh 阶段）
        if: matrix.lang == 'zh'
        shell: bash
        run: |
          chmod +x scripts/preflight-license.sh
          ./scripts/preflight-license.sh
```

理由：CI 上工作区天然干净；只在 lang=zh job 跑一次 preflight 即可（两个 lang 共用同一份代码，不必跑两次）。

- [ ] **Step 2: 在 generate-updater-json job 下载英文 notes asset**

定位到 `generate-updater-json` job 的 "下载更新包签名文件" step 之后、"生成 latest-zh.json 和 latest-en.json" step 之前，插入：

```yaml
      - name: 下载双语 release notes
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          gh release download "${TAG_NAME}" \
            --dir release-assets \
            --repo "${{ github.repository }}" \
            --pattern "dimkey-release-notes-*.md" \
            || true
          echo "=== 下载到的 notes 文件 ==="
          ls -la release-assets/dimkey-release-notes-*.md 2>/dev/null || echo "（无）"
```

- [ ] **Step 3: 修改 latest-{zh,en}.json 生成逻辑使用双语 notes**

定位到 "生成 latest-zh.json 和 latest-en.json" step 中的 jq 命令段。把：

```bash
            jq -n \
              --arg version "$VERSION" \
              --arg pub_date "$PUB_DATE" \
              --arg notes "请查看更新日志了解详细变更。" \
              --argjson platforms "$PLATFORMS" \
              '{version: $version, notes: $notes, pub_date: $pub_date, platforms: $platforms}' \
              > "latest-${LANG}.json"
```

替换为：

```bash
            # 按 lang 选 notes 文件
            NOTES_FILE="release-assets/dimkey-release-notes-${TAG_NAME}.${LANG}.md"
            if [ -s "$NOTES_FILE" ]; then
              NOTES=$(cat "$NOTES_FILE")
            elif [ "$LANG" = "en" ]; then
              NOTES="See https://dimkey.com/releases/${TAG_NAME} for changelog."
            else
              NOTES="请查看 https://dimkey.com 了解详细变更。"
            fi

            jq -n \
              --arg version "$VERSION" \
              --arg pub_date "$PUB_DATE" \
              --arg notes "$NOTES" \
              --argjson platforms "$PLATFORMS" \
              '{version: $version, notes: $notes, pub_date: $pub_date, platforms: $platforms}' \
              > "latest-${LANG}.json"
```

- [ ] **Step 4: 验证 YAML 合法性**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))" && echo OK`
Expected: `OK`（无 YAML 语法错误）

如果 PyYAML 未装可用 alternative：

Run: `yq eval '.jobs.build.steps[].name' .github/workflows/release.yml | head -20`
Expected: 输出 step 名字列表，包含新加的 "License preflight 校验（lang=zh 阶段）"

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "$(cat <<'EOF'
feat(release): Windows CI 接入 preflight + latest-en.json 英文 notes

- build job 在 lang=zh 矩阵跑 preflight-license.sh（CI 工作区天然干净）
- workflow_dispatch 阶段下载 release asset 中的 dimkey-release-notes-*.md
- latest-zh.json / latest-en.json 的 notes 字段分别填中英文版本
  （从 release asset 读取，由 mac 发版脚本上传的双语 notes 文件提供）

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: dev guide — 本地实机激活流程文档

**Files:**
- Create: `docs/dev-license-activation.md`

**目的：** 开发者在 dimkey-web staging Worker 部署后，能照文档跑通本地实机激活，验证客户端 license 流程。

- [ ] **Step 1: 创建文档**

新建 `docs/dev-license-activation.md`：

```markdown
# 本地实机 license 激活流程

> dev/QA 用。生产用户无需关心。

## 适用场景

- 验证 license 完整流程：激活 → 写 .lic → heartbeat → 吊销 → 重新激活
- 验证跨 zh/en 包共享 license（关键 e2e 场景）
- 客户端 license 代码改动后做实机回归

## 前置条件

1. dimkey-web 仓库已部署一个 **staging Worker**，URL 形如 `https://dimkey-staging.your-account.workers.dev/api/v1`
2. staging 用一对 **测试 keypair**（与生产 keypair 区分开），公钥已临时填进本地 `src-tauri/src/license/certificate.rs:15`（**不要 commit 这个临时改动**）
3. dimkey-web staging 数据库中已预置一个 test license：`DK-TEST1-TEST2-TEST3-TEST4-TEST5`、email `dev@dimkey.local`

## 操作步骤

### 1. 启动 dev 服务

\`\`\`bash
# 指向 staging API
export DIMKEY_API_BASE=https://dimkey-staging.your-account.workers.dev/api/v1

# 默认中文版
TAURI_DEV_HOST=127.0.0.1 cargo tauri dev
\`\`\`

### 2. 激活

应用启动后：
- 打开 About → License → "激活"
- 输入 test license_key + email
- 确认激活成功，UI 显示 Plan 类型

### 3. 验证 .lic 已写入共享路径

\`\`\`bash
ls -la ~/Library/Application\\ Support/com.dimkey/license.lic
\`\`\`

注意：是 `com.dimkey/`，**不是** `com.dimkey.cn/` 或 `com.dimkey.en/`。

### 4. 验证跨 lang 共享

\`\`\`bash
# 关闭当前 dev，切到英文 feature
cargo tauri dev --no-default-features --features lang-en
# 同时前端：VITE_DIMKEY_LANG=en npm run dev
\`\`\`

英文版启动后应**直接读到** license 已激活，无需重新输入 key。

### 5. 验证 heartbeat

heartbeat 任务每 24h 跑一次。本地手动触发：在 `src-tauri/src/license/heartbeat.rs` 临时把间隔改为 60s，重启 app，观察日志输出 "heartbeat: ok" 或对应错误码。

测完**还原间隔**到 24h，不要 commit。

### 6. 验证吊销

在 dimkey-web staging 后台 SQL 把该 device 的 `revoked_at` 设为 now。等下次 heartbeat 触发，客户端应进入 Revoked 状态、UI 显示横幅。

## 常见坑

- **PUBKEY 没换 staging**：客户端用生产 PUBKEY，staging keypair 签的 .lic 必然 SignatureInvalid。检查 `certificate.rs:15` 是否是 staging 公钥。
- **`DIMKEY_API_BASE` 未生效**：env var 必须在 `cargo tauri dev` 之前 export，且 reqwest client 是 OnceLock 缓存的，运行时改 env 无效——必须重启 app。
- **共享路径未创建**：第一次启动应在 `~/Library/Application Support/com.dimkey/` 自动建目录。如果失败检查 lib.rs 中 `create_dir_all` 是否 silently failing。

## 切换回生产 PUBKEY

\`\`\`bash
git checkout src-tauri/src/license/certificate.rs
\`\`\`

确保临时填入的 staging 公钥不会被误 commit。
```

- [ ] **Step 2: Commit**

```bash
git add docs/dev-license-activation.md
git commit -m "$(cat <<'EOF'
docs(license): 本地实机激活流程文档（dev/QA 用）

补齐 spec §3.4.2 提到的 dev guide：dimkey-web staging Worker 部署后，
开发者怎么本地跑通激活、验证跨 zh/en 共享、heartbeat、吊销。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: CHANGELOG.md v0.8.0 条目

**Files:**
- Modify: `CHANGELOG.md`

**目的：** 在发版前补 v0.8.0 的更新日志条目，让 release-macos.sh 能抽取到内容。

- [ ] **Step 1: 在 CHANGELOG.md 顶部插入 v0.8.0 段**

打开 `CHANGELOG.md`，在 `# 更新日志` 标题之后、`## [v0.6.0]` 之前（注意主线最新是 v0.6.0，因为 v0.7.0 还没正式发，0.8.0 直接跳到），插入：

```markdown
## [v0.8.0] — 2026-05-21

**下载：** [官网下载](https://dimkey.com/#download)

### 新功能
- **许可证控制（License）**：30 天试用期 + Ed25519 离线证书验签 + 设备指纹绑定 + 24h 心跳吊销检测
  - 同一份 license 可在同设备上同时激活中文版和英文版（跨 bundle id 共享存储）
  - 试用期 ≤7 天时右上角倒计时角标，过期后导出文件自动注入水印（xlsx/csv/docx/txt 四格式）
  - About 弹窗集成 license 区块：激活 / 停用 / 设备管理 / 邮箱找回
  - 试用期防篡改：3 处冗余存储（config_dir / hidden file / keyring）取最早 first_run_at
- **跨语言版本共享 license**：从中文版切换到英文版无需重新激活（hidden file + keyring 跨 bundle 共享）

### 重构
- **license 存储路径**：从 `app_config_dir`（带 bundle id）改为 `dirs::config_dir/com.dimkey`（跨 lang 共享）

### CI
- **发版前置 preflight 校验**：卡占位 PUBKEY / API 不通 / 版本不一致 / 工作区脏，避免坏包流入生产
- **CHANGELOG 双语**：release 脚本调 claude -p 自动翻译中文 changelog 为英文，注入 latest-en.json
- **api_client mockito 测试**：CI 上不依赖真实后端即可验证 5 个 license endpoint 业务逻辑

```

注意空一行以确保 Markdown 标题间距正确。

- [ ] **Step 2: 验证 awk 抽取能拿到 v0.8.0 section**

Run:
```bash
awk '
  /^## \[v0.8.0\]/ { found=1 }
  /^## \[v/ && found && !/\[v0.8.0\]/ { exit }
  found { print }
' CHANGELOG.md
```

Expected: 输出 v0.8.0 完整 section，不包含 v0.6.0 后续内容

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "$(cat <<'EOF'
docs(changelog): v0.8.0 — license 控制 + 跨语言共享存储

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: 真公钥烧入 + 测试改造（**阻塞于 dimkey-web 部署**）

**Files:**
- Modify: `src-tauri/src/license/certificate.rs`（PUBKEY_V1 + 改造 `read_certificate_with_placeholder_pubkey_always_fails_signature` 测试）
- Create: `src-tauri/tests/fixtures/test-license.lic`（测试 fixture）

**前置：** dimkey-web 仓库已生成 keypair，公钥（32 字节）+ 测试 fixture .lic（用测试 license_key 签名）已交付。

**临时跳过判定：** 如果在 dimkey-web 完成前需要继续 plan 后续任务，本 task 可标 `BLOCKED` 跳过；T11 发版任务会再次卡这里。

- [ ] **Step 1: 替换 PUBKEY_V1**

打开 `src-tauri/src/license/certificate.rs`，把第 11-18 行：

```rust
// ⚠️ TODO(plan-a): 必须用 dimkey-web 仓库 scripts/gen-ed25519-keypair.ts 输出的
// pub array 替换以下 32 个 0。后端私钥已存为 Workers Secret ED25519_PRIVATE_KEY。
// 在 Plan A 部署前，本占位会让所有 verify 调用失败 (SignatureInvalid)，预期行为：
// 客户端无法激活 → 走 Trial 分支。Task 4.2 (集成测试) 同样依赖此真公钥。
pub const PUBKEY_V1: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
```

替换为 dimkey-web issue 出的真公钥字节数组：

```rust
/// 生产 ed25519 公钥（dimkey-web Cloudflare Workers ED25519_PRIVATE_KEY 对应）
/// 轮换流程：追加新公钥到 KNOWN_PUBKEYS，逐步发版让客户端识别新公钥后再下线旧的
pub const PUBKEY_V1: [u8; 32] = [
    <从 dimkey-web 拿到的 32 字节，例如：0x1a, 0x2b, ...>
];
```

- [ ] **Step 2: 把测试 fixture .lic 提交进仓库**

新建目录：

Run: `mkdir -p src-tauri/tests/fixtures`

把 dimkey-web issue 出的测试 fixture（用真公钥 + 测试 license_key 签名的 .lic 文件）保存到：

`src-tauri/tests/fixtures/test-license.lic`

内容形如：

```json
{
  "v": 1,
  "payload_b64": "<base64 of LicensePayload with license_id=test-* , license_key=DK-TEST1-..., device_id=test-device, fingerprint=test-fp, key_version=1>",
  "sig_b64": "<base64 of Ed25519 signature of payload_bytes>"
}
```

**安全性确认**：fixture 用的 license_id 必须以 `test-` 前缀，生产数据库 `licenses` 表禁止该前缀；fingerprint 用占位字符串 `test-fp-do-not-trust`。这样泄露公开仓库也无法在生产环境激活任何设备。

- [ ] **Step 3: 改造 placeholder fail 测试为 fixture pass 测试**

打开 `src-tauri/src/license/certificate.rs`，定位到 `fn read_certificate_with_placeholder_pubkey_always_fails_signature` 测试（约第 259-280 行）。整个删除，替换为：

```rust
    #[test]
    fn read_certificate_with_fixture_lic_passes_signature() {
        // 用 dimkey-web issue 出的测试 fixture：真公钥 + 测试 license_key 签名的 .lic
        // payload 中的 license_id 以 test- 前缀，生产数据库禁止该前缀，安全可 commit
        let d = tempdir().unwrap();
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/test-license.lic");
        let fixture = std::fs::read(&fixture_path)
            .expect("fixture should be committed in tests/fixtures/test-license.lic");
        std::fs::write(d.path().join(CERTIFICATE_FILE), fixture).unwrap();

        let r = read_certificate(d.path()).expect("fixture .lic should verify");
        assert!(r.license_id.starts_with("test-"), "fixture license_id should be test-*");
        assert_eq!(r.key_version, 1);
    }
```

- [ ] **Step 4: 跑测试验证**

Run: `cd src-tauri && cargo test --lib license::certificate`
Expected: 所有 certificate 测试 PASS，包含新的 `read_certificate_with_fixture_lic_passes_signature`

- [ ] **Step 5: 再跑全量测试确认无回归**

Run: `cd src-tauri && cargo test`
Expected: 全部 PASS

- [ ] **Step 6: 跑 preflight 验证 PUBKEY 非占位**

Run: `./scripts/preflight-license.sh`
Expected:
- ✅ PUBKEY_V1 已烧入真公钥
- ❓ API health 视 dimkey-web 部署状态而定（部署后通过；未部署仍 fail）
- ✅ 版本号一致

如果 health 检查仍 fail，等 dimkey-web 部署完成再回来跑。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/license/certificate.rs src-tauri/tests/fixtures/test-license.lic
git commit -m "$(cat <<'EOF'
feat(license): 烧入真 PUBKEY_V1 + fixture .lic 替换占位失败测试

PUBKEY_V1 替换为 dimkey-web 仓库生成的 ed25519 真公钥（生产 Workers
Secret 对应）。原"占位 PUBKEY 必然失败"测试改为用 test-* 前缀的
fixture .lic 做正向验签 —— fixture 安全可 commit，因为生产数据库禁
止 test-* 前缀的 license_id。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 11: 发版执行（**阻塞于 T10 真公钥烧入完成 + dimkey-web 部署完成**）

**Files:** 无代码改动；操作步骤记录。

**前置 checklist：**
- [ ] dimkey-web 仓库已合并 keypair 生成、Workers 部署完成
- [ ] `curl https://dimkey.app/api/v1/health` 返回 200
- [ ] T10 已 commit，PUBKEY_V1 是真公钥
- [ ] CHANGELOG v0.8.0 条目已确认（T9）
- [ ] 当前在 `feat/license-control` 分支，所有 T1-T10 已 commit
- [ ] mac 端的发版前置环境变量都已 export：
  - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
  - `APPLE_ID`
  - `APPLE_PASSWORD`
- [ ] `~/.tauri/dimkey.key` 存在
- [ ] gh CLI 已登录、cargo tauri 已安装

- [ ] **Step 1: 合并 feat/license-control 到 main**

```bash
git checkout main
git pull origin main
git merge --no-ff feat/license-control -m "Merge feat/license-control: license 控制 + zh/en 共享存储集成"
git push origin main
```

如果有 merge conflict，逐个 resolve（CHANGELOG 在最上方插入可能冲突）。

- [ ] **Step 2: 打 tag 并推送**

```bash
git tag v0.8.0
git push origin v0.8.0
```

push tag 会触发 Windows CI build（zh + en 矩阵），可同时进行 mac 本地构建。

- [ ] **Step 3: 跑 mac 本地构建（zh）**

```bash
./scripts/release-macos.sh v0.8.0 zh
```

Expected:
- preflight 通过
- 编译 + 公证 + DMG 生成
- 上传到 cube1/dimkey + cube1/dimkey-site Release
- /tmp/dimkey-release-notes-v0.8.0.{zh,en}.md 已生成
- Release 描述更新为中文 notes
- 双语 notes 文件作为 release asset 已上传

- [ ] **Step 4: 跑 mac 本地构建（en）**

```bash
./scripts/release-macos.sh v0.8.0 en
```

Expected:
- preflight 不跑（lang=en 时跳过，避免重复）
- 编译 + 公证 + DMG 生成
- 上传到两个仓库 Release
- Release 描述追加英文 notes（中英并列）
- 自动触发 workflow_dispatch 生成 latest-{zh,en}.json

- [ ] **Step 5: 等待 Windows CI 完成**

如果 mac en 完成时 Windows CI 还在跑：
```bash
gh run watch
```

mac en 脚本会自动等待 CI 完成再触发 workflow_dispatch，无需手动干预。

- [ ] **Step 6: 验收**

按 spec §5 验收标准过一遍：

1. ✅ `curl https://dimkey.com/latest-zh.json | jq .notes` 含中文 release notes
2. ✅ `curl https://dimkey.com/latest-en.json | jq .notes` 含英文 release notes
3. ✅ `gh release view v0.8.0 --repo cube1/dimkey-site` 含 zh + en 全部 6 个产物（dmg/tar.gz/sig × 2）+ exe/setup.exe/sig × 2
4. ✅ 装 zh 包 → About 显示 trial → 激活成功 → 装 en 包 → 直接看到激活态（验证跨 lang 共享）

- [ ] **Step 7: 在 dimkey-site Release 描述上标记 latest**

Run: `gh release edit v0.8.0 --latest --repo cube1/dimkey-site`

Expected: dimkey.com 落地页"最新版本"自动指向 v0.8.0

---

## Self-Review

**1. Spec coverage check:**

| Spec § | 对应 Task |
|---|---|
| 3.1.1 PUBKEY 烧入 | T10 |
| 3.1.2 存储路径跨 lang 共享 | T1 |
| 3.1.3 版本号 0.8.0 | T3 |
| 3.1.4 不做迁移 | （仅文档化决定，无 task 实现） |
| 3.2.1 preflight 脚本 | T4 + T6 + T7 |
| 3.2.2 CHANGELOG LLM 翻译 | T5 + T6 + T7 |
| 3.3 跨仓库顺序 | T11 前置 checklist |
| 3.4.1 mockito 测试 | T2 |
| 3.4.2 dev guide | T8 |
| 5 验收标准 | T11 Step 6 |

**2. Placeholder scan**: 无 "TBD" / "TODO" / "fill in later"。T10 提到的真公钥字节、fixture 文件内容必须由 dimkey-web 仓库提供——这是外部依赖而非 plan 内的占位。

**3. Type consistency**: `LicenseError::DeviceLimitReached { max, .. }`、`DevicesListData { max_devices }`、`ActivateBody` 字段都与 api_client.rs 现有定义对齐。

---

## Execution Notes

- **阶段一 (T1-T3)** 可独立合入 main，无外部依赖
- **阶段二 (T4-T7)** 可独立合入 main，无外部依赖
- **阶段三 (T8-T9)** 可独立合入 main，无外部依赖
- **T10** 必须等 dimkey-web 仓库完成 keypair + Workers 部署，且拿到测试 fixture
- **T11** 必须 T10 完成

T1-T9 可以**先在 feat/license-control 分支推进**并合 main。T10 单独一次 commit，T11 是操作步骤不涉及代码改动。
