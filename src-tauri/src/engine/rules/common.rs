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
    ]
}
