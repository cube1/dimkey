// src-tauri/src/license/api_client.rs
//
// 调用 dimkey.app/api/v1/* 的 HTTP 客户端。
// - 5 个接口：activate / deactivate / heartbeat / devices/list / recover
// - 业务错误（HTTP 200 + ok:false）映射为 LicenseError
// - 网络/超时映射为 LicenseError::NetworkUnavailable
// - 默认 base URL https://dimkey.app/api/v1，可通过 DIMKEY_API_BASE 环境变量覆盖

use crate::license::certificate::CertEnvelope;
use crate::license::errors::LicenseError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

const DEFAULT_API_BASE: &str = "https://dimkey.app/api/v1";
const REQUEST_TIMEOUT_SECS: u64 = 15;

pub fn api_base() -> String {
    std::env::var("DIMKEY_API_BASE").unwrap_or_else(|_| DEFAULT_API_BASE.into())
}

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent(format!("Dimkey/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client build")
    })
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    ok: bool,
    code: Option<String>,
    /// dimkey-site server 用 "msg"；保留 "message" alias 兼容 spec 原始定义
    #[serde(alias = "msg")]
    message: Option<String>,
    data: Option<Value>,
}

/// 把后端错误码（SCREAMING_SNAKE_CASE）映射为 LicenseError 变体。
/// 容忍 dimkey-site server 的 ERR_* 前缀（如 ERR_RATE_LIMIT），先剥前缀再 match。
/// 未知 code 兜底为 ServerError，保留原始 code+message 便于排查。
pub fn map_err_code(code: &str, message: &str, data: &Option<Value>) -> LicenseError {
    // 服务端可能返回 "ERR_RATE_LIMIT" 或 "RATE_LIMITED"，归一化后 match
    let normalized = code.strip_prefix("ERR_").unwrap_or(code);
    match normalized {
        "INVALID_LICENSE" => LicenseError::InvalidLicense,
        "LICENSE_REVOKED" => LicenseError::LicenseRevoked { reason: message.into() },
        "LICENSE_EXPIRED" => LicenseError::LicenseExpired,
        "DEVICE_LIMIT_REACHED" => {
            let devices = data
                .as_ref()
                .and_then(|d| d.get("devices"))
                .cloned()
                .unwrap_or(Value::Null);
            // 优先 max_devices（后端 spec），兜底 max（兼容老接口），最后兜底 3
            let max = data
                .as_ref()
                .and_then(|d| d.get("max_devices").or_else(|| d.get("max")))
                .and_then(|v| v.as_u64())
                .unwrap_or(3) as u32;
            LicenseError::DeviceLimitReached { devices, max }
        }
        "DEVICE_NOT_FOUND" => LicenseError::DeviceNotFound,
        "FINGERPRINT_MISMATCH" => LicenseError::FingerprintMismatch,
        "SIGNATURE_INVALID" => LicenseError::SignatureInvalid,
        // server 用 "RATE_LIMIT"（单数），spec 原 "RATE_LIMITED"（过去式），都接住
        "RATE_LIMIT" | "RATE_LIMITED" => LicenseError::RateLimited,
        "EMAIL_FORMAT_INVALID" => LicenseError::EmailFormatInvalid,
        "KEY_FORMAT_INVALID" => LicenseError::KeyFormatInvalid,
        _ => LicenseError::ServerError {
            code: code.into(),
            message: message.into(),
        },
    }
}

/// 通用 POST：网络/JSON 错误一律映射 NetworkUnavailable；
/// 业务错误（ok:false）走 map_err_code。
async fn post_json<T: Serialize>(path: &str, body: &T) -> Result<Value, LicenseError> {
    let url = format!("{}{}", api_base(), path);
    let res = client()
        .post(&url)
        .json(body)
        .send()
        .await
        .map_err(|_| LicenseError::NetworkUnavailable)?;
    let api: ApiResponse = res
        .json()
        .await
        .map_err(|_| LicenseError::NetworkUnavailable)?;
    if api.ok {
        Ok(api.data.unwrap_or(Value::Null))
    } else {
        let code = api.code.unwrap_or_else(|| "SERVER_ERROR".into());
        let msg = api.message.unwrap_or_else(|| "服务异常".into());
        Err(map_err_code(&code, &msg, &api.data))
    }
}

// ───────────────────────── /activate ─────────────────────────

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
    serde_json::from_value(v).map_err(|e| LicenseError::ServerError {
        code: "PARSE".into(),
        message: e.to_string(),
    })
}

// ───────────────────────── /deactivate ─────────────────────────

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

// ───────────────────────── /heartbeat ─────────────────────────

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
    serde_json::from_value(v).map_err(|e| LicenseError::ServerError {
        code: "PARSE".into(),
        message: e.to_string(),
    })
}

// ───────────────────────── /devices/list ─────────────────────────

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
    serde_json::from_value(v).map_err(|e| LicenseError::ServerError {
        code: "PARSE".into(),
        message: e.to_string(),
    })
}

// ───────────────────────── /recover ─────────────────────────

#[derive(Debug, Serialize)]
pub struct RecoverBody<'a> {
    pub email: &'a str,
}

