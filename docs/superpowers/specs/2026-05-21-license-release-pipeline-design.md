# License 集成进发版流水线 — 设计稿

**日期**：2026-05-21
**目标版本**：0.7.0 → **0.8.0**
**关联文档**：
- `docs/superpowers/specs/2026-05-14-license-control-design.md`（许可证控制设计稿，本文是其发版集成配套）
- `docs/superpowers/specs/2026-04-02-i18n-english-support-design.md`（中英文分包基础）

---

## 0. 背景

`feat/license-control` 分支实现了 license 控制模块（trial / 证书验签 / heartbeat / 水印），但**还未集成到 zh/en 双语发版流水线**。同时 dimkey-web 后端（license API、ed25519 keypair）也尚未部署。本设计稿合并解决：

1. license 模块本身的两个**跨 lang 共享**改造（存储路径 + PUBKEY 烧入）
2. 现有 `release.yml` + `release-macos.sh` 双语流水线的**集成点**（preflight 校验 + CHANGELOG 双语）
3. **跨仓库发版顺序**（dimkey-web 先于 dimkey）
4. **测试策略**（CI / 本地 / 实机三层）

明确**不在范围**（后续独立立项，仅在文末挂 TODO）：
- Windows Authenticode 代码签名
- LemonSqueezy webhook（dimkey-web 仓库职责）
- 国内手动激活渠道

---

## 1. 现状盘点

### 1.1 已实现（main 分支上）

| 维度 | zh | en |
|---|---|---|
| Bundle identifier | `com.dimkey.cn` | `com.dimkey.en` |
| Cargo feature | `lang-zh` | `lang-en` |
| Vite env | `VITE_DIMKEY_LANG=zh` | `VITE_DIMKEY_LANG=en` |
| NER 模型 | chinese (bert4ner) | distilbert-ner (dslim) |
| Updater endpoint | `https://dimkey.com/latest-zh.json` | `https://dimkey.com/latest-en.json` |
| 产物命名 | `Dimkey-zh_*.dmg` / `*-setup.exe` | `Dimkey-en_*.dmg` / `*-setup.exe` |

发版触发：`./scripts/release-macos.sh vX.Y.Z zh` → `... vX.Y.Z en` → push tag 触发 Windows CI 矩阵 → mac en 完成时调用 `workflow_dispatch` 生成 `latest-{zh,en}.json` 并同步到 `cube1/dimkey-site` 的 GitHub Pages（`dimkey.com`）。

### 1.2 license 分支引入但未集成

| 项目 | 现状 |
|---|---|
| `certificate.rs:15` `PUBKEY_V1` | `[0; 32]` 占位，所有 verify 失败 |
| `api_client.rs:16` `DEFAULT_API_BASE` | `https://dimkey.app/api/v1`（生产域）|
| license 存储路径 | 走 `app.path().app_config_dir()` → 在 zh/en 间**分裂** |
| dimkey-web 后端 | keypair 未生成、Workers 未部署 |

---

## 2. 设计原则

1. **license 一份激活 zh+en**：用户买一份证书在同一台设备上同时激活中英文版（业务决策，参见 brainstorming 记录）。
2. **PUBKEY 写死代码**：公钥不是秘密，编译期硬编码最简单。preflight 脚本卡占位 PUBKEY 流入生产。
3. **流水线零侵入**：现有 release.yml + release-macos.sh 结构不动，只新增 preflight 校验 + CHANGELOG 双语注入。
4. **跨仓库顺序硬约束**：dimkey-web 后端必须先部署。preflight 卡 API 健康检查。
5. **本轮聚焦 license 集成**：Windows 签名等独立问题不并发处理。

---

## 3. 设计详解

### 3.1 客户端代码改动

#### 3.1.1 PUBKEY 真公钥烧入

**文件**：`src-tauri/src/license/certificate.rs`

将第 11–18 行：
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

替换为 dimkey-web 仓库 `scripts/gen-ed25519-keypair.ts` 输出的真公钥（32 字节）。TODO 注释删除。

**测试**：`certificate.rs:222` 处的 sign+verify roundtrip 单测保留（用临时 keypair，不依赖 PUBKEY_V1）；`certificate.rs:261` 处的"占位 PUBKEY 必然失败"测试**需要删除**——切换为真公钥后这个断言会反向（任何持有合法签名的 envelope 都应验证成功）。改写为：用 PUBKEY_V1 对应的**测试 fixture .lic 文件**做正向验证。

