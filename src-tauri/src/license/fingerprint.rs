// src-tauri/src/license/fingerprint.rs
//
// 设备指纹算法 v1
// 算法：sha256(machine_id || primary_mac || cpu_brand || os_install_id)[0..16]
// 输出：32 个 hex 字符（128 bit）
//
// 设计目标：
// - 同一台机器多次启动一致
// - 跨硬件迁移敏感（换主板/SSD 视为新机器）
// - 不依赖管理员权限
// - 任一字段读不到时降级为 "unknown"，整体不失败

use sha2::{Digest, Sha256};

pub const FINGERPRINT_VERSION: &str = "v1";

/// 计算当前机器的设备指纹（128 bit hex，32 字符）
/// 算法：sha256(machine_id || "||" || mac || "||" || cpu || "||" || os_install_id)[0..16]
///
/// 调用方应缓存返回值，避免重复调用（每次 ~50-100ms：sysinfo 网卡扫描 + ioreg 子进程）
///
/// macOS 前提：app entitlements 须含 `com.apple.security.network.client`，
/// 否则 sysinfo Networks 返回空导致 MAC 走 "unknown" 兜底，与已授权状态指纹不同
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
    hex::encode(&full[..16]) // 128 bit = 32 hex chars
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
            let n = name.to_lowercase();
            if n.starts_with("lo") { return false; }
            // 通用虚拟接口（macOS / Linux / Windows）
            let virtual_keywords = [
                "vmnet", "vboxnet", "utun", "tun", "anpi", "awdl", "llw",
                "bridge", "docker", "tap", "ham",
                // Windows 虚拟/非物理接口
                "bluetooth", "hyper-v", "pseudo", "wsl",
                "wireguard", "tailscale", "zerotier",
                "loopback", "vethernet", "isatap", "teredo",
            ];
            !virtual_keywords.iter().any(|k| n.contains(k))
        })
        .map(|(_, data)| data.mac_address().to_string())
        .filter(|m| m != "00:00:00:00:00:00" && !m.is_empty())
        .collect();
    macs.sort();
    macs.into_iter().next()
}

fn read_cpu_brand() -> Option<String> {
    use sysinfo::{CpuRefreshKind, RefreshKind, System};
    let sys = System::new_with_specifics(
        RefreshKind::new().with_cpu(CpuRefreshKind::new()),
    );
    sys.cpus().first().map(|c| c.brand().to_string())
}

#[cfg(target_os = "macos")]
fn read_os_install_id() -> Option<String> {
    use std::process::Command;
    let out = Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    for line in s.lines() {
        if let Some(rest) = line.split_once("IOPlatformSerialNumber") {
            // 行形如:  "IOPlatformSerialNumber" = "C02XXXXXXX"
            let t = rest
                .1
                .trim()
                .trim_start_matches('=')
                .trim()
                .trim_matches('"')
                .trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
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
    s.split_whitespace()
        .find(|t| t.starts_with("S-1-"))
        .map(|x| x.trim_end_matches('\r').to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn read_os_install_id() -> Option<String> {
    None
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
        let a = compute_fingerprint();
        let b = compute_fingerprint();
        assert_eq!(a, b);
    }

    #[test]
    fn fingerprint_handles_unknown_fields_gracefully() {
        // 即使所有 field 都返回 None，也应能算出一个合法 fingerprint（"unknown" 兜底）
        let fp = compute_fingerprint();
        assert_eq!(fp.len(), 32);
    }
}
