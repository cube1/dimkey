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

/// 把后端错误码（SCREAMING_SNAKE_CASE）映射为 LicenseError 变体。
/// 未知 code 兜底为 ServerError，保留原始 code+message 便于排查。
pub fn map_err_code(code: &str, message: &str, data: &Option<Value>) -> LicenseError {
    match code {
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
        "RATE_LIMITED" => LicenseError::RateLimited,
        "EMAIL_FORMAT_INVALID" => LicenseError::EmailFormatInvalid,
        "KEY_FORMAT_INVALID" => LicenseError::KeyFormatInvalid,
        other => LicenseError::ServerError {
            code: other.into(),
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

    #[test]
    fn api_base_default_is_dimkey_app() {
        std::env::remove_var("DIMKEY_API_BASE");
        assert_eq!(api_base(), "https://dimkey.app/api/v1");
    }

    #[test]
    fn api_base_env_override_works() {
        std::env::set_var("DIMKEY_API_BASE", "http://localhost:8788/api/v1");
        assert_eq!(api_base(), "http://localhost:8788/api/v1");
        std::env::remove_var("DIMKEY_API_BASE");
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
}
