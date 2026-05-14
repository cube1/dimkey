// src-tauri/src/license/errors.rs
use serde::Serialize;

/// 许可证错误码 — 与后端 spec §9.1 对齐
/// 序列化时 `code` 字段对应前端 i18n key
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "code", content = "data", rename_all = "SCREAMING_SNAKE_CASE")]
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unit_variant_serializes_to_screaming_snake() {
        let v = serde_json::to_value(LicenseError::InvalidLicense).unwrap();
        assert_eq!(v, json!({ "code": "INVALID_LICENSE" }));
    }

    #[test]
    fn struct_variant_serializes_with_data() {
        let v = serde_json::to_value(LicenseError::LicenseRevoked { reason: "refund".into() }).unwrap();
        assert_eq!(v, json!({ "code": "LICENSE_REVOKED", "data": { "reason": "refund" } }));
    }

    #[test]
    fn device_limit_reached_serializes_correctly() {
        let v = serde_json::to_value(LicenseError::DeviceLimitReached {
            devices: json!([]), max: 3
        }).unwrap();
        assert_eq!(v, json!({
            "code": "DEVICE_LIMIT_REACHED",
            "data": { "devices": [], "max": 3 }
        }));
    }

    #[test]
    fn server_error_serializes_correctly() {
        let v = serde_json::to_value(LicenseError::ServerError {
            code: "X".into(), message: "boom".into(),
        }).unwrap();
        assert_eq!(v, json!({
            "code": "SERVER_ERROR",
            "data": { "code": "X", "message": "boom" }
        }));
    }
}