pub async fn recover(body: &RecoverBody<'_>) -> Result<(), LicenseError> {
    post_json("/recover", body).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn map_err_code_invalid_license() {
        let e = map_err_code("INVALID_LICENSE", "x", &None);
        assert!(matches!(e, LicenseError::InvalidLicense));
    }

    #[test]
    fn map_err_code_license_revoked_carries_message() {
        let e = map_err_code("LICENSE_REVOKED", "退款", &None);
        match e {
            LicenseError::LicenseRevoked { reason } => assert_eq!(reason, "退款"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn map_err_code_device_limit_reached_extracts_devices_and_max() {
        let data = Some(json!({
            "devices": [{ "device_id": "d1" }],
            "max_devices": 5
        }));
        let e = map_err_code("DEVICE_LIMIT_REACHED", "x", &data);
        match e {
            LicenseError::DeviceLimitReached { devices, max } => {
                assert_eq!(max, 5);
                assert_eq!(devices.as_array().unwrap().len(), 1);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn map_err_code_device_limit_falls_back_to_3_when_max_missing() {
        let data = Some(json!({ "devices": [] }));
        let e = map_err_code("DEVICE_LIMIT_REACHED", "x", &data);
        match e {
            LicenseError::DeviceLimitReached { max, .. } => assert_eq!(max, 3),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn map_err_code_unknown_falls_into_server_error() {
        let e = map_err_code("WEIRD_NEW_CODE", "boom", &None);
        match e {
            LicenseError::ServerError { code, message } => {
                assert_eq!(code, "WEIRD_NEW_CODE");
                assert_eq!(message, "boom");
            }
            _ => panic!("wrong variant"),
        }
    }

    /// dimkey-site server 返回 ERR_RATE_LIMIT，spec 原命名是 RATE_LIMITED — 都要接住
    #[test]
    fn map_err_code_accepts_err_prefix_and_singular_rate_limit() {
        assert!(matches!(
            map_err_code("ERR_RATE_LIMIT", "x", &None),
            LicenseError::RateLimited
        ));
        assert!(matches!(
            map_err_code("RATE_LIMIT", "x", &None),
            LicenseError::RateLimited
        ));
        assert!(matches!(
            map_err_code("RATE_LIMITED", "x", &None),
            LicenseError::RateLimited
        ));
        // ERR_ 前缀其他业务码也应剥前缀
        assert!(matches!(
            map_err_code("ERR_INVALID_LICENSE", "x", &None),
            LicenseError::InvalidLicense
        ));
    }

    /// 未知码即使带 ERR_ 前缀也应保留原始 code 在 ServerError 中（便于排查）
    #[test]
    fn map_err_code_unknown_with_err_prefix_keeps_original_code() {
        match map_err_code("ERR_SOMETHING_NEW", "boom", &None) {
            LicenseError::ServerError { code, .. } => assert_eq!(code, "ERR_SOMETHING_NEW"),
            _ => panic!("wrong variant"),
        }
    }

    /// server 用 "msg" 字段，client ApiResponse 通过 #[serde(alias)] 兼容
    #[test]
    fn api_response_accepts_msg_alias() {
        let raw = r#"{"ok":false,"code":"ERR_RATE_LIMIT","msg":"请求过于频繁，请稍后再试"}"#;
        let parsed: ApiResponse = serde_json::from_str(raw).expect("parse with msg alias");
        assert!(!parsed.ok);
        assert_eq!(parsed.code.as_deref(), Some("ERR_RATE_LIMIT"));
        assert_eq!(
            parsed.message.as_deref(),
            Some("请求过于频繁，请稍后再试")
        );
    }

    /// "message" 字段也仍然能解析（向后兼容 spec 原始定义）
    #[test]
    fn api_response_still_accepts_message_field() {
        let raw = r#"{"ok":false,"code":"INVALID_LICENSE","message":"bad key"}"#;
        let parsed: ApiResponse = serde_json::from_str(raw).expect("parse with message field");
        assert_eq!(parsed.message.as_deref(), Some("bad key"));
    }

    #[test]
    fn api_base_default_and_env_override() {
        // 保存原始值（如果有），最后恢复，避免污染其他测试
        let original = std::env::var("DIMKEY_API_BASE").ok();

        // 默认值
        std::env::remove_var("DIMKEY_API_BASE");
        assert_eq!(api_base(), "https://dimkey.app/api/v1");

        // env 覆盖
        std::env::set_var("DIMKEY_API_BASE", "http://localhost:8788/api/v1");
        assert_eq!(api_base(), "http://localhost:8788/api/v1");

        // 恢复
        match original {
            Some(v) => std::env::set_var("DIMKEY_API_BASE", v),
            None => std::env::remove_var("DIMKEY_API_BASE"),
        }
    }

    #[test]
    fn rate_limited_maps_correctly() {
        assert!(matches!(
            map_err_code("RATE_LIMITED", "", &None),
            LicenseError::RateLimited
        ));
    }

    #[test]
    fn fingerprint_mismatch_maps_correctly() {
        assert!(matches!(
            map_err_code("FINGERPRINT_MISMATCH", "", &None),
            LicenseError::FingerprintMismatch
        ));
    }

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

        // RAII guard：即使 panic 也恢复原 DIMKEY_API_BASE
        struct EnvGuard {
            key: &'static str,
            original: Option<String>,
        }
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                match &self.original {
                    Some(v) => std::env::set_var(self.key, v),
                    None => std::env::remove_var(self.key),
                }
            }
        }
        let _env_guard = EnvGuard {
            key: "DIMKEY_API_BASE",
            original: std::env::var("DIMKEY_API_BASE").ok(),
        };
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

    }
}
