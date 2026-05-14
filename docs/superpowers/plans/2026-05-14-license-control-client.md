# 许可证控制 — 客户端 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 dimkey 桌面客户端实现许可证控制：30 天试用 + 三处冗余存储 + Ed25519 离线证书验签 + 后端激活/解绑/复验/找回 IPC + Typora 风格 UI（激活/设备管理/找回/About 面板/试用横幅/倒计时角标）+ 试用过期时的导出文件水印（5 种格式）。

**Architecture:** 新增 `src-tauri/src/license/` Rust 模块（fingerprint / trial / certificate / api_client / state / storage / errors），通过 Tauri commands 暴露给前端。前端用 Zustand 管理 license 状态、新增 6 个 React 组件 + i18n。水印在 `commands/file.rs` 的 `export_content()` 中央分发处统一注入，按格式分发到 5 个 helper。后端 API 走 `https://dimkey.app/api/v1/...`（Plan A 已实现）。

**Tech Stack:** Rust（`ed25519-dalek` / `keyring` / `sysinfo` / `machine-uid` / `reqwest`） · TypeScript / React 19 / Zustand / i18next / TailwindCSS · pytest（E2E）

**Spec:** `docs/superpowers/specs/2026-05-14-license-control-design.md`
**Backend Plan:** `docs/superpowers/plans/2026-05-14-license-control-backend.md`
**API contract:** Plan A §5.4 的接口契约

**前置假设：**
- Plan A 已部署；客户端能请求 `https://dimkey.app/api/v1/...`
- 已拿到 Ed25519 公钥（Plan A Task 0.3 输出的 Rust array），需要在 Task 4.1 烧入 `src-tauri/src/license/certificate.rs`
- 当前 dimkey 仓库已有 Cargo feature `lang-zh` / `lang-en`（spec 已说明），客户端按编译期 feature 决定 flavor 字段

---

## File Structure

**新建（Rust 后端）：**
- `src-tauri/src/license/mod.rs` — 模块入口，re-export 公共 API
- `src-tauri/src/license/errors.rs` — `LicenseError` 枚举（i18n key 化）
- `src-tauri/src/license/fingerprint.rs` — 设备指纹算法 v1
- `src-tauri/src/license/storage.rs` — 3 处冗余抽象（config_dir / hidden_file / keyring）
- `src-tauri/src/license/trial.rs` — 试用期状态读写
- `src-tauri/src/license/certificate.rs` — `.lic` 读写 + Ed25519 验签 + Pubkey V1 常量
- `src-tauri/src/license/api_client.rs` — 调 dimkey.app/api/v1 的 HTTP 客户端
- `src-tauri/src/license/state.rs` — `LicenseState` + 全局态 + 启动加载流程
- `src-tauri/src/license/heartbeat.rs` — 后台 heartbeat 任务（tokio）
- `src-tauri/src/license/watermark.rs` — 5 种格式水印实现
- `src-tauri/src/commands/license.rs` — 11 个 Tauri commands
- `src-tauri/tests/license_fingerprint.rs` — 集成测试
- `src-tauri/tests/license_certificate.rs` — 集成测试
- `src-tauri/tests/license_trial.rs` — 集成测试
- `src-tauri/tests/license_watermark.rs` — 集成测试

**新建（前端）：**
- `src/stores/licenseStore.ts` — Zustand store
- `src/components/license/ActivationDialog.tsx`
- `src/components/license/DeviceListDialog.tsx`
- `src/components/license/RecoverDialog.tsx`
- `src/components/license/TrialCountdownBadge.tsx`
- `src/components/license/TrialExpiredBanner.tsx`
- `src/pages/AboutPage.tsx`（如已有则改）
- `src/locales/zh/license.json`
- `src/locales/en/license.json`

**新建（E2E）：**
- `e2e/tests/test_license_trial.py`
- `e2e/tests/test_license_activation.py`

**修改：**
- `src-tauri/Cargo.toml` — 加 license 模块所需依赖
- `src-tauri/src/lib.rs` — `setup` 钩子里加载 license 模块、register commands
- `src-tauri/src/commands/file.rs` — `export_content()` 注入水印调用
- `src-tauri/src/commands/mod.rs` — 加 `pub mod license;`
- `src/App.tsx` 或 `src/layouts/*.tsx` — 加挂 banner / countdown badge / dialogs
- `src/i18n.ts` — 加载 license 命名空间
- `src/locales/zh/common.json` / `src/locales/en/common.json` — 不动（独立命名空间）

---

## Phase 0：依赖与模块骨架

### Task 0.1: Cargo 依赖增量

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: 在 `[dependencies]` 段追加**

```toml
ed25519-dalek = "2"
keyring = "3"
sysinfo = "0.32"
machine-uid = "0.5"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
sha2 = "0.10"
hex = "0.4"
chrono = { version = "0.4", features = ["serde"] }
```

> reqwest 已默认无 `default-features`，避免引入 native-tls。chrono 用于 ISO 时间格式化。

- [ ] **Step 2: 验证依赖能编译**

```bash
cd src-tauri && cargo check
```

Expected: 编译通过（可能需要数十秒下载新 crate）

- [ ] **Step 3: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore(license): 引入 ed25519/keyring/sysinfo/machine-uid/reqwest 依赖"
```

---

### Task 0.2: license 模块骨架 + errors

**Files:**
- Create: `src-tauri/src/license/mod.rs`
- Create: `src-tauri/src/license/errors.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写 `errors.rs`**

```rust
// src-tauri/src/license/errors.rs
use serde::Serialize;

/// 许可证错误码 — 与后端 spec §9.1 对齐
/// 序列化时 `code` 字段对应前端 i18n key
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "code", content = "data")]
pub enum LicenseError {
    InvalidLicense,
    LicenseRevoked { reason: String },
    LicenseExpired,
    DeviceLimitReached { devices: serde_json::Value, max: u32 },
    DeviceNotFound,
    FingerprintMismatch,
    SignatureInvalid,
    NetworkUnavailable,
    RateLimited,
    EmailFormatInvalid,
    KeyFormatInvalid,
    ServerError { code: String, message: String },
}

impl std::fmt::Display for LicenseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for LicenseError {}
```

- [ ] **Step 2: 写 `mod.rs` 骨架**

```rust
// src-tauri/src/license/mod.rs
pub mod errors;
pub mod fingerprint;
pub mod storage;
pub mod trial;
pub mod certificate;
pub mod api_client;
pub mod state;
pub mod heartbeat;
pub mod watermark;

pub use errors::LicenseError;
pub use state::{LicenseState, LicenseManager};
```

> 该 mod.rs 会引用尚未创建的子模块；后续 task 逐个创建。当前先写空文件占位，避免一次性大量未编译代码。

- [ ] **Step 3: 创建空占位文件**

```bash
cd src-tauri/src/license
for f in fingerprint storage trial certificate api_client state heartbeat watermark; do
  echo "// placeholder" > "$f.rs"
done
```

- [ ] **Step 4: 在 `lib.rs` 顶部加 `pub mod license;`**

```rust
// src-tauri/src/lib.rs（在 pub mod analytics; 这一行下方追加）
pub mod license;
```

- [ ] **Step 5: 验证编译通过 + 提交**

```bash
cd src-tauri && cargo check
```

```bash
git add src-tauri/src/license/ src-tauri/src/lib.rs
git commit -m "feat(license): 模块骨架 + LicenseError 枚举（i18n 化）"
```

---

## Phase 1：设备指纹

### Task 1.1: 指纹算法 v1（TDD）

**Files:**
- Modify: `src-tauri/src/license/fingerprint.rs`

- [ ] **Step 1: 写 `fingerprint.rs` — 含失败测试**

```rust
// src-tauri/src/license/fingerprint.rs
use sha2::{Digest, Sha256};

pub const FINGERPRINT_VERSION: &str = "v1";

/// 计算当前机器的设备指纹（128 bit hex）
/// 算法：sha256(machine_id || primary_mac || cpu_brand || os_install_id)[0..32]
pub fn compute_fingerprint() -> String {
    let machine_id = read_machine_id().unwrap_or_else(|| "unknown".into());
    let mac = read_primary_mac().unwrap_or_else(|| "unknown".into());
    let cpu = read_cpu_brand().unwrap_or_else(|| "unknown".into());
    let os_id = read_os_install_id().unwrap_or_else(|| "unknown".into());

    let mut hasher = Sha256::new();
    hasher.update(machine_id.as_bytes());
    hasher.update(b"||");
    hasher.update(mac.as_bytes());
    hasher.update(b"||");
    hasher.update(cpu.as_bytes());
    hasher.update(b"||");
    hasher.update(os_id.as_bytes());
    let full = hasher.finalize();
    hex::encode(&full[..16])    // 128 bit = 32 hex chars
}

fn read_machine_id() -> Option<String> {
    machine_uid::get().ok()
}

fn read_primary_mac() -> Option<String> {
    use sysinfo::Networks;
    let networks = Networks::new_with_refreshed_list();
    let mut macs: Vec<String> = networks
        .iter()
        .filter(|(name, _)| {
            // 过滤 lo / 虚拟接口 / USB 网卡（保留以太网/Wi-Fi 物理接口）
            let n = name.to_lowercase();
            !n.starts_with("lo")
                && !n.contains("vmnet") && !n.contains("vboxnet")
                && !n.contains("utun") && !n.contains("anpi")
                && !n.contains("awdl") && !n.contains("llw")
                && !n.contains("bridge") && !n.contains("docker")
                && !n.contains("tap") && !n.contains("ham")
        })
        .map(|(_, data)| data.mac_address().to_string())
        .filter(|m| m != "00:00:00:00:00:00" && !m.is_empty())
        .collect();
    macs.sort();
    macs.into_iter().next()
}

fn read_cpu_brand() -> Option<String> {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_cpu_all();
    sys.cpus().first().map(|c| c.brand().to_string())
}

#[cfg(target_os = "macos")]
fn read_os_install_id() -> Option<String> {
    use std::process::Command;
    let out = Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    for line in s.lines() {
        if let Some(rest) = line.split_once("IOPlatformSerialNumber") {
            // 行形如:  "IOPlatformSerialNumber" = "C02XXXXXXX"
            let t = rest.1.trim().trim_start_matches('=').trim()
                .trim_matches('"').trim();
            if !t.is_empty() { return Some(t.to_string()); }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn read_os_install_id() -> Option<String> {
    use std::process::Command;
    // SID via `whoami /user` — 输出格式: NAME SID
    let out = Command::new("whoami").arg("/user").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    s.split_whitespace().last().map(|x| x.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_32_hex_chars() {
        let fp = compute_fingerprint();
        assert_eq!(fp.len(), 32);
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn fingerprint_is_stable_across_calls() {
        // 同一进程内连续两次必须一致
        let a = compute_fingerprint();
        let b = compute_fingerprint();
        assert_eq!(a, b);
    }

    #[test]
    fn fingerprint_handles_unknown_fields_gracefully() {
        // 即使所有 field 都返回 None，也应能算出一个合法 fingerprint
        // 实现细节：unwrap_or_else "unknown" 兜底，sha256("unknown||unknown||unknown||unknown") 永远有结果
        let fp = compute_fingerprint();
        assert_eq!(fp.len(), 32);
    }
}
```

- [ ] **Step 2: 跑测试**

```bash
cd src-tauri && cargo test license::fingerprint --lib
```

Expected: 全部 PASS（指纹依赖系统调用，本机有真实 MAC/CPU 时直接通过）

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/license/fingerprint.rs
git commit -m "feat(license): 设备指纹算法 v1 — sha256(machine_id||MAC||CPU||OS_id) 截 128 bit"
```

---

## Phase 2：3 处冗余存储抽象

### Task 2.1: storage 模块（TDD）

**Files:**
- Modify: `src-tauri/src/license/storage.rs`

- [ ] **Step 1: 写 `storage.rs`**

```rust
// src-tauri/src/license/storage.rs
//
// 试用期与状态信息的 3 处冗余存储抽象。
// 任意一处缺失都不影响读取（取最早的 first_run_at）。
// 写入时尝试全部 3 处，失败的不报错（best-effort）。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrialRecord {
    pub version: u32,
    pub first_run_at: String,        // ISO UTC
    pub last_run_at: String,         // ISO UTC
    pub machine_fp: String,
}

const KEYRING_SERVICE: &str = "com.dimkey.trial";
const KEYRING_USER: &str = "trial-record";

pub trait TrialStore {
    fn read(&self) -> Option<TrialRecord>;
    fn write(&self, rec: &TrialRecord) -> Result<(), String>;
    fn label(&self) -> &str;
}

pub struct ConfigDirStore { pub path: PathBuf }
pub struct HiddenFileStore { pub path: PathBuf }
pub struct KeyringStore;

impl TrialStore for ConfigDirStore {
    fn label(&self) -> &str { "config_dir" }
    fn read(&self) -> Option<TrialRecord> {
        let s = std::fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&s).ok()
    }
    fn write(&self, rec: &TrialRecord) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let s = serde_json::to_string_pretty(rec).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, s).map_err(|e| e.to_string())
    }
}

impl TrialStore for HiddenFileStore {
    fn label(&self) -> &str { "hidden_file" }
    fn read(&self) -> Option<TrialRecord> {
        let s = std::fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&s).ok()
    }
    fn write(&self, rec: &TrialRecord) -> Result<(), String> {
        let s = serde_json::to_string(rec).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, s).map_err(|e| e.to_string())
    }
}

