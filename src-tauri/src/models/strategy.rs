use serde::{Deserialize, Serialize};
use super::sensitive::SensitiveType;

/// 替换风格枚举
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ReplaceStyle {
    /// 假数据替换（默认）
    Fake,
    /// 某式替换（如：某某、某公司）
    Mou,
    /// 序号式替换（如：人员1、人员2）
    Ordinal,
}

impl Default for ReplaceStyle {
    fn default() -> Self {
        ReplaceStyle::Fake
    }
}

/// 脱敏策略枚举
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Strategy {
    /// 掩码替换，如：张** / 138****8888
    Mask { keep_prefix: usize, keep_suffix: usize },
    /// 假数据替换，如：用 fake 库生成
    Replace { style: ReplaceStyle },
    /// 泛化，如：北京市朝阳区 → 北京市
    Generalize,
}

impl<'de> serde::Deserialize<'de> for Strategy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match &value {
            serde_json::Value::String(s) => match s.as_str() {
                "Replace" => Ok(Strategy::Replace { style: ReplaceStyle::default() }),
                "Generalize" => Ok(Strategy::Generalize),
                other => Err(serde::de::Error::unknown_variant(other, &["Replace", "Generalize"])),
            },
            serde_json::Value::Object(map) => {
                if let Some(v) = map.get("Mask") {
                    #[derive(Deserialize)]
                    struct M { keep_prefix: usize, keep_suffix: usize }
                    let m: M = serde_json::from_value(v.clone()).map_err(serde::de::Error::custom)?;
                    Ok(Strategy::Mask { keep_prefix: m.keep_prefix, keep_suffix: m.keep_suffix })
                } else if let Some(v) = map.get("Replace") {
                    #[derive(Deserialize)]
                    struct R { style: ReplaceStyle }
                    let r: R = serde_json::from_value(v.clone()).map_err(serde::de::Error::custom)?;
                    Ok(Strategy::Replace { style: r.style })
                } else {
                    Err(serde::de::Error::custom("未知的策略类型"))
                }
            }
            _ => Err(serde::de::Error::custom("期望字符串或对象")),
        }
    }
}

/// 某种敏感类型对应的脱敏策略配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub sensitive_type: SensitiveType,
    pub strategy: Strategy,
    /// 是否启用一致性替换（相同原文 → 相同脱敏结果）
    pub consistent: bool,
}

/// 全局脱敏配置（内部格式，供 apply_desensitize 使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 每种类型的策略配置
    pub strategies: Vec<StrategyConfig>,
}

/// 自定义词典条目（与前端 DictEntry 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictEntry {
    /// 词条文本
    pub text: String,
    /// 敏感信息类型
    pub sensitive_type: SensitiveType,
    /// 匹配模式
    pub match_mode: MatchMode,
    /// 模版替换时的替换值（可选，仅在模版替换模式下使用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    /// 词条所属语言（None = 所有语言生效）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// 是否为内置词条（内置词条不可删除）
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub builtin: bool,
}

