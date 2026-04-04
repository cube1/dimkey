use regex::Regex;
use crate::models::sensitive::SensitiveType;
use super::super::regex_engine::{RegexRule, BoundaryCheck};

/// Email + IPv4 通用规则
pub fn rules() -> Vec<RegexRule> {
    vec![
        // 邮箱地址
        RegexRule {
            regex: Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap(),
            sensitive_type: SensitiveType::Email,
            boundary: BoundaryCheck::None,
        },
        // IPv4 地址
        RegexRule {
            regex: Regex::new(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}").unwrap(),
            sensitive_type: SensitiveType::IpAddress,
            boundary: BoundaryCheck::NotDigit,
        },
        // IPv6 地址（完整格式 + :: 压缩格式）
        // 注意顺序：混合模式（前后都有组）必须在纯前缀模式之前，避免贪婪匹配截断
        // 不匹配裸 `::`（避免误匹配 C++/Rust 路径分隔符），至少需要一个 hex 组
        RegexRule {
            regex: Regex::new(r"(?i)(?:[0-9a-f]{1,4}:){7}[0-9a-f]{1,4}|(?:[0-9a-f]{1,4}:){6}:[0-9a-f]{1,4}|(?:[0-9a-f]{1,4}:){5}(?::[0-9a-f]{1,4}){1,2}|(?:[0-9a-f]{1,4}:){4}(?::[0-9a-f]{1,4}){1,3}|(?:[0-9a-f]{1,4}:){3}(?::[0-9a-f]{1,4}){1,4}|(?:[0-9a-f]{1,4}:){2}(?::[0-9a-f]{1,4}){1,5}|(?:[0-9a-f]{1,4}:){1}(?::[0-9a-f]{1,4}){1,6}|(?:[0-9a-f]{1,4}:){1,7}:|:(?::[0-9a-f]{1,4}){1,7}").unwrap(),
            sensitive_type: SensitiveType::IpAddress,
            boundary: BoundaryCheck::NotAlphanumeric,
        },
    ]
}