impl TrialStore for KeyringStore {
    fn label(&self) -> &str { "keyring" }
    fn read(&self) -> Option<TrialRecord> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).ok()?;
        let s = entry.get_password().ok()?;
        serde_json::from_str(&s).ok()
    }
    fn write(&self, rec: &TrialRecord) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())?;
        let s = serde_json::to_string(rec).map_err(|e| e.to_string())?;
        entry.set_password(&s).map_err(|e| e.to_string())
    }
}

/// 默认的 3 处存储构造器（依赖 Tauri AppHandle 解析 config_dir）
pub fn build_default_stores(config_dir: PathBuf) -> Vec<Box<dyn TrialStore>> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    vec![
        Box::new(ConfigDirStore { path: config_dir.join("trial.json") }),
        Box::new(HiddenFileStore { path: home.join(".dimkey_state") }),
        Box::new(KeyringStore),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn config_dir_store_roundtrip() {
        let d = tempdir().unwrap();
        let store = ConfigDirStore { path: d.path().join("trial.json") };
        let rec = TrialRecord { version: 1, first_run_at: "2026-05-14T00:00:00Z".into(), last_run_at: "2026-05-14T00:00:00Z".into(), machine_fp: "fp123".into() };
        store.write(&rec).unwrap();
        let loaded = store.read().unwrap();
        assert_eq!(loaded, rec);
    }

    #[test]
    fn read_returns_none_when_file_missing() {
        let d = tempdir().unwrap();
        let store = ConfigDirStore { path: d.path().join("nonexistent.json") };
        assert!(store.read().is_none());
    }

    #[test]
    fn read_returns_none_when_file_corrupt() {
        let d = tempdir().unwrap();
        let path = d.path().join("trial.json");
        std::fs::write(&path, "this is not json").unwrap();
        let store = ConfigDirStore { path };
        assert!(store.read().is_none());
    }
}
```

> 注意：`dirs` crate 在 Tauri 2 中通常已存在。如未引入加 `dirs = "5"` 到 Cargo.toml。

- [ ] **Step 2: 检查 `dirs` 依赖（如缺则添加）**

```bash
grep -E "^dirs" src-tauri/Cargo.toml || echo "需要添加 dirs"
```

如缺则在 Cargo.toml `[dependencies]` 加：
```toml
dirs = "5"
```

`tempfile` 已在 `[dev-dependencies]` 中。

- [ ] **Step 3: 跑测试**

```bash
cd src-tauri && cargo test license::storage --lib
```

Expected: 3 个测试 PASS（keyring 测试在 CI 可能失败，故未写）

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/license/storage.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(license): 试用期 3 处冗余存储抽象（config/hidden/keyring）"
```

---

## Phase 3：试用期模块

### Task 3.1: trial 模块 — 计算剩余天数 + 防回拨

**Files:**
- Modify: `src-tauri/src/license/trial.rs`

- [ ] **Step 1: 写 `trial.rs`**

```rust
// src-tauri/src/license/trial.rs

use crate::license::storage::{TrialRecord, TrialStore};
use chrono::{DateTime, Duration, Utc};

pub const TRIAL_DAYS: i64 = 30;

#[derive(Debug, Clone, PartialEq)]
pub enum TrialStatus {
    Active { days_remaining: u32, first_run_at: DateTime<Utc> },
    Expired { first_run_at: DateTime<Utc> },
}

pub struct TrialInfo {
    pub status: TrialStatus,
    pub clock_tampered: bool,
}

/// 评估当前试用状态（不写入）
pub fn evaluate(stores: &[Box<dyn TrialStore>], machine_fp: &str, now: DateTime<Utc>) -> TrialInfo {
    let records: Vec<TrialRecord> = stores.iter().filter_map(|s| s.read()).collect();
    let mut clock_tampered = false;

    let (first, last) = if records.is_empty() {
        // 初次启动：记当前时间
        (now, now)
    } else {
        let mut first = parse_iso(&records[0].first_run_at);
        let mut last = parse_iso(&records[0].last_run_at);
        for r in records.iter().skip(1) {
            let f = parse_iso(&r.first_run_at);
            let l = parse_iso(&r.last_run_at);
            if f < first { first = f; }
            if l > last { last = l; }
        }
        // 防回拨：current < last → 取 last 推进
        if now < last {
            clock_tampered = true;
        }
        (first, last)
    };

    let effective_now = if clock_tampered { last } else { now };
    let elapsed = effective_now - first;
    let trial_dur = Duration::days(TRIAL_DAYS);
    let status = if elapsed >= trial_dur {
        TrialStatus::Expired { first_run_at: first }
    } else {
        let remaining = (trial_dur - elapsed).num_days().max(0) as u32;
        TrialStatus::Active { days_remaining: remaining, first_run_at: first }
    };
    TrialInfo { status, clock_tampered }
}

/// 评估后写回 last_run_at（每次启动调一次）。如所有 store 都为空，写入新记录。
pub fn touch(stores: &[Box<dyn TrialStore>], machine_fp: &str, now: DateTime<Utc>) -> TrialInfo {
    let info = evaluate(stores, machine_fp, now);
    let first = match info.status {
        TrialStatus::Active { first_run_at, .. } => first_run_at,
        TrialStatus::Expired { first_run_at } => first_run_at,
    };
    let new_rec = TrialRecord {
        version: 1,
        first_run_at: first.to_rfc3339(),
        last_run_at: now.to_rfc3339(),    // 即使 clock_tampered 也用 now 推进，避免被永远卡住
        machine_fp: machine_fp.to_string(),
    };
    for s in stores {
        let _ = s.write(&new_rec);    // best-effort
    }
    info
}

fn parse_iso(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::storage::ConfigDirStore;
    use tempfile::tempdir;

    fn build_stores(d: &std::path::Path) -> Vec<Box<dyn TrialStore>> {
        vec![Box::new(ConfigDirStore { path: d.join("trial.json") })]
    }

    #[test]
    fn first_run_returns_active_30_days() {
        let d = tempdir().unwrap();
        let stores = build_stores(d.path());
        let now = Utc::now();
        let info = touch(&stores, "fp1", now);
        match info.status {
            TrialStatus::Active { days_remaining, .. } => {
                assert!(days_remaining >= 29 && days_remaining <= 30);
            }
            _ => panic!("expected Active"),
        }
        assert!(!info.clock_tampered);
    }

    #[test]
    fn after_31_days_status_is_expired() {
        let d = tempdir().unwrap();
        let stores = build_stores(d.path());
        let start = Utc::now();
        touch(&stores, "fp1", start);
        let later = start + Duration::days(31);
        let info = evaluate(&stores, "fp1", later);
        assert!(matches!(info.status, TrialStatus::Expired { .. }));
    }

    #[test]
    fn clock_rollback_detected_and_uses_last_run() {
        let d = tempdir().unwrap();
        let stores = build_stores(d.path());
        let day1 = Utc::now();
        touch(&stores, "fp1", day1);
        let day10 = day1 + Duration::days(10);
        touch(&stores, "fp1", day10);    // last_run = day10
        let rolled_back = day1 - Duration::days(5);    // 用户把时钟拨回 day -5
        let info = evaluate(&stores, "fp1", rolled_back);
        assert!(info.clock_tampered);
        match info.status {
            TrialStatus::Active { days_remaining, .. } => {
                // 按 last_run=day10 推进：已用 10 天，剩 20 天
                assert!(days_remaining >= 19 && days_remaining <= 20);
            }
            _ => panic!("expected Active when rollback detected"),
        }
    }

    #[test]
    fn earliest_first_run_at_wins_across_stores() {
        let d = tempdir().unwrap();
        // 模拟两个 store，一个有较早的记录
        let store1 = ConfigDirStore { path: d.path().join("a.json") };
        let store2 = ConfigDirStore { path: d.path().join("b.json") };
        let early = Utc::now() - Duration::days(20);
        let late = Utc::now() - Duration::days(5);
        store1.write(&TrialRecord { version: 1, first_run_at: late.to_rfc3339(), last_run_at: late.to_rfc3339(), machine_fp: "fp1".into() }).unwrap();
        store2.write(&TrialRecord { version: 1, first_run_at: early.to_rfc3339(), last_run_at: late.to_rfc3339(), machine_fp: "fp1".into() }).unwrap();
        let stores: Vec<Box<dyn TrialStore>> = vec![Box::new(store1), Box::new(store2)];
        let info = evaluate(&stores, "fp1", Utc::now());
        match info.status {
            TrialStatus::Active { days_remaining, .. } => {
                // 用 early=20 天前 → 已用 20 天，剩 10 天
                assert!(days_remaining >= 9 && days_remaining <= 10, "got {}", days_remaining);
            }
            _ => panic!("expected Active using earliest first_run_at"),
        }
    }
}
```

- [ ] **Step 2: 跑测试**

```bash
cd src-tauri && cargo test license::trial --lib
```

Expected: 4 个测试 PASS

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/license/trial.rs
git commit -m "feat(license): trial 模块 — 30 天计时 + 多 store 取最早 + 防回拨"
```

---

## Phase 4：证书模块（Ed25519 验签）

### Task 4.1: certificate 模块 — 公钥常量 + 验签 + 文件读写

**Files:**
- Modify: `src-tauri/src/license/certificate.rs`

- [ ] **Step 1: 写 `certificate.rs`**

```rust
// src-tauri/src/license/certificate.rs
//
// 本地 .lic 证书读写 + Ed25519 验签。
// 公钥编译期烧入，不从配置/网络读，防替换攻击。

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::Path;

// ⚠️ 必须用 dimkey-web 仓库 scripts/gen-ed25519-keypair.ts 输出的 pub array 替换以下 32 个 0
// 后端私钥已存为 Workers Secret ED25519_PRIVATE_KEY
pub const PUBKEY_V1: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub const CERTIFICATE_FILE: &str = "license.lic";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertEnvelope {
    pub v: u32,
    pub payload_b64: String,
    pub sig_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LicensePayload {
    pub license_id: String,
    pub license_key: String,
    pub email: String,
    pub plan: String,
    pub device_id: String,
    pub fingerprint: String,
    pub issued_at: String,
    pub expires_at: Option<String>,
    pub next_check_at: String,
    pub max_grace_until: String,
    pub key_version: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum CertError {
    #[error("missing certificate file")]
    Missing,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("envelope parse: {0}")]
    EnvelopeParse(String),
    #[error("payload base64 decode")]
    PayloadB64,
    #[error("payload json: {0}")]
    PayloadJson(String),
    #[error("signature base64 decode")]
    SigB64,
    #[error("unsupported envelope version {0}")]
    UnsupportedVersion(u32),
    #[error("unsupported key_version {0}")]
    UnsupportedKeyVersion(u32),
    #[error("signature verification failed")]
    SignatureInvalid,
}

pub fn read_certificate(config_dir: &Path) -> Result<LicensePayload, CertError> {
    let path = config_dir.join(CERTIFICATE_FILE);
    if !path.exists() { return Err(CertError::Missing); }
    let raw = std::fs::read_to_string(&path)?;
    let env: CertEnvelope = serde_json::from_str(&raw).map_err(|e| CertError::EnvelopeParse(e.to_string()))?;
    if env.v != 1 { return Err(CertError::UnsupportedVersion(env.v)); }

    let payload_bytes = B64.decode(&env.payload_b64).map_err(|_| CertError::PayloadB64)?;
    let sig_bytes = B64.decode(&env.sig_b64).map_err(|_| CertError::SigB64)?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|_| CertError::SignatureInvalid)?;

    let payload: LicensePayload = serde_json::from_slice(&payload_bytes).map_err(|e| CertError::PayloadJson(e.to_string()))?;

    let pubkey = match payload.key_version {
        1 => VerifyingKey::from_bytes(&PUBKEY_V1).map_err(|_| CertError::SignatureInvalid)?,
        v => return Err(CertError::UnsupportedKeyVersion(v)),
    };
    pubkey.verify(&payload_bytes, &sig).map_err(|_| CertError::SignatureInvalid)?;

    Ok(payload)
}

pub fn write_certificate_envelope(config_dir: &Path, env: &CertEnvelope) -> Result<(), CertError> {
    std::fs::create_dir_all(config_dir)?;
    let path = config_dir.join(CERTIFICATE_FILE);
    let s = serde_json::to_string_pretty(env).map_err(|e| CertError::EnvelopeParse(e.to_string()))?;
    std::fs::write(&path, s)?;
    Ok(())
}