/// 词典匹配模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchMode {
    /// 精确匹配
    Exact,
    /// 模糊匹配
    Fuzzy,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            strategies: vec![
                StrategyConfig {
                    sensitive_type: SensitiveType::Phone,
                    strategy: Strategy::Mask { keep_prefix: 3, keep_suffix: 4 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::IdCard,
                    strategy: Strategy::Mask { keep_prefix: 3, keep_suffix: 4 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::Email,
                    strategy: Strategy::Mask { keep_prefix: 1, keep_suffix: 0 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::BankCard,
                    strategy: Strategy::Mask { keep_prefix: 4, keep_suffix: 4 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::PersonName,
                    strategy: Strategy::Mask { keep_prefix: 1, keep_suffix: 0 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::Address,
                    strategy: Strategy::Generalize,
                    consistent: false,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::OrgName,
                    strategy: Strategy::Mask { keep_prefix: 2, keep_suffix: 0 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::IpAddress,
                    strategy: Strategy::Mask { keep_prefix: 0, keep_suffix: 0 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::LandlinePhone,
                    strategy: Strategy::Mask { keep_prefix: 4, keep_suffix: 0 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::LicensePlate,
                    strategy: Strategy::Mask { keep_prefix: 2, keep_suffix: 0 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::CreditCode,
                    strategy: Strategy::Mask { keep_prefix: 4, keep_suffix: 4 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::Title,
                    strategy: Strategy::Replace { style: ReplaceStyle::Fake },
                    consistent: false,
                },
                // 英文特有类型
                StrategyConfig {
                    sensitive_type: SensitiveType::Ssn,
                    strategy: Strategy::Mask { keep_prefix: 0, keep_suffix: 4 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::CreditCard,
                    strategy: Strategy::Mask { keep_prefix: 0, keep_suffix: 4 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::UsPhone,
                    strategy: Strategy::Mask { keep_prefix: 0, keep_suffix: 4 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::UkPhone,
                    strategy: Strategy::Mask { keep_prefix: 0, keep_suffix: 4 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::Passport,
                    strategy: Strategy::Mask { keep_prefix: 2, keep_suffix: 0 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::Iban,
                    strategy: Strategy::Mask { keep_prefix: 4, keep_suffix: 4 },
                    consistent: true,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::ZipCode,
                    strategy: Strategy::Generalize,
                    consistent: false,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::UkPostcode,
                    strategy: Strategy::Generalize,
                    consistent: false,
                },
                StrategyConfig {
                    sensitive_type: SensitiveType::DriversLicense,
                    strategy: Strategy::Mask { keep_prefix: 2, keep_suffix: 0 },
                    consistent: true,
                },
            ],
        }
    }
}

/// IPC 传输用的策略配置格式（与前端 Record<string, Strategy> 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyMap {
    pub strategies: std::collections::HashMap<String, Strategy>,
    #[serde(default)]
    pub replace_style: ReplaceStyle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_style_default() {
        assert_eq!(ReplaceStyle::default(), ReplaceStyle::Fake);
    }

    #[test]
    fn test_deserialize_legacy_replace_string() {
        let json = r#""Replace""#;
        let strategy: Strategy = serde_json::from_str(json).unwrap();
        assert_eq!(strategy, Strategy::Replace { style: ReplaceStyle::Fake });
    }

    #[test]
    fn test_deserialize_new_replace_with_style() {
        let json = r#"{"Replace":{"style":"Mou"}}"#;
        let strategy: Strategy = serde_json::from_str(json).unwrap();
        assert_eq!(strategy, Strategy::Replace { style: ReplaceStyle::Mou });
    }

    #[test]
    fn test_deserialize_generalize_unchanged() {
        let json = r#""Generalize""#;
        let strategy: Strategy = serde_json::from_str(json).unwrap();
        assert_eq!(strategy, Strategy::Generalize);
    }

    #[test]
    fn test_deserialize_mask_unchanged() {
        let json = r#"{"Mask":{"keep_prefix":3,"keep_suffix":4}}"#;
        let strategy: Strategy = serde_json::from_str(json).unwrap();
        assert_eq!(strategy, Strategy::Mask { keep_prefix: 3, keep_suffix: 4 });
    }

    #[test]
    fn test_serialize_replace_with_style() {
        let strategy = Strategy::Replace { style: ReplaceStyle::Ordinal };
        let json = serde_json::to_string(&strategy).unwrap();
        assert_eq!(json, r#"{"Replace":{"style":"Ordinal"}}"#);
    }

    #[test]
    fn test_strategy_map_with_replace_style() {
        let json = r#"{"strategies":{"PersonName":{"Replace":{"style":"Mou"}},"Phone":{"Mask":{"keep_prefix":3,"keep_suffix":4}}},"replace_style":"Mou"}"#;
        let map: StrategyMap = serde_json::from_str(json).unwrap();
        assert_eq!(map.replace_style, ReplaceStyle::Mou);
    }

    #[test]
    fn test_strategy_map_legacy_no_replace_style() {
        let json = r#"{"strategies":{"PersonName":"Replace"}}"#;
        let map: StrategyMap = serde_json::from_str(json).unwrap();
        assert_eq!(map.replace_style, ReplaceStyle::Fake);
    }
}
