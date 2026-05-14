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
    pub first_run_at: String, // ISO UTC
    pub last_run_at: String,  // ISO UTC
    pub machine_fp: String,
}

const KEYRING_SERVICE: &str = "com.dimkey.trial";
const KEYRING_USER: &str = "trial-record";

pub trait TrialStore: Send + Sync {
    fn read(&self) -> Option<TrialRecord>;
    fn write(&self, rec: &TrialRecord) -> Result<(), String>;
    fn label(&self) -> &str;
}

pub struct ConfigDirStore {
    pub path: PathBuf,
}
pub struct HiddenFileStore {
    pub path: PathBuf,
}
pub struct KeyringStore;

impl TrialStore for ConfigDirStore {
    fn label(&self) -> &str {
        "config_dir"
    }
    fn read(&self) -> Option<TrialRecord> {
        let s = std::fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&s).ok()
    }
    fn write(&self, rec: &TrialRecord) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let s = serde_json::to_string(rec).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, s).map_err(|e| e.to_string())
    }
}

impl TrialStore for HiddenFileStore {
    fn label(&self) -> &str {
        "hidden_file"
    }
    fn read(&self) -> Option<TrialRecord> {
        let s = std::fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&s).ok()
    }
    fn write(&self, rec: &TrialRecord) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        let s = serde_json::to_string(rec).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, s).map_err(|e| e.to_string())
    }
}

impl TrialStore for KeyringStore {
    fn label(&self) -> &str {
        "keyring"
    }
    fn read(&self) -> Option<TrialRecord> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).ok()?;
        let s = entry.get_password().ok()?;
        serde_json::from_str(&s).ok()
    }
    fn write(&self, rec: &TrialRecord) -> Result<(), String> {
        let entry =
            keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())?;
        let s = serde_json::to_string(rec).map_err(|e| e.to_string())?;
        entry.set_password(&s).map_err(|e| e.to_string())
    }
}

/// 默认的 3 处存储构造器（依赖 Tauri AppHandle 解析 config_dir）
pub fn build_default_stores(config_dir: PathBuf) -> Vec<Box<dyn TrialStore>> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    vec![
        Box::new(ConfigDirStore {
            path: config_dir.join("trial.json"),
        }),
        Box::new(HiddenFileStore {
            path: home.join(".dimkey_state"),
        }),
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
        let store = ConfigDirStore {
            path: d.path().join("trial.json"),
        };
        let rec = TrialRecord {
            version: 1,
            first_run_at: "2026-05-14T00:00:00Z".into(),
            last_run_at: "2026-05-14T00:00:00Z".into(),
            machine_fp: "fp123".into(),
        };
        store.write(&rec).unwrap();
        let loaded = store.read().unwrap();
        assert_eq!(loaded, rec);
    }

    #[test]
    fn config_dir_store_creates_parent_dir() {
        let d = tempdir().unwrap();
        // 使用嵌套不存在的子目录
        let store = ConfigDirStore {
            path: d.path().join("nested/sub/trial.json"),
        };
        let rec = TrialRecord {
            version: 1,
            first_run_at: "2026-05-14T00:00:00Z".into(),
            last_run_at: "2026-05-14T00:00:00Z".into(),
            machine_fp: "fp".into(),
        };
        store.write(&rec).unwrap();
        assert!(d.path().join("nested/sub").exists());
        assert_eq!(store.read().unwrap(), rec);
    }

    #[test]
    fn read_returns_none_when_file_missing() {
        let d = tempdir().unwrap();
        let store = ConfigDirStore {
            path: d.path().join("nonexistent.json"),
        };
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

    #[test]
    fn hidden_file_store_roundtrip() {
        let d = tempdir().unwrap();
        let store = HiddenFileStore {
            path: d.path().join(".dimkey_state"),
        };
        let rec = TrialRecord {
            version: 1,
            first_run_at: "2026-05-14T00:00:00Z".into(),
            last_run_at: "2026-05-14T00:00:00Z".into(),
            machine_fp: "fp".into(),
        };
        store.write(&rec).unwrap();
        assert_eq!(store.read().unwrap(), rec);
    }

    #[test]
    fn store_labels_are_distinct() {
        let cd = ConfigDirStore {
            path: PathBuf::from("/tmp/x"),
        };
        let hf = HiddenFileStore {
            path: PathBuf::from("/tmp/y"),
        };
        let kr = KeyringStore;
        assert_eq!(cd.label(), "config_dir");
        assert_eq!(hf.label(), "hidden_file");
        assert_eq!(kr.label(), "keyring");
    }

    // 注：不为 KeyringStore 写测试。Keychain 在 CI/沙盒环境会失败，
    // 实机手动验证即可。

    #[test]
    fn hidden_file_store_creates_parent_dir() {
        let d = tempdir().unwrap();
        let store = HiddenFileStore { path: d.path().join("nested/sub/.state") };
        let rec = TrialRecord {
            version: 1, first_run_at: "2026-05-14T00:00:00Z".into(),
            last_run_at: "2026-05-14T00:00:00Z".into(), machine_fp: "fp".into(),
        };
        store.write(&rec).unwrap();
        assert!(d.path().join("nested/sub").exists());
        assert_eq!(store.read().unwrap(), rec);
    }
}