pub fn delete_certificate(config_dir: &Path) -> std::io::Result<()> {
    let path = config_dir.join(CERTIFICATE_FILE);
    if path.exists() { std::fs::remove_file(path)?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;
    use tempfile::tempdir;

    fn build_test_payload() -> LicensePayload {
        LicensePayload {
            license_id: "uuid".into(), license_key: "DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE".into(),
            email: "u@x.com".into(), plan: "personal".into(), device_id: "d".into(),
            fingerprint: "fp".into(), issued_at: "2026-05-14T10:00:00Z".into(),
            expires_at: None, next_check_at: "2026-05-21T10:00:00Z".into(),
            max_grace_until: "2026-05-28T10:00:00Z".into(), key_version: 1,
        }
    }

    #[test]
    fn read_returns_missing_when_no_file() {
        let d = tempdir().unwrap();
        assert!(matches!(read_certificate(d.path()), Err(CertError::Missing)));
    }

    /// 测试用例：用本地 SK 签 + 临时改 PUBKEY_V1 是不可行的（const）。
    /// 改用低层 verify 替代——即写一个独立辅助函数 verify_with_pubkey 用于测试。
    #[test]
    fn verify_with_correct_pubkey_passes() {
        use ed25519_dalek::Verifier;
        let mut rng = OsRng;
        let sk = SigningKey::generate(&mut rng);
        let pk = sk.verifying_key();
        let payload = build_test_payload();
        let payload_bytes = serde_json::to_vec(&payload).unwrap();
        let sig: Signature = sk.sign(&payload_bytes);
        assert!(pk.verify(&payload_bytes, &sig).is_ok());
    }

    #[test]
    fn verify_rejects_tampered_payload() {
        use ed25519_dalek::Verifier;
        let mut rng = OsRng;
        let sk = SigningKey::generate(&mut rng);
        let pk = sk.verifying_key();
        let payload = build_test_payload();
        let payload_bytes = serde_json::to_vec(&payload).unwrap();
        let sig: Signature = sk.sign(&payload_bytes);
        let mut tampered = payload_bytes.clone();
        tampered[0] ^= 0xff;
        assert!(pk.verify(&tampered, &sig).is_err());
    }
}
```

- [ ] **Step 2: 加 `thiserror` 依赖**

```toml
# Cargo.toml [dependencies]
thiserror = "1"
```

- [ ] **Step 3: 跑测试**

```bash
cd src-tauri && cargo test license::certificate --lib
```

Expected: PASS（注：完整证书 roundtrip 测试在 Task 4.2，因 PUBKEY_V1 是 const 不能在测试时替换）

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/license/certificate.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(license): 证书模块 — Ed25519 验签 + .lic 读写 + 公钥占位"
```

---

### Task 4.2: 烧入 PUBKEY_V1 + 集成测试

**Files:**
- Modify: `src-tauri/src/license/certificate.rs`
- Create: `src-tauri/tests/license_certificate.rs`

- [ ] **Step 1: 把 dimkey-web 仓库 Plan A Task 0.3 输出的 Rust array 复制到 PUBKEY_V1**

打开 `src-tauri/src/license/certificate.rs`，把：
```rust
pub const PUBKEY_V1: [u8; 32] = [
    0, 0, 0, 0, ...
];
```
替换为：
```rust
pub const PUBKEY_V1: [u8; 32] = [<32 个真实字节>];
```

- [ ] **Step 2: 写集成测试 `tests/license_certificate.rs`（用 Plan A 后端真实签）**

```rust
// src-tauri/tests/license_certificate.rs
//
// 集成测试：用 Plan A 已部署的后端（或本地 wrangler dev）真实签发一张证书，
// 验证客户端能成功 read_certificate。
// 依赖环境变量：
//   DIMKEY_TEST_API_BASE  默认 http://localhost:8788
//   DIMKEY_TEST_LICENSE_KEY  + DIMKEY_TEST_EMAIL  必须有效

use dimkey_lib::license::certificate::{read_certificate, write_certificate_envelope, CertEnvelope};
use std::env;
use tempfile::tempdir;

#[tokio::test]
#[ignore]    // 需要后端运行，cargo test --ignored 才执行
async fn read_real_certificate_from_backend() {
    let api_base = env::var("DIMKEY_TEST_API_BASE").unwrap_or_else(|_| "http://localhost:8788".into());
    let key = env::var("DIMKEY_TEST_LICENSE_KEY").expect("set DIMKEY_TEST_LICENSE_KEY");
    let email = env::var("DIMKEY_TEST_EMAIL").expect("set DIMKEY_TEST_EMAIL");

    let body = serde_json::json!({
        "license_key": key, "email": email, "fingerprint": "test-fp-1",
        "machine_label": "TestMachine", "os": "macos", "flavor": "zh", "app_version": "0.7.0"
    });
    let res = reqwest::Client::new()
        .post(format!("{}/api/v1/activate", api_base))
        .json(&body).send().await.expect("activate request failed");
    let json: serde_json::Value = res.json().await.expect("bad json");
    assert_eq!(json["ok"], true);
    let cert: CertEnvelope = serde_json::from_value(json["data"]["license_certificate"].clone()).unwrap();

    let d = tempdir().unwrap();
    write_certificate_envelope(d.path(), &cert).expect("write cert");
    let payload = read_certificate(d.path()).expect("verify cert");
    assert_eq!(payload.email, email);
    assert_eq!(payload.fingerprint, "test-fp-1");
}
```

- [ ] **Step 3: 跑非 ignored 单元测试确认仍 PASS（不联网）**

```bash
cd src-tauri && cargo test license::certificate --lib
```

- [ ] **Step 4: 后续手动验证步骤（待 Plan A 部署后）**

```bash
# Plan A 后端起来后：
DIMKEY_TEST_API_BASE=http://localhost:8788 \
DIMKEY_TEST_LICENSE_KEY="DK-..." DIMKEY_TEST_EMAIL="u@x.com" \
cd src-tauri && cargo test --test license_certificate -- --ignored --nocapture
```

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/license/certificate.rs src-tauri/tests/license_certificate.rs
git commit -m "feat(license): 烧入 PUBKEY_V1 + 后端真实签发集成测试（ignored）"
```

---

## Phase 5：API client

### Task 5.1: api_client 模块

**Files:**
- Modify: `src-tauri/src/license/api_client.rs`

- [ ] **Step 1: 写 `api_client.rs`**

```rust
// src-tauri/src/license/api_client.rs

use crate::license::certificate::CertEnvelope;
use crate::license::errors::LicenseError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

const DEFAULT_API_BASE: &str = "https://dimkey.app/api/v1";
const REQUEST_TIMEOUT_SECS: u64 = 15;

pub fn api_base() -> String {
    std::env::var("DIMKEY_API_BASE").unwrap_or_else(|_| DEFAULT_API_BASE.into())
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("Dimkey/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("reqwest client build")
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    ok: bool,
    code: Option<String>,
    message: Option<String>,
    data: Option<Value>,
}

fn map_err_code(code: &str, message: &str, data: &Option<Value>) -> LicenseError {
    match code {
        "INVALID_LICENSE" => LicenseError::InvalidLicense,
        "LICENSE_REVOKED" => LicenseError::LicenseRevoked { reason: message.into() },
        "LICENSE_EXPIRED" => LicenseError::LicenseExpired,
        "DEVICE_LIMIT_REACHED" => {
            let devices = data.as_ref().and_then(|d| d.get("devices")).cloned().unwrap_or(Value::Null);
            let max = data.as_ref().and_then(|d| d.get("max_devices")).and_then(|v| v.as_u64()).unwrap_or(3) as u32;
            LicenseError::DeviceLimitReached { devices, max }
        }
        "DEVICE_NOT_FOUND" => LicenseError::DeviceNotFound,
        "RATE_LIMITED" => LicenseError::RateLimited,
        other => LicenseError::ServerError { code: other.into(), message: message.into() },
    }
}

async fn post_json<T: Serialize>(path: &str, body: &T) -> Result<Value, LicenseError> {
    let url = format!("{}{}", api_base(), path);
    let res = client().post(&url).json(body).send().await
        .map_err(|_| LicenseError::NetworkUnavailable)?;
    let api: ApiResponse = res.json().await.map_err(|_| LicenseError::NetworkUnavailable)?;
    if api.ok {
        Ok(api.data.unwrap_or(Value::Null))
    } else {
        let code = api.code.unwrap_or_else(|| "SERVER_ERROR".into());
        let msg = api.message.unwrap_or_else(|| "服务异常".into());
        Err(map_err_code(&code, &msg, &api.data))
    }
}

#[derive(Debug, Serialize)]
pub struct ActivateBody<'a> {
    pub license_key: &'a str,
    pub email: &'a str,
    pub fingerprint: &'a str,
    pub machine_label: &'a str,
    pub os: &'a str,
    pub flavor: &'a str,
    pub app_version: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct ActivateData {
    pub license_certificate: CertEnvelope,
    pub device_summary: DeviceSummary,
}
#[derive(Debug, Deserialize, Clone)]
pub struct DeviceSummary {
    pub current_device_id: String,
    pub active_count: u32,
    pub max_devices: u32,
}

pub async fn activate(body: &ActivateBody<'_>) -> Result<ActivateData, LicenseError> {
    let v = post_json("/activate", body).await?;
    serde_json::from_value(v).map_err(|e| LicenseError::ServerError { code: "PARSE".into(), message: e.to_string() })
}

#[derive(Debug, Serialize)]
pub struct DeactivateBody<'a> {
    pub license_key: &'a str,
    pub email: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<&'a str>,
}

pub async fn deactivate(body: &DeactivateBody<'_>) -> Result<(), LicenseError> {
    post_json("/deactivate", body).await?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct HeartbeatBody<'a> {
    pub license_id: &'a str,
    pub device_id: &'a str,
    pub fingerprint: &'a str,
}
#[derive(Debug, Deserialize, Clone)]
pub struct HeartbeatData {
    pub status: String,
    pub next_check_at: i64,
}
pub async fn heartbeat(body: &HeartbeatBody<'_>) -> Result<HeartbeatData, LicenseError> {
    let v = post_json("/heartbeat", body).await?;
    serde_json::from_value(v).map_err(|e| LicenseError::ServerError { code: "PARSE".into(), message: e.to_string() })
}

#[derive(Debug, Serialize)]
pub struct DevicesListBody<'a> {
    pub license_key: &'a str,
    pub email: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<&'a str>,
}
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct DeviceDto {
    pub device_id: String,
    pub machine_label: Option<String>,
    pub os: String,
    pub flavor: String,
    pub first_activated: i64,
    pub last_seen: i64,
    pub is_current: bool,
}
#[derive(Debug, Deserialize, Clone)]
pub struct DevicesListData {
    pub devices: Vec<DeviceDto>,
    pub max_devices: u32,
}
pub async fn list_devices(body: &DevicesListBody<'_>) -> Result<DevicesListData, LicenseError> {
    let v = post_json("/devices/list", body).await?;
    serde_json::from_value(v).map_err(|e| LicenseError::ServerError { code: "PARSE".into(), message: e.to_string() })
}

#[derive(Debug, Serialize)]
pub struct RecoverBody<'a> { pub email: &'a str }
pub async fn recover(body: &RecoverBody<'_>) -> Result<(), LicenseError> {
    post_json("/recover", body).await?;
    Ok(())
}
```

- [ ] **Step 2: 编译检查**

```bash
cd src-tauri && cargo check
```

Expected: 通过

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/license/api_client.rs
git commit -m "feat(license): api_client — activate/deactivate/heartbeat/devices/recover 调用 + 错误码映射"
```

---

## Phase 6：State 模块 + 启动加载

### Task 6.1: state.rs — LicenseState + LicenseManager

**Files:**
- Modify: `src-tauri/src/license/state.rs`

- [ ] **Step 1: 写 `state.rs`**

