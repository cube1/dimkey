// src-tauri/src/license/certificate.rs
//
// 本地 .lic 证书读写 + Ed25519 验签。
// 公钥编译期烧入，不从配置/网络读，防替换攻击。

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::Path;

// ⚠️ TODO(plan-a): 必须用 dimkey-web 仓库 scripts/gen-ed25519-keypair.ts 输出的
// pub array 替换以下 32 个 0。后端私钥已存为 Workers Secret ED25519_PRIVATE_KEY。
// 在 Plan A 部署前，本占位会让所有 verify 调用失败 (SignatureInvalid)，预期行为：
// 客户端无法激活 → 走 Trial 分支。Task 4.2 (集成测试) 同样依赖此真公钥。
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

/// 已知公钥列表（按 key_version 索引）
/// 当前只有 v1；未来轮换 v2 时追加 (2, &PUBKEY_V2)
const KNOWN_PUBKEYS: &[(u32, &[u8; 32])] = &[
    (1, &PUBKEY_V1),
];

pub fn read_certificate(config_dir: &Path) -> Result<LicensePayload, CertError> {
    let path = config_dir.join(CERTIFICATE_FILE);
    if !path.exists() {
        return Err(CertError::Missing);
    }
    let raw = std::fs::read_to_string(&path)?;
    let env: CertEnvelope =
        serde_json::from_str(&raw).map_err(|e| CertError::EnvelopeParse(e.to_string()))?;
    if env.v != 1 {
        return Err(CertError::UnsupportedVersion(env.v));
    }

    let payload_bytes = B64.decode(&env.payload_b64).map_err(|_| CertError::PayloadB64)?;
    let sig_bytes = B64.decode(&env.sig_b64).map_err(|_| CertError::SigB64)?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|_| CertError::SignatureInvalid)?;

    // Verify-first：用所有已知公钥试一遍。任一成功才继续。
    // 这样攻击者无法通过 JSON 解析失败 / UnsupportedKeyVersion 等错误码探测客户端状态。
    let mut verified_key_version: Option<u32> = None;
    for (kv, pk_bytes) in KNOWN_PUBKEYS {
        if let Ok(pk) = VerifyingKey::from_bytes(pk_bytes) {
            if pk.verify(&payload_bytes, &sig).is_ok() {
                verified_key_version = Some(*kv);
                break;
            }
        }
    }
    let verified_kv = verified_key_version.ok_or(CertError::SignatureInvalid)?;

    // 此时签名已通过验证，可安全反序列化 payload
    let payload: LicensePayload = serde_json::from_slice(&payload_bytes)
        .map_err(|e| CertError::PayloadJson(e.to_string()))?;

    // 防御性：payload 内的 key_version 必须与验证通过的公钥版本一致
    // （理论上只要签名 OK 就一定一致，因为 payload 字节包含 key_version；
    //  但加一道断言防 payload 字段被攻击者操纵）
    if payload.key_version != verified_kv {
        return Err(CertError::UnsupportedKeyVersion(payload.key_version));
    }

    Ok(payload)
}

pub fn write_certificate_envelope(config_dir: &Path, env: &CertEnvelope) -> Result<(), CertError> {
    std::fs::create_dir_all(config_dir)?;
    let path = config_dir.join(CERTIFICATE_FILE);
    let s = serde_json::to_string_pretty(env)
        .map_err(|e| CertError::EnvelopeParse(e.to_string()))?;
    std::fs::write(&path, s)?;
    Ok(())
}