fixture 安全性：fixture 用一个**测试专用的 license_id / license_key**（dimkey-web 后端在测试数据库中预置、生产数据库不含），device_id 和 fingerprint 用占位字符串。这样 fixture commit 进公开仓库不会暴露任何真实用户数据，也无法在生产环境被滥用（生产后端查不到该 license_id）。

#### 3.1.2 license/trial 存储跨 lang 共享

**文件**：`src-tauri/src/lib.rs:155`

当前：
```rust
let config_dir = app.path().app_config_dir()
    .unwrap_or_else(|_| std::path::PathBuf::from("."));
```

改为：
```rust
// license 数据跨 zh/en 包共享：不依赖 Tauri app_config_dir
// （那个路径会附加 bundle identifier，导致 com.dimkey.cn 和 com.dimkey.en 分裂）
let license_config_dir = dirs::config_dir()
    .map(|d| d.join("com.dimkey"))
    .unwrap_or_else(|| std::path::PathBuf::from("."));
std::fs::create_dir_all(&license_config_dir).ok();
let machine_fp = compute_fingerprint();
let trial_stores = build_default_stores(license_config_dir.clone());
let manager = Arc::new(LicenseManager::new(trial_stores, license_config_dir, machine_fp));
```

**路径**：
- macOS: `~/Library/Application Support/com.dimkey/{license.lic, trial.json}`
- Windows: `%APPDATA%/com.dimkey/{license.lic, trial.json}`
- Linux: `~/.config/com.dimkey/{license.lic, trial.json}`

**注意**：
- 应用本身的其他数据（导出历史、用户配置等）继续走 `app_config_dir()` 拿 zh/en 各自的目录，本改动**只影响 license 模块**。
- HiddenFileStore 已经用 `~/.dimkey_state`（跨 lang 共享）、KeyringStore 已经用 `com.dimkey.trial` service name（跨 lang 共享）；本次只补齐 ConfigDirStore + license.lic 这一处分裂。

#### 3.1.3 版本号升 0.8.0

- `src-tauri/Cargo.toml`：`version = "0.8.0"`
- `src-tauri/tauri.conf.json`：`"version": "0.8.0"`
- `package.json` 若有 version 字段，同步

升 minor 的语义：license 是新 feature，且引入了 breaking 行为变化（trial 过期后会加水印）。

#### 3.1.4 旧路径迁移

**结论：不做。**

0.7.x 现网包不含 license 模块（license 仅在 `feat/license-control` 分支）。0.8.0 首次启动时不存在"老路径"的 license/trial 数据需要迁移。spec 在此明确记录该决策，避免后续无谓加迁移代码。

### 3.2 流水线改动

#### 3.2.1 新增 preflight 校验脚本

**文件**：`scripts/preflight-license.sh`（新建）

职责（任一不通过即 exit 1）：
1. grep `certificate.rs` 中 `PUBKEY_V1` 不是 `[0, 0, 0, ...]` 全 0 占位
2. `curl -sf https://dimkey.app/api/v1/health` 200 OK（health endpoint 需 dimkey-web 实现，spec 中要求加上）
3. `Cargo.toml` 与 `tauri.conf.json` 的 version 字段一致
4. 当前在 git 提交干净状态（无未提交改动），避免脏构建

**调用点**：
- `scripts/release-macos.sh`：**在参数解析与既有前置检查之后、临时改写 tauri.conf.json/Cargo.toml 之前**调用。理由：本脚本本身会 sed 改写两个配置文件再用 trap 还原；如果 preflight 在改写之后跑，第 4 项「工作区干净」必然失败。
- `release.yml` Windows job：在 checkout 之后、「临时改写 tauri.conf.json 和 Cargo.toml」step 之前调用。CI 工作区天然干净，主要校验 PUBKEY + API + 版本一致。

**为何 health endpoint 而不是 ping**：避免被 Cloudflare 缓存层 200 假阳性。后端实现 `/api/v1/health` 返回 `{ok: true, ed25519_pubkey_fingerprint: "<sha256(pubkey)[:16]>"}`，preflight 可顺便比对客户端 PUBKEY_V1 的 sha256 前 16 字节是否匹配。这一步把"客户端公钥与后端私钥不配对"的事故彻底卡死。

#### 3.2.2 CHANGELOG 英文自动翻译

