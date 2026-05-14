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

#[derive(Debug, Clone)]
pub struct TrialInfo {
    pub status: TrialStatus,
    pub clock_tampered: bool,
}

/// 评估当前试用状态（只读）
pub fn evaluate(stores: &[&dyn TrialStore], machine_fp: &str, now: DateTime<Utc>) -> TrialInfo {
    let _ = machine_fp; // v1 不参与评估，仅在 touch 时写入；保留参数为后续扩展（按 fingerprint 做反作弊）
    // 反作弊：若某 record 的 first_run_at 或 last_run_at 解析失败，整条 record 跳过
    // （绝不能 fallback 到 now，否则攻击者篡改时间戳即可静默重置试用期）
    let parsed: Vec<(DateTime<Utc>, DateTime<Utc>)> = stores
        .iter()
        .filter_map(|s| s.read())
        .filter_map(|r| {
            let f = parse_iso(&r.first_run_at);
            let l = parse_iso(&r.last_run_at);
            match (f, l) {
                (Some(f), Some(l)) => Some((f, l)),
                _ => {
                    eprintln!(
                        "[license::trial] skipping record with unparseable timestamp: first={:?} last={:?}",
                        r.first_run_at, r.last_run_at
                    );
                    None
                }
            }
        })
        .collect();

    let mut clock_tampered = false;
    let (first, last) = if parsed.is_empty() {
        // 没有任何有效记录：当前时间作为开始点（调用方负责通过 touch() 落地）
        (now, now)
    } else {
        let mut first = parsed[0].0;
        let mut last = parsed[0].1;
        for (f, l) in parsed.iter().skip(1) {
            if *f < first {
                first = *f;
            }
            if *l > last {
                last = *l;
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
        if let Err(e) = s.write(&new_rec) {
            // best-effort：写失败不阻断流程，但写出可观察日志便于排查
            eprintln!(
                "[license::trial] write failed: store={} err={}",
                s.label(),
                e
            );
        }
    }
    info
}

fn parse_iso(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .ok()
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

    #[test]
    fn corrupt_timestamp_in_record_is_skipped_not_fallback_to_now() {
        let d = tempdir().unwrap();
        let owned = vec![
            ConfigDirStore { path: d.path().join("a.json") },
            ConfigDirStore { path: d.path().join("b.json") },
        ];
        let early = Utc::now() - Duration::days(20);
        // store a 完好（早 20 天）
        owned[0].write(&TrialRecord {
            version: 1, first_run_at: early.to_rfc3339(), last_run_at: early.to_rfc3339(),
            machine_fp: "fp".into(),
        }).unwrap();
        // store b 有损坏的时间戳（如果不跳过，会被当作 now 参与最早算法 → 试用被错误延长到 30 天）
        owned[1].write(&TrialRecord {
            version: 1, first_run_at: "totally-not-rfc3339".into(),
            last_run_at: "also-broken".into(), machine_fp: "fp".into(),
        }).unwrap();
        let stores = as_refs(&owned);
        let info = evaluate(&stores, "fp", Utc::now());
        match info.status {
            TrialStatus::Active { days_remaining, .. } => {
                // 期望使用 store a 的 early=20 天前 → 剩 10 天（不是 30 天，证明 b 被跳过而非 fallback now）
                assert!(days_remaining >= 9 && days_remaining <= 10, "got {} — corrupt record should be skipped, not fallback to now", days_remaining);
            }
            _ => panic!("expected Active"),
        }
    }

    #[test]
    fn now_equals_last_does_not_trigger_clock_tampered() {
        let d = tempdir().unwrap();
        let owned = make_stores(d.path());
        let stores = as_refs(&owned);
        let now = Utc::now();
        touch(&stores, "fp1", now);
        // 立即用同一时间 evaluate
        let info = evaluate(&stores, "fp1", now);
        assert!(!info.clock_tampered, "now == last should not trigger tamper");
    }
}