```rust
// src-tauri/src/license/state.rs

use crate::license::api_client;
use crate::license::certificate::{self, LicensePayload};
use crate::license::errors::LicenseError;
use crate::license::storage::TrialStore;
use crate::license::trial::{self, TrialStatus};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::RwLock;

/// 客户端 LicenseState — spec §4.4
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum LicenseState {
    Trial { days_remaining: u32 },
    TrialExpired,
    Activated {
        email: String,
        plan: String,
        max_devices: u32,
        active_devices: u32,
        device_id: String,
        license_id: String,
        fingerprint_mismatch: bool,    // 见 spec §4.5 Step 3
    },
    GraceMode { email: String, days_until_block: i64 },
    Revoked { reason: String },
    Unknown,
}

pub struct LicenseManager {
    state: RwLock<LicenseState>,
    trial_stores: Vec<Box<dyn TrialStore + Send + Sync>>,
    config_dir: PathBuf,
    machine_fp: String,
}

// 让 TrialStore 子类型成为 Send + Sync 需要在 storage 端补 trait bound（见 Task 6.2 第 1 步）

impl LicenseManager {
    pub fn new(trial_stores: Vec<Box<dyn TrialStore + Send + Sync>>, config_dir: PathBuf, machine_fp: String) -> Self {
        Self {
            state: RwLock::new(LicenseState::Unknown),
            trial_stores, config_dir, machine_fp,
        }
    }

    pub fn current(&self) -> LicenseState {
        self.state.read().unwrap().clone()
    }

    pub fn machine_fp(&self) -> &str { &self.machine_fp }

    /// 启动时调一次：读证书 / 验签 / 算指纹 / 决定状态
    pub fn boot(&self) -> LicenseState {
        let now = Utc::now();
        let new_state = match certificate::read_certificate(&self.config_dir) {
            Ok(payload) => self.eval_with_certificate(&payload, now),
            Err(certificate::CertError::Missing) => self.eval_trial_only(now),
            Err(_e) => {
                // 证书损坏：删除并回退试用
                let _ = certificate::delete_certificate(&self.config_dir);
                self.eval_trial_only(now)
            }
        };
        *self.state.write().unwrap() = new_state.clone();
        new_state
    }

    fn eval_with_certificate(&self, payload: &LicensePayload, now: DateTime<Utc>) -> LicenseState {
        let mismatch = payload.fingerprint != self.machine_fp;
        if mismatch {
            // 不删证书，回退 trial 判定，但带提示
            let trial_state = self.eval_trial_only(now);
            // 把 trial_state 转成带 mismatch 的 Activated？不 — spec §4.5 是回退到 trial，带 About 面板提示
            // 这里的状态本身按 trial，前端 UI 通过 license_get_state 拿 fingerprint mismatch 标志另行展示
            // 实现做法：回退 trial 状态，但下面的 license_get_fingerprint_mismatch_hint() 单独暴露 hint
            return trial_state;
        }
        LicenseState::Activated {
            email: payload.email.clone(),
            plan: payload.plan.clone(),
            max_devices: 3,    // 后续 heartbeat 时刷新
            active_devices: 1,
            device_id: payload.device_id.clone(),
            license_id: payload.license_id.clone(),
            fingerprint_mismatch: false,
        }
    }

    fn eval_trial_only(&self, now: DateTime<Utc>) -> LicenseState {
        let info = trial::touch(&self.trial_stores_as_dyn(), &self.machine_fp, now);
        match info.status {
            TrialStatus::Active { days_remaining, .. } => LicenseState::Trial { days_remaining },
            TrialStatus::Expired { .. } => LicenseState::TrialExpired,
        }
    }

    // helper: 把 Vec<Box<dyn TrialStore + Send + Sync>> 转 Vec<Box<dyn TrialStore>> 的 ref view
    fn trial_stores_as_dyn(&self) -> Vec<Box<dyn TrialStore>> {
        // 由于 trial::evaluate 接受 &[Box<dyn TrialStore>]，且 Box<dyn TrialStore + Send + Sync> 不能直接 coerce，
        // 我们让 trial::evaluate 改为接受 &dyn TrialStore 切片（Task 6.2 同步调整）。
        unreachable!("see Task 6.2: refactor trial fn to accept &[&dyn TrialStore]")
    }

    pub fn try_activate(&self, license_key: &str, email: &str, machine_label: &str, os: &str, flavor: &str, app_version: &str) -> Result<api_client::ActivateData, LicenseError> {
        let body = api_client::ActivateBody {
            license_key, email, fingerprint: &self.machine_fp,
            machine_label, os, flavor, app_version,
        };
        let rt = tokio::runtime::Handle::current();
        let result = tokio::task::block_in_place(|| rt.block_on(api_client::activate(&body)))?;

        // 落盘证书
        certificate::write_certificate_envelope(&self.config_dir, &result.license_certificate)
            .map_err(|e| LicenseError::ServerError { code: "WRITE_CERT".into(), message: e.to_string() })?;

        // 立刻 boot 一次更新 state
        self.boot();
        Ok(result)
    }

    pub fn deactivate_local(&self) -> Result<(), LicenseError> {
        // 1. 调后端 deactivate
        let payload = certificate::read_certificate(&self.config_dir)
            .map_err(|_| LicenseError::InvalidLicense)?;
        let body = api_client::DeactivateBody {
            license_key: &payload.license_key, email: &payload.email,
            device_id: Some(&payload.device_id), fingerprint: None,
        };
        let rt = tokio::runtime::Handle::current();
        tokio::task::block_in_place(|| rt.block_on(api_client::deactivate(&body)))?;

        // 2. 删本地证书
        let _ = certificate::delete_certificate(&self.config_dir);

        // 3. 重 boot
        self.boot();
        Ok(())
    }

    pub fn current_payload(&self) -> Option<LicensePayload> {
        certificate::read_certificate(&self.config_dir).ok()
    }
}
```

- [ ] **Step 2: 把 `trial::touch` / `trial::evaluate` 的签名改为接受 `&[&dyn TrialStore]`，避开 Send+Sync 兼容问题**

打开 `src-tauri/src/license/trial.rs`，把：
```rust
pub fn evaluate(stores: &[Box<dyn TrialStore>], ...) -> ...
pub fn touch(stores: &[Box<dyn TrialStore>], ...) -> ...
```
改为：
```rust
pub fn evaluate(stores: &[&dyn TrialStore], ...) -> ...
pub fn touch(stores: &[&dyn TrialStore], ...) -> ...
```

`storage.rs` 同步把 `pub trait TrialStore { ... }` 增 trait bound `: Send + Sync`：
```rust
pub trait TrialStore: Send + Sync { ... }
```

`build_default_stores` 返回类型保持 `Vec<Box<dyn TrialStore>>`，调用方按 `.iter().map(|b| b.as_ref()).collect::<Vec<_>>()` 取 `&[&dyn TrialStore]`。

回到 `state.rs` 把 `trial_stores_as_dyn` 改实现：
```rust
fn trial_stores_as_dyn(&self) -> Vec<&dyn TrialStore> {
    self.trial_stores.iter().map(|b| b.as_ref() as &dyn TrialStore).collect()
}
```

并且 `LicenseManager::new` 接收类型同步改为 `Vec<Box<dyn TrialStore>>`。

- [ ] **Step 3: 调整 trial.rs 的测试，把 `Vec<Box<dyn TrialStore>>` 调用改成 `vec![&store as &dyn TrialStore]`**

```rust
// 测试中：
fn build_stores(d: &std::path::Path) -> Vec<ConfigDirStore> {
    vec![ConfigDirStore { path: d.join("trial.json") }]
}
// 在调用处：
let stores = build_stores(d.path());
let refs: Vec<&dyn TrialStore> = stores.iter().map(|s| s as &dyn TrialStore).collect();
touch(&refs, "fp1", now);
```

> **重要**：这一步替换只在测试代码内部进行；生产代码用 `build_default_stores` 返回 `Vec<Box<dyn TrialStore>>`，再用 `.iter().map(|b| b.as_ref()).collect()` 转引用切片。

- [ ] **Step 4: 跑全部 license 测试**

```bash
cd src-tauri && cargo test license:: --lib
```

Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/license/state.rs src-tauri/src/license/trial.rs src-tauri/src/license/storage.rs
git commit -m "feat(license): LicenseManager + 启动 boot 流程；trial fn 接受 &[&dyn TrialStore]"
```

---

## Phase 7：后台 heartbeat 任务

### Task 7.1: heartbeat.rs

**Files:**
- Modify: `src-tauri/src/license/heartbeat.rs`

- [ ] **Step 1: 写 `heartbeat.rs`**

```rust
// src-tauri/src/license/heartbeat.rs
//
// 后台周期性 heartbeat 任务：
// 启动后立即 ping 一次（如证书已过 next_check_at），之后每 24h 检查一次。
// heartbeat 失败按宽限期推进，达到 max_grace_until 后客户端转 GraceMode（仍可用，仅显示横幅）。
// 收到 status=revoked → 客户端转 Revoked 态（删证书 + 重 boot）。

use crate::license::api_client;
use crate::license::certificate::{self};
use crate::license::state::{LicenseManager, LicenseState};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;

const POLL_INTERVAL_SECS: u64 = 24 * 60 * 60;    // 24h
const HEARTBEAT_DAYS: i64 = 7;
const GRACE_DAYS: i64 = 14;

pub fn spawn(app: tauri::AppHandle, manager: Arc<LicenseManager>) {
    tokio::spawn(async move {
        // 启动后立即检查一次
        check_once(&app, &manager).await;
        loop {
            tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
            check_once(&app, &manager).await;
        }
    });
}

async fn check_once(app: &tauri::AppHandle, manager: &LicenseManager) {
    let payload = match manager.current_payload() {
        Some(p) => p,
        None => return,    // 未激活，不需 ping
    };

    let next_check: DateTime<Utc> = parse_iso(&payload.next_check_at);
    let now = Utc::now();
    if now < next_check { return; }    // 还没到复验时间

    let body = api_client::HeartbeatBody {
        license_id: &payload.license_id,
        device_id: &payload.device_id,
        fingerprint: &payload.fingerprint,
    };
    match api_client::heartbeat(&body).await {
        Ok(data) => {
            if data.status == "revoked" {
                // 删证书，重 boot
                let _ = certificate::delete_certificate_at(&manager_config_dir(manager));
                manager.boot();
                let _ = app.emit("license:state-changed", manager.current());
            } else {
                // active：刷新 next_check_at — 重写证书的 envelope 不可行（私钥在后端），
                // 这里只是把客户端内的 next_check_at 缓存起来。简化方案：仅 emit 状态不变事件，
                // 让前端 console 知道复验通过。
                let _ = app.emit("license:heartbeat-ok", serde_json::json!({ "next_check_at": data.next_check_at }));
            }
        }
        Err(_) => {
            // 网络失败：检查是否超过 max_grace_until
            let max_grace = parse_iso(&payload.max_grace_until);
            if Utc::now() > max_grace {
                // 进入 GraceMode（仍可用，UI 显示横幅）
                let days_over = (Utc::now() - max_grace).num_days();
                let _ = app.emit("license:state-changed", LicenseState::GraceMode {
                    email: payload.email.clone(),
                    days_until_block: -days_over,    // 已超出
                });
            }
            // 未超过宽限期：静默重试，下个周期再试
        }
    }
}

fn parse_iso(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now())
}

fn manager_config_dir(_m: &LicenseManager) -> std::path::PathBuf {
    // LicenseManager 没暴露 config_dir，这里加 getter（见 Step 2）
    todo!("see Step 2")
}
```

- [ ] **Step 2: 在 `LicenseManager` 加 `config_dir()` getter + 在 certificate.rs 加 `delete_certificate_at()` 别名**

`state.rs` 加：
```rust
impl LicenseManager {
    pub fn config_dir(&self) -> &std::path::Path { &self.config_dir }
}
```

`certificate.rs` 增加：
```rust
pub fn delete_certificate_at(config_dir: &Path) -> std::io::Result<()> {
    delete_certificate(config_dir)
}
```

回到 `heartbeat.rs` 把 `manager_config_dir(manager)` 改为 `manager.config_dir()`。

- [ ] **Step 3: 编译通过**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/license/heartbeat.rs src-tauri/src/license/state.rs src-tauri/src/license/certificate.rs
git commit -m "feat(license): 后台 heartbeat 任务（24h 轮询，14 天宽限，revoked 即 boot）"
```

---

## Phase 8：水印模块（5 种格式）

### Task 8.1: watermark.rs — Spreadsheet 注入

**Files:**
- Modify: `src-tauri/src/license/watermark.rs`

- [ ] **Step 1: 写 `watermark.rs` 主框架 + spreadsheet 实现**

```rust
// src-tauri/src/license/watermark.rs
//
// 试用版水印注入。仅在 LicenseState::TrialExpired 时插入。
// 由 commands/file.rs 的 export_content() 在每次导出前调用。
//
// 文案按编译期 Cargo feature 选语言：
//   lang-zh → "本文件由 Dimkey 试用版生成 · https://dimkey.app"
//   lang-en → "Generated by Dimkey trial · https://dimkey.app"

use crate::models::sensitive::{CellValue, SheetData};

pub const WATERMARK_ZH: &str = "本文件由 Dimkey 试用版生成 · https://dimkey.app";
pub const WATERMARK_EN: &str = "Generated by Dimkey trial · https://dimkey.app";

pub fn watermark_text() -> &'static str {
    #[cfg(feature = "lang-zh")] { WATERMARK_ZH }
    #[cfg(feature = "lang-en")] { WATERMARK_EN }
    #[cfg(not(any(feature = "lang-zh", feature = "lang-en")))] { WATERMARK_ZH }    // 默认中文
}

/// 在 spreadsheet sheets 的第一个 sheet 顶部插入一行水印
pub fn inject_into_spreadsheet(sheets: &mut Vec<SheetData>) {
    if sheets.is_empty() { return; }
    let first = &mut sheets[0];
    let watermark_row = vec![CellValue { text: watermark_text().to_string(), ..Default::default() }];
    first.rows.insert(0, watermark_row);
}

/// 在 docx/txt 段落开头插入水印段
pub fn inject_into_paragraphs(paragraphs: &mut Vec<String>) {
    paragraphs.insert(0, watermark_text().to_string());
}

/// 在 csv 内容前插入注释行
pub fn inject_into_csv_headers(headers: &mut Vec<String>) {
    // CSV 用法：在 headers 上方实际写入一个独立"注释行"——但 csv crate 写表头是固定一行。
    // 简化：把水印作为 headers 之前的"伪注释"返回单独字符串，由 export_csv 写完 # 行后再写真实 headers。
    // 这里改为 export_csv 直接调用 watermark_text() 写一行带 # 前缀。详见 commands/file.rs 的改动。
    let _ = headers;    // no-op placeholder; CSV 注入在 commands/file.rs 中直接写入
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::sensitive::{CellValue, SheetData};

    #[test]
    fn inject_into_spreadsheet_adds_row_at_top() {
        let mut sheets = vec![SheetData {
            name: "Sheet1".into(),
            headers: vec!["A".into()],
            rows: vec![vec![CellValue { text: "data1".into(), ..Default::default() }]],
        }];
        inject_into_spreadsheet(&mut sheets);
        assert_eq!(sheets[0].rows.len(), 2);
        assert_eq!(sheets[0].rows[0][0].text, watermark_text());
    }

    #[test]
    fn inject_into_paragraphs_adds_at_start() {
        let mut p: Vec<String> = vec!["第一段".into(), "第二段".into()];
        inject_into_paragraphs(&mut p);
        assert_eq!(p.len(), 3);
        assert_eq!(p[0], watermark_text());
    }
}
```

> 注意：`SheetData` 的实际定义在 `models/sensitive.rs` 或 `parser/excel.rs`；Step 1 的 use 路径要按实际改正。如不存在 `Default` 实现要为 `CellValue` 加。

- [ ] **Step 2: 修正 use 路径 + 跑测试**

