mod common;

use std::collections::HashMap;

use dimkey_lib::commands::alias_group::{create_alias_group_internal, select_primary};
use dimkey_lib::models::sensitive::SensitiveType;
use dimkey_lib::models::strategy::{ReplaceStyle, Strategy};
use dimkey_lib::models::task::StrategyType;
use dimkey_lib::models::workspace::*;

/// 构造空的 Workspace（用于别名组测试）
fn make_empty_workspace() -> Workspace {
    Workspace {
        id: "test-ws".to_string(),
        name: "测试工作区".to_string(),
        source: WorkspaceSource::File,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
        strategies: HashMap::new(),
        dict_entries: vec![],
        column_rules: HashMap::new(),
        output_dir: None,
        consistency_mappings: vec![],
        enabled_types: vec!["Phone".into(), "OrgName".into()],
        replace_style: ReplaceStyle::Fake,
        replace_seed: 42,
        replace_counters: HashMap::new(),
        mode: WorkspaceMode::Desensitize,
        whitelist: vec![],
        alias_groups: vec![],
    }
}

// ============================================================
// K03: 别名组一致性 — 创建别名组后成员共享替换值
// ============================================================

/// K03-1: 创建别名组 — 基本功能
#[test]
fn test_create_alias_group_basic() {
    let mut ws = make_empty_workspace();

    let members = vec![
        "阿里巴巴集团".to_string(),
        "阿里巴巴".to_string(),
        "阿里".to_string(),
    ];

    let group = create_alias_group_internal(&mut ws, members.clone(), "OrgName".to_string())
        .expect("创建别名组失败");

    // 主名应为最长的成员
    assert_eq!(group.primary, "阿里巴巴集团", "主名应为最长成员");
    assert_eq!(group.members.len(), 3);
    assert_eq!(group.sensitive_type_key, "OrgName");

    // workspace 中应新增一个别名组
    assert_eq!(ws.alias_groups.len(), 1);
    assert_eq!(ws.alias_groups[0].id, group.id);
}

/// K03-2: 别名组成员少于 2 个应报错
#[test]
fn test_create_alias_group_too_few_members() {
    let mut ws = make_empty_workspace();

    let result = create_alias_group_internal(
        &mut ws,
        vec!["单独一个".to_string()],
        "OrgName".to_string(),
    );

    assert!(result.is_err(), "成员少于 2 应报错");
    assert!(result.unwrap_err().contains("至少需要 2 个成员"));
}

/// K03-3: 成员不能重复加入同类型的其他别名组
#[test]
fn test_create_alias_group_duplicate_member() {
    let mut ws = make_empty_workspace();

    // 先创建一个组
    create_alias_group_internal(
        &mut ws,
        vec!["阿里巴巴".to_string(), "阿里".to_string()],
        "OrgName".to_string(),
    ).expect("第一个组创建失败");

    // 再创建包含 "阿里" 的另一个组 → 应失败
    let result = create_alias_group_internal(
        &mut ws,
        vec!["阿里".to_string(), "淘宝".to_string()],
        "OrgName".to_string(),
    );

    assert!(result.is_err(), "重复成员应报错");
    assert!(result.unwrap_err().contains("已属于其他别名组"));
}

/// K03-4: 不同类型的相同成员可以分别加入不同组
#[test]
fn test_create_alias_group_different_types_ok() {
    let mut ws = make_empty_workspace();

    // OrgName 组
    create_alias_group_internal(
        &mut ws,
        vec!["阿里巴巴".to_string(), "阿里".to_string()],
        "OrgName".to_string(),
    ).expect("OrgName 组创建失败");

    // PersonName 组中包含 "阿里" → 不同类型，应成功
    let result = create_alias_group_internal(
        &mut ws,
        vec!["阿里".to_string(), "阿里先生".to_string()],
        "PersonName".to_string(),
    );

    assert!(result.is_ok(), "不同类型的相同成员应允许");
    assert_eq!(ws.alias_groups.len(), 2);
}

/// K03-5: 已有一致性映射时，创建别名组应同步替换值
#[test]
fn test_alias_group_syncs_consistency_mappings() {
    let mut ws = make_empty_workspace();

    // 预先添加一致性映射：阿里巴巴集团 → 假名1
    ws.consistency_mappings.push(ConsistencyMapping {
        original_text: "阿里巴巴集团".to_string(),
        sensitive_type_key: "OrgName".to_string(),
        replaced_text: "创新科技集团".to_string(),
        strategy: StrategyType::Replace,
        alias_group_id: None,
    });
    ws.consistency_mappings.push(ConsistencyMapping {
        original_text: "阿里".to_string(),
        sensitive_type_key: "OrgName".to_string(),
        replaced_text: "某某".to_string(),
        strategy: StrategyType::Replace,
        alias_group_id: None,
    });

    // 创建别名组
    let group = create_alias_group_internal(
        &mut ws,
        vec!["阿里巴巴集团".to_string(), "阿里".to_string()],
        "OrgName".to_string(),
    ).expect("创建别名组失败");

    // 所有成员的映射应关联到同一组 ID
    for m in &ws.consistency_mappings {
        if m.sensitive_type_key == "OrgName" {
            assert_eq!(
                m.alias_group_id.as_deref(),
                Some(group.id.as_str()),
                "'{}' 应关联到别名组",
                m.original_text
            );
        }
    }

    // 主名为 "阿里巴巴集团"（最长），"阿里" 的替换值应同步为主名的替换值
    let ali_mapping = ws.consistency_mappings.iter()
        .find(|m| m.original_text == "阿里")
        .expect("应有 '阿里' 的映射");
    assert_eq!(
        ali_mapping.replaced_text, "创新科技集团",
        "'阿里' 的替换值应同步为主名的替换值"
    );
}

/// select_primary 应选择字符数最多的成员
#[test]
fn test_select_primary_by_char_count() {
    // 中文字符计数
    assert_eq!(
        select_primary(&["AB".to_string(), "ABC科技有限公司".to_string(), "ABC科技".to_string()]),
        "ABC科技有限公司"
    );
    // 空列表
    assert_eq!(select_primary(&[]), "");
    // 单成员
    assert_eq!(select_primary(&["唯一".to_string()]), "唯一");
}
