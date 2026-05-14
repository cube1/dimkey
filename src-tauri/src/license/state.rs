// src-tauri/src/license/state.rs
//
// LicenseManager — 客户端状态机的"大脑"。
// 聚合 fingerprint / storage / trial / certificate / api_client，
// 对外暴露 boot / try_activate / deactivate_local / current 等高阶 API。

use crate::license::api_client::{self, ActivateData};
use crate::license::certificate::{self, CertError, LicensePayload};
use crate::license::errors::LicenseError;
use crate::license::storage::TrialStore;
use crate::license::trial::{self, TrialStatus};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// 客户端 LicenseState — spec §4.4 状态机
/// 序列化时 kind 字段输出 PascalCase（前端 Zustand store 直接 match）
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum LicenseState {
    Trial {
        days_remaining: u32,
    },
    TrialExpired,
    Activated {
        email: String,
        plan: String,
        max_devices: u32,
        active_devices: u32,
        device_id: String,
        license_id: String,
        fingerprint_mismatch: bool,
    },
    GraceMode {
        email: String,
        days_until_block: i64,
    },
    Revoked {
        reason: String,
    },
    Unknown,
}

pub struct LicenseManager {
    state: RwLock<LicenseState>,
    trial_stores: Vec<Box<dyn TrialStore>>,
    config_dir: PathBuf,
    machine_fp: String,
}

impl LicenseManager {
    pub fn new(
        trial_stores: Vec<Box<dyn TrialStore>>,
        config_dir: PathBuf,
        machine_fp: String,
    ) -> Self {
        Self {
            state: RwLock::new(LicenseState::Unknown),
            trial_stores,
            config_dir,
            machine_fp,
        }
    }

    pub fn current(&self) -> LicenseState {
        self.state.read().unwrap().clone()
    }

