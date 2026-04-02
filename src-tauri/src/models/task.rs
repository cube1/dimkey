use serde::{Deserialize, Serialize};
use super::sensitive::{SensitiveType, FileType};

/// 脱敏任务记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    /// 任务唯一标识
    pub id: String,
    /// 原始文件名
    pub original_file_name: String,
    /// 文件类型
    pub file_type: FileType,
    /// 创建时间
    pub created_at: String,
    /// 敏感项总数
    pub sensitive_count: usize,
    /// 已替换数
    pub replaced_count: usize,
    /// 脱敏映射记录
    pub mappings: Vec<MappingEntry>,
}

/// 脱敏映射条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingEntry {
    /// 原始文本
    pub original_text: String,
    /// 替换后文本
    pub replaced_text: String,
    /// 敏感信息类型
    pub sensitive_type: SensitiveType,
    /// 脱敏策略类型
    pub strategy: StrategyType,
    /// 出现次数
    pub occurrences: usize,
}

/// 脱敏策略类型（用于映射记录）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StrategyType {
    /// 掩码
    Mask,
    /// 替换
    Replace,
    /// 泛化
    Generalize,
}
