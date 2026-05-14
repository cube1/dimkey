// src-tauri/src/license/trial.rs
//
// 30 天试用期计时模块。
// - 多 store 取最早 first_run_at（最大化用户已用时长，反作弊：删一处不重置）
// - 防系统时间回拨：当前 < 任一 store 的 last_run_at 时，用 last_run 推进
// - touch() 即使在 clock_tampered 时也用 now 推进 last_run，避免被永久卡住

use crate::license::storage::{TrialRecord, TrialStore};
use chrono::{DateTime, Duration, Utc};

pub const TRIAL_DAYS: i64 = 30;

#[derive(Debug, Clone, PartialEq)]
pub enum TrialStatus {
    Active {
        days_remaining: u32,
        first_run_at: DateTime<Utc>,
    },
    Expired {
        first_run_at: DateTime<Utc>,
    },
}

pub struct TrialInfo {
    pub status: TrialStatus,
    pub clock_tampered: bool,
}

/// 评估当前试用状态（只读）
pub fn evaluate(stores: &[&dyn TrialStore], machine_fp: &str, now: DateTime<Utc>) -> TrialInfo {
    let _ = machine_fp; // v1 不参与评估，仅在 touch 时写入；保留参数为后续扩展（按 fingerprint 做反作弊）
    let records: Vec<TrialRecord> = stores.iter().filter_map(|s| s.read()).collect();
    let mut clock_tampered = false;

    let (first, last) = if records.is_empty() {
        // 没有任何记录：当前时间作为开始点（调用方负责通过 touch() 落地）
        (now, now)
    } else {
        let mut first = parse_iso(&records[0].first_run_at);
        let mut last = parse_iso(&records[0].last_run_at);
        for r in records.iter().skip(1) {
            let f = parse_iso(&r.first_run_at);
            let l = parse_iso(&r.last_run_at);
            if f < first {
                first = f;
            }
            if l > last {
                last = l;
            }
        }
        // 防回拨：当前 < last_run_at → 时钟被拨回
        if now < last {
            clock_tampered = true;
        }
        (first, last)
    };

    let effective_now = if clock_tampered { last } else { now };
    let elapsed = effective_now - first;
    let trial_dur = Duration::days(TRIAL_DAYS);
    let status = if elapsed >= trial_dur {
        TrialStatus::Expired { first_run_at: first }
    } else {
        let remaining = (trial_dur - elapsed).num_days().max(0) as u32;
        TrialStatus::Active {
            days_remaining: remaining,
            first_run_at: first,
        }
    };
    TrialInfo {
        status,
        clock_tampered,
    }
}

/// 评估后写回 last_run_at（每次启动调一次）。如所有 store 都为空，写入新记录。
/// best-effort：任意 store 写失败不报错，确保试用期不被局部失败破坏
pub fn touch(stores: &[&dyn TrialStore], machine_fp: &str, now: DateTime<Utc>) -> TrialInfo {
    let info = evaluate(stores, machine_fp, now);
    let first = match info.status {
        TrialStatus::Active { first_run_at, .. } => first_run_at,
        TrialStatus::Expired { first_run_at } => first_run_at,
    };
    let new_rec = TrialRecord {
        version: 1,
        first_run_at: first.to_rfc3339(),
        last_run_at: now.to_rfc3339(), // 即使 clock_tampered 也用 now 推进，避免被永久卡住
        machine_fp: machine_fp.to_string(),
    };
    for s in stores {
        let _ = s.write(&new_rec); // best-effort
    }
    info
}

fn parse_iso(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::storage::ConfigDirStore;
    use tempfile::tempdir;

    fn make_stores(d: &std::path::Path) -> Vec<ConfigDirStore> {
        vec![ConfigDirStore {
            path: d.join("trial.json"),
        }]
    }

    fn as_refs(stores: &[ConfigDirStore]) -> Vec<&dyn TrialStore> {
        stores.iter().map(|s| s as &dyn TrialStore).collect()
    }

    #[test]
    fn first_run_returns_active_30_days() {
        let d = tempdir().unwrap();
        let owned = make_stores(d.path());
        let stores = as_refs(&owned);
        let now = Utc::now();
        let info = touch(&stores, "fp1", now);
        match info.status {
            TrialStatus::Active { days_remaining, .. } => {
                assert!(
                    days_remaining >= 29 && days_remaining <= 30,
                    "got {}",
                    days_remaining
                );
            }
            _ => panic!("expected Active"),
        }
        assert!(!info.clock_tampered);
    }

    #[test]
    fn after_31_days_status_is_expired() {
        let d = tempdir().unwrap();
        let owned = make_stores(d.path());
        let stores = as_refs(&owned);
        let start = Utc::now();
        touch(&stores, "fp1", start);
        let later = start + Duration::days(31);
        let info = evaluate(&stores, "fp1", later);
        assert!(matches!(info.status, TrialStatus::Expired { .. }));
    }

    #[test]
    fn clock_rollback_detected_and_uses_last_run() {
        let d = tempdir().unwrap();
        let owned = make_stores(d.path());
        let stores = as_refs(&owned);
        let day1 = Utc::now();
        touch(&stores, "fp1", day1);
        let day10 = day1 + Duration::days(10);
        touch(&stores, "fp1", day10); // last_run = day10
        let rolled_back = day1 - Duration::days(5);
        let info = evaluate(&stores, "fp1", rolled_back);
        assert!(info.clock_tampered);
        match info.status {
            TrialStatus::Active { days_remaining, .. } => {
                // 用 last_run=day10 推进：已用 10 天，剩 20 天
                assert!(
                    days_remaining >= 19 && days_remaining <= 20,
                    "got {}",
                    days_remaining
                );
            }
            _ => panic!("expected Active when rollback detected"),
        }
    }

    #[test]
    fn earliest_first_run_at_wins_across_stores() {
        let d = tempdir().unwrap();
        let owned = vec![
            ConfigDirStore {
                path: d.path().join("a.json"),
            },
            ConfigDirStore {
                path: d.path().join("b.json"),
            },
        ];
        let early = Utc::now() - Duration::days(20);
        let late = Utc::now() - Duration::days(5);
        owned[0]
            .write(&TrialRecord {
                version: 1,
                first_run_at: late.to_rfc3339(),
                last_run_at: late.to_rfc3339(),
                machine_fp: "fp1".into(),
            })
            .unwrap();
        owned[1]
            .write(&TrialRecord {
                version: 1,
                first_run_at: early.to_rfc3339(),
                last_run_at: late.to_rfc3339(),
                machine_fp: "fp1".into(),
            })
            .unwrap();
        let stores = as_refs(&owned);
        let info = evaluate(&stores, "fp1", Utc::now());
        match info.status {
            TrialStatus::Active { days_remaining, .. } => {
                // 用 early=20 天前 → 已用 20 天，剩 10 天
                assert!(
                    days_remaining >= 9 && days_remaining <= 10,
                    "got {}",
                    days_remaining
                );
            }
            _ => panic!("expected Active using earliest first_run_at"),
        }
    }

    #[test]
    fn touch_writes_to_all_stores() {
        let d = tempdir().unwrap();
        let owned = vec![
            ConfigDirStore {
                path: d.path().join("a.json"),
            },
            ConfigDirStore {
                path: d.path().join("b.json"),
            },
        ];
        let stores = as_refs(&owned);
        let now = Utc::now();
        touch(&stores, "fp1", now);
        // 两个 store 都应有内容
        assert!(d.path().join("a.json").exists());
        assert!(d.path().join("b.json").exists());
    }
}