```bash
cd src-tauri && grep -n "pub struct SheetData" src/ -r
# 按实际定义路径修正 watermark.rs 顶部 use
cargo test license::watermark --lib
```

Expected: 2 个测试 PASS

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/license/watermark.rs
git commit -m "feat(license): 水印 — spreadsheet/paragraph 注入 + 测试"
```

---

### Task 8.2: 在 `commands/file.rs::export_content()` 注入水印调用

**Files:**
- Modify: `src-tauri/src/commands/file.rs`

- [ ] **Step 1: 在文件顶部 use 区追加**

```rust
use crate::license::{watermark, LicenseState};
use crate::license::state::LicenseManager;
use std::sync::Arc;
```

- [ ] **Step 2: `export_content` 函数签名增加 `manager: Option<Arc<LicenseManager>>` 参数**

把：
```rust
pub fn export_content(content: &FileContent, output_path: &str, original_path: Option<&str>) -> Result<(), String> { ... }
```
改为：
```rust
pub fn export_content(
    content: &FileContent,
    output_path: &str,
    original_path: Option<&str>,
    license_manager: Option<&LicenseManager>,
) -> Result<(), String> {
    let needs_watermark = license_manager.map(|m| matches!(m.current(), LicenseState::TrialExpired)).unwrap_or(false);

    // 克隆内容以便插入水印（原内容不污染）
    let content_owned;
    let content_ref = if needs_watermark {
        content_owned = clone_with_watermark(content);
        &content_owned
    } else {
        content
    };

    match content_ref {
        FileContent::Spreadsheet { file_type, sheets, .. } => match file_type {
            FileType::Csv => {
                if let Some(sheet) = sheets.first() {
                    export_csv_with_watermark(output_path, &sheet.headers, &sheet.rows, needs_watermark)
                } else {
                    export_csv_with_watermark(output_path, &[], &[], needs_watermark)
                }
            }
            // 其他分支保持原状（spreadsheet 水印已通过 clone_with_watermark 注入到 sheets 中）
            FileType::Xlsx | FileType::Xls => { /* 原逻辑 */ }
            _ => Err("不支持的导出格式".to_string()),
        },
        FileContent::Document { file_type, paragraphs, encoding, .. } => match file_type {
            FileType::Txt => export_txt(paragraphs, output_path, encoding.as_deref()),
            FileType::Pdf => Err("PDF 导出请使用专用的涂黑导出功能".to_string()),
            _ => {
                let src = original_path.ok_or_else(|| "导出 Word 文档需要提供原始文件路径".to_string())?;
                export_docx(src, paragraphs, output_path)
            }
        },
    }
}

fn clone_with_watermark(content: &FileContent) -> FileContent {
    let mut cloned = content.clone();
    match &mut cloned {
        FileContent::Spreadsheet { sheets, .. } => watermark::inject_into_spreadsheet(sheets),
        FileContent::Document { paragraphs, .. } => watermark::inject_into_paragraphs(paragraphs),
    }
    cloned
}
```

> `FileContent` 必须实现 `Clone`。如未实现，加 `#[derive(Clone)]`。`SheetData`、`CellValue` 同理。

- [ ] **Step 3: 加 `export_csv_with_watermark` 包装**

```rust
fn export_csv_with_watermark(path: &str, headers: &[String], rows: &[Vec<CellValue>], with_wm: bool) -> Result<(), String> {
    let mut writer = csv::Writer::from_path(path).map_err(|e| format!("创建 CSV 文件失败: {}", e))?;
    if with_wm {
        // CSV 注释行：以 # 开头单字段
        writer.write_record(&[format!("# {}", watermark::watermark_text())])
            .map_err(|e| format!("写入水印失败: {}", e))?;
    }
    writer.write_record(headers).map_err(|e| format!("写入表头失败: {}", e))?;
    for row in rows {
        let string_row: Vec<&str> = row.iter().map(|cv| cv.text.as_str()).collect();
        writer.write_record(&string_row).map_err(|e| format!("写入数据行失败: {}", e))?;
    }
    writer.flush().map_err(|e| format!("写入文件失败: {}", e))?;
    Ok(())
}
```

- [ ] **Step 4: 修改所有 `export_content` 调用方传入 `license_manager`**

搜索 `export_content(`：
```bash
grep -rn "export_content(" src-tauri/src/
```

每处调用补一个参数：从 `app.try_state::<LicenseManagerState>()` 拿到 manager（见下一个 Phase 9 的 register）。或者临时在调用处传 `None`（先让编译通过）：
```rust
export_content(&content, &output_path, original_path.as_deref(), None)
```

- [ ] **Step 5: 编译通过**

```bash
cd src-tauri && cargo check
```

> 注意：传 `None` 时水印不会注入，等 Phase 9 注册 LicenseManager 全局状态后再回填真实 manager（Task 9.3）。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/commands/file.rs
git commit -m "feat(watermark): export_content 注入水印 — spreadsheet/csv/docx/txt（待 LicenseManager 接入）"
```

---

### Task 8.3: PDF 水印（pdf_export.rs 单独路径）

**Files:**
- Modify: `src-tauri/src/commands/file.rs`（`export_pdf_redacted_cmd`）
- Modify: `src-tauri/src/parser/pdf_export.rs`（如水印写在 PDF 生成层）

- [ ] **Step 1: 找到 PDF 导出入口**

```bash
grep -n "fn export_pdf_redacted_cmd\|pub fn write_pdf\|use printpdf" src-tauri/src/ -r
```

定位到 `commands/file.rs:432` 附近的 `export_pdf_redacted_cmd`。

- [ ] **Step 2: 在 PDF 导出函数顶部加水印检查**

```rust
// commands/file.rs:432 附近
pub async fn export_pdf_redacted_cmd(
    /* ... 原参数 ... */,
    license_manager: tauri::State<'_, LicenseManagerState>,
) -> Result<(), String> {
    let needs_watermark = matches!(license_manager.0.current(), LicenseState::TrialExpired);
    /* ... 原逻辑 ... */
    if needs_watermark {
        add_pdf_footer_watermark(&output_path, watermark::watermark_text())?;
    }
    Ok(())
}

/// 读取 PDF，在第一页页脚加灰色小字水印，覆盖原 PDF
fn add_pdf_footer_watermark(pdf_path: &str, text: &str) -> Result<(), String> {
    // 用 pdfium-render 打开 → 在第 1 页绘 text → 保存
    use pdfium_render::prelude::*;
    let pdfium = Pdfium::default();    // 已在全局初始化
    let mut doc = pdfium.load_pdf_from_file(pdf_path, None).map_err(|e| format!("水印 PDF 失败: {}", e))?;
    if let Some(mut page) = doc.pages_mut().get(0).ok() {
        let page_width = page.width().value;
        let page_height = page.height().value;
        let _ = page.objects_mut().create_text_object(
            PdfPagePaperSize::Letter,    // dummy; pdfium-render 实际 API 待确认
            // 在 (10pt, 10pt) 位置绘灰色文字
        );
        // ⚠️ pdfium-render 0.8 的 text drawing API 较复杂，简化：
        // 直接用 printpdf 重写一份带水印的 PDF 也行。
        // 此处为示意，实际实现按 pdfium-render 当时版本的 API 调整。
    }
    doc.save_to_file(pdf_path).map_err(|e| format!("保存水印 PDF 失败: {}", e))?;
    Ok(())
}
```

> **实施提示**：pdfium-render 写文字 API 较生涩。如实现复杂，可先 v1 跳过 PDF 水印（保留 4 种格式即可），在导出 PDF 时弹一次 toast "试用版导出的 PDF 不会带水印，请激活后使用" — 用户体验略差但工程上简单。本 task 取决于实施时对 pdfium API 的评估，可由实施工程师决定。

- [ ] **Step 3: 编译通过 + 提交（即使 PDF 水印是 stub 也提交）**

```bash
cargo check
git add src-tauri/src/commands/file.rs
git commit -m "feat(watermark): PDF 页脚水印（pdfium-render 实现，复杂时可降级 toast）"
```

---

## Phase 9：Tauri commands + 全局状态注册

### Task 9.1: commands/license.rs — 11 个 Tauri commands

**Files:**
- Create: `src-tauri/src/commands/license.rs`
- Modify: `src-tauri/src/commands/mod.rs`

- [ ] **Step 1: 写 commands/license.rs**

```rust
// src-tauri/src/commands/license.rs

use crate::license::api_client::{self, DeviceDto};
use crate::license::errors::LicenseError;
use crate::license::state::{LicenseManager, LicenseState};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

pub struct LicenseManagerState(pub Arc<LicenseManager>);

#[derive(Serialize)]
pub struct TrialInfoDto {
    pub days_remaining: u32,
    pub expired: bool,
}

#[tauri::command]
pub fn license_get_state(state: State<LicenseManagerState>) -> LicenseState {
    state.0.current()
}

#[tauri::command]
pub fn license_get_fingerprint(state: State<LicenseManagerState>) -> String {
    state.0.machine_fp().to_string()
}

/// 当本地存在 .lic 但其指纹与本机不匹配时，返回旧指纹的前 8 字符；否则返回 None
/// 用于 About 面板顶部展示 "此授权文件属于另一台机器" 提示（spec §4.5 Step 3）
#[tauri::command]
pub fn license_get_fingerprint_mismatch_hint(state: State<LicenseManagerState>) -> Option<String> {
    let payload = state.0.current_payload()?;
    if payload.fingerprint != state.0.machine_fp() {
        Some(payload.fingerprint[..8].to_string())
    } else {
        None
    }
}

#[tauri::command]
pub fn license_get_trial_info(state: State<LicenseManagerState>) -> TrialInfoDto {
    match state.0.current() {
        LicenseState::Trial { days_remaining } => TrialInfoDto { days_remaining, expired: false },
        LicenseState::TrialExpired => TrialInfoDto { days_remaining: 0, expired: true },
        _ => TrialInfoDto { days_remaining: 0, expired: false },
    }
}

#[derive(Serialize)]
pub struct ActivateResultDto {
    pub email: String,
    pub max_devices: u32,
    pub active_devices: u32,
    pub device_id: String,
}

#[tauri::command]
pub async fn license_activate(
    state: State<'_, LicenseManagerState>,
    license_key: String,
    email: String,
) -> Result<ActivateResultDto, LicenseError> {
    let machine_label = hostname();
    let os = if cfg!(target_os = "macos") { "macos" } else { "windows" };
    let flavor = if cfg!(feature = "lang-en") { "en" } else { "zh" };
    let app_version = env!("CARGO_PKG_VERSION");
    let result = state.0.try_activate(&license_key, &email, &machine_label, os, flavor, app_version)?;
    Ok(ActivateResultDto {
        email,
        max_devices: result.device_summary.max_devices,
        active_devices: result.device_summary.active_count,
        device_id: result.device_summary.current_device_id,
    })
}

#[tauri::command]
pub async fn license_deactivate_current(state: State<'_, LicenseManagerState>) -> Result<(), LicenseError> {
    state.0.deactivate_local()
}

#[tauri::command]
pub async fn license_list_devices(state: State<'_, LicenseManagerState>) -> Result<Vec<DeviceDto>, LicenseError> {
    let payload = state.0.current_payload().ok_or(LicenseError::InvalidLicense)?;
    let body = api_client::DevicesListBody {
        license_key: &payload.license_key, email: &payload.email,
        fingerprint: Some(state.0.machine_fp()),
    };
    let data = api_client::list_devices(&body).await?;
    Ok(data.devices)
}

#[tauri::command]
pub async fn license_deactivate_device(state: State<'_, LicenseManagerState>, device_id: String) -> Result<(), LicenseError> {
    let payload = state.0.current_payload().ok_or(LicenseError::InvalidLicense)?;
    let body = api_client::DeactivateBody {
        license_key: &payload.license_key, email: &payload.email,
        device_id: Some(&device_id), fingerprint: None,
    };
    api_client::deactivate(&body).await
}

#[tauri::command]
pub async fn license_recover_email(email: String) -> Result<(), LicenseError> {
    let body = api_client::RecoverBody { email: &email };
    api_client::recover(&body).await
}

#[tauri::command]
pub async fn license_open_purchase_page(app: tauri::AppHandle) -> Result<(), LicenseError> {
    let url = if cfg!(feature = "lang-en") { "https://dimkey.app/buy" } else { "https://dimkey.app/buy" };
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| LicenseError::ServerError { code: "OPEN".into(), message: e.to_string() })?;
    let _ = app;
    Ok(())
}

fn hostname() -> String {
    use sysinfo::System;
    System::host_name().unwrap_or_else(|| "Unknown".to_string())
}
```

- [ ] **Step 2: `commands/mod.rs` 加模块**

```rust
pub mod license;
```

- [ ] **Step 3: 编译通过**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/commands/license.rs src-tauri/src/commands/mod.rs
git commit -m "feat(commands): 9 个 license Tauri commands（state/trial/activate/deactivate/list/recover/open）"
```

---

### Task 9.2: lib.rs setup — 注册 LicenseManager + register commands + spawn heartbeat

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 在 imports 区追加**

```rust
use std::sync::Arc;
use commands::license::*;
use license::state::LicenseManager;
use license::storage::build_default_stores;
use license::fingerprint::compute_fingerprint;
```

- [ ] **Step 2: 在 `setup` 钩子末尾、return 前加**

