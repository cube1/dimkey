use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::sensitive::FileType;
use super::strategy::{Strategy, DictEntry, ReplaceStyle, MatchMode};
use super::task::{MappingEntry, StrategyType};

/// 工作区来源类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkspaceSource {
    /// 文件导入
    File,
    /// 粘贴板输入
    Clipboard,
}

impl Default for WorkspaceSource {
    fn default() -> Self {
        WorkspaceSource::File
    }
}

/// 跨文件一致性替换映射条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyMapping {
    /// 原始文本
    pub original_text: String,
    /// 敏感类型键名（如 "PersonName"、"Address"）
    pub sensitive_type_key: String,
    /// 替换后文本
    pub replaced_text: String,
    /// 使用的脱敏策略
    pub strategy: StrategyType,
    /// 关联的别名组 ID（属于某个别名组时非 None）
    #[serde(default)]
    pub alias_group_id: Option<String>,
}

/// 别名组：将同一实体的多个名称关联在一起，脱敏时统一替换
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasGroup {
    /// 唯一标识 (UUID)
    pub id: String,
    /// 主名（组内最长文本，用于生成替换值）
    pub primary: String,
    /// 所有成员文本（含主名）
    pub members: Vec<String>,
    /// 敏感类型键名（如 "OrgName"）
    pub sensitive_type_key: String,
    /// 创建时间 ISO 8601
    pub created_at: String,
}

/// 处理状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProcessingStatus {
    /// 已完成
    Completed,
    /// 已还原
    Restored,
}

/// 工作区模式
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum WorkspaceMode {
    /// 脱敏模式（默认）
    #[default]
    Desensitize,
    /// 模版替换模式
    TemplateReplace,
}

/// 白名单排除条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistEntry {
    /// 排除的文本
    pub text: String,
    /// 匹配模式（精确/模糊）
    pub match_mode: MatchMode,
}

/// 工作区
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// 唯一标识
    pub id: String,
    /// 用户自定义名称
    pub name: String,
    /// 工作区来源（文件/粘贴板），默认文件
    #[serde(default)]
    pub source: WorkspaceSource,
    /// 创建时间 ISO 8601
    pub created_at: String,
    /// 更新时间 ISO 8601
    pub updated_at: String,
    /// 各敏感类型的脱敏策略（key 为类型键名，如 "Phone"）
    pub strategies: HashMap<String, Strategy>,
    /// 自定义词典条目
    pub dict_entries: Vec<DictEntry>,
    /// 列索引 → 敏感类型键名（Excel/CSV 专用，JSON key 为字符串）
    pub column_rules: HashMap<String, String>,
    /// 默认输出目录
    pub output_dir: Option<String>,
    /// 跨文件一致性替换映射表
    #[serde(default)]
    pub consistency_mappings: Vec<ConsistencyMapping>,
    /// 启用的敏感类型列表（用户可勾选/取消）
    #[serde(default = "default_enabled_types")]
    pub enabled_types: Vec<String>,

    /// 替换风格（Fake/Mou/Ordinal），旧版 JSON 缺失时默认 Fake
    #[serde(default)]
    pub replace_style: ReplaceStyle,

    /// 替换生成器随机种子（创建时随机生成，工作区生命周期内不变）
    #[serde(default)]
    pub replace_seed: u64,

    /// 各敏感类型的替换计数器（key: "PersonName" 等）
    #[serde(default)]
    pub replace_counters: HashMap<String, usize>,

    /// 工作区模式（脱敏/模版替换），旧版 JSON 缺失时默认脱敏
    #[serde(default)]
    pub mode: WorkspaceMode,

    /// 白名单排除列表（工作区级别，所有引擎生效）
    #[serde(default)]
    pub whitelist: Vec<WhitelistEntry>,

    /// 别名组列表（将全称/简称关联为同一实体）
    #[serde(default)]
    pub alias_groups: Vec<AliasGroup>,
}

/// 默认启用全部 12 种敏感类型
pub fn default_enabled_types() -> Vec<String> {
    vec![
        "Phone".into(), "IdCard".into(), "BankCard".into(), "Email".into(),
        "IpAddress".into(), "LandlinePhone".into(), "LicensePlate".into(),
        "CreditCode".into(), "PersonName".into(), "OrgName".into(),
        "Address".into(), "Title".into(),
    ]
}

/// 处理记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingRecord {
    /// 唯一标识
    pub id: String,
    /// 文件名
    pub file_name: String,
    /// 原始文件路径
    pub file_path: String,
    /// 文件类型
    pub file_type: FileType,
    /// 处理时间 ISO 8601
    pub processed_at: String,
    /// 脱敏映射记录
    pub mappings: Vec<MappingEntry>,
    /// 敏感项数量
    pub sensitive_count: usize,
    /// 处理状态
    pub status: ProcessingStatus,
    /// 码本文件路径（列级脱敏模式下生成）
    #[serde(default)]
    pub codebook_path: Option<String>,
}

/// 工作区完整数据（含历史记录）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceData {
    pub workspace: Workspace,
    #[serde(default)]
    pub history: Vec<ProcessingRecord>,
}

/// 工作区列表项（轻量结构，用于左栏展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceListItem {
    pub id: String,
    pub name: String,
    pub updated_at: String,
    pub history_count: usize,
    /// 工作区来源
    #[serde(default)]
    pub source: WorkspaceSource,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_group_serde_roundtrip() {
        let group = AliasGroup {
            id: "g1".to_string(),
            primary: "ABC科技有限公司".to_string(),
            members: vec!["ABC科技有限公司".to_string(), "ABC".to_string()],
            sensitive_type_key: "OrgName".to_string(),
            created_at: "2026-03-27T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&group).unwrap();
        let decoded: AliasGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "g1");
        assert_eq!(decoded.primary, "ABC科技有限公司");
        assert_eq!(decoded.members.len(), 2);
    }

    #[test]
    fn test_workspace_backward_compat_no_alias_groups() {
        let json = r#"{
            "id": "ws1", "name": "test", "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z", "strategies": {},
            "dict_entries": [], "column_rules": {}, "output_dir": null,
            "consistency_mappings": [], "enabled_types": ["Phone"]
        }"#;
        let ws: Workspace = serde_json::from_str(json).unwrap();
        assert!(ws.alias_groups.is_empty());
    }

    #[test]
    fn test_consistency_mapping_backward_compat_no_group_id() {
        let json = r#"{
            "original_text": "张三",
            "sensitive_type_key": "PersonName",
            "replaced_text": "李四",
            "strategy": "Replace"
        }"#;
        let m: ConsistencyMapping = serde_json::from_str(json).unwrap();
        assert!(m.alias_group_id.is_none());
    }
}
