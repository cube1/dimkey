use crate::models::sensitive::SensitiveType;
use crate::models::language::Language;

/// 泛化脱敏：降低信息精度
pub fn apply_generalize(text: &str, sensitive_type: &SensitiveType) -> String {
    match sensitive_type {
        SensitiveType::Address => generalize_address(text),
        SensitiveType::Title => generalize_title(text),
        SensitiveType::IdCard => generalize_id_card(text),
        SensitiveType::Phone => generalize_phone(text),
        _ => {
            // 默认泛化：保留首字符 + "**"
            let chars: Vec<char> = text.chars().collect();
            if chars.len() > 1 {
                format!("{}**", chars[0])
            } else {
                "**".to_string()
            }
        }
    }
}

/// 地址泛化：降低精度，用通用占位符替代具体地名
fn generalize_address(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();

    // 长地址：包含省+市或市+区等多级行政区划，截断到较高级别
    if let Some(byte_pos) = text.find("特别行政区") {
        let end = byte_pos + "特别行政区".len();
        if end < text.len() {
            return text[..end].to_string();
        }
    }

    if let Some(byte_pos) = text.find("自治区") {
        let end = byte_pos + "自治区".len();
        if end < text.len() {
            return text[..end].to_string();
        }
    }

    // 包含"省"且后面还有内容，截断到省级
    if let Some(pos) = chars.iter().position(|&c| c == '省') {
        if pos + 1 < chars.len() {
            return chars[..=pos].iter().collect();
        }
    }

    // 包含"市"且后面还有内容，截断到市级
    if let Some(pos) = chars.iter().position(|&c| c == '市') {
        if pos + 1 < chars.len() {
            return chars[..=pos].iter().collect();
        }
    }

    // 短地名或无法进一步截断的地址，用通用占位符替换
    // 识别行政区划后缀并替换为"某X"
    for suffix in &["特别行政区", "自治区", "省", "市", "区", "县", "镇", "村", "街道"] {
        if text.ends_with(suffix) {
            return format!("某{}", suffix);
        }
    }

    // 无行政区划后缀的地名，替换为"某地"
    "某地".to_string()
}

/// 职位泛化：映射到通用类别
fn generalize_title(text: &str) -> String {
    let management = ["总裁", "总经理", "副总", "董事", "CEO", "CFO", "CTO", "COO"];
    let director = ["总监", "主任", "部长", "处长", "科长", "局长"];
    let manager = ["经理", "主管", "组长", "队长"];
    let tech = ["工程师", "架构师", "程序员", "开发", "技术"];
    let staff = ["专员", "助理", "文员", "秘书"];

    for kw in management {
        if text.contains(kw) {
            return "高级管理人员".to_string();
        }
    }
    for kw in director {
        if text.contains(kw) {
            return "中级管理人员".to_string();
        }
    }
    for kw in manager {
        if text.contains(kw) {
            return "基层管理人员".to_string();
        }
    }
    for kw in tech {
        if text.contains(kw) {
            return "技术人员".to_string();
        }
    }
    for kw in staff {
        if text.contains(kw) {
            return "一般职员".to_string();
        }
    }

    "职员".to_string()
}

/// 身份证泛化：仅保留出生年份
fn generalize_id_card(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() >= 10 {
        let year: String = chars[6..10].iter().collect();
        format!("{}年出生", year)
    } else {
        "****".to_string()
    }
}

/// 手机号泛化：保留前三位
fn generalize_phone(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() >= 3 {
        let prefix: String = chars[..3].iter().collect();
        format!("{}********", prefix)
    } else {
        "***".to_string()
    }
}

/// 按语言分发泛化
pub fn apply_generalize_for_language(text: &str, sensitive_type: &SensitiveType, lang: Language) -> String {
    match lang {
        Language::Zh => apply_generalize(text, sensitive_type),
        Language::En => apply_generalize_en(text, sensitive_type),
    }
}

/// 英文泛化入口
fn apply_generalize_en(text: &str, sensitive_type: &SensitiveType) -> String {
    match sensitive_type {
        SensitiveType::Address => generalize_en_address(text),
        SensitiveType::Title => generalize_en_title(text),
        SensitiveType::Ssn => generalize_en_ssn(text),
        SensitiveType::ZipCode => generalize_en_zip(text),
        SensitiveType::UkPostcode => generalize_en_uk_postcode(text),
        SensitiveType::UsPhone | SensitiveType::UkPhone => {
            let chars: Vec<char> = text.chars().collect();
            if chars.len() > 6 {
                let prefix: String = chars[..chars.len() / 2].iter().collect();
                let mask = "*".repeat(chars.len() - chars.len() / 2);
                format!("{}{}", prefix, mask)
            } else {
                "***".to_string()
            }
        }
        _ => {
            let chars: Vec<char> = text.chars().collect();
            if chars.len() > 1 {
                format!("{}**", chars[0])
            } else {
                "**".to_string()
            }
        }
    }
}

/// 英文地址泛化：保留城市/州，去掉门牌和街道
fn generalize_en_address(text: &str) -> String {
    let parts: Vec<&str> = text.split(',').map(|s| s.trim()).collect();
    if parts.len() >= 2 {
        parts[parts.len() - 2..].join(", ")
    } else {
        text.to_string()
    }
}