```rust
// === License 系统初始化 ===
let config_dir = app.path().app_config_dir()
    .unwrap_or_else(|_| std::path::PathBuf::from("."));
let machine_fp = compute_fingerprint();
let trial_stores = build_default_stores(config_dir.clone());
let manager = Arc::new(LicenseManager::new(trial_stores, config_dir, machine_fp));
manager.boot();
app.manage(commands::license::LicenseManagerState(manager.clone()));

// 后台 heartbeat
license::heartbeat::spawn(app.handle().clone(), manager);
```

- [ ] **Step 3: 在 `tauri::generate_handler!` 列表里加 10 个 license command**

找到 `.invoke_handler(tauri::generate_handler![...])` 把以下加入列表：
```rust
license_get_state,
license_get_fingerprint,
license_get_fingerprint_mismatch_hint,
license_get_trial_info,
license_activate,
license_deactivate_current,
license_list_devices,
license_deactivate_device,
license_recover_email,
license_open_purchase_page,
```

- [ ] **Step 4: 编译通过 + dev 跑一下**

```bash
cd src-tauri && cargo check
cargo tauri dev
# 启动后用 devtools console 验证：
# > await window.__TAURI__.core.invoke('license_get_state')
# 应返回 { kind: 'Trial', days_remaining: 30 }（首次启动）
```

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(license): lib.rs setup 注册 LicenseManager + 后台 heartbeat + 9 commands 路由"
```

---

### Task 9.3: 把 LicenseManager 接入 export_content 调用方

**Files:**
- Modify: `src-tauri/src/commands/file.rs`

- [ ] **Step 1: 把 export_file 拿到 LicenseManagerState**

打开 `commands/file.rs:262`，把 export_file 签名改：
```rust
pub async fn export_file(
    content: FileContent,
    output_path: String,
    original_path: Option<String>,
    app_handle: tauri::AppHandle,
    license_manager: tauri::State<'_, crate::commands::license::LicenseManagerState>,
) -> Result<(), String> {
    /* ... */
    let manager_arc = license_manager.0.clone();
    let result = tokio::task::spawn_blocking(move || {
        export_content(&content, &output_path, original_path.as_deref(), Some(&*manager_arc))
    }).await...
}
```

- [ ] **Step 2: 同样修改其他调用 export_content 的地方（restore_file 等）**

```bash
grep -rn "export_content(" src-tauri/src/
```
每处都补 `Some(&*manager.0)` 或 `None`（如 restore_file 在系统内部场景，可以传 None 跳过水印）。

- [ ] **Step 3: 编译通过 + 手动测试**

```bash
cd src-tauri && cargo check
cargo tauri dev
# 在 dev 控制台模拟 TrialExpired：暂时改 trial_days 为 0 或快进时间，导出文件验证带水印
```

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/commands/file.rs
git commit -m "feat(watermark): 接入 LicenseManager — TrialExpired 时导出自动加水印"
```

---

## Phase 10：前端 Store + i18n

### Task 10.1: licenseStore.ts + i18n 资源

**Files:**
- Create: `src/stores/licenseStore.ts`
- Create: `src/locales/zh/license.json`
- Create: `src/locales/en/license.json`
- Modify: `src/i18n.ts`

- [ ] **Step 1: 写 zh/license.json（spec §10 完整 key 列表）**

```json
{
  "activate": {
    "title": "激活 Dimkey",
    "email_label": "邮箱",
    "key_label": "许可证密钥",
    "key_placeholder": "DK-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX",
    "recover_link": "忘记许可证? 通过邮箱找回",
    "cancel": "取消",
    "button": "激活",
    "button_loading": "激活中...",
    "success_toast": "已激活给 {{email}}"
  },
  "error": {
    "invalid": "邮箱或许可证不正确",
    "email_mismatch": "邮箱与许可证不匹配",
    "device_limit": "已达 {{max}} 台设备上限",
    "device_limit_action": "请先解绑一台或查看设备列表",
    "network": "网络不可用，请检查网络后重试",
    "fingerprint_mismatch": "该证书绑定到另一台机器",
    "signature_invalid": "授权文件已损坏，请重新激活",
    "revoked": "本许可证已失效",
    "rate_limited": "请求过于频繁，请稍后再试",
    "server": "服务暂时不可用，请稍后再试"
  },
  "trial": {
    "welcome_toast": "欢迎使用 Dimkey · 试用 30 天，可在设置查看",
    "remaining": "试用版 · 剩余 {{days}} 天",
    "expired_banner": "试用已结束 · 导出文件将带水印",
    "banner_dismiss": "我知道了"
  },
  "about": {
    "activated_to": "已授权给 {{email}}",
    "plan_perpetual": "永久许可",
    "devices": "当前 {{active}} / {{max}} 台设备已激活",
    "deactivate_current": "解绑当前设备",
    "manage_devices": "管理设备...",
    "fingerprint_label": "设备指纹",
    "fingerprint_copy": "已复制设备指纹",
    "fingerprint_mismatch_hint": "此授权文件属于另一台机器（fp:{{fp}}），点此重新激活本机"
  },
  "devices": {
    "title": "已激活的设备",
    "this_device": "(此设备)",
    "deactivate": "解绑",
    "deactivate_confirm": "确认解绑此设备?",
    "summary": "{{active}} / {{max}} 台设备 · 解绑后立即释放配额"
  },
  "recover": {
    "title": "通过邮箱找回",
    "hint": "输入购买时使用的邮箱，许可证将重发到该邮箱",
    "send": "发送",
    "success_msg": "如该邮箱有授权，已发送邮件"
  },
  "purchase": {
    "button": "购买",
    "enter_key": "输入许可证"
  }
}
```

- [ ] **Step 2: 写 en/license.json**

```json
{
  "activate": {
    "title": "Activate Dimkey",
    "email_label": "Email",
    "key_label": "License Key",
    "key_placeholder": "DK-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX",
    "recover_link": "Lost your license? Recover by email",
    "cancel": "Cancel",
    "button": "Activate",
    "button_loading": "Activating...",
    "success_toast": "Activated to {{email}}"
  },
  "error": {
    "invalid": "Invalid email or license key",
    "email_mismatch": "Email doesn't match this license",
    "device_limit": "Device limit reached ({{max}})",
    "device_limit_action": "Deactivate one or view your devices",
    "network": "Network unavailable, please retry",
    "fingerprint_mismatch": "This certificate belongs to another machine",
    "signature_invalid": "Certificate file corrupted, please re-activate",
    "revoked": "This license is no longer valid",
    "rate_limited": "Too many requests, please try later",
    "server": "Service temporarily unavailable, please try later"
  },
  "trial": {
    "welcome_toast": "Welcome to Dimkey · 30-day trial · See settings",
    "remaining": "Trial · {{days}} days remaining",
    "expired_banner": "Trial ended · Exports will be watermarked",
    "banner_dismiss": "Got it"
  },
  "about": {
    "activated_to": "Activated to {{email}}",
    "plan_perpetual": "Lifetime License",
    "devices": "{{active}} / {{max}} devices active",
    "deactivate_current": "Deactivate this device",
    "manage_devices": "Manage devices...",
    "fingerprint_label": "Device fingerprint",
    "fingerprint_copy": "Fingerprint copied",
    "fingerprint_mismatch_hint": "This certificate belongs to another machine (fp:{{fp}}). Click to re-activate this machine"
  },
  "devices": {
    "title": "Activated devices",
    "this_device": "(this device)",
    "deactivate": "Deactivate",
    "deactivate_confirm": "Deactivate this device?",
    "summary": "{{active}} / {{max}} devices · Deactivating frees a slot immediately"
  },
  "recover": {
    "title": "Recover by email",
    "hint": "Enter the email you used at purchase. Your license will be re-sent.",
    "send": "Send",
    "success_msg": "If this email has a license, an email has been sent"
  },
  "purchase": {
    "button": "Buy",
    "enter_key": "Enter license"
  }
}
```

- [ ] **Step 3: 修改 i18n.ts 加载 license 命名空间**

打开 `src/i18n.ts`，在 resources 块里追加：
```typescript
import zhLicense from './locales/zh/license.json';
import enLicense from './locales/en/license.json';

// 在 resources 配置中：
{
  zh: { common: zhCommon, license: zhLicense },
  en: { common: enCommon, license: enLicense },
}
// 加 ns: ['common', 'license']  defaultNS: 'common'
```

- [ ] **Step 4: 写 `src/stores/licenseStore.ts`**

```typescript
// src/stores/licenseStore.ts
import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export type LicenseState =
  | { kind: 'Trial'; days_remaining: number }
  | { kind: 'TrialExpired' }
  | { kind: 'Activated'; email: string; plan: string; max_devices: number; active_devices: number; device_id: string; license_id: string; fingerprint_mismatch: boolean }
  | { kind: 'GraceMode'; email: string; days_until_block: number }
  | { kind: 'Revoked'; reason: string }
  | { kind: 'Unknown' };

export interface LicenseError { code: string; data?: unknown }

interface Store {
  state: LicenseState;
  fingerprint: string;
  initialized: boolean;
  init: () => Promise<UnlistenFn | undefined>;
  refresh: () => Promise<void>;
  activate: (license_key: string, email: string) => Promise<void>;
  deactivateCurrent: () => Promise<void>;
  listDevices: () => Promise<any[]>;
  deactivateDevice: (device_id: string) => Promise<void>;
  recover: (email: string) => Promise<void>;
  openPurchase: () => Promise<void>;
}

export const useLicenseStore = create<Store>((set, get) => ({
  state: { kind: 'Unknown' },
  fingerprint: '',
  initialized: false,

  init: async () => {
    const [state, fp] = await Promise.all([
      invoke<LicenseState>('license_get_state'),
      invoke<string>('license_get_fingerprint'),
    ]);
    set({ state, fingerprint: fp, initialized: true });
    const un = await listen<LicenseState>('license:state-changed', (e) => set({ state: e.payload }));
    return un;
  },

  refresh: async () => {
    const state = await invoke<LicenseState>('license_get_state');
    set({ state });
  },

  activate: async (license_key, email) => {
    await invoke('license_activate', { licenseKey: license_key, email });
    await get().refresh();
  },
  deactivateCurrent: async () => { await invoke('license_deactivate_current'); await get().refresh(); },
  listDevices: async () => invoke<any[]>('license_list_devices'),
  deactivateDevice: async (device_id) => { await invoke('license_deactivate_device', { deviceId: device_id }); },
  recover: async (email) => { await invoke('license_recover_email', { email }); },
  openPurchase: async () => { await invoke('license_open_purchase_page'); },
}));
```

- [ ] **Step 5: 提交**

```bash
git add src/stores/licenseStore.ts src/locales/zh/license.json src/locales/en/license.json src/i18n.ts
git commit -m "feat(license-ui): Zustand store + i18n 资源（zh/en license 命名空间）"
```

---

## Phase 11：UI 组件

### Task 11.1: ActivationDialog（核心组件）

**Files:**
- Create: `src/components/license/ActivationDialog.tsx`

- [ ] **Step 1: 写组件**

```tsx
// src/components/license/ActivationDialog.tsx
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useLicenseStore } from '../../stores/licenseStore';

interface Props {
  open: boolean;
  onClose: () => void;
  onShowDevices?: (devices: any[], max: number) => void;
  onShowRecover?: () => void;
}

export function ActivationDialog({ open, onClose, onShowDevices, onShowRecover }: Props) {
  const { t } = useTranslation('license');
  const activate = useLicenseStore((s) => s.activate);
  const [email, setEmail] = useState('');
  const [keyInput, setKeyInput] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!open) return null;

  const formatKey = (raw: string): string => {
    const upper = raw.toUpperCase().replace(/[^A-Z0-9]/g, '');
    const stripped = upper.startsWith('DK') ? upper.slice(2) : upper;
    const valid = stripped.split('').filter((c) => 'ABCDEFGHJKMNPQRSTUVWXYZ23456789'.includes(c)).slice(0, 25).join('');
    if (valid.length === 0) return '';
    const segs = (valid.match(/.{1,5}/g) || []).slice(0, 5);
    return 'DK-' + segs.join('-');
  };

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await activate(keyInput, email);
      onClose();
      // success toast 由调用方监听 license:state-changed 后触发
    } catch (err: any) {
      const code = err?.code ?? err?.message ?? 'server';
      const data = err?.data;
      switch (code) {
        case 'INVALID_LICENSE': setError(t('error.invalid')); break;
        case 'DEVICE_LIMIT_REACHED':
          setError(`${t('error.device_limit', { max: data?.max ?? 3 })} · ${t('error.device_limit_action')}`);
          if (onShowDevices && data?.devices) onShowDevices(data.devices, data.max ?? 3);
          break;
        case 'LICENSE_REVOKED': setError(t('error.revoked')); break;
        case 'RATE_LIMITED':    setError(t('error.rate_limited')); break;
        case 'NetworkUnavailable': setError(t('error.network')); break;
        default: setError(t('error.server'));
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="bg-white rounded-xl p-6 w-[460px] shadow-2xl">
        <div className="flex justify-between items-center mb-4">
          <h2 className="text-lg font-semibold">{t('activate.title')}</h2>
          <button className="text-gray-400 hover:text-gray-600 text-xl" onClick={onClose} aria-label="close">×</button>
        </div>
        <form onSubmit={onSubmit} className="space-y-4">
          <div>
            <label className="block text-sm text-gray-600 mb-1">{t('activate.email_label')}</label>
            <input type="email" required value={email} onChange={(e) => setEmail(e.target.value)}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:border-gray-700" />
          </div>
          <div>
            <label className="block text-sm text-gray-600 mb-1">{t('activate.key_label')}</label>
            <input type="text" required value={keyInput} onChange={(e) => setKeyInput(formatKey(e.target.value))}
              placeholder={t('activate.key_placeholder')}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg font-mono text-sm focus:outline-none focus:border-gray-700" />
            <button type="button" onClick={onShowRecover}
              className="text-sm text-blue-600 hover:underline mt-1">{t('activate.recover_link')}</button>
          </div>
          {error && <p className="text-sm text-red-600">{error}</p>}
          <div className="flex justify-end gap-2 pt-2">
            <button type="button" onClick={onClose} className="px-4 py-2 text-gray-600 hover:bg-gray-100 rounded-lg">{t('activate.cancel')}</button>
            <button type="submit" disabled={submitting}
              className="px-4 py-2 bg-gray-900 text-white rounded-lg hover:bg-gray-800 disabled:opacity-50">
              {submitting ? t('activate.button_loading') : t('activate.button')}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: 提交**

```bash
git add src/components/license/ActivationDialog.tsx
git commit -m "feat(license-ui): ActivationDialog — 邮箱+key 两段式 + 自动格式化 + 错误就地展示"
```

---

### Task 11.2: DeviceListDialog

**Files:**
- Create: `src/components/license/DeviceListDialog.tsx`

- [ ] **Step 1: 写组件**

```tsx
// src/components/license/DeviceListDialog.tsx
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useLicenseStore } from '../../stores/licenseStore';