    pub fn machine_fp(&self) -> &str {
        &self.machine_fp
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// 启动时调一次：读证书 / 验签 / 算指纹 / 决定状态
    pub fn boot(&self) -> LicenseState {
        let now = Utc::now();
        let new_state = match certificate::read_certificate(&self.config_dir) {
            Ok(payload) => self.eval_with_certificate(&payload, now),
            Err(CertError::Missing) => self.eval_trial_only(now),
            Err(_e) => {
                // 证书损坏 → 删除并回退试用
                let _ = certificate::delete_certificate(&self.config_dir);
                self.eval_trial_only(now)
            }
        };
        *self.state.write().unwrap() = new_state.clone();
        new_state
    }

    fn eval_with_certificate(
        &self,
        payload: &LicensePayload,
        now: DateTime<Utc>,
    ) -> LicenseState {
        let mismatch = payload.fingerprint != self.machine_fp;
        if mismatch {
            // 不删证书，回退 trial 判定
            // 前端通过 license_get_fingerprint_mismatch_hint command 单独取 hint 字符串
            return self.eval_trial_only(now);
        }
        LicenseState::Activated {
            email: payload.email.clone(),
            plan: payload.plan.clone(),
            max_devices: 3, // 后续 heartbeat 时更新（v1 用默认值）
            active_devices: 1,
            device_id: payload.device_id.clone(),
            license_id: payload.license_id.clone(),
            fingerprint_mismatch: false,
        }
    }

    fn eval_trial_only(&self, now: DateTime<Utc>) -> LicenseState {
        let refs: Vec<&dyn TrialStore> =
            self.trial_stores.iter().map(|b| b.as_ref()).collect();
        let info = trial::touch(&refs, &self.machine_fp, now);
        match info.status {
            TrialStatus::Active { days_remaining, .. } => {
                LicenseState::Trial { days_remaining }
            }
            TrialStatus::Expired { .. } => LicenseState::TrialExpired,
        }
    }

    /// 异步调用后端激活，成功后落证书，重 boot 状态
    pub async fn try_activate(
        &self,
        license_key: &str,
        email: &str,
        machine_label: &str,
        os: &str,
        flavor: &str,
        app_version: &str,
    ) -> Result<ActivateData, LicenseError> {
        let body = api_client::ActivateBody {
            license_key,
            email,
            fingerprint: &self.machine_fp,
            machine_label,
            os,
            flavor,
            app_version,
        };
        let result = api_client::activate(&body).await?;
        certificate::write_certificate_envelope(&self.config_dir, &result.license_certificate)
            .map_err(|e| LicenseError::ServerError {
                code: "WRITE_CERT".into(),
                message: e.to_string(),
            })?;
        // 立刻 boot 一次更新 state
        self.boot();
        // 用真实激活返回数据更新 active_devices / max_devices
        if let LicenseState::Activated {
            ref mut max_devices,
            ref mut active_devices,
            ..
        } = *self.state.write().unwrap()
        {
            *max_devices = result.device_summary.max_devices;
            *active_devices = result.device_summary.active_count;
        }
        Ok(result)
    }

    /// 异步解绑当前设备：调后端 + 删本地证书 + 重 boot
    pub async fn deactivate_local(&self) -> Result<(), LicenseError> {
        let payload = certificate::read_certificate(&self.config_dir)
            .map_err(|_| LicenseError::InvalidLicense)?;
        let body = api_client::DeactivateBody {
            license_key: &payload.license_key,
            email: &payload.email,
            device_id: Some(&payload.device_id),
            fingerprint: None,
        };
        api_client::deactivate(&body).await?;
        let _ = certificate::delete_certificate(&self.config_dir);
        self.boot();
        Ok(())
    }

    /// 读当前证书 payload（如果 .lic 存在且签名正确）
    pub fn current_payload(&self) -> Option<LicensePayload> {
        certificate::read_certificate(&self.config_dir).ok()
    }

    /// 强制设置状态（heartbeat 任务收到 revoked 后用）
    pub fn set_state(&self, new_state: LicenseState) {
        *self.state.write().unwrap() = new_state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::storage::ConfigDirStore;
    use tempfile::tempdir;

    fn build_manager(d: &Path, fp: &str) -> LicenseManager {
        let stores: Vec<Box<dyn TrialStore>> = vec![Box::new(ConfigDirStore {
            path: d.join("trial.json"),
        })];
        LicenseManager::new(stores, d.to_path_buf(), fp.into())
    }

    #[test]
    fn boot_with_no_certificate_returns_trial_30_days() {
        let d = tempdir().unwrap();
        let m = build_manager(d.path(), "test-fp");
        let state = m.boot();
        match state {
            LicenseState::Trial { days_remaining } => {
                assert!(days_remaining >= 29 && days_remaining <= 30);
            }
            _ => panic!("expected Trial, got {:?}", state),
        }
    }

    #[test]
    fn current_returns_unknown_before_boot() {
        let d = tempdir().unwrap();
        let m = build_manager(d.path(), "test-fp");
        assert!(matches!(m.current(), LicenseState::Unknown));
    }

    #[test]
    fn current_after_boot_matches_returned_state() {
        let d = tempdir().unwrap();
        let m = build_manager(d.path(), "test-fp");
        let returned = m.boot();
        let current = m.current();
        assert!(matches!(returned, LicenseState::Trial { .. }));
        assert!(matches!(current, LicenseState::Trial { .. }));
    }

    #[test]
    fn boot_with_corrupt_certificate_deletes_and_falls_back_to_trial() {
        let d = tempdir().unwrap();
        std::fs::write(d.path().join("license.lic"), "not json").unwrap();
        let m = build_manager(d.path(), "test-fp");
        let state = m.boot();
        assert!(matches!(state, LicenseState::Trial { .. }));
        // 损坏证书已被删除
        assert!(!d.path().join("license.lic").exists());
    }

    #[test]
    fn machine_fp_getter_returns_provided_value() {
        let d = tempdir().unwrap();
        let m = build_manager(d.path(), "fp-xyz-123");
        assert_eq!(m.machine_fp(), "fp-xyz-123");
    }

    #[test]
    fn config_dir_getter_returns_provided_path() {
        let d = tempdir().unwrap();
        let m = build_manager(d.path(), "fp");
        assert_eq!(m.config_dir(), d.path());
    }

    #[test]
    fn set_state_updates_current() {
        let d = tempdir().unwrap();
        let m = build_manager(d.path(), "fp");
        m.set_state(LicenseState::Revoked {
            reason: "test".into(),
        });
        match m.current() {
            LicenseState::Revoked { reason } => assert_eq!(reason, "test"),
            _ => panic!("expected Revoked"),
        }
    }

    #[test]
    fn license_state_serializes_with_pascal_case_kind() {
        // 锁定与前端 Zustand store 的契约
        let s = LicenseState::Trial { days_remaining: 30 };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["kind"], "Trial");
        assert_eq!(v["days_remaining"], 30);

        let s = LicenseState::TrialExpired;
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["kind"], "TrialExpired");

        let s = LicenseState::Revoked {
            reason: "x".into(),
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["kind"], "Revoked");
        assert_eq!(v["reason"], "x");
    }
}
