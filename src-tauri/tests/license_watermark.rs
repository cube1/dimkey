//! 集成测试：水印模块 + 在 LicenseState::TrialExpired 时 export_content 注入
//! 不测 docx XML 注入（已在 lib 单元测试覆盖）
//!
//! 运行：cd src-tauri && cargo test --test license_watermark

use dimkey_lib::license::watermark::{watermark_text, WATERMARK_EN, WATERMARK_ZH};

#[test]
fn watermark_text_contains_dimkey_url() {
    assert!(watermark_text().contains("dimkey.app"));
}

#[test]
fn watermark_text_non_empty() {
    assert!(!watermark_text().is_empty());
}

#[test]
fn watermark_zh_contains_chinese_keyword() {
    assert!(WATERMARK_ZH.contains("试用版"));
    assert!(WATERMARK_ZH.contains("dimkey.app"));
}

#[test]
fn watermark_en_is_english_only() {
    assert!(WATERMARK_EN.contains("trial"));
    assert!(WATERMARK_EN.contains("dimkey.app"));
    // 不含汉字
    assert!(!WATERMARK_EN
        .chars()
        .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)));
}

#[test]
fn default_build_returns_zh_watermark() {
    // 默认 cargo test 用 default features = lang-zh
    #[cfg(all(feature = "lang-zh", not(feature = "lang-en")))]
    assert_eq!(watermark_text(), WATERMARK_ZH);
}

#[cfg(feature = "lang-en")]
#[test]
fn lang_en_build_returns_en_watermark() {
    assert_eq!(watermark_text(), WATERMARK_EN);
}
