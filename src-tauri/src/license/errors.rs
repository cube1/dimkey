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
