use serde::{Deserialize, Serialize};

/// 应用支持的语言
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Zh,
    En,
}

impl Default for Language {
    fn default() -> Self {
        Language::Zh
    }
}

impl Language {
    /// 从字符串解析语言（宽松匹配）
    pub fn from_str_loose(s: &str) -> Self {
        let lower = s.to_lowercase();
        if lower.starts_with("en") {
            Language::En
        } else {
            Language::Zh
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str_loose() {
        assert_eq!(Language::from_str_loose("en"), Language::En);
        assert_eq!(Language::from_str_loose("en-US"), Language::En);
        assert_eq!(Language::from_str_loose("zh"), Language::Zh);
        assert_eq!(Language::from_str_loose("zh-CN"), Language::Zh);
        assert_eq!(Language::from_str_loose("unknown"), Language::Zh);
    }

    #[test]
    fn test_default() {
        assert_eq!(Language::default(), Language::Zh);
    }
}