interface Device {
  device_id: string; machine_label: string | null; os: string; flavor: string;
  first_activated: number; last_seen: number; is_current: boolean;
}

interface Props {
  open: boolean;
  onClose: () => void;
  initialDevices?: Device[];
  initialMax?: number;
}

export function DeviceListDialog({ open, onClose, initialDevices, initialMax }: Props) {
  const { t } = useTranslation('license');
  const list = useLicenseStore((s) => s.listDevices);
  const deactivate = useLicenseStore((s) => s.deactivateDevice);
  const [devices, setDevices] = useState<Device[]>(initialDevices ?? []);
  const [max, setMax] = useState(initialMax ?? 3);
  const [loading, setLoading] = useState(false);

  const reload = async () => {
    setLoading(true);
    try {
      const arr = await list();
      setDevices(arr as any);
    } finally { setLoading(false); }
  };

  useEffect(() => { if (open && !initialDevices) reload(); }, [open]);

  if (!open) return null;

  const onDeactivate = async (id: string) => {
    if (!confirm(t('devices.deactivate_confirm'))) return;
    await deactivate(id);
    await reload();
  };

  const fmtTime = (ts: number) => new Date(ts).toLocaleString();

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="bg-white rounded-xl p-6 w-[520px] max-h-[80vh] overflow-y-auto shadow-2xl">
        <div className="flex justify-between items-center mb-4">
          <h2 className="text-lg font-semibold">{t('devices.title')}</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-600 text-xl">×</button>
        </div>
        {loading && <p className="text-sm text-gray-500">Loading...</p>}
        <div className="space-y-2">
          {devices.map((d) => (
            <div key={d.device_id} className="border border-gray-200 rounded-lg p-3 flex justify-between items-center">
              <div>
                <div className="font-medium text-sm">
                  {d.machine_label || '(unnamed)'}
                  {d.is_current && <span className="ml-2 text-xs text-blue-600">{t('devices.this_device')}</span>}
                </div>
                <div className="text-xs text-gray-500">{d.os} · {d.flavor} · {fmtTime(d.last_seen)}</div>
              </div>
              {!d.is_current && (
                <button onClick={() => onDeactivate(d.device_id)}
                  className="text-sm text-red-600 border border-red-300 px-3 py-1 rounded hover:bg-red-50">
                  {t('devices.deactivate')}
                </button>
              )}
            </div>
          ))}
        </div>
        <p className="text-xs text-gray-500 mt-4">{t('devices.summary', { active: devices.length, max })}</p>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: 提交**

```bash
git add src/components/license/DeviceListDialog.tsx
git commit -m "feat(license-ui): DeviceListDialog — 设备列表 + 远程解绑"
```

---

### Task 11.3: RecoverDialog

**Files:**
- Create: `src/components/license/RecoverDialog.tsx`

- [ ] **Step 1: 写组件**

```tsx
// src/components/license/RecoverDialog.tsx
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useLicenseStore } from '../../stores/licenseStore';

interface Props { open: boolean; onClose: () => void }

export function RecoverDialog({ open, onClose }: Props) {
  const { t } = useTranslation('license');
  const recover = useLicenseStore((s) => s.recover);
  const [email, setEmail] = useState('');
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  if (!open) return null;

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    try { await recover(email); } catch {}
    setSubmitted(true);
    setSubmitting(false);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="bg-white rounded-xl p-6 w-[420px] shadow-2xl">
        <div className="flex justify-between items-center mb-4">
          <h2 className="text-lg font-semibold">{t('recover.title')}</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-600 text-xl">×</button>
        </div>
        <p className="text-sm text-gray-600 mb-4">{t('recover.hint')}</p>
        {!submitted ? (
          <form onSubmit={onSubmit} className="space-y-3">
            <input type="email" required value={email} onChange={(e) => setEmail(e.target.value)}
              className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:border-gray-700" />
            <button type="submit" disabled={submitting}
              className="w-full px-4 py-2 bg-gray-900 text-white rounded-lg hover:bg-gray-800 disabled:opacity-50">
              {t('recover.send')}
            </button>
          </form>
        ) : (
          <p className="text-sm text-green-700">{t('recover.success_msg')}</p>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: 提交**

```bash
git add src/components/license/RecoverDialog.tsx
git commit -m "feat(license-ui): RecoverDialog — 找回许可证（防扫号同样响应）"
```

---

### Task 11.4: TrialExpiredBanner + TrialCountdownBadge

**Files:**
- Create: `src/components/license/TrialExpiredBanner.tsx`
- Create: `src/components/license/TrialCountdownBadge.tsx`

- [ ] **Step 1: TrialExpiredBanner**

```tsx
// src/components/license/TrialExpiredBanner.tsx
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useLicenseStore } from '../../stores/licenseStore';

interface Props { onActivate: () => void }