**当前问题**：
- `release-macos.sh` 只在 `lang=zh` 时从 `CHANGELOG.md` 抽取 release notes 设到 Release 描述
- `release.yml` 生成 `latest-{zh,en}.json` 时 notes 写死中文 "请查看更新日志了解详细变更。"
- 英文用户看到中文 notes 体验差

**方案**：发版时 LLM CLI 自动翻译

**新文件**：`scripts/translate-changelog.sh`

输入：tag（如 `v0.8.0`）
逻辑：
1. 从 `CHANGELOG.md` awk 抽取该版本的 section（沿用 `release-macos.sh` 既有逻辑）
2. 调用 LLM CLI 翻译为英文。优先级：`claude` → `codex` → fallback。**fallback 不报错退出**，写入固定英文文本 `"See https://dimkey.com/releases/{TAG} for changelog."` 并输出告警 `[WARN] LLM CLI not found, using fallback English notes`
3. 把英文写到 `/tmp/dimkey-release-notes-${TAG}.en.md`
4. 同时把中文 section 写到 `/tmp/dimkey-release-notes-${TAG}.zh.md`，方便 release.yml 复用

**release-macos.sh 集成点**：
- 在 `lang=zh` 抽完中文 notes 后，调用 `translate-changelog.sh` 生成英文版本
- `lang=en` 完成时，把英文 notes 设为 Release 的英文段落（追加到已有中文 notes，不替换）

**release.yml workflow_dispatch 集成点**：
- 生成 `latest-zh.json` 时 notes 用中文 section
- 生成 `latest-en.json` 时 notes 用英文 section
- 中英文 section 通过 Release body 抓取或 artifact 上传都可——选 artifact 路径（mac 脚本上传 `release-notes-*.md` 作为 release asset，CI 下载使用）

**降级方案**：若 LLM CLI 不可用，preflight 不卡，但 release-macos.sh 提示用户「未生成英文 notes，latest-en.json 将使用 fallback 文本」并继续。fallback 文本：`"See https://dimkey.com/releases/{TAG} for changelog."`

### 3.3 跨仓库发版顺序

**硬约束**：dimkey-web 后端必须在 dimkey 客户端发版**之前**完成部署。

#### 3.3.1 dimkey-web 仓库需完成的事

1. 运行 `scripts/gen-ed25519-keypair.ts` 输出 keypair JSON（pubkey 32 字节、私钥 b64）
2. 私钥执行 `wrangler secret put ED25519_PRIVATE_KEY`
3. Workers 部署生产 `dimkey.app/api/v1`，5 个业务 endpoint + `/health` endpoint 通畅
4. 把公钥（hex 或 byte array 形式）以 PR 形式提到 dimkey 仓库

#### 3.3.2 dimkey 仓库的发版步骤（详尽）

```bash
# 0. 前置：在 main 分支
git checkout main && git pull

# 1. 合并 feat/license-control（需先 review + 测试通过）
git merge --no-ff feat/license-control

# 2. 替换 PUBKEY_V1 真公钥（从 dimkey-web 拿来的）
$EDITOR src-tauri/src/license/certificate.rs

# 3. 版本号 0.8.0
$EDITOR src-tauri/Cargo.toml src-tauri/tauri.conf.json
# CHANGELOG.md 加入 ## [v0.8.0] section

# 4. 提交 + tag
git add -A && git commit -m "chore(release): v0.8.0 — license 控制 + 跨语言共享存储"
git tag v0.8.0
git push origin main v0.8.0  # 触发 Windows CI

# 5. mac 本地构建（preflight 在脚本内自动跑）
./scripts/release-macos.sh v0.8.0 zh
./scripts/release-macos.sh v0.8.0 en
# en 完成后会自动触发 workflow_dispatch 生成 latest-*.json
```

### 3.4 测试策略

| 层次 | CI | 本地 dev | 备注 |
|---|---|---|---|
| Rust 单元测试 | ✓ | ✓ | 全跑，不依赖网络 |
| api_client mock 测试 | ✓ | ✓ | **本次新增**：用 `mockito` 起本地 HTTP server，覆盖 5 个 endpoint × {happy/error code/network timeout} |
| certificate sign+verify roundtrip | ✓ | ✓ | 已有，不依赖 PUBKEY_V1 |
| certificate read with real PUBKEY | ✓ | ✓ | **本次替换**：用 dimkey-web issue 出的 fixture .lic 做正向验证（替换 `certificate.rs:261` 的占位失败测试） |
| trial 流程 e2e | ✓ | ✓ | 已有，不依赖网络 |
| UI Playwright e2e | ✓ | ✓ | 已有，license 部分用 mock IPC |
| **实机激活（连真 API）** | ✗ | ✓ 手动 | dev guide 文档化操作步骤；dimkey-web staging Worker 部署后才能测试 |

