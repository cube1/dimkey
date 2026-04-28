//! K09: 中英 counter 隔离 — 文档含中英混合 PersonName，counter 中独立计数
//!
//! 契约：apply_replace 内部走 detect_language(text) 自动判断语言，
//! 中文 PersonName 进 PersonName_zh counter，英文 PersonName 进 PersonName_en counter，
//! 两 counter 互不串扰。
//!
//! 测试分工：
//! - 本文件 = apply_replace 直测（契约）。
//! - 端到端 (detect_full_pipeline + mixed_bilingual.xlsx) 不放这里：当前 distilbert-ner
//!   对中文 PersonName 召回率近 0，端到端跑出来要么命中数为 0 静默通过，要么只命中
//!   英文一侧，无法验证"中英 counter 互不串扰"这个核心契约。
//!   待 NER 模型升级后再补端到端。

mod common;

use std::collections::HashMap;

use dimkey_lib::desensitizer::replace::{apply_replace, ReplaceState};
use dimkey_lib::models::sensitive::*;
use dimkey_lib::models::strategy::*;

#[allow(unused_imports)]
use common::*;

fn has_han(s: &str) -> bool {
    s.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
}

#[test]
fn test_k09_counter_isolation_zh_en_via_apply_replace() {
    // 中英交替调用 apply_replace，验证 counter 分桶契约（不依赖 NER 召回）
    let mut state = ReplaceState::new(42, HashMap::new());

    let zh_inputs = ["张伟", "李娜", "王芳", "赵明"];
    let en_inputs = ["John Smith", "Michael Chen", "Emily Johnson", "Sarah Wang"];

    let mut zh_outputs: Vec<String> = Vec::new();
    let mut en_outputs: Vec<String> = Vec::new();

    for (i, name) in zh_inputs.iter().enumerate() {
        zh_outputs.push(apply_replace(name, &SensitiveType::PersonName, &mut state, &ReplaceStyle::Fake));
        en_outputs.push(apply_replace(en_inputs[i], &SensitiveType::PersonName, &mut state, &ReplaceStyle::Fake));
    }

    let counters = state.export_counters();
    let zh_count = *counters.get("PersonName_zh").unwrap_or(&0);
    let en_count = *counters.get("PersonName_en").unwrap_or(&0);

    assert_eq!(zh_count, 4, "PersonName_zh counter 应为 4: {:?}", counters);
    assert_eq!(en_count, 4, "PersonName_en counter 应为 4: {:?}", counters);

    // 中文输入 → 中文 fake（含汉字）
    for (orig, replaced) in zh_inputs.iter().zip(zh_outputs.iter()) {
        assert!(
            has_han(replaced),
            "中文 PersonName '{}' 应替换为含汉字的 fake，实际: {}",
            orig, replaced
        );
        assert_ne!(replaced, orig, "替换值不应等于原文");
    }

    // 英文输入 → 英文 fake（不含汉字 + 含空格 + 纯 ASCII）
    for (orig, replaced) in en_inputs.iter().zip(en_outputs.iter()) {
        assert!(
            !has_han(replaced),
            "英文 PersonName '{}' 不应含汉字，实际: {}",
            orig, replaced
        );
        assert!(
            replaced.contains(' '),
            "英文 PersonName 替换应含空格分隔: {}",
            replaced
        );
        assert!(
            replaced.is_ascii(),
            "英文 PersonName 替换应为纯 ASCII: {}",
            replaced
        );
        assert_ne!(replaced, orig, "替换值不应等于原文");
    }

    // 仅向 zh 加调 4 次，验证两个 counter 完全独立递增（互不串扰）
    let zh_second_round: Vec<String> = zh_inputs
        .iter()
        .map(|n| apply_replace(n, &SensitiveType::PersonName, &mut state, &ReplaceStyle::Fake))
        .collect();
    let counters2 = state.export_counters();
    assert_eq!(*counters2.get("PersonName_zh").unwrap_or(&0), 8, "再调 4 次中文后 zh counter 应为 8");
    assert_eq!(*counters2.get("PersonName_en").unwrap_or(&0), 4, "未调 en，en counter 应保持 4 — 互不串扰");
    let _ = zh_second_round;
}
