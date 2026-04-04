use regex::Regex;
use crate::models::sensitive::SensitiveType;
use super::super::regex_engine::{RegexRule, BoundaryCheck};

/// 英文/国际识别规则
pub fn rules() -> Vec<RegexRule> {
    vec![
        // 1. IBAN（优先级最高，避免被其他规则截断）
        RegexRule {
            regex: Regex::new(r"[A-Z]{2}\d{2}\s?[A-Z0-9]{4}\s?\d{4}\s?\d{3}[A-Z0-9\s]{0,20}").unwrap(),
            sensitive_type: SensitiveType::Iban,
            boundary: BoundaryCheck::NotAlphanumeric,
        },
        // 2. SSN（美国社会安全号码：xxx-xx-xxxx）
        RegexRule {
            regex: Regex::new(r"\d{3}-\d{2}-\d{4}").unwrap(),
            sensitive_type: SensitiveType::Ssn,
            boundary: BoundaryCheck::NotDigit,
        },
        // 3. 信用卡号（16位，可有分隔符）
        RegexRule {
            regex: Regex::new(r"\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}").unwrap(),
            sensitive_type: SensitiveType::CreditCard,
            boundary: BoundaryCheck::NotDigit,
        },
        // 4. 美国电话（多种格式）
        RegexRule {
            regex: Regex::new(r"(?:\+1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}").unwrap(),
            sensitive_type: SensitiveType::UsPhone,
            boundary: BoundaryCheck::NotDigit,
        },
        // 5. 英国电话
        RegexRule {
            regex: Regex::new(r"(?:\+44[-.\s]?|0)\d{2,4}[-.\s]?\d{3,4}[-.\s]?\d{3,4}").unwrap(),
            sensitive_type: SensitiveType::UkPhone,
            boundary: BoundaryCheck::NotDigit,
        },
        // 6. 护照号（1-2个大写字母 + 6-9位数字）
        RegexRule {
            regex: Regex::new(r"[A-Z]{1,2}\d{6,9}").unwrap(),
            sensitive_type: SensitiveType::Passport,
            boundary: BoundaryCheck::NotAlphanumeric,
        },
        // 7. 美国 ZIP Code（5位或 5+4 格式）
        RegexRule {
            regex: Regex::new(r"\d{5}(?:-\d{4})?").unwrap(),
            sensitive_type: SensitiveType::ZipCode,
            boundary: BoundaryCheck::NotDigit,
        },
        // 8. 英国 Postcode
        RegexRule {
            regex: Regex::new(r"[A-Z]{1,2}\d[A-Z\d]?\s?\d[A-Z]{2}").unwrap(),
            sensitive_type: SensitiveType::UkPostcode,
            boundary: BoundaryCheck::NotAlphanumeric,
        },
        // 9. 美国驾照号（字母+数字+短横线格式：D123-4567-8901）
        RegexRule {
            regex: Regex::new(r"[A-Z]\d{3}-\d{4}-\d{4}").unwrap(),
            sensitive_type: SensitiveType::DriversLicense,
            boundary: BoundaryCheck::NotAlphanumeric,
        },
        // 10. 英国驾照号（DVLA 格式：5字母+6数字+2字母+2字母数字，16位）
        RegexRule {
            regex: Regex::new(r"[A-Z]{5}\d{6}[A-Z0-9]{2}\d[A-Z]{2}").unwrap(),
            sensitive_type: SensitiveType::DriversLicense,
            boundary: BoundaryCheck::NotAlphanumeric,
        },
    ]
}