#### 3.4.1 mockito 测试新增项

**文件**：`src-tauri/src/license/api_client.rs`（tests 模块扩展）

新增覆盖：
- `activate_device` × {ok, INVALID_LICENSE, DEVICE_LIMIT_REACHED, LICENSE_REVOKED, network timeout}
- `heartbeat` × {ok, LICENSE_REVOKED, LICENSE_EXPIRED, 5xx}
- `deactivate_device` × {ok, INVALID_LICENSE}
- `list_devices` × {ok}
- `recover_license` × {ok, INVALID_EMAIL}

Mockito 用法参考：每个 test 起一个 server，set env `DIMKEY_API_BASE=<mock URL>`，post body 验证、response 注入。`api_client.rs:280` 已有 env override 模式。

#### 3.4.2 dev guide 实机激活

**新文件**：`docs/dev-license-activation.md`

内容：
- 启动 dimkey-web staging Worker（dimkey-web 仓库提供命令）
- 设置 `DIMKEY_API_BASE=https://dimkey-staging.workers.dev/api/v1`
- 用 staging keypair（dimkey-web 仓库 issue 出测试公钥）替换 `PUBKEY_V1`（本地临时改、不提交）
- 用 staging license key 走 `cargo tauri dev` 试激活
- 验证 `~/Library/Application Support/com.dimkey/license.lic` 写入成功

---

## 4. 不在本设计范围内（挂 TODO）

| 项目 | 后续在哪儿处理 |
|---|---|
| Windows Authenticode 代码签名 | 独立设计稿，预计在 license 完整可用后下一轮 |
| LemonSqueezy webhook 与 license 后端的对接 | dimkey-web 仓库自己的设计稿（参见 `1d0e387` 提的 LS 渠道） |
| 国内手动激活渠道 | 同上，dimkey-web 仓库 |
| dimkey.com 落地页针对 license 的文案/购买入口 | dimkey-site 仓库独立处理 |
| 公钥轮换（PUBKEY_V2）流程 | `certificate.rs` 已有 `KNOWN_PUBKEYS` 多版本支持的架子，但轮换 SOP 单独立项 |

---

## 5. 验收标准

实现完成的判定：

1. `cargo test --workspace` 全绿（含新增的 mockito 测试）
2. `e2e/.venv/bin/pytest e2e/tests/` 全绿
3. `./scripts/preflight-license.sh` 在 PUBKEY 占位 / API 不通 / 版本不一致 / 工作区脏 任一情况下 exit 1
4. `./scripts/release-macos.sh v0.8.0 zh && ./scripts/release-macos.sh v0.8.0 en` 构建 + 公证 + 上传成功，两个 lang 的 dmg/exe 都出现在 `cube1/dimkey-site` Release
5. `https://dimkey.com/latest-zh.json` 和 `https://dimkey.com/latest-en.json` 均有英文/中文 notes
6. 手动安装 zh 包激活后，安装 en 包能**直接看到 license 已激活**（验证跨 lang 共享存储）
7. Trial 期内 zh 用 7 天后切 en，剩余 trial 天数应是 **23 天**（验证 trial 跨 lang 共享）
8. PUBKEY 占位时 `./scripts/release-macos.sh v0.8.0 zh` **拒绝构建**

---

## 6. 风险与缓解

| 风险 | 缓解 |
|---|---|
| dimkey-web 部署延后导致客户端发版被卡 | preflight 卡 API 健康检查，本仓库不能单独 ship 0.8.0 |
| LLM CLI 在 mac 上未安装/未授权导致翻译失败 | 提供 fallback 文本，preflight 不卡，给出明确告警 |
| 用户跨 lang 包安装时，旧 trial 数据被识别为篡改 | 三处存储取最早 `first_run_at`，且 `~/.dimkey_state` + keyring 已跨 lang，不需迁移即可继续计时 |
| 公钥轮换时旧 .lic 失效 | `KNOWN_PUBKEYS` 多版本机制已就绪；客户端通过 `next_check_at` 触发 heartbeat 自动续签 |
| 公开仓库（dimkey-site）泄露公钥 | 公钥本就是公开信息，无风险 |
| 私钥（Workers Secret）泄露 | dimkey-web 仓库独立处理，本仓库不接触 |
