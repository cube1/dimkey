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
        None => return, // 未激活，无需 ping
    };

    let next_check: DateTime<Utc> = parse_iso(&payload.next_check_at);
    let now = Utc::now();
    if now < next_check {
        return; // 还没到复验时间
    }

    let body = api_client::HeartbeatBody {
        license_id: &payload.license_id,
        device_id: &payload.device_id,
        fingerprint: &payload.fingerprint,
    };
    match api_client::heartbeat(&body).await {
        Ok(data) => {
            if data.status == "revoked" {
                // 删证书，重 boot
                let _ = certificate::delete_certificate(manager.config_dir());
                manager.boot();
                let _ = app.emit("license:state-changed", manager.current());
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
            let max_grace = parse_iso(&payload.max_grace_until);
            if Utc::now() > max_grace {
                let days_over = (Utc::now() - max_grace).num_days();
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

fn parse_iso(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}
