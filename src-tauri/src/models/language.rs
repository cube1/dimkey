use serde::{Deserialize, Serialize};

/// 应用支持的语言
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Zh,
    En,
}

impl Default for Language {
    fn default() -> Self {
        Self::current()
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

    /// 编译期决定的应用语言。中英文分包通过 Cargo feature 切换：
    /// `--features lang-zh`（默认） / `--features lang-en --no-default-features`
    pub const fn current() -> Self {
        #[cfg(feature = "lang-en")]
        { Language::En }
        #[cfg(all(feature = "lang-zh", not(feature = "lang-en")))]
        { Language::Zh }
        #[cfg(all(not(feature = "lang-zh"), not(feature = "lang-en")))]
        { Language::Zh }
    }

    /// 用于前端的语言代码（"zh" / "en"）
    pub const fn code(self) -> &'static str {
        match self {
            Language::Zh => "zh",
            Language::En => "en",
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
    fn test_default_matches_build_feature() {
        // 编译期常量：default == current()，由 Cargo feature 决定
        assert_eq!(Language::default(), Language::current());
    }

    #[cfg(all(feature = "lang-zh", not(feature = "lang-en")))]
    #[test]
    fn test_current_lang_zh() {
        assert_eq!(Language::current(), Language::Zh);
    }

    #[cfg(feature = "lang-en")]
    #[test]
    fn test_current_lang_en() {
        assert_eq!(Language::current(), Language::En);
    }
}