/// 英文职位泛化
fn generalize_en_title(text: &str) -> String {
    let lower = text.to_lowercase();
    let executive = ["chief", "ceo", "cfo", "cto", "coo", "president", "chairman"];
    let director = ["director", "vp", "vice president", "head of"];
    let manager = ["manager", "supervisor", "lead", "team lead"];
    let tech = ["engineer", "developer", "architect", "programmer", "analyst"];
    let staff = ["assistant", "coordinator", "specialist", "clerk", "associate"];

    for kw in executive { if lower.contains(kw) { return "Senior Executive".to_string(); } }
    for kw in director { if lower.contains(kw) { return "Director-Level".to_string(); } }
    for kw in manager { if lower.contains(kw) { return "Management".to_string(); } }
    for kw in tech { if lower.contains(kw) { return "Technical Staff".to_string(); } }
    for kw in staff { if lower.contains(kw) { return "Staff".to_string(); } }

    "Employee".to_string()
}

/// SSN 泛化：保留后4位
fn generalize_en_ssn(text: &str) -> String {
    if text.len() >= 11 {
        format!("***-**-{}", &text[7..])
    } else {
        "***-**-****".to_string()
    }
}

/// 美国 ZIP 泛化：保留前3位
fn generalize_en_zip(text: &str) -> String {
    let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 3 {
        format!("{}**", &digits[..3])
    } else {
        "***".to_string()
    }
}

/// 英国 Postcode 泛化：保留外码（前半部分）
fn generalize_en_uk_postcode(text: &str) -> String {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() >= 2 {
        format!("{} ***", parts[0])
    } else if text.len() > 3 {
        format!("{} ***", &text[..text.len() - 3])
    } else {
        "*** ***".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generalize_address_with_city_and_detail() {
        // 长地址：截断到市级
        assert_eq!(
            apply_generalize("北京市朝阳区建国路88号", &SensitiveType::Address),
            "北京市"
        );
    }

    #[test]
    fn test_generalize_address_with_province_and_city() {
        // 省+市+区：截断到省级
        assert_eq!(
            apply_generalize("广东省深圳市南山区", &SensitiveType::Address),
            "广东省"
        );
    }

    #[test]
    fn test_generalize_address_short_city() {
        // 短地名"北京市"无法进一步截断，替换为"某市"
        assert_eq!(
            apply_generalize("北京市", &SensitiveType::Address),
            "某市"
        );
    }

    #[test]
    fn test_generalize_address_short_district() {
        // "海淀区"替换为"某区"
        assert_eq!(
            apply_generalize("海淀区", &SensitiveType::Address),
            "某区"
        );
    }

    #[test]
    fn test_generalize_address_bare_name() {
        // 无行政区划后缀的地名
        assert_eq!(
            apply_generalize("北京", &SensitiveType::Address),
            "某地"
        );
    }

    #[test]
    fn test_generalize_address_province_only() {
        // "广东省"无法进一步截断，替换为"某省"
        assert_eq!(
            apply_generalize("广东省", &SensitiveType::Address),
            "某省"
        );
    }

    #[test]
    fn test_generalize_address_autonomous_region() {
        // 自治区 + 后续内容：截断到自治区
        assert_eq!(
            apply_generalize("内蒙古自治区呼和浩特市", &SensitiveType::Address),
            "内蒙古自治区"
        );
    }

    #[test]
    fn test_generalize_address_autonomous_region_only() {
        // 仅自治区，替换为"某自治区"
        assert_eq!(
            apply_generalize("内蒙古自治区", &SensitiveType::Address),
            "某自治区"
        );
    }

    #[test]
    fn test_generalize_title_management() {
        assert_eq!(
            apply_generalize("技术总监", &SensitiveType::Title),
            "中级管理人员"
        );
    }

    #[test]
    fn test_generalize_title_tech() {
        assert_eq!(
            apply_generalize("高级工程师", &SensitiveType::Title),
            "技术人员"
        );
    }

    #[test]
    fn test_generalize_id_card() {
        assert_eq!(
            apply_generalize("110101199001011234", &SensitiveType::IdCard),
            "1990年出生"
        );
    }

    #[test]
    fn test_generalize_phone() {
        assert_eq!(
            apply_generalize("13812345678", &SensitiveType::Phone),
            "138********"
        );
    }

    #[test]
    fn test_generalize_en_address() {
        assert_eq!(
            generalize_en_address("123 Main St, New York, NY 10001"),
            "New York, NY 10001"
        );
    }

    #[test]
    fn test_generalize_en_title_executive() {
        assert_eq!(generalize_en_title("Chief Executive Officer"), "Senior Executive");
    }

    #[test]
    fn test_generalize_en_title_tech() {
        assert_eq!(generalize_en_title("Senior Software Engineer"), "Technical Staff");
    }

    #[test]
    fn test_generalize_en_ssn() {
        assert_eq!(generalize_en_ssn("123-45-6789"), "***-**-6789");
    }

    #[test]
    fn test_generalize_en_zip() {
        assert_eq!(generalize_en_zip("10001"), "100**");
        assert_eq!(generalize_en_zip("90210-1234"), "902**");
    }

    #[test]
    fn test_generalize_en_uk_postcode() {
        assert_eq!(generalize_en_uk_postcode("SW1A 1AA"), "SW1A ***");
    }
}
