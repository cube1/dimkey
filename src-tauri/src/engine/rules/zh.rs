use regex::Regex;
use crate::models::sensitive::SensitiveType;
use super::super::regex_engine::{RegexRule, BoundaryCheck};

/// 中文特有识别规则
pub fn rules() -> Vec<RegexRule> {
    vec![
        // 1. 身份证号（18位，末位可能是 X/x）
        RegexRule {
            regex: Regex::new(r"\d{17}[\dXx]").unwrap(),
            sensitive_type: SensitiveType::IdCard,
            boundary: BoundaryCheck::NotDigit,
        },
        // 2. 统一社会信用代码（18位）
        // 标准 GB 32100-2015 排除 I/O/S/V/Z，但检测场景需宽松匹配 OCR 误差和非标数据
        RegexRule {
            regex: Regex::new(r"[0-9A-Z]{2}\d{6}[0-9A-Z]{10}").unwrap(),
            sensitive_type: SensitiveType::CreditCode,
            boundary: BoundaryCheck::NotAlphanumeric,
        },
        // 3. 银行卡号（16-19位纯数字）
        RegexRule {
            regex: Regex::new(r"\d{16,19}").unwrap(),
            sensitive_type: SensitiveType::BankCard,
            boundary: BoundaryCheck::NotDigit,
        },
        // 4. 手机号（11位，1开头 3-9 第二位）
        RegexRule {
            regex: Regex::new(r"1[3-9]\d{9}").unwrap(),
            sensitive_type: SensitiveType::Phone,
            boundary: BoundaryCheck::NotDigit,
        },
        // 5. 固定电话
        RegexRule {
            regex: Regex::new(r"0\d{2,3}-?\d{7,8}").unwrap(),
            sensitive_type: SensitiveType::LandlinePhone,
            boundary: BoundaryCheck::NotDigit,
        },
        // 6. 车牌号（支持中间点分隔符：京A·12345）
        RegexRule {
            regex: Regex::new(r"[京津沪渝冀豫云辽黑湘皖鲁新苏浙赣鄂桂甘晋蒙陕吉闽贵粤川青藏琼宁][A-Z][·.]?[A-HJ-NP-Z0-9]{5}").unwrap(),
            sensitive_type: SensitiveType::LicensePlate,
            boundary: BoundaryCheck::None,
        },
    ]
}
