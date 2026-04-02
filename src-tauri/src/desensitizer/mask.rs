use crate::models::sensitive::SensitiveType;

/// 掩码脱敏：将部分字符替换为 *
/// keep_prefix: 保留前缀字符数
/// keep_suffix: 保留后缀字符数
pub fn apply_mask(text: &str, _sensitive_type: &SensitiveType, keep_prefix: usize, keep_suffix: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    if len == 0 {
        return String::new();
    }

    // 如果保留的前后缀长度之和超过文本长度，则全部掩码
    if keep_prefix + keep_suffix >= len {
        return "*".repeat(len);
    }

    let prefix: String = chars[..keep_prefix].iter().collect();
    let suffix: String = chars[len - keep_suffix..].iter().collect();
    let mask_len = len - keep_prefix - keep_suffix;
    format!("{}{}{}", prefix, "*".repeat(mask_len), suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_mask() {
        let result = apply_mask("张三", &SensitiveType::PersonName, 1, 0);
        assert_eq!(result, "张*");
    }

    #[test]
    fn test_phone_mask() {
        let result = apply_mask("13812345678", &SensitiveType::Phone, 3, 4);
        assert_eq!(result, "138****5678");
    }

    #[test]
    fn test_empty_mask() {
        let result = apply_mask("", &SensitiveType::Phone, 3, 4);
        assert_eq!(result, "");
    }

    #[test]
    fn test_full_mask() {
        let result = apply_mask("abc", &SensitiveType::Phone, 0, 0);
        assert_eq!(result, "***");
    }
}
