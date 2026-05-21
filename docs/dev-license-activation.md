# 本地实机 license 激活流程

> dev/QA 用。生产用户无需关心。

## 适用场景

- 验证 license 完整流程：激活 → 写 .lic → heartbeat → 吊销 → 重新激活
- 验证跨 zh/en 包共享 license（关键 e2e 场景）
- 客户端 license 代码改动后做实机回归

## 前置条件

1. **dimkey-site** 后端（Go server）已部署 staging 实例，URL 形如 `https://dimkey-staging.example.com/api/v1` 或本地 `http://localhost:8080/api/v1`
2. staging 用一对 **测试 keypair**（与生产 keypair 区分开），通过 dimkey-site 仓库 `go run ./cmd/dimkey-keygen` 生成；公钥已临时填进本地 `src-tauri/src/license/certificate.rs:15`（**不要 commit 这个临时改动**）；私钥放进 staging server 的 `ED25519_PRIVATE_KEY` 配置
3. staging 数据库中已预置一个 test license：`DK-TEST1-TEST2-TEST3-TEST4-TEST5`、email `dev@dimkey.local`

## 操作步骤

### 1. 启动 dev 服务

```bash
# 指向 staging API（按你的 dimkey-site 部署地址替换）
export DIMKEY_API_BASE=https://dimkey-staging.example.com/api/v1
# 或本地：export DIMKEY_API_BASE=http://localhost:8080/api/v1

# 默认中文版
TAURI_DEV_HOST=127.0.0.1 cargo tauri dev
```

### 2. 激活

应用启动后：
- 打开 About → License → "激活"
- 输入 test license_key + email
- 确认激活成功，UI 显示 Plan 类型

### 3. 验证 .lic 已写入共享路径

```bash
ls -la ~/Library/Application\ Support/com.dimkey/license.lic
```

注意：是 `com.dimkey/`，**不是** `com.dimkey.cn/` 或 `com.dimkey.en/`。

### 4. 验证跨 lang 共享

```bash
# 关闭当前 dev，切到英文 feature
cargo tauri dev --no-default-features --features lang-en
# 同时前端：VITE_DIMKEY_LANG=en npm run dev
```

英文版启动后应**直接读到** license 已激活，无需重新输入 key。

### 5. 验证 heartbeat

heartbeat 任务每 24h 跑一次。本地手动触发：在 `src-tauri/src/license/heartbeat.rs` 临时把间隔改为 60s，重启 app，观察日志输出 "heartbeat: ok" 或对应错误码。

测完**还原间隔**到 24h，不要 commit。

### 6. 验证吊销

在 dimkey-site staging 数据库把该 device 的 `revoked_at` 设为 now（具体 SQL/admin 入口见 dimkey-site 仓库文档）。等下次 heartbeat 触发，客户端应进入 Revoked 状态、UI 显示横幅。

## 常见坑

- **PUBKEY 没换 staging**：客户端用生产 PUBKEY，staging keypair 签的 .lic 必然 SignatureInvalid。检查 `certificate.rs:15` 是否是 staging 公钥。
- **`DIMKEY_API_BASE` 未生效**：env var 必须在 `cargo tauri dev` 之前 export，且 reqwest client 是 OnceLock 缓存的，运行时改 env 无效——必须重启 app。
- **共享路径未创建**：第一次启动应在 `~/Library/Application Support/com.dimkey/` 自动建目录。如果失败检查 lib.rs 中 `create_dir_all` 是否 silently failing。

## 切换回生产 PUBKEY

```bash
git checkout src-tauri/src/license/certificate.rs
```

确保临时填入的 staging 公钥不会被误 commit。