pub fn delete_certificate(config_dir: &Path) -> std::io::Result<()> {
    let path = config_dir.join(CERTIFICATE_FILE);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
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
            license_id: "uuid".into(),
            license_key: "DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE".into(),
            email: "u@x.com".into(),
            plan: "personal".into(),
            device_id: "d".into(),
            fingerprint: "fp".into(),
            issued_at: "2026-05-14T10:00:00Z".into(),
            expires_at: None,
            next_check_at: "2026-05-21T10:00:00Z".into(),
            max_grace_until: "2026-05-28T10:00:00Z".into(),
            key_version: 1,
        }
    }

    #[test]
    fn read_returns_missing_when_no_file() {
        let d = tempdir().unwrap();
        assert!(matches!(read_certificate(d.path()), Err(CertError::Missing)));
    }

    #[test]
    fn write_then_read_envelope_file_succeeds() {
        // 测试 envelope 读写本身（不验签，因为占位 PUBKEY 总会失败）
        let d = tempdir().unwrap();
        let env = CertEnvelope {
            v: 1,
            payload_b64: "abc".into(),
            sig_b64: "def".into(),
        };
        write_certificate_envelope(d.path(), &env).unwrap();
        let path = d.path().join(CERTIFICATE_FILE);
        assert!(path.exists());
        let raw = std::fs::read_to_string(&path).unwrap();
        let loaded: CertEnvelope = serde_json::from_str(&raw).unwrap();
        assert_eq!(loaded.payload_b64, "abc");
    }

    #[test]
    fn write_creates_parent_dir() {
        let d = tempdir().unwrap();
        let nested = d.path().join("nested/sub");
        let env = CertEnvelope {
            v: 1,
            payload_b64: "a".into(),
            sig_b64: "b".into(),
        };
        write_certificate_envelope(&nested, &env).unwrap();
        assert!(nested.join(CERTIFICATE_FILE).exists());
    }

    #[test]
    fn read_returns_envelope_parse_error_when_corrupt() {
        let d = tempdir().unwrap();
        std::fs::write(d.path().join(CERTIFICATE_FILE), "not json").unwrap();
        let r = read_certificate(d.path());
        assert!(matches!(r, Err(CertError::EnvelopeParse(_))));
    }

    #[test]
    fn read_returns_unsupported_version_for_v2_envelope() {
        let d = tempdir().unwrap();
        std::fs::write(
            d.path().join(CERTIFICATE_FILE),
            r#"{"v":2,"payload_b64":"a","sig_b64":"b"}"#,
        )
        .unwrap();
        let r = read_certificate(d.path());
        assert!(matches!(r, Err(CertError::UnsupportedVersion(2))));
    }

    #[test]
    fn delete_certificate_removes_file_or_no_op() {
        let d = tempdir().unwrap();
        // 文件不存在时不报错
        assert!(delete_certificate(d.path()).is_ok());
        // 创建文件后删除
        std::fs::write(d.path().join(CERTIFICATE_FILE), "x").unwrap();
        assert!(delete_certificate(d.path()).is_ok());
        assert!(!d.path().join(CERTIFICATE_FILE).exists());
    }

    /// Ed25519 sign + verify roundtrip 单独验证 — 不通过 read_certificate（因为 PUBKEY_V1 是占位）
    #[test]
    fn ed25519_sign_verify_roundtrip_works() {
        let mut rng = OsRng;
        let sk = SigningKey::generate(&mut rng);
        let pk = sk.verifying_key();
        let payload = build_test_payload();
        let payload_bytes = serde_json::to_vec(&payload).unwrap();
        let sig: Signature = sk.sign(&payload_bytes);
        assert!(pk.verify(&payload_bytes, &sig).is_ok());
    }

    #[test]
    fn ed25519_verify_rejects_tampered_payload() {
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

    #[test]
    fn ed25519_verify_rejects_wrong_pubkey() {
        let mut rng = OsRng;
        let sk1 = SigningKey::generate(&mut rng);
        let sk2 = SigningKey::generate(&mut rng);
        let pk2 = sk2.verifying_key();
        let payload = build_test_payload();
        let payload_bytes = serde_json::to_vec(&payload).unwrap();
        let sig: Signature = sk1.sign(&payload_bytes);
        assert!(pk2.verify(&payload_bytes, &sig).is_err());
    }

    #[test]
    fn read_certificate_with_placeholder_pubkey_always_fails_signature() {
        // 占位 PUBKEY_V1 是全 0，无法对应任何真实私钥，所以任何包含真实签名的 .lic
        // 走到 verify 时都会失败。这个测试锁定该行为，便于 Plan A 真公钥烧入后改为通过 case。
        use ed25519_dalek::{Signer, SigningKey};
        use rand::rngs::OsRng;
        let d = tempdir().unwrap();
        let mut rng = OsRng;
        let sk = SigningKey::generate(&mut rng);
        let payload = build_test_payload();
        let payload_bytes = serde_json::to_vec(&payload).unwrap();
        let sig: Signature = sk.sign(&payload_bytes);
        let env = CertEnvelope {
            v: 1,
            payload_b64: B64.encode(&payload_bytes),
            sig_b64: B64.encode(sig.to_bytes()),
        };
        write_certificate_envelope(d.path(), &env).unwrap();
        let r = read_certificate(d.path());
        assert!(matches!(r, Err(CertError::SignatureInvalid)),
            "expected SignatureInvalid (because PUBKEY_V1 placeholder won't match any real key), got: {:?}", r);
    }
}