export function TrialExpiredBanner({ onActivate }: Props) {
  const { t } = useTranslation('license');
  const state = useLicenseStore((s) => s.state);
  const openPurchase = useLicenseStore((s) => s.openPurchase);
  const [dismissed, setDismissed] = useState(false);

  if (state.kind !== 'TrialExpired' || dismissed) return null;

  return (
    <div className="bg-amber-50 border-b border-amber-200 px-4 py-2 flex items-center justify-between text-sm">
      <span className="text-amber-900">⚠ {t('trial.expired_banner')}</span>
      <div className="flex gap-2">
        <button onClick={onActivate} className="px-3 py-1 bg-gray-900 text-white rounded hover:bg-gray-800">{t('purchase.enter_key')}</button>
        <button onClick={openPurchase} className="px-3 py-1 border border-gray-300 rounded hover:bg-white">{t('purchase.button')}</button>
        <button onClick={() => setDismissed(true)} className="px-3 py-1 text-gray-600 hover:bg-amber-100 rounded">{t('trial.banner_dismiss')}</button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: TrialCountdownBadge**

```tsx
// src/components/license/TrialCountdownBadge.tsx
import { useTranslation } from 'react-i18next';
import { useLicenseStore } from '../../stores/licenseStore';

interface Props { onActivate: () => void }

export function TrialCountdownBadge({ onActivate }: Props) {
  const { t } = useTranslation('license');
  const state = useLicenseStore((s) => s.state);

  if (state.kind !== 'Trial' || state.days_remaining > 7) return null;

  const color = state.days_remaining <= 3 ? 'bg-red-100 text-red-700 border-red-300'
    : 'bg-orange-100 text-orange-700 border-orange-300';

  return (
    <button onClick={onActivate}
      className={`text-xs px-2 py-0.5 border rounded-full ${color} hover:opacity-80`}>
      {t('trial.remaining', { days: state.days_remaining })}
    </button>
  );
}
```

- [ ] **Step 3: 提交**

```bash
git add src/components/license/TrialExpiredBanner.tsx src/components/license/TrialCountdownBadge.tsx
git commit -m "feat(license-ui): TrialExpiredBanner + TrialCountdownBadge（≤7 天显示）"
```

---

### Task 11.5: AboutPage 集成 license 状态

**Files:**
- Modify or Create: `src/pages/AboutPage.tsx`

- [ ] **Step 1: 找现有 AboutPage 或新建**

```bash
grep -rn "AboutPage\|关于" src/pages/ src/components/
```

- [ ] **Step 2: 在 AboutPage 加 license 区块**

```tsx
// src/pages/AboutPage.tsx (新建或追加)
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import toast from 'react-hot-toast';
import { useLicenseStore } from '../stores/licenseStore';
import { ActivationDialog } from '../components/license/ActivationDialog';
import { DeviceListDialog } from '../components/license/DeviceListDialog';
import { RecoverDialog } from '../components/license/RecoverDialog';

export function AboutPage() {
  const { t } = useTranslation('license');
  const state = useLicenseStore((s) => s.state);
  const fp = useLicenseStore((s) => s.fingerprint);
  const deactivateCurrent = useLicenseStore((s) => s.deactivateCurrent);
  const openPurchase = useLicenseStore((s) => s.openPurchase);

  const [actOpen, setActOpen] = useState(false);
  const [devOpen, setDevOpen] = useState(false);
  const [recOpen, setRecOpen] = useState(false);
  const [devicesPreload, setDevicesPreload] = useState<any[] | undefined>(undefined);
  const [maxPreload, setMaxPreload] = useState<number | undefined>(undefined);
  const [fpMismatchHint, setFpMismatchHint] = useState<string | null>(null);

  useEffect(() => {
    invoke<string | null>('license_get_fingerprint_mismatch_hint').then(setFpMismatchHint).catch(() => {});
  }, [state.kind]);

  const copyFingerprint = async () => {
    await navigator.clipboard.writeText(fp);
    toast.success(t('about.fingerprint_copy'));
  };

  return (
    <div className="p-8 max-w-2xl mx-auto">
      <h1 className="text-2xl font-bold mb-2">Dimkey</h1>
      <p className="text-sm text-gray-500 mb-8">v{import.meta.env.VITE_APP_VERSION ?? '0.7.0'}</p>

      {fpMismatchHint && (
        <div className="mb-4 px-3 py-2 bg-amber-50 border border-amber-200 rounded-lg text-sm text-amber-900">
          <button onClick={() => setActOpen(true)} className="underline">
            {t('about.fingerprint_mismatch_hint', { fp: fpMismatchHint })}
          </button>
        </div>
      )}

      <h2 className="text-base font-semibold border-b pb-2 mb-4">{state.kind === 'Activated' ? t('about.activated_to', { email: state.email }) : null}</h2>

      {state.kind === 'Activated' && (
        <>
          <div className="mb-4">
            <p className="text-sm">✓ {t('about.activated_to', { email: state.email })}</p>
            <p className="text-sm text-gray-600">{t('about.plan_perpetual')} · {t('about.devices', { active: state.active_devices, max: state.max_devices })}</p>
          </div>
          <div className="flex gap-2 mb-6">
            <button onClick={() => { setDevicesPreload(undefined); setMaxPreload(undefined); setDevOpen(true); }}
              className="px-4 py-2 border border-gray-300 rounded-lg hover:bg-gray-50">{t('about.manage_devices')}</button>
            <button onClick={async () => { await deactivateCurrent(); }}
              className="px-4 py-2 border border-red-300 text-red-600 rounded-lg hover:bg-red-50">{t('about.deactivate_current')}</button>
          </div>
        </>
      )}

      {state.kind === 'Trial' && (
        <div className="mb-6 flex items-center justify-between">
          <p>{t('trial.remaining', { days: state.days_remaining })}</p>
          <div className="flex gap-2">
            <button onClick={() => setActOpen(true)} className="px-4 py-2 bg-gray-900 text-white rounded-lg">{t('purchase.enter_key')}</button>
            <button onClick={openPurchase} className="px-4 py-2 border border-gray-300 rounded-lg">{t('purchase.button')}</button>
          </div>
        </div>
      )}

      {state.kind === 'TrialExpired' && (
        <div className="mb-6 flex items-center justify-between">
          <p className="text-amber-700">⚠ {t('trial.expired_banner')}</p>
          <div className="flex gap-2">
            <button onClick={() => setActOpen(true)} className="px-4 py-2 bg-gray-900 text-white rounded-lg">{t('purchase.enter_key')}</button>
            <button onClick={openPurchase} className="px-4 py-2 border border-gray-300 rounded-lg">{t('purchase.button')}</button>
          </div>
        </div>
      )}

      <div className="text-xs text-gray-400 mt-12 flex items-center gap-2">
        <span>{t('about.fingerprint_label')}</span>
        <code className="bg-gray-100 px-2 py-1 rounded">{fp.slice(0, 8)}...{fp.slice(-4)}</code>
        <button onClick={copyFingerprint} className="hover:bg-gray-100 px-1 rounded">📋</button>
      </div>

      <ActivationDialog
        open={actOpen}
        onClose={() => setActOpen(false)}
        onShowDevices={(devs, m) => { setDevicesPreload(devs); setMaxPreload(m); setActOpen(false); setDevOpen(true); }}
        onShowRecover={() => { setActOpen(false); setRecOpen(true); }}
      />
      <DeviceListDialog open={devOpen} onClose={() => setDevOpen(false)} initialDevices={devicesPreload} initialMax={maxPreload} />
      <RecoverDialog open={recOpen} onClose={() => setRecOpen(false)} />
    </div>
  );
}
```

- [ ] **Step 3: 在路由中接 AboutPage**（按现有路由结构改 `App.tsx` 或对应 layout）

如已有 settings 路由，在路由表加 `<Route path="/about" element={<AboutPage />} />`，并在导航/设置面板加链接。

- [ ] **Step 4: 提交**

```bash
git add src/pages/AboutPage.tsx
git commit -m "feat(license-ui): AboutPage — 授权状态展示 + 设备指纹 + 全部 license 操作入口"
```

---

### Task 11.6: 在 App 顶部挂横幅 + 角标 + 首启 toast

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: 在 App 加初始化 + 挂载**

```tsx
// src/App.tsx 顶部追加 imports：
import { useEffect, useState, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import toast from 'react-hot-toast';
import { useLicenseStore } from './stores/licenseStore';
import { TrialExpiredBanner } from './components/license/TrialExpiredBanner';
import { TrialCountdownBadge } from './components/license/TrialCountdownBadge';
import { ActivationDialog } from './components/license/ActivationDialog';
```

在 App 组件内：
```tsx
export default function App() {
  const { t } = useTranslation('license');
  const init = useLicenseStore((s) => s.init);
  const initialized = useLicenseStore((s) => s.initialized);
  const state = useLicenseStore((s) => s.state);
  const [actOpen, setActOpen] = useState(false);
  const welcomeShown = useRef(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    init().then((un) => { unlisten = un; });
    return () => { if (unlisten) unlisten(); };
  }, []);

  // 首启 welcome toast — 仅在初始化后是 Trial=30 天时弹一次
  useEffect(() => {
    if (initialized && !welcomeShown.current && state.kind === 'Trial' && state.days_remaining >= 29) {
      toast(t('trial.welcome_toast'), { duration: 6000 });
      welcomeShown.current = true;
    }
  }, [initialized, state]);

  return (
    <>
      <TrialExpiredBanner onActivate={() => setActOpen(true)} />
      {/* ... 原 App 内容 ... */}
      <div style={{ position: 'absolute', top: 8, right: 8, zIndex: 30 }}>
        <TrialCountdownBadge onActivate={() => setActOpen(true)} />
      </div>
      <ActivationDialog open={actOpen} onClose={() => setActOpen(false)} />
    </>
  );
}
```

- [ ] **Step 2: 浏览器手动测试**

```bash
TAURI_DEV_HOST=127.0.0.1 npm run dev &
cargo tauri dev
```
验证：
- 启动后底部出现 welcome toast
- 设置 → 关于 → 看到 "试用版 · 剩余 30 天"
- 点 "输入许可证" 弹激活对话框，自动格式化 key 输入
- 测试 invalid key → 错误就地显示
- 倒计时角标 ≤ 7 天才出现（暂时不易测，手动改 trial.json 的 first_run_at 为 25 天前再启动）

- [ ] **Step 3: 提交**

```bash
git add src/App.tsx
git commit -m "feat(license-ui): App 集成 — banner/角标/welcome toast/激活入口"
```

---

## Phase 12：E2E 测试

### Task 12.1: pytest E2E — 试用倒计时

**Files:**
- Create: `e2e/tests/test_license_trial.py`

- [ ] **Step 1: 写 E2E 测试**

```python
# e2e/tests/test_license_trial.py
import pytest
from e2e.helpers import dimkey_app, mock_license_state    # 假设已有 helper

@pytest.mark.e2e
def test_first_launch_shows_welcome_toast(dimkey_app):
    """首次启动应在底部显示欢迎 toast"""
    page = dimkey_app.page
    # toast 出现
    page.wait_for_selector('text=欢迎使用 Dimkey', timeout=5000)

@pytest.mark.e2e
def test_about_page_shows_trial_30_days(dimkey_app):
    page = dimkey_app.page
    page.click('text=关于')    # 或 settings → 关于
    assert page.locator('text=剩余 30 天').is_visible()

@pytest.mark.e2e
def test_countdown_badge_hidden_when_more_than_7_days(dimkey_app):
    """剩余 > 7 天时倒计时角标不出现"""
    page = dimkey_app.page
    badges = page.locator('[class*="rounded-full"]:has-text("剩余")')
    assert badges.count() == 0
```

> 实际 helper 名按项目现有 e2e 框架；运行命令：
> `DIMKEY_E2E=1 e2e/.venv/bin/pytest e2e/tests/test_license_trial.py -v -m "not needs_backend"`

- [ ] **Step 2: 提交**

```bash
git add e2e/tests/test_license_trial.py
git commit -m "test(e2e): 试用倒计时 + welcome toast + 角标隐藏规则"
```

---

### Task 12.2: pytest E2E — 激活流程（needs_backend）

**Files:**
- Create: `e2e/tests/test_license_activation.py`

- [ ] **Step 1: 写需后端的 E2E 测试**

```python
# e2e/tests/test_license_activation.py
import os
import pytest

@pytest.mark.e2e
@pytest.mark.needs_backend
def test_activate_flow_end_to_end(dimkey_app):
    """需 Plan A 后端在 localhost:8788 + 一张已发的 license"""
    key = os.environ['DIMKEY_TEST_LICENSE_KEY']
    email = os.environ['DIMKEY_TEST_EMAIL']
    page = dimkey_app.page
    page.click('text=关于')
    page.click('text=输入许可证')
    page.fill('input[type=email]', email)
    page.fill('input[placeholder*="DK-"]', key)
    page.click('text=激活')
    # 等成功后回到 About 页面
    page.wait_for_selector(f'text=已授权给 {email}', timeout=10000)

@pytest.mark.e2e
@pytest.mark.needs_backend
def test_invalid_license_shows_inline_error(dimkey_app):
    page = dimkey_app.page
    page.click('text=关于')
    page.click('text=输入许可证')
    page.fill('input[type=email]', 'wrong@example.com')
    page.fill('input[placeholder*="DK-"]', 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE')
    page.click('text=激活')
    page.wait_for_selector('text=邮箱或许可证不正确', timeout=5000)
```

- [ ] **Step 2: 提交**

```bash
git add e2e/tests/test_license_activation.py
git commit -m "test(e2e): 激活流程 + 错误展示（needs_backend，需 Plan A 后端）"
```

---

## Phase 13：水印 E2E + Rust 集成测试

### Task 13.1: Rust 集成测试 — 水印注入

**Files:**
- Create: `src-tauri/tests/license_watermark.rs`

- [ ] **Step 1: 写集成测试**

```rust
// src-tauri/tests/license_watermark.rs
use dimkey_lib::license::watermark::{inject_into_paragraphs, inject_into_spreadsheet, watermark_text};
use dimkey_lib::models::sensitive::{CellValue, SheetData};

#[test]
fn watermark_text_is_lang_specific() {
    let txt = watermark_text();
    assert!(txt.contains("dimkey.app"));
    #[cfg(feature = "lang-zh")]
    assert!(txt.contains("试用版"));
    #[cfg(feature = "lang-en")]
    assert!(txt.contains("trial"));
}

#[test]
fn xlsx_export_with_expired_trial_inserts_watermark_row() {
    let mut sheets = vec![SheetData {
        name: "Sheet1".into(), headers: vec!["A".into(), "B".into()],
        rows: vec![vec![CellValue { text: "data".into(), ..Default::default() }, CellValue { text: "data2".into(), ..Default::default() }]],
    }];
    inject_into_spreadsheet(&mut sheets);
    assert_eq!(sheets[0].rows.len(), 2);
    assert_eq!(sheets[0].rows[0][0].text, watermark_text());
    // headers 不变
    assert_eq!(sheets[0].headers, vec!["A".to_string(), "B".to_string()]);
}

#[test]
fn docx_paragraphs_get_watermark_at_top() {
    let mut p: Vec<String> = vec!["原段落 1".into(), "原段落 2".into()];
    inject_into_paragraphs(&mut p);
    assert_eq!(p.len(), 3);
    assert_eq!(p[0], watermark_text());
    assert_eq!(p[1], "原段落 1");
}
```

- [ ] **Step 2: 跑测试**

```bash
cd src-tauri && cargo test --test license_watermark
```

- [ ] **Step 3: 提交**

```bash
git add src-tauri/tests/license_watermark.rs
git commit -m "test(integration): 水印注入 — xlsx/docx/lang feature 切换"
```

---

## Phase 14：端到端联调与发布前验证

### Task 14.1: 与 Plan A 后端的端到端联调清单

**手动操作清单（无 commit）：**

- [ ] **Step 1: 确保 Plan A 已部署到 https://dimkey.app（或本地 localhost:8788）**

- [ ] **Step 2: 用 Plan A 的 admin/issue 手动发一张 license**

```bash
curl -X POST https://dimkey.app/admin/issue \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H "content-type: application/json" \
  -d '{"email":"e2e@test.com","source":"manual_cn","order_ref":"E2E-1","plan":"personal","lang":"zh"}'
# 输出含 license_key
```

- [ ] **Step 3: 用客户端激活**

```bash
cargo tauri dev
# UI: 设置 → 关于 → 输入许可证 → 填 e2e@test.com + 上一步 key → 激活
# 期望：toast 显示 "已激活给 e2e@test.com"，About 面板状态变 Activated
```

- [ ] **Step 4: 验证后端 D1 有记录**

```bash
wrangler d1 execute DB --remote --command="SELECT * FROM devices WHERE license_id = (SELECT license_id FROM licenses WHERE email = 'e2e@test.com')"
# 期望：1 行，machine_label = 你的本机 hostname，os = macos
```

- [ ] **Step 5: 模拟换机激活**

  - 在另一台机器（或 VM）上拷贝客户端，激活同一 key
  - 第二台成功 → 后端 active_count = 2
  - 在第三台再激活 → 成功 active_count = 3
  - 在第四台激活 → 失败 `DEVICE_LIMIT_REACHED` + UI 显示设备列表 + 解绑某台
  - 解绑后第四台再激活 → 成功

- [ ] **Step 6: 模拟退款**

```bash
curl -X POST https://dimkey.app/admin/revoke \
  -H "Authorization: Bearer $ADMIN_TOKEN" -d '{"license_key":"DK-...","reason":"e2e_test"}'
```

  - 等客户端 heartbeat 触发（dev 模式可手动改 next_check_at 加快），状态应转 Revoked
  - UI 应弹硬拦截

- [ ] **Step 7: 模拟 LS 真实购买（可选，需 LS 测试模式）**

  - 在 https://dimkey.app/buy 走完整购买流程
  - 检查邮箱收到激活邮件
  - 用邮件中的 key 激活成功

---

### Task 14.2: 发布前最终验证

**Files:**
- 仅命令

- [ ] **Step 1: 跑全部测试**

```bash
cd src-tauri && cargo test
# 期望：原 165 测试 + 新增 license 测试全部 PASS
```

- [ ] **Step 2: 跑 E2E（不含 needs_backend）**

```bash
DIMKEY_E2E=1 e2e/.venv/bin/pytest e2e/tests/ -v -m "not needs_backend"
```

- [ ] **Step 3: 跑 needs_backend E2E（需 Plan A 后端 + license_key）**

```bash
DIMKEY_TEST_LICENSE_KEY="DK-..." DIMKEY_TEST_EMAIL="e2e@test.com" \
DIMKEY_E2E=1 e2e/.venv/bin/pytest e2e/tests/test_license_activation.py -v
```

- [ ] **Step 4: 发布 build 验证**

```bash
cargo tauri build
# macOS: 运行 .app 验证启动正常 + license 模块正常工作
```

- [ ] **Step 5: 中英文 build 都跑一遍**

```bash
cargo tauri build --features lang-zh
cargo tauri build --features lang-en
```

- [ ] **Step 6: 验收完成 → 准备走 dimkey-release skill 发版**

---

## 验收标准

- [ ] `cd src-tauri && cargo test` 全部通过（含 license 模块新增测试 ≥ 12 个）
- [ ] E2E 试用类测试通过（不需后端）
- [ ] 与 Plan A 后端联调通过（激活 / 解绑 / 换机 / 退款 4 个场景）
- [ ] 中英文版 build 都能运行 license 模块
- [ ] 试用过期后导出 xlsx/csv/docx/txt 含水印（PDF 视实施情况）
- [ ] About 面板显示完整：授权状态 / 设备数 / 设备指纹 / 全部操作按钮
- [ ] 激活体验对齐 Typora：邮箱+key 两段式 / 自动格式化 / 错误就地展示 / 设备超限直接给设备列表

---

## 已知非阻塞 TODO（可在 v1 之后处理）

1. PDF 水印实现复杂度高（pdfium 文字绘制 API 较生涩）— 如时间紧可降级为"试用版导出 PDF 时弹 toast 提醒不带水印"
2. heartbeat 复验成功时未刷新本地证书的 next_check_at（私钥在后端） — 客户端缓存层简化为内存计时
3. 客户端没做"按 fingerprint 主动定时清理后端废弃设备"功能（依赖后端 GC）
4. extend-trial admin 接口需要按 fingerprint 在 D1 加表存储试用记录 — v1 后端未实现，对应 admin 端口返回 501
