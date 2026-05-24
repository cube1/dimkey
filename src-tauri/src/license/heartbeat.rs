// src-tauri/src/license/heartbeat.rs
//
// 后台周期性 heartbeat 任务：
// 启动后立即 ping 一次（如证书已过 next_check_at），之后每 24h 检查一次。
// heartbeat 失败按宽限期推进；达到 max_grace_until 后客户端转 GraceMode（仍可用，仅显示横幅）。
// 收到 status=revoked → 客户端转 Revoked 态（删证书 + 重 boot + emit）。

use crate::license::api_client;
use crate::license::certificate;
use crate::license::state::{LicenseManager, LicenseState};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;

const POLL_INTERVAL_SECS: u64 = 24 * 60 * 60; // 24h

pub fn spawn(app: tauri::AppHandle, manager: Arc<LicenseManager>) {
    // 用 Tauri 自带的 async_runtime，避免 setup 钩子非 Tokio 上下文导致 panic。
    // sleep 仍用 tokio::time::sleep —— Tauri v2 默认 tokio 后端，spawn 出的
    // future 必然跑在 tokio runtime 上下文，无需也无法切换（tauri::async_runtime
    // 不暴露 sleep API）
    tauri::async_runtime::spawn(async move {
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
        None => return, // 未激活，无需 ping
    };

    let now = Utc::now();

    // next_check_at 解析失败 → 视为已过期，立即 ping（不静默忽略）
    let next_check = parse_iso(&payload.next_check_at);
    if let Some(nc) = next_check {
        if now < nc {
            return; // 还没到复验时间
        }
    } else {
        eprintln!(
            "[license::heartbeat] invalid next_check_at: {:?}",
            payload.next_check_at
        );
    }

    let body = api_client::HeartbeatBody {
        license_id: &payload.license_id,
        device_id: &payload.device_id,
        fingerprint: &payload.fingerprint,
    };
    match api_client::heartbeat(&body).await {
        Ok(data) => {
            if data.status == "revoked" {
                // 删证书，重 boot，强制进入 Revoked 态（spec §4.4 状态机要求）
                let _ = certificate::delete_certificate(manager.config_dir());
                manager.boot();
                let revoked = LicenseState::Revoked {
                    reason: "服务端吊销".to_string(),
                };
                manager.set_state(revoked.clone());
                let _ = app.emit("license:state-changed", revoked);
            } else {
                // active：emit 事件让前端知道复验通过（不持久化 next_check_at —— 私钥在后端，
                // 客户端用内存中的下次检查时间隐式推进，下次启动会重新基于证书的 next_check_at 计算）
                let _ = app.emit(
                    "license:heartbeat-ok",
                    serde_json::json!({ "next_check_at": data.next_check_at }),
                );
            }
        }
        Err(_) => {
            // 网络失败：检查是否超过 max_grace_until
            // 解析失败 → 视为很久前 → 立即进入 GraceMode（不静默忽略）
            let max_grace_parsed = parse_iso(&payload.max_grace_until);
            let exceeded_grace = match max_grace_parsed {
                Some(max_grace) => now > max_grace,
                None => {
                    eprintln!(
                        "[license::heartbeat] invalid max_grace_until: {:?}, treating as exceeded",
                        payload.max_grace_until
                    );
                    true
                }
            };
            if exceeded_grace {
                // ceiling 除法：超过宽限即使 1 秒也算 1 天，避免 num_days() 截断
                // 到 0 让前端拿到的 days_until_block=0 与"还 active"语义混淆
                let days_over = match max_grace_parsed {
                    Some(max_grace) => {
                        let secs = (now - max_grace).num_seconds().max(0);
                        (secs + 86_400 - 1) / 86_400
                    }
                    None => 0,
                };
                let new_state = LicenseState::GraceMode {
                    email: payload.email.clone(),
                    days_until_block: -days_over,
                };
                manager.set_state(new_state.clone());
                let _ = app.emit("license:state-changed", new_state);
            }
            // 未超过宽限期：静默重试，下个周期再 ping
        }
    }
}

fn parse_iso(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_iso_valid_rfc3339() {
        let r = parse_iso("2026-05-14T10:00:00Z");
        assert!(r.is_some());
    }

    #[test]
    fn parse_iso_invalid_returns_none() {
        assert!(parse_iso("not-rfc3339").is_none());
        assert!(parse_iso("").is_none());
        assert!(parse_iso("2026-13-50T99:99:99Z").is_none());
    }
}
