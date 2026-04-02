# i18n 英文支持实现计划（一期）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为Dimkey添加英文界面和英文敏感信息识别引擎（正则），支持语言切换。

**Architecture:** 后端通过 Tauri State 维护全局语言状态，正则引擎按语言加载不同规则集，假数据按语言分目录。前端用 react-i18next 管理翻译，启动时同步语言到后端。

**Tech Stack:** react-i18next, i18next-browser-languagedetector (前端); Rust Language enum + RwLock State (后端)

**Spec:** `docs/superpowers/specs/2026-04-02-i18n-english-support-design.md`

---

## File Structure

### Rust 后端新增/修改

| 操作 | 文件 | 职责 |
|------|------|------|
| Create | `src-tauri/src/models/language.rs` | Language 枚举定义 |
| Create | `src-tauri/src/engine/rules/mod.rs` | 规则模块入口 |
| Create | `src-tauri/src/engine/rules/common.rs` | 通用规则（Email, IP） |
| Create | `src-tauri/src/engine/rules/zh.rs` | 中文规则（现有规则迁移） |
| Create | `src-tauri/src/engine/rules/en.rs` | 英文规则（SSN, US Phone 等） |
| Create | `src-tauri/src/commands/language.rs` | set_language / get_language 命令 |
| Create | `src-tauri/resources/fake_data/en/person_names.json` | 英文人名 |
| Create | `src-tauri/resources/fake_data/en/org_components.json` | 英文组织名 |
| Create | `src-tauri/resources/fake_data/en/address_components.json` | 英文地址 |
| Create | `src-tauri/resources/fake_data/en/titles.json` | 英文职位 |
| Create | `src-tauri/resources/fake_data/en/patterns.json` | 英文格式模式 |
| Modify | `src-tauri/src/models/mod.rs` | 添加 language 模块 |
| Modify | `src-tauri/src/models/sensitive.rs` | 扩展 SensitiveType 枚举 |
| Modify | `src-tauri/src/models/strategy.rs` | 添加英文类型默认策略 |
| Modify | `src-tauri/src/engine/mod.rs` | 添加 rules 模块 |
| Modify | `src-tauri/src/engine/regex_engine.rs` | 按语言加载规则 |
| Modify | `src-tauri/src/desensitizer/replace.rs` | 按语言加载假数据，英文替换风格 |
| Modify | `src-tauri/src/desensitizer/generalize.rs` | 英文泛化逻辑 |
| Modify | `src-tauri/src/commands/desensitize.rs` | 扩展类型映射函数 |
| Modify | `src-tauri/src/commands/mod.rs` | 添加 language 模块 |
| Modify | `src-tauri/src/lib.rs` | 注册全局语言状态和命令 |

### 前端新增/修改

| 操作 | 文件 | 职责 |
|------|------|------|
| Create | `src/i18n.ts` | i18next 初始化配置 |
| Create | `src/locales/zh.json` | 中文翻译文件 |
| Create | `src/locales/en.json` | 英文翻译文件 |
| Create | `src/components/LanguageSwitcher/index.tsx` | 语言切换器组件 |
| Modify | `src/main.tsx` | 引入 i18n 初始化 |
| Modify | `src/types/index.ts` | 扩展类型定义，标签改为 i18n key |
| Modify | `src/layouts/WorkspaceLayout.tsx` | 替换硬编码中文 |
| Modify | 各 pages/ 和 components/ | 替换硬编码中文为 t() |

---

## Task 1: Language 枚举与全局状态

**Files:**
- Create: `src-tauri/src/models/language.rs`
- Modify: `src-tauri/src/models/mod.rs:1-5`
- Modify: `src-tauri/src/lib.rs:8,31-34,44-45,81,130-183`
- Create: `src-tauri/src/commands/language.rs`
- Modify: `src-tauri/src/commands/mod.rs`

- [ ] **Step 1: 创建 Language 枚举**

```rust
// src-tauri/src/models/language.rs
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
```

- [ ] **Step 2: 注册模块**

在 `src-tauri/src/models/mod.rs` 头部添加:
```rust
pub mod language;
```

- [ ] **Step 3: 创建语言命令**

```rust
// src-tauri/src/commands/language.rs
use std::sync::RwLock;
use tauri::State;
use crate::models::language::Language;

/// 全局语言状态
pub struct AppLanguage(pub RwLock<Language>);

#[tauri::command]
pub fn set_language(lang: String, state: State<AppLanguage>) -> Result<(), String> {
    let language = Language::from_str_loose(&lang);
    let mut current = state.0.write().map_err(|e| format!("语言状态锁失败: {}", e))?;
    *current = language;
    Ok(())
}

#[tauri::command]
pub fn get_language(state: State<AppLanguage>) -> Result<String, String> {
    let current = state.0.read().map_err(|e| format!("语言状态锁失败: {}", e))?;
    let s = match *current {
        Language::Zh => "zh",
        Language::En => "en",
    };
    Ok(s.to_string())
}
```

在 `src-tauri/src/commands/mod.rs` 添加:
```rust
pub mod language;
```

- [ ] **Step 4: 在 lib.rs 注册全局状态和命令**

在 `src-tauri/src/lib.rs` 中:

1. 添加 import:
```rust
use commands::language::{AppLanguage, set_language, get_language};
```

2. 在 `setup` 闭包中（NER 引擎初始化之后）添加:
```rust
// 初始化语言状态（默认中文）
app.manage(AppLanguage(std::sync::RwLock::new(
    crate::models::language::Language::default(),
)));
```

3. 在 `invoke_handler` 中添加:
```rust
set_language,
get_language,
```

- [ ] **Step 5: 运行测试验证**

Run: `cd src-tauri && cargo test models::language`
Expected: 2 tests pass

- [ ] **Step 6: 运行编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过，无错误

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/models/language.rs src-tauri/src/models/mod.rs \
  src-tauri/src/commands/language.rs src-tauri/src/commands/mod.rs \
  src-tauri/src/lib.rs
git commit -m "feat(i18n): 添加 Language 枚举和全局语言状态管理"
```

---

## Task 2: 扩展 SensitiveType 枚举

**Files:**
- Modify: `src-tauri/src/models/sensitive.rs:125-154`
- Modify: `src-tauri/src/commands/desensitize.rs:16-36,331-347`

- [ ] **Step 1: 扩展 SensitiveType 枚举**

在 `src-tauri/src/models/sensitive.rs` 中，替换 SensitiveType 枚举定义（第125-154行）:

```rust
/// 敏感信息类型枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SensitiveType {
    // ---- 通用类型 ----
    /// 邮箱
    Email,
    /// IP 地址
    IpAddress,

    // ---- 中文特有 ----
    /// 手机号（中国）
    Phone,
    /// 身份证号（中国）
    IdCard,
    /// 银行卡号（中国）
    BankCard,
    /// 固定电话（中国）
    LandlinePhone,
    /// 车牌号（中国）
    LicensePlate,
    /// 统一社会信用代码（中国）
    CreditCode,

    // ---- 英文特有 ----
    /// 社会安全号码（美国 SSN）
    Ssn,
    /// 信用卡号（国际，Luhn 校验）
    CreditCard,
    /// 美国电话
    UsPhone,
    /// 英国电话
    UkPhone,
    /// 护照号
    Passport,
    /// 国际银行账号（IBAN）
    Iban,
    /// 邮政编码（美国 ZIP）
    ZipCode,
    /// 邮编（英国 Postcode）
    UkPostcode,
    /// 驾照号码
    DriversLicense,

    // ---- NER 实体（中英通用） ----
    /// 人名
    PersonName,
    /// 机构名
    OrgName,
    /// 地址
    Address,
    /// 职位
    Title,

    // ---- 自定义 ----
    /// 自定义词条
    Custom(String),
}
```

- [ ] **Step 2: 扩展 string_to_sensitive_type 和 sensitive_type_to_key**

在 `src-tauri/src/commands/desensitize.rs` 中更新两个函数:

`string_to_sensitive_type`（第16-36行）添加英文类型分支:
```rust
pub fn string_to_sensitive_type(key: &str) -> SensitiveType {
    match key {
        "Phone" => SensitiveType::Phone,
        "IdCard" => SensitiveType::IdCard,
        "BankCard" => SensitiveType::BankCard,
        "Email" => SensitiveType::Email,
        "IpAddress" => SensitiveType::IpAddress,
        "LandlinePhone" => SensitiveType::LandlinePhone,
        "LicensePlate" => SensitiveType::LicensePlate,
        "CreditCode" => SensitiveType::CreditCode,
        "Ssn" => SensitiveType::Ssn,
        "CreditCard" => SensitiveType::CreditCard,
        "UsPhone" => SensitiveType::UsPhone,
        "UkPhone" => SensitiveType::UkPhone,
        "Passport" => SensitiveType::Passport,
        "Iban" => SensitiveType::Iban,
        "ZipCode" => SensitiveType::ZipCode,
        "UkPostcode" => SensitiveType::UkPostcode,
        "DriversLicense" => SensitiveType::DriversLicense,
        "PersonName" => SensitiveType::PersonName,
        "OrgName" => SensitiveType::OrgName,
        "Address" => SensitiveType::Address,
        "Title" => SensitiveType::Title,
        other => {
            let custom_text = other.strip_prefix("Custom:").unwrap_or(other);
            SensitiveType::Custom(custom_text.to_string())
        }
    }
}
```

`sensitive_type_to_key`（第331-347行）添加英文类型分支:
```rust
pub fn sensitive_type_to_key(st: &SensitiveType) -> String {
    match st {
        SensitiveType::Phone => "Phone".to_string(),
        SensitiveType::IdCard => "IdCard".to_string(),
        SensitiveType::BankCard => "BankCard".to_string(),
        SensitiveType::Email => "Email".to_string(),
        SensitiveType::IpAddress => "IpAddress".to_string(),
        SensitiveType::LandlinePhone => "LandlinePhone".to_string(),
        SensitiveType::LicensePlate => "LicensePlate".to_string(),
        SensitiveType::CreditCode => "CreditCode".to_string(),
        SensitiveType::Ssn => "Ssn".to_string(),
        SensitiveType::CreditCard => "CreditCard".to_string(),
        SensitiveType::UsPhone => "UsPhone".to_string(),
        SensitiveType::UkPhone => "UkPhone".to_string(),
        SensitiveType::Passport => "Passport".to_string(),
        SensitiveType::Iban => "Iban".to_string(),
        SensitiveType::ZipCode => "ZipCode".to_string(),
        SensitiveType::UkPostcode => "UkPostcode".to_string(),
        SensitiveType::DriversLicense => "DriversLicense".to_string(),
        SensitiveType::PersonName => "PersonName".to_string(),
        SensitiveType::OrgName => "OrgName".to_string(),
        SensitiveType::Address => "Address".to_string(),
        SensitiveType::Title => "Title".to_string(),
        SensitiveType::Custom(s) => format!("Custom:{}", s),
    }
}
```

- [ ] **Step 3: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过（可能有未使用变体警告，忽略）

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/models/sensitive.rs src-tauri/src/commands/desensitize.rs
git commit -m "feat(i18n): 扩展 SensitiveType 枚举，添加英文敏感信息类型"
```

---

## Task 3: 正则引擎按语言加载规则

**Files:**
- Create: `src-tauri/src/engine/rules/mod.rs`
- Create: `src-tauri/src/engine/rules/common.rs`
- Create: `src-tauri/src/engine/rules/zh.rs`
- Create: `src-tauri/src/engine/rules/en.rs`
- Modify: `src-tauri/src/engine/mod.rs:1-4`
- Modify: `src-tauri/src/engine/regex_engine.rs:1-88`

- [ ] **Step 1: 创建规则模块入口**

```rust
// src-tauri/src/engine/rules/mod.rs
pub mod common;
pub mod zh;
pub mod en;
```

- [ ] **Step 2: 提取 RegexRule 和 BoundaryCheck 到公共位置**

修改 `src-tauri/src/engine/regex_engine.rs`，将 `BoundaryCheck` 和 `RegexRule` 改为 pub，以便规则模块引用:

```rust
/// 边界检查类型
#[derive(Clone)]
pub enum BoundaryCheck {
    /// 前后不能是数字
    NotDigit,
    /// 前后不能是字母或数字
    NotAlphanumeric,
    /// 不检查边界
    None,
}

/// 单条正则规则定义
pub struct RegexRule {
    /// 编译后的正则表达式
    pub regex: Regex,
    /// 对应的敏感类型
    pub sensitive_type: SensitiveType,
    /// 边界检查
    pub boundary: BoundaryCheck,
}
```

- [ ] **Step 3: 创建通用规则**

```rust
// src-tauri/src/engine/rules/common.rs
use regex::Regex;
use crate::models::sensitive::SensitiveType;
use super::super::regex_engine::{RegexRule, BoundaryCheck};

/// Email + IPv4 通用规则
pub fn rules() -> Vec<RegexRule> {
    vec![
        RegexRule {
            regex: Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap(),
            sensitive_type: SensitiveType::Email,
            boundary: BoundaryCheck::None,
        },
        RegexRule {
            regex: Regex::new(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}").unwrap(),
            sensitive_type: SensitiveType::IpAddress,
            boundary: BoundaryCheck::NotDigit,
        },
    ]
}
```

- [ ] **Step 4: 迁移中文规则**

```rust
// src-tauri/src/engine/rules/zh.rs
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
        RegexRule {
            regex: Regex::new(r"[0-9A-HJ-NPQRTUWXY]{2}\d{6}[0-9A-HJ-NPQRTUWXY]{10}").unwrap(),
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
        // 6. 车牌号
        RegexRule {
            regex: Regex::new(r"[京津沪渝冀豫云辽黑湘皖鲁新苏浙赣鄂桂甘晋蒙陕吉闽贵粤川青藏琼宁][A-Z][A-HJ-NP-Z0-9]{5}").unwrap(),
            sensitive_type: SensitiveType::LicensePlate,
            boundary: BoundaryCheck::None,
        },
    ]
}
```

- [ ] **Step 5: 创建英文规则**

```rust
// src-tauri/src/engine/rules/en.rs
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
        // 3. 信用卡号（16位，可有分隔符）— 后续 detect 时用 Luhn 校验过滤
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
    ]
}
```

- [ ] **Step 6: 修改 RegexEngine 支持按语言构建**

修改 `src-tauri/src/engine/regex_engine.rs` 的 `new()` 方法，添加 `for_language()`:

```rust
use crate::models::language::Language;

impl RegexEngine {
    /// 创建中文正则引擎（向后兼容）
    pub fn new() -> Self {
        Self::for_language(Language::Zh)
    }

    /// 按语言创建正则引擎
    pub fn for_language(lang: Language) -> Self {
        let mut rules = Vec::new();

        // 语言特有规则（优先级高，先匹配）
        match lang {
            Language::Zh => rules.extend(super::rules::zh::rules()),
            Language::En => rules.extend(super::rules::en::rules()),
        }

        // 通用规则（Email, IP）
        rules.extend(super::rules::common::rules());

        Self { rules }
    }
}
```

删除 `new()` 中原来的 `let rules = vec![...]` 硬编码规则。

- [ ] **Step 7: 注册 rules 模块**

在 `src-tauri/src/engine/mod.rs` 添加:
```rust
pub mod rules;
```

- [ ] **Step 8: 编译和测试**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

Run: `cd src-tauri && cargo test engine::regex_engine`
Expected: 现有测试通过（`new()` 仍默认中文）

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/engine/rules/ src-tauri/src/engine/mod.rs \
  src-tauri/src/engine/regex_engine.rs
git commit -m "feat(i18n): 正则引擎按语言加载规则，添加英文识别规则"
```

---

## Task 4: 英文信用卡 Luhn 校验

**Files:**
- Modify: `src-tauri/src/engine/regex_engine.rs` (detect 逻辑中添加 Luhn 过滤)

- [ ] **Step 1: 写测试**

在 `src-tauri/src/engine/regex_engine.rs` 的 tests 模块中添加:

```rust
#[test]
fn test_luhn_check() {
    assert!(luhn_check("4111111111111111")); // Visa 测试卡号
    assert!(luhn_check("5500000000000004")); // Mastercard
    assert!(!luhn_check("1234567890123456")); // 无效
}
```

- [ ] **Step 2: 实现 Luhn 校验函数**

在 `regex_engine.rs` 中添加（`impl RegexEngine` 外部）:

```rust
/// Luhn 算法校验信用卡号
fn luhn_check(number: &str) -> bool {
    let digits: Vec<u32> = number
        .chars()
        .filter(|c| c.is_ascii_digit())
        .filter_map(|c| c.to_digit(10))
        .collect();
    if digits.len() < 13 {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                d
            }
        })
        .sum();
    sum % 10 == 0
}
```

- [ ] **Step 3: 在 detect_text 中对 CreditCard 类型添加 Luhn 过滤**

在 `regex_engine.rs` 的 `detect_text` 方法中，匹配到 `SensitiveType::CreditCard` 后追加校验:

在现有 match 找到结果后、push 到 items 之前:
```rust
// 信用卡号额外 Luhn 校验
if rule.sensitive_type == SensitiveType::CreditCard {
    let digits_only: String = matched_text.chars().filter(|c| c.is_ascii_digit()).collect();
    if !luhn_check(&digits_only) {
        continue;
    }
}
```

- [ ] **Step 4: 运行测试**

Run: `cd src-tauri && cargo test engine::regex_engine`
Expected: 所有测试通过

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/engine/regex_engine.rs
git commit -m "feat(i18n): 英文信用卡号 Luhn 校验过滤"
```

---

## Task 5: 英文假数据资源

**Files:**
- Create: `src-tauri/resources/fake_data/en/person_names.json`
- Create: `src-tauri/resources/fake_data/en/org_components.json`
- Create: `src-tauri/resources/fake_data/en/address_components.json`
- Create: `src-tauri/resources/fake_data/en/titles.json`
- Create: `src-tauri/resources/fake_data/en/patterns.json`
- Rename: 现有 `fake_data/*.json` → `fake_data/zh/*.json`

- [ ] **Step 1: 将现有假数据移入 zh/ 子目录**

```bash
mkdir -p src-tauri/resources/fake_data/zh
mv src-tauri/resources/fake_data/person_names.json src-tauri/resources/fake_data/zh/
mv src-tauri/resources/fake_data/org_components.json src-tauri/resources/fake_data/zh/
mv src-tauri/resources/fake_data/address_components.json src-tauri/resources/fake_data/zh/
mv src-tauri/resources/fake_data/titles.json src-tauri/resources/fake_data/zh/
mv src-tauri/resources/fake_data/patterns.json src-tauri/resources/fake_data/zh/
```

- [ ] **Step 2: 更新 replace.rs 中的 include_str! 路径**

将 `src-tauri/src/desensitizer/replace.rs` 第12-16行的路径更新:

```rust
const PERSON_NAMES_JSON: &str = include_str!("../../resources/fake_data/zh/person_names.json");
const ORG_COMPONENTS_JSON: &str = include_str!("../../resources/fake_data/zh/org_components.json");
const TITLES_JSON: &str = include_str!("../../resources/fake_data/zh/titles.json");
const ADDRESS_COMPONENTS_JSON: &str = include_str!("../../resources/fake_data/zh/address_components.json");
const PATTERNS_JSON: &str = include_str!("../../resources/fake_data/zh/patterns.json");
```

- [ ] **Step 3: 编译验证路径正确**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 4: 创建英文人名数据**

```json
// src-tauri/resources/fake_data/en/person_names.json
{
  "first_names": [
    "James", "Mary", "Robert", "Patricia", "John", "Jennifer", "Michael", "Linda",
    "David", "Elizabeth", "William", "Barbara", "Richard", "Susan", "Joseph", "Jessica",
    "Thomas", "Sarah", "Christopher", "Karen", "Charles", "Lisa", "Daniel", "Nancy",
    "Matthew", "Betty", "Anthony", "Margaret", "Mark", "Sandra", "Donald", "Ashley",
    "Steven", "Dorothy", "Paul", "Kimberly", "Andrew", "Emily", "Joshua", "Donna",
    "Kenneth", "Michelle", "Kevin", "Carol", "Brian", "Amanda", "George", "Melissa",
    "Timothy", "Deborah", "Ronald", "Stephanie", "Edward", "Rebecca", "Jason", "Sharon",
    "Jeffrey", "Laura", "Ryan", "Cynthia", "Jacob", "Kathleen", "Gary", "Amy",
    "Nicholas", "Angela", "Eric", "Shirley", "Jonathan", "Anna", "Stephen", "Brenda"
  ],
  "last_names": [
    "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller", "Davis",
    "Rodriguez", "Martinez", "Hernandez", "Lopez", "Gonzalez", "Wilson", "Anderson",
    "Thomas", "Taylor", "Moore", "Jackson", "Martin", "Lee", "Perez", "Thompson",
    "White", "Harris", "Sanchez", "Clark", "Ramirez", "Lewis", "Robinson", "Walker",
    "Young", "Allen", "King", "Wright", "Scott", "Torres", "Nguyen", "Hill",
    "Flores", "Green", "Adams", "Nelson", "Baker", "Hall", "Rivera", "Campbell",
    "Mitchell", "Carter", "Roberts", "Turner", "Phillips", "Evans", "Collins", "Stewart",
    "Morris", "Reed", "Cook", "Morgan", "Bell", "Murphy", "Bailey", "Cooper"
  ]
}
```

- [ ] **Step 5: 创建英文组织名数据**

```json
// src-tauri/resources/fake_data/en/org_components.json
{
  "prefixes": [
    "Global", "Pacific", "Atlantic", "Summit", "Pioneer", "Apex", "Vertex", "Horizon",
    "Sterling", "Meridian", "Pinnacle", "Nexus", "Vanguard", "Crest", "Beacon",
    "Alliance", "Catalyst", "Quantum", "Zenith", "Ascent", "Nova", "Radiant",
    "Prime", "Elite", "Core", "Bridge", "Emerald", "Titan", "Arc", "Pulse"
  ],
  "industries": [
    "Technologies", "Solutions", "Systems", "Dynamics", "Analytics", "Consulting",
    "Industries", "Ventures", "Capital", "Financial", "Healthcare", "Energy",
    "Logistics", "Communications", "Media", "Engineering", "Digital", "Bio",
    "Pharma", "Defense", "Research", "Trading", "Properties", "Insurance"
  ],
  "suffixes": [
    "Inc.", "Corp.", "LLC", "Ltd.", "Group", "Holdings", "Partners", "Associates",
    "International", "Co."
  ]
}
```

- [ ] **Step 6: 创建英文地址数据**

```json
// src-tauri/resources/fake_data/en/address_components.json
{
  "cities": [
    "New York, NY", "Los Angeles, CA", "Chicago, IL", "Houston, TX", "Phoenix, AZ",
    "Philadelphia, PA", "San Antonio, TX", "San Diego, CA", "Dallas, TX", "San Jose, CA",
    "Austin, TX", "Jacksonville, FL", "Fort Worth, TX", "Columbus, OH", "Charlotte, NC",
    "Seattle, WA", "Denver, CO", "Boston, MA", "Nashville, TN", "Portland, OR",
    "London", "Manchester", "Birmingham", "Edinburgh", "Bristol"
  ],
  "streets": [
    "Main St", "Oak Ave", "Maple Dr", "Cedar Ln", "Pine St", "Elm Rd",
    "Washington Blvd", "Park Ave", "Lake St", "Hill Rd", "River Dr", "Spring St",
    "Church St", "High St", "Mill Rd", "School Ln", "Forest Ave", "Meadow Dr",
    "Sunset Blvd", "Broadway", "Market St", "Union St", "Liberty Ave", "Valley Rd"
  ],
  "numbers": [1, 5, 12, 23, 47, 88, 100, 155, 200, 312, 456, 789, 1024, 1500, 2200]
}
```

- [ ] **Step 7: 创建英文职位数据**

```json
// src-tauri/resources/fake_data/en/titles.json
[
  "Software Engineer", "Product Manager", "Data Analyst", "Project Manager",
  "Marketing Director", "Sales Representative", "Financial Analyst", "HR Manager",
  "Operations Manager", "Business Analyst", "UX Designer", "DevOps Engineer",
  "Account Manager", "Quality Assurance Engineer", "Research Scientist",
  "Technical Lead", "Customer Success Manager", "Content Strategist",
  "Supply Chain Manager", "Legal Counsel", "Executive Assistant",
  "Chief Technology Officer", "Vice President", "Senior Consultant",
  "Program Director", "Systems Administrator", "Database Administrator",
  "Network Engineer", "Security Analyst", "Compliance Officer"
]
```

- [ ] **Step 8: 创建英文模式数据**

```json
// src-tauri/resources/fake_data/en/patterns.json
{
  "us_phone_area_codes": ["212", "310", "312", "415", "512", "617", "713", "818", "202", "305", "404", "503"],
  "ssn_area_prefixes": ["001", "100", "200", "300", "400", "500", "600", "700"],
  "zip_prefixes": ["100", "900", "606", "770", "850", "191", "782", "941", "752", "802"],
  "credit_card_prefixes": ["4111", "4012", "5100", "5200", "5300", "3714", "6011"],
  "uk_phone_prefixes": ["020", "0121", "0131", "0141", "0151", "0161", "07911", "07700"],
  "iban_country_codes": ["GB", "DE", "FR", "ES", "IT", "NL", "BE", "AT", "CH"],
  "email_names": ["john", "jane", "mike", "sarah", "david", "emma", "alex", "lisa", "chris", "anna"],
  "email_domains": ["gmail.com", "yahoo.com", "outlook.com", "company.com", "example.org"]
}
```

- [ ] **Step 9: Commit**

```bash
git add src-tauri/resources/fake_data/zh/ src-tauri/resources/fake_data/en/ \
  src-tauri/src/desensitizer/replace.rs
git rm src-tauri/resources/fake_data/person_names.json \
  src-tauri/resources/fake_data/org_components.json \
  src-tauri/resources/fake_data/address_components.json \
  src-tauri/resources/fake_data/titles.json \
  src-tauri/resources/fake_data/patterns.json 2>/dev/null || true
git commit -m "feat(i18n): 假数据按语言分目录，添加英文假数据"
```

---

## Task 6: Replace 脱敏器支持英文

**Files:**
- Modify: `src-tauri/src/desensitizer/replace.rs`

这是工作量最大的 Task。需要:
1. 加载英文假数据
2. 英文版 Mou 风格 → `[REDACTED]`
3. 英文版 Ordinal 风格 → `Person-1`
4. 英文特有类型的 Fake 替换逻辑

- [ ] **Step 1: 添加英文假数据结构和加载**

在 `replace.rs` 顶部添加英文假数据常量和结构:

```rust
// 英文假数据
const EN_PERSON_NAMES_JSON: &str = include_str!("../../resources/fake_data/en/person_names.json");
const EN_ORG_COMPONENTS_JSON: &str = include_str!("../../resources/fake_data/en/org_components.json");
const EN_TITLES_JSON: &str = include_str!("../../resources/fake_data/en/titles.json");
const EN_ADDRESS_COMPONENTS_JSON: &str = include_str!("../../resources/fake_data/en/address_components.json");
const EN_PATTERNS_JSON: &str = include_str!("../../resources/fake_data/en/patterns.json");

#[derive(Deserialize)]
struct EnPersonNames {
    first_names: Vec<String>,
    last_names: Vec<String>,
}

#[derive(Deserialize)]
struct EnAddressComponents {
    cities: Vec<String>,
    streets: Vec<String>,
    numbers: Vec<u32>,
}

#[derive(Deserialize)]
struct EnPatterns {
    us_phone_area_codes: Vec<String>,
    ssn_area_prefixes: Vec<String>,
    zip_prefixes: Vec<String>,
    credit_card_prefixes: Vec<String>,
    uk_phone_prefixes: Vec<String>,
    iban_country_codes: Vec<String>,
    email_names: Vec<String>,
    email_domains: Vec<String>,
}

struct EnFakeData {
    person_names: EnPersonNames,
    org_components: OrgComponents,
    titles: Vec<String>,
    address_components: EnAddressComponents,
    patterns: EnPatterns,
}

static EN_FAKE_DATA: OnceLock<EnFakeData> = OnceLock::new();

fn get_en_fake_data() -> &'static EnFakeData {
    EN_FAKE_DATA.get_or_init(|| EnFakeData {
        person_names: serde_json::from_str(EN_PERSON_NAMES_JSON).expect("parse en/person_names.json"),
        org_components: serde_json::from_str(EN_ORG_COMPONENTS_JSON).expect("parse en/org_components.json"),
        titles: serde_json::from_str(EN_TITLES_JSON).expect("parse en/titles.json"),
        address_components: serde_json::from_str(EN_ADDRESS_COMPONENTS_JSON).expect("parse en/address_components.json"),
        patterns: serde_json::from_str(EN_PATTERNS_JSON).expect("parse en/patterns.json"),
    })
}
```

- [ ] **Step 2: 给 ReplaceState 添加英文假数据生成方法**

在 `impl ReplaceState` 中添加英文方法:

```rust
    // ---- 英文假数据生成 ----

    /// 英文人名
    pub fn next_en_name(&mut self) -> String {
        let data = get_en_fake_data();
        let first_pool = data.person_names.first_names.len() as u32;
        let last_pool = data.person_names.last_names.len() as u32;

        let counter = self.counters.entry("en_name".to_string()).or_insert(0);
        let mut rng = StdRng::seed_from_u64(self.seed.wrapping_add(NAME_SEED_OFFSET).wrapping_add(*counter as u64));
        let first = &data.person_names.first_names[rng.gen_range(0..first_pool as usize)];
        let last = &data.person_names.last_names[rng.gen_range(0..last_pool as usize)];
        *counter += 1;
        format!("{} {}", first, last)
    }

    /// 英文组织名
    pub fn next_en_org(&mut self) -> String {
        let data = get_en_fake_data();
        let counter = self.counters.entry("en_org".to_string()).or_insert(0);
        let mut rng = StdRng::seed_from_u64(self.seed.wrapping_add(ORG_SEED_OFFSET).wrapping_add(*counter as u64));
        let prefix = &data.org_components.prefixes[rng.gen_range(0..data.org_components.prefixes.len())];
        let industry = &data.org_components.industries[rng.gen_range(0..data.org_components.industries.len())];
        let suffix = &data.org_components.suffixes[rng.gen_range(0..data.org_components.suffixes.len())];
        *counter += 1;
        format!("{} {} {}", prefix, industry, suffix)
    }

    /// 英文地址
    pub fn next_en_address(&mut self) -> String {
        let data = get_en_fake_data();
        let counter = self.counters.entry("en_address".to_string()).or_insert(0);
        let mut rng = StdRng::seed_from_u64(self.seed.wrapping_add(ADDRESS_SEED_OFFSET).wrapping_add(*counter as u64));
        let num = data.address_components.numbers[rng.gen_range(0..data.address_components.numbers.len())];
        let street = &data.address_components.streets[rng.gen_range(0..data.address_components.streets.len())];
        let city = &data.address_components.cities[rng.gen_range(0..data.address_components.cities.len())];
        *counter += 1;
        format!("{} {}, {}", num, street, city)
    }

    /// 英文职位
    pub fn next_en_title(&mut self) -> String {
        let data = get_en_fake_data();
        let counter = self.counters.entry("en_title".to_string()).or_insert(0);
        let idx = *counter % data.titles.len();
        *counter += 1;
        data.titles[idx].clone()
    }

    // ---- 英文 Redacted 风格（替代中文"某式"） ----

    pub fn next_redacted_name(&mut self) -> String {
        let key = "redacted_name".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        if *count == 1 { "[REDACTED]".to_string() } else { format!("[REDACTED-{}]", count) }
    }

    pub fn next_redacted_org(&mut self) -> String {
        let key = "redacted_org".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        if *count == 1 { "[REDACTED ORG]".to_string() } else { format!("[REDACTED ORG-{}]", count) }
    }

    pub fn next_redacted_address(&mut self) -> String {
        let key = "redacted_address".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        if *count == 1 { "[REDACTED ADDR]".to_string() } else { format!("[REDACTED ADDR-{}]", count) }
    }

    pub fn next_redacted_title(&mut self) -> String {
        let key = "redacted_title".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        if *count == 1 { "[REDACTED]".to_string() } else { format!("[REDACTED-{}]", count) }
    }

    // ---- 英文序号式 ----

    pub fn next_en_ordinal_name(&mut self) -> String {
        let key = "en_ordinal_name".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        format!("Person-{}", count)
    }

    pub fn next_en_ordinal_org(&mut self) -> String {
        let key = "en_ordinal_org".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        format!("Organization-{}", count)
    }

    pub fn next_en_ordinal_address(&mut self) -> String {
        let key = "en_ordinal_address".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        format!("Address-{}", count)
    }

    pub fn next_en_ordinal_title(&mut self) -> String {
        let key = "en_ordinal_title".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        format!("Title-{}", count)
    }
```

- [ ] **Step 3: 添加英文版 apply_replace 函数**

在 `replace.rs` 中添加新函数（与现有 `apply_replace` 并列）:

```rust
/// 英文假数据替换
pub fn apply_replace_en(
    text: &str,
    sensitive_type: &SensitiveType,
    state: &mut ReplaceState,
    style: &ReplaceStyle,
) -> String {
    let data = get_en_fake_data();
    let mut rng = rand::thread_rng();

    match sensitive_type {
        SensitiveType::PersonName => match style {
            ReplaceStyle::Fake => state.next_en_name(),
            ReplaceStyle::Mou => state.next_redacted_name(),
            ReplaceStyle::Ordinal => state.next_en_ordinal_name(),
        },
        SensitiveType::OrgName => match style {
            ReplaceStyle::Fake => state.next_en_org(),
            ReplaceStyle::Mou => state.next_redacted_org(),
            ReplaceStyle::Ordinal => state.next_en_ordinal_org(),
        },
        SensitiveType::Title => match style {
            ReplaceStyle::Fake => state.next_en_title(),
            ReplaceStyle::Mou => state.next_redacted_title(),
            ReplaceStyle::Ordinal => state.next_en_ordinal_title(),
        },
        SensitiveType::Address => match style {
            ReplaceStyle::Fake => state.next_en_address(),
            ReplaceStyle::Mou => state.next_redacted_address(),
            ReplaceStyle::Ordinal => state.next_en_ordinal_address(),
        },
        SensitiveType::Ssn => {
            let p = &data.patterns;
            let area = pick(&mut rng, &p.ssn_area_prefixes);
            let group: u32 = rng.gen_range(10..99);
            let serial: u32 = rng.gen_range(1000..9999);
            format!("{}-{:02}-{:04}", area, group, serial)
        },
        SensitiveType::UsPhone => {
            let p = &data.patterns;
            let area = pick(&mut rng, &p.us_phone_area_codes);
            let mid: u32 = rng.gen_range(200..999);
            let last: u32 = rng.gen_range(1000..9999);
            format!("({}) {}-{}", area, mid, last)
        },
        SensitiveType::UkPhone => {
            let p = &data.patterns;
            let prefix = pick(&mut rng, &p.uk_phone_prefixes);
            let suffix: u32 = rng.gen_range(100000..999999);
            format!("{} {}", prefix, suffix)
        },
        SensitiveType::CreditCard => {
            let p = &data.patterns;
            let prefix = pick(&mut rng, &p.credit_card_prefixes);
            let remaining = 16 - prefix.len();
            let mut digits = String::with_capacity(remaining);
            for _ in 0..remaining {
                digits.push(char::from(b'0' + rng.gen_range(0..10u8)));
            }
            let raw = format!("{}{}", prefix, digits);
            format!("{}-{}-{}-{}", &raw[0..4], &raw[4..8], &raw[8..12], &raw[12..16])
        },
        SensitiveType::Iban => {
            let p = &data.patterns;
            let country = pick(&mut rng, &p.iban_country_codes);
            let check: u32 = rng.gen_range(10..99);
            let bank: u32 = rng.gen_range(1000..9999);
            let account: u64 = rng.gen_range(10000000..99999999);
            format!("{}{} {} {:08}", country, check, bank, account)
        },
        SensitiveType::ZipCode => {
            let p = &data.patterns;
            let prefix = pick(&mut rng, &p.zip_prefixes);
            let suffix: u32 = rng.gen_range(10..99);
            format!("{}{}", prefix, suffix)
        },
        SensitiveType::UkPostcode => {
            let letters = "ABCDEFGHJKLMNPRSTUVWXYZ";
            let chars: Vec<char> = letters.chars().collect();
            let l1 = chars[rng.gen_range(0..chars.len())];
            let d1: u32 = rng.gen_range(1..9);
            let d2: u32 = rng.gen_range(1..9);
            let l2 = chars[rng.gen_range(0..chars.len())];
            let l3 = chars[rng.gen_range(0..chars.len())];
            format!("{}{} {}{}{}", l1, d1, d2, l2, l3)
        },
        SensitiveType::Passport => {
            let letter = (b'A' + rng.gen_range(0..26u8)) as char;
            let num: u32 = rng.gen_range(1000000..9999999);
            format!("{}{}", letter, num)
        },
        SensitiveType::DriversLicense => {
            // 通用格式：字母+数字
            let mut dl = String::with_capacity(10);
            for _ in 0..2 { dl.push((b'A' + rng.gen_range(0..26u8)) as char); }
            for _ in 0..8 { dl.push(char::from(b'0' + rng.gen_range(0..10u8))); }
            dl
        },
        // 通用类型复用中文版逻辑
        SensitiveType::Email => {
            let p = &data.patterns;
            let name = pick(&mut rng, &p.email_names);
            let num: u32 = rng.gen_range(100..999);
            let domain = pick(&mut rng, &p.email_domains);
            format!("{}{}@{}", name, num, domain)
        },
        SensitiveType::IpAddress => {
            format!("{}.{}.{}.{}", rng.gen_range(10..200), rng.gen_range(0..256), rng.gen_range(0..256), rng.gen_range(1..255))
        },
        SensitiveType::Custom(_) => "[REDACTED]".to_string(),
        // 中文类型在英文模式下不应出现，保底处理
        _ => "[REDACTED]".to_string(),
    }
}
```

- [ ] **Step 4: 运行测试**

Run: `cd src-tauri && cargo test desensitizer`
Expected: 现有测试通过

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/desensitizer/replace.rs
git commit -m "feat(i18n): Replace 脱敏器支持英文假数据生成"
```

---

## Task 7: Generalize 脱敏器支持英文

**Files:**
- Modify: `src-tauri/src/desensitizer/generalize.rs`

- [ ] **Step 1: 写英文泛化测试**

在 `generalize.rs` 的 tests 模块中添加:

```rust
#[test]
fn test_generalize_en_address() {
    let result = generalize_en_address("123 Main St, New York, NY 10001");
    assert!(result.contains("NY") || result.contains("New York"));
}

#[test]
fn test_generalize_en_title() {
    assert_eq!(generalize_en_title("Senior Software Engineer"), "Technical Staff");
    assert_eq!(generalize_en_title("Chief Executive Officer"), "Senior Executive");
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
```

- [ ] **Step 2: 实现英文泛化函数**

```rust
use crate::models::language::Language;

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
            // 保留区号，隐藏后半段
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
    // 尝试按逗号分割，保留最后的城市/州部分
    let parts: Vec<&str> = text.split(',').map(|s| s.trim()).collect();
    if parts.len() >= 2 {
        // 返回最后两部分（通常是 city, state）
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
```

- [ ] **Step 3: 运行测试**

Run: `cd src-tauri && cargo test desensitizer::generalize`
Expected: 所有测试通过（新旧）

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/desensitizer/generalize.rs
git commit -m "feat(i18n): Generalize 脱敏器支持英文泛化逻辑"
```

---

## Task 8: 英文类型默认策略 + 脱敏命令语言感知

**Files:**
- Modify: `src-tauri/src/models/strategy.rs:103-170`
- Modify: `src-tauri/src/commands/desensitize.rs` (replace/generalize 调用处)
- Modify: `src-tauri/src/commands/detect.rs` (正则引擎按语言构建)

- [ ] **Step 1: 添加英文类型默认策略**

在 `strategy.rs` 的 `impl Default for AppConfig` 中，添加英文类型的默认策略:

```rust
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
```

- [ ] **Step 2: 修改 detect 命令读取全局语言状态**

在 `commands/detect.rs` 的 `detect_by_regex` 中:

1. 添加参数:
```rust
use crate::commands::language::AppLanguage;
// 函数签名添加 language_state
language_state: tauri::State<'_, AppLanguage>,
```

2. 构建引擎时使用语言:
```rust
let lang = *language_state.0.read().map_err(|e| format!("语言状态锁失败: {}", e))?;
let engine = RegexEngine::for_language(lang);
```

- [ ] **Step 3: 修改 desensitize 命令中的 replace/generalize 调用**

在 `commands/desensitize.rs` 的 `apply_desensitize` 中:

1. 添加语言状态参数:
```rust
language_state: tauri::State<'_, AppLanguage>,
```

2. 读取语言:
```rust
let lang = *language_state.0.read().map_err(|e| format!("语言状态锁失败: {}", e))?;
```

3. Replace 调用处（约第159行）改为:
```rust
Strategy::Replace { ref style } => {
    let r = match lang {
        Language::En => replace::apply_replace_en(&item.text, &item.sensitive_type, &mut replace_state, style),
        Language::Zh => replace::apply_replace(&item.text, &item.sensitive_type, &mut replace_state, style),
    };
    (r, StrategyType::Replace)
}
```

4. Generalize 调用处（约第163行）改为:
```rust
Strategy::Generalize => {
    let r = generalize::apply_generalize_for_language(&item.text, &item.sensitive_type, lang);
    (r, StrategyType::Generalize)
}
```

5. 对 `apply_desensitize_by_columns` 做同样的修改。

- [ ] **Step 4: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 5: 运行全部测试**

Run: `cd src-tauri && cargo test`
Expected: 所有测试通过

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/models/strategy.rs src-tauri/src/commands/detect.rs \
  src-tauri/src/commands/desensitize.rs
git commit -m "feat(i18n): 英文默认策略 + 脱敏命令语言感知"
```

---

## Task 9: 前端 i18n 基础设施

**Files:**
- Create: `src/i18n.ts`
- Create: `src/locales/zh.json`
- Create: `src/locales/en.json`
- Modify: `src/main.tsx`
- Modify: `package.json` (添加依赖)

- [ ] **Step 1: 安装 i18n 依赖**

```bash
npm install react-i18next i18next i18next-browser-languagedetector
```

- [ ] **Step 2: 创建 i18n 配置**

```typescript
// src/i18n.ts
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";
import zh from "./locales/zh.json";
import en from "./locales/en.json";

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      zh: { translation: zh },
      en: { translation: en },
    },
    fallbackLng: "zh",
    interpolation: {
      escapeValue: false,
    },
    detection: {
      order: ["localStorage", "navigator"],
      caches: ["localStorage"],
      lookupLocalStorage: "dimkey-lang",
    },
  });

export default i18n;
```

- [ ] **Step 3: 创建中文翻译文件**

创建 `src/locales/zh.json`，提取现有硬编码文本。文件较大，关键结构如下:

```json
{
  "app": {
    "name": "Dimkey",
    "tagline": "本地运行，零网络通信，保护您的数据隐私"
  },
  "sensitiveType": {
    "Phone": "手机号",
    "IdCard": "身份证",
    "BankCard": "银行卡",
    "Email": "邮箱",
    "IpAddress": "IP地址",
    "LandlinePhone": "固定电话",
    "LicensePlate": "车牌号",
    "CreditCode": "信用代码",
    "PersonName": "人名",
    "OrgName": "机构/公司名",
    "Address": "地址",
    "Title": "职位",
    "Custom": "自定义",
    "Ssn": "SSN",
    "CreditCard": "信用卡",
    "UsPhone": "美国电话",
    "UkPhone": "英国电话",
    "Passport": "护照号",
    "Iban": "IBAN",
    "ZipCode": "邮编(US)",
    "UkPostcode": "邮编(UK)",
    "DriversLicense": "驾照号"
  },
  "strategy": {
    "Mask": "掩码",
    "Replace": "替换",
    "Generalize": "泛化"
  },
  "replaceStyle": {
    "Fake": "假数据",
    "Mou": "某式",
    "Ordinal": "序号式"
  },
  "common": {
    "confirm": "确认",
    "cancel": "取消",
    "save": "保存",
    "delete": "删除",
    "add": "添加",
    "back": "返回",
    "close": "关闭",
    "export": "导出",
    "loading": "加载中...",
    "copyFailed": "复制到剪贴板失败",
    "copied": "已复制",
    "exact": "精确",
    "fuzzy": "模糊"
  },
  "steps": {
    "parsing": "解析文件中…",
    "detecting": "扫描识别中…",
    "desensitizing": "执行脱敏中…",
    "saving": "保存记录中…",
    "done": "处理完成"
  },
  "home": {
    "dropHint": "拖拽文件到此处，自动脱敏",
    "dropRelease": "松开导入文件",
    "selectFile": "点击选择文件",
    "supportedFormats": "支持 Excel / CSV / Word / TXT / PDF",
    "pasteText": "粘贴文本脱敏",
    "recentTasks": "最近脱敏任务",
    "viewAll": "查看全部",
    "items": "{{count}} 处",
    "restore": "还原",
    "restoring": "还原中..."
  },
  "preview": {
    "detectError": "识别过程出错",
    "noContent": "未加载文件内容",
    "noSensitive": "未识别到敏感信息，无需脱敏",
    "desensitizeFailed": "脱敏处理失败",
    "detecting": "正在识别敏感信息...",
    "processing": "脱敏处理中...",
    "startDesensitize": "开始脱敏",
    "nerDetecting": "NER识别中..."
  },
  "result": {
    "exportSuccess": "文件导出成功",
    "openDir": "打开目录",
    "copyFile": "复制文件",
    "totalItems": "共 {{count}} 处"
  },
  "history": {
    "loadFailed": "加载历史任务失败",
    "deleted": "任务已删除",
    "deleteFailed": "删除失败",
    "empty": "暂无脱敏历史",
    "emptyHint": "完成一次脱敏导出后，任务会自动保存在这里",
    "sensitiveCount": "{{count}} 处敏感信息",
    "restored": "已还原",
    "viewComparison": "查看对比"
  },
  "restorePage": {
    "restoreSuccess": "还原文件导出成功",
    "exportFailed": "导出失败",
    "totalRestored": "共还原 {{count}} 处",
    "noResult": "暂无还原结果",
    "pasteAiReply": "粘贴 AI 回复",
    "importRestore": "导入文件还原",
    "noMatch": "AI回复中没有匹配到脱敏占位符"
  },
  "strategyPanel": {
    "title": "策略配置",
    "selectWorkspace": "选择工作区查看配置",
    "desensitizeMode": "脱敏模式",
    "templateMode": "模版替换",
    "rules": "识别规则",
    "replaceStyleLabel": "替换风格",
    "keepPrefix": "前",
    "keepSuffix": "后",
    "dict": "自定义词典",
    "dictMapping": "词典映射 ({{set}}/{{total}} 已设替换值)",
    "inputSensitiveText": "输入敏感词文本",
    "replaceTo": "替换为",
    "replaceToRequired": "替换为（必填）",
    "whitelist": "排除词",
    "whitelistHint": "白名单中的文本不会被任何引擎识别为敏感信息",
    "removeFromWhitelist": "从白名单移除",
    "aliasGroup": "别名关联",
    "noAliasGroup": "暂无别名组，在预览中点击敏感项可创建",
    "members": "{{count}}个",
    "deleteGroup": "删除别名组",
    "primary": "主名",
    "addAlias": "添加别名...",
    "memberAdded": "成员已添加",
    "groupDeleted": "别名组已删除",
    "output": "输出设置",
    "defaultOutputDir": "默认输出目录",
    "clearDir": "清除",
    "selectDir": "点击选择目录"
  },
  "aliasLink": {
    "mode": "关联模式",
    "clickToAdd": "点击敏感项继续添加...",
    "confirmLink": "确认关联（{{count}}）",
    "minMembers": "至少需要选择 2 个成员",
    "createSuccess": "别名组创建成功"
  },
  "password": {
    "title": "文件已加密",
    "hint": "Word / Excel 文件需要密码才能打开",
    "placeholder": "请输入文件密码",
    "decrypting": "解密中...",
    "decrypt": "解密导入",
    "safetyHint": "密码仅用于本次解密，不会保存"
  },
  "about": {
    "name": "Dimkey",
    "description": "本地文档脱敏工具，纯本地运行，零网络通信。",
    "feedback": "反馈建议",
    "copyEmail": "复制邮箱"
  },
  "analytics": {
    "title": "匿名使用统计",
    "description": "我们会收集匿名使用统计以改进产品，例如功能使用频率和文件类型分布。不包含任何文件内容或个人信息。你可以随时在左侧栏底部关闭此功能。",
    "decline": "不参与",
    "accept": "好的"
  },
  "update": {
    "title": "软件更新",
    "newVersion": "新版本 v{{version}} 可用",
    "updateNow": "立即更新",
    "downloading": "正在下载更新...",
    "downloaded": "更新已下载完成，重启后生效",
    "restart": "立即重启",
    "failed": "更新失败: {{message}}"
  },
  "fileQueue": {
    "exported": "该文件已导出",
    "failed": "处理失败",
    "waitOrder": "请按顺序处理文件",
    "allDone": "所有文件已处理完成"
  },
  "dropzone": {
    "formatNotSupported": "{{count}} 个文件格式不支持，已跳过",
    "maxQueue": "最多同时处理 {{max}} 个文件，已取前 {{max}} 个",
    "clipboardFailed": "读取剪贴板失败，请检查浏览器权限",
    "waitQueue": "请先完成当前队列中的文件处理",
    "restoreTitle": "还原操作",
    "historyTitle": "处理历史"
  },
  "fileSuffix": {
    "desensitized": "_脱敏",
    "restored": "_还原",
    "template": "_替换"
  },
  "topBar": {
    "dictManager": "词典管理",
    "strategyConfig": "策略配置",
    "historyTasks": "历史任务",
    "desensitizeTool": "脱敏工具"
  },
  "summaryBar": {
    "totalDetected": "共识别 {{count}} 处",
    "showAll": "全部显示",
    "hideAll": "全部隐藏"
  },
  "typeSelector": {
    "label": "脱敏项"
  },
  "textToolbar": {
    "selectType": "选择类型",
    "moreTypes": "更多类型"
  }
}
```

- [ ] **Step 4: 创建英文翻译文件**

创建 `src/locales/en.json`，结构同上，文本为英文:

```json
{
  "app": {
    "name": "Dimkey",
    "tagline": "Runs locally, zero network traffic, protecting your data privacy"
  },
  "sensitiveType": {
    "Phone": "Phone (CN)",
    "IdCard": "ID Card (CN)",
    "BankCard": "Bank Card (CN)",
    "Email": "Email",
    "IpAddress": "IP Address",
    "LandlinePhone": "Landline (CN)",
    "LicensePlate": "License Plate (CN)",
    "CreditCode": "Credit Code (CN)",
    "PersonName": "Person Name",
    "OrgName": "Organization",
    "Address": "Address",
    "Title": "Job Title",
    "Custom": "Custom",
    "Ssn": "SSN",
    "CreditCard": "Credit Card",
    "UsPhone": "US Phone",
    "UkPhone": "UK Phone",
    "Passport": "Passport",
    "Iban": "IBAN",
    "ZipCode": "ZIP Code",
    "UkPostcode": "UK Postcode",
    "DriversLicense": "Driver's License"
  },
  "strategy": {
    "Mask": "Mask",
    "Replace": "Replace",
    "Generalize": "Generalize"
  },
  "replaceStyle": {
    "Fake": "Fake Data",
    "Mou": "Redacted",
    "Ordinal": "Ordinal"
  },
  "common": {
    "confirm": "Confirm",
    "cancel": "Cancel",
    "save": "Save",
    "delete": "Delete",
    "add": "Add",
    "back": "Back",
    "close": "Close",
    "export": "Export",
    "loading": "Loading...",
    "copyFailed": "Failed to copy to clipboard",
    "copied": "Copied",
    "exact": "Exact",
    "fuzzy": "Fuzzy"
  },
  "steps": {
    "parsing": "Parsing file...",
    "detecting": "Detecting sensitive data...",
    "desensitizing": "Desensitizing...",
    "saving": "Saving record...",
    "done": "Complete"
  },
  "home": {
    "dropHint": "Drop files here to desensitize",
    "dropRelease": "Release to import file",
    "selectFile": "Click to select file",
    "supportedFormats": "Supports Excel / CSV / Word / TXT / PDF",
    "pasteText": "Paste text to desensitize",
    "recentTasks": "Recent tasks",
    "viewAll": "View all",
    "items": "{{count}} items",
    "restore": "Restore",
    "restoring": "Restoring..."
  },
  "preview": {
    "detectError": "Detection error",
    "noContent": "No file content loaded",
    "noSensitive": "No sensitive data detected",
    "desensitizeFailed": "Desensitization failed",
    "detecting": "Detecting sensitive data...",
    "processing": "Processing...",
    "startDesensitize": "Start desensitization",
    "nerDetecting": "NER detecting..."
  },
  "result": {
    "exportSuccess": "File exported successfully",
    "openDir": "Open folder",
    "copyFile": "Copy file",
    "totalItems": "{{count}} total items"
  },
  "history": {
    "loadFailed": "Failed to load history",
    "deleted": "Task deleted",
    "deleteFailed": "Delete failed",
    "empty": "No history yet",
    "emptyHint": "Tasks will be saved here after export",
    "sensitiveCount": "{{count}} sensitive items",
    "restored": "Restored",
    "viewComparison": "View comparison"
  },
  "restorePage": {
    "restoreSuccess": "Restored file exported successfully",
    "exportFailed": "Export failed",
    "totalRestored": "{{count}} items restored",
    "noResult": "No restore results",
    "pasteAiReply": "Paste AI reply",
    "importRestore": "Import file to restore",
    "noMatch": "No desensitized placeholders found in AI reply"
  },
  "strategyPanel": {
    "title": "Strategy Settings",
    "selectWorkspace": "Select workspace to view settings",
    "desensitizeMode": "Desensitize",
    "templateMode": "Template Replace",
    "rules": "Detection Rules",
    "replaceStyleLabel": "Replace Style",
    "keepPrefix": "Keep",
    "keepSuffix": "Last",
    "dict": "Custom Dictionary",
    "dictMapping": "Dictionary ({{set}}/{{total}} with replacements)",
    "inputSensitiveText": "Enter sensitive text",
    "replaceTo": "Replace with",
    "replaceToRequired": "Replace with (required)",
    "whitelist": "Exclusions",
    "whitelistHint": "Whitelisted text will not be detected as sensitive",
    "removeFromWhitelist": "Remove from whitelist",
    "aliasGroup": "Alias Groups",
    "noAliasGroup": "No alias groups. Click a sensitive item in preview to create one.",
    "members": "{{count}} members",
    "deleteGroup": "Delete alias group",
    "primary": "Primary",
    "addAlias": "Add alias...",
    "memberAdded": "Member added",
    "groupDeleted": "Alias group deleted",
    "output": "Output Settings",
    "defaultOutputDir": "Default output directory",
    "clearDir": "Clear",
    "selectDir": "Select directory"
  },
  "aliasLink": {
    "mode": "Link Mode",
    "clickToAdd": "Click sensitive items to add...",
    "confirmLink": "Confirm ({{count}})",
    "minMembers": "At least 2 members required",
    "createSuccess": "Alias group created"
  },
  "password": {
    "title": "File is encrypted",
    "hint": "This file requires a password to open",
    "placeholder": "Enter file password",
    "decrypting": "Decrypting...",
    "decrypt": "Decrypt & Import",
    "safetyHint": "Password is used only for this session and will not be saved"
  },
  "about": {
    "name": "Dimkey",
    "description": "Local document desensitization tool. Runs entirely offline.",
    "feedback": "Feedback",
    "copyEmail": "Copy email"
  },
  "analytics": {
    "title": "Anonymous Usage Statistics",
    "description": "We collect anonymous usage statistics to improve the product, such as feature usage frequency and file type distribution. No file content or personal information is included. You can disable this anytime in the sidebar.",
    "decline": "Decline",
    "accept": "OK"
  },
  "update": {
    "title": "Software Update",
    "newVersion": "New version v{{version}} available",
    "updateNow": "Update Now",
    "downloading": "Downloading update...",
    "downloaded": "Update downloaded. Restart to apply.",
    "restart": "Restart Now",
    "failed": "Update failed: {{message}}"
  },
  "fileQueue": {
    "exported": "File already exported",
    "failed": "Processing failed",
    "waitOrder": "Please process files in order",
    "allDone": "All files processed"
  },
  "dropzone": {
    "formatNotSupported": "{{count}} unsupported files skipped",
    "maxQueue": "Maximum {{max}} files at once",
    "clipboardFailed": "Failed to read clipboard",
    "waitQueue": "Please finish current queue first",
    "restoreTitle": "Restore",
    "historyTitle": "History"
  },
  "fileSuffix": {
    "desensitized": "_desensitized",
    "restored": "_restored",
    "template": "_replaced"
  },
  "topBar": {
    "dictManager": "Dictionary",
    "strategyConfig": "Settings",
    "historyTasks": "History",
    "desensitizeTool": "Desensitize Tool"
  },
  "summaryBar": {
    "totalDetected": "{{count}} detected",
    "showAll": "Show all",
    "hideAll": "Hide all"
  },
  "typeSelector": {
    "label": "Types"
  },
  "textToolbar": {
    "selectType": "Select type",
    "moreTypes": "More types"
  }
}
```

- [ ] **Step 5: 在 main.tsx 引入 i18n**

在 `src/main.tsx` 顶部导入（在 React import 之后）:
```typescript
import "./i18n";
```

- [ ] **Step 6: 编译验证**

Run: `npm run build`
Expected: 编译通过

- [ ] **Step 7: Commit**

```bash
git add src/i18n.ts src/locales/ src/main.tsx package.json package-lock.json
git commit -m "feat(i18n): 前端 react-i18next 基础设施 + 中英翻译文件"
```

---

## Task 10: 前端类型定义改造

**Files:**
- Modify: `src/types/index.ts:9-25,126-131,348-370`

- [ ] **Step 1: 扩展前端 SensitiveType 类型**

在 `src/types/index.ts` 中，扩展 SensitiveType 联合类型（第9-25行）:

```typescript
export type SensitiveType =
  // 通用
  | "Email"
  | "IpAddress"
  // 中文特有
  | "Phone"
  | "IdCard"
  | "BankCard"
  | "LandlinePhone"
  | "LicensePlate"
  | "CreditCode"
  // 英文特有
  | "Ssn"
  | "CreditCard"
  | "UsPhone"
  | "UkPhone"
  | "Passport"
  | "Iban"
  | "ZipCode"
  | "UkPostcode"
  | "DriversLicense"
  // NER
  | "PersonName"
  | "OrgName"
  | "Address"
  | "Title"
  // 自定义
  | { Custom: string };
```

- [ ] **Step 2: 将 SENSITIVE_TYPE_CONFIG 改为函数式获取**

将 `SENSITIVE_TYPE_CONFIG`（第349-363行）改为从 i18n 动态获取 label，颜色保持静态:

```typescript
import i18n from "../i18n";

/** 各敏感类型的颜色配置（静态） */
export const SENSITIVE_TYPE_COLORS: Record<string, { bgClass: string; textClass: string }> = {
  Phone:         { bgClass: "bg-blue-50",    textClass: "text-blue-700" },
  IdCard:        { bgClass: "bg-red-50",     textClass: "text-red-700" },
  BankCard:      { bgClass: "bg-orange-50",  textClass: "text-orange-700" },
  Email:         { bgClass: "bg-purple-50",  textClass: "text-purple-700" },
  IpAddress:     { bgClass: "bg-slate-100",  textClass: "text-slate-700" },
  LandlinePhone: { bgClass: "bg-cyan-50",    textClass: "text-cyan-700" },
  LicensePlate:  { bgClass: "bg-yellow-50",  textClass: "text-yellow-700" },
  CreditCode:    { bgClass: "bg-pink-50",    textClass: "text-pink-700" },
  PersonName:    { bgClass: "bg-green-50",   textClass: "text-green-700" },
  OrgName:       { bgClass: "bg-indigo-50",  textClass: "text-indigo-700" },
  Address:       { bgClass: "bg-amber-50",   textClass: "text-amber-700" },
  Title:         { bgClass: "bg-lime-50",    textClass: "text-lime-700" },
  Custom:        { bgClass: "bg-slate-50",   textClass: "text-slate-700" },
  // 英文类型
  Ssn:            { bgClass: "bg-red-50",     textClass: "text-red-700" },
  CreditCard:     { bgClass: "bg-orange-50",  textClass: "text-orange-700" },
  UsPhone:        { bgClass: "bg-blue-50",    textClass: "text-blue-700" },
  UkPhone:        { bgClass: "bg-cyan-50",    textClass: "text-cyan-700" },
  Passport:       { bgClass: "bg-pink-50",    textClass: "text-pink-700" },
  Iban:           { bgClass: "bg-indigo-50",  textClass: "text-indigo-700" },
  ZipCode:        { bgClass: "bg-amber-50",   textClass: "text-amber-700" },
  UkPostcode:     { bgClass: "bg-yellow-50",  textClass: "text-yellow-700" },
  DriversLicense: { bgClass: "bg-lime-50",    textClass: "text-lime-700" },
};

/** 获取敏感类型的显示配置（label 从 i18n 获取） */
export function getSensitiveTypeConfig(key: string): SensitiveTypeInfo {
  const colors = SENSITIVE_TYPE_COLORS[key] ?? SENSITIVE_TYPE_COLORS.Custom;
  const label = i18n.t(`sensitiveType.${key}`, { defaultValue: key });
  return { label, ...colors };
}

/** 获取策略标签 */
export function getStrategyLabel(type: StrategyType): string {
  return i18n.t(`strategy.${type}`);
}

/** 获取替换风格标签 */
export function getReplaceStyleLabel(style: ReplaceStyle): string {
  return i18n.t(`replaceStyle.${style}`);
}
```

保留原 `SENSITIVE_TYPE_CONFIG` 作为兼容导出（后续组件迁移时逐步替换）:
```typescript
/** @deprecated 使用 getSensitiveTypeConfig 替代 */
export const SENSITIVE_TYPE_CONFIG: Record<string, SensitiveTypeInfo> = new Proxy({} as Record<string, SensitiveTypeInfo>, {
  get(_target, key: string) {
    return getSensitiveTypeConfig(key);
  },
});

/** @deprecated 使用 getStrategyLabel 替代 */
export const STRATEGY_LABELS: Record<StrategyType, string> = new Proxy({} as Record<StrategyType, string>, {
  get(_target, key: string) {
    return getStrategyLabel(key as StrategyType);
  },
});

/** @deprecated 使用 getReplaceStyleLabel 替代 */
export const REPLACE_STYLE_LABELS: Record<ReplaceStyle, string> = new Proxy({} as Record<ReplaceStyle, string>, {
  get(_target, key: string) {
    return getReplaceStyleLabel(key as ReplaceStyle);
  },
});
```

- [ ] **Step 3: 编译验证**

Run: `npm run build`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add src/types/index.ts
git commit -m "feat(i18n): 前端类型定义扩展 + 标签函数化"
```

---

## Task 11: 语言切换器组件 + 启动同步

**Files:**
- Create: `src/components/LanguageSwitcher/index.tsx`
- Modify: `src/layouts/WorkspaceLayout.tsx` (集成切换器 + 替换关键硬编码文本)

- [ ] **Step 1: 创建语言切换器**

```typescript
// src/components/LanguageSwitcher/index.tsx
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

export default function LanguageSwitcher() {
  const { i18n } = useTranslation();

  const toggleLanguage = async () => {
    const next = i18n.language.startsWith("en") ? "zh" : "en";
    await i18n.changeLanguage(next);
    localStorage.setItem("dimkey-lang", next);
    await invoke("set_language", { lang: next });
  };

  const isEn = i18n.language.startsWith("en");

  return (
    <button
      onClick={toggleLanguage}
      className="flex items-center gap-1 px-2 py-1 text-xs text-gray-500 hover:text-gray-700 hover:bg-gray-100 rounded transition-colors"
      title={isEn ? "切换到中文" : "Switch to English"}
    >
      <span className="text-sm">{isEn ? "中" : "EN"}</span>
    </button>
  );
}
```

- [ ] **Step 2: 添加启动时语言同步逻辑**

在 `src/i18n.ts` 末尾添加:

```typescript
// 启动时同步语言到 Rust 后端
import { invoke } from "@tauri-apps/api/core";

i18n.on("initialized", () => {
  invoke("set_language", { lang: i18n.language }).catch(() => {});
});

i18n.on("languageChanged", (lng) => {
  invoke("set_language", { lang: lng }).catch(() => {});
});
```

- [ ] **Step 3: 在布局中集成语言切换器**

在 `WorkspaceLayout.tsx` 中导入并放置（具体位置取决于现有布局，通常在侧边栏底部或顶部标题栏）:

```typescript
import LanguageSwitcher from "../components/LanguageSwitcher";
// 在合适位置添加 <LanguageSwitcher />
```

- [ ] **Step 4: 编译验证**

Run: `npm run build`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add src/components/LanguageSwitcher/ src/i18n.ts src/layouts/WorkspaceLayout.tsx
git commit -m "feat(i18n): 语言切换器组件 + 启动时语言同步"
```

---

## Task 12: 前端组件 i18n 迁移（分批）

这是最大的前端改造 Task，需要将约 200+ 处硬编码中文替换为 `t()` 调用。建议分批进行:

**Batch 1: 核心布局和步骤**
- `src/layouts/WorkspaceLayout.tsx`

**Batch 2: 主要页面**
- `src/pages/HomePage/index.tsx`
- `src/pages/PreviewPage/index.tsx`
- `src/pages/HistoryPage/index.tsx`
- `src/pages/RestorePage/index.tsx`

**Batch 3: 策略面板组件**
- `src/components/StrategyPanel/` 下所有文件

**Batch 4: 中栏组件**
- `src/components/CenterPanel/` 下所有文件

**Batch 5: 其余组件**
- `src/components/TopBar/index.tsx`
- `src/components/SummaryBar/index.tsx`
- `src/components/TypeSelector/index.tsx`
- `src/components/SensitivePopover/index.tsx`
- `src/components/TextSelectionToolbar/index.tsx`
- `src/components/FileDropZone/index.tsx`
- `src/components/PasswordModal/index.tsx`
- `src/components/AboutModal/index.tsx`
- `src/components/AnalyticsConsent/index.tsx`
- `src/components/UpdateChecker/index.tsx`
- `src/components/AliasLinkMode/index.tsx`
- `src/components/DictManager/index.tsx`
- `src/components/ColumnRulePopover/index.tsx`

**每个文件的改造模式一致:**

- [ ] **Step 1: 在组件顶部添加 useTranslation hook**

```typescript
import { useTranslation } from "react-i18next";
// 在组件函数体内:
const { t } = useTranslation();
```

- [ ] **Step 2: 替换硬编码中文为 t() 调用**

示例（WorkspaceLayout.tsx 的 STEP_LABELS）:
```typescript
// Before:
const STEP_LABELS: Record<string, string> = {
  parsing: "解析文件中…",
  detecting: "扫描识别中…",
  // ...
};

// After:
// 移除静态常量，在使用处直接:
t(`steps.${step}`)
```

示例（按钮文本）:
```typescript
// Before:
<button>开始脱敏</button>

// After:
<button>{t("preview.startDesensitize")}</button>
```

- [ ] **Step 3: 每个 batch 完成后编译验证**

Run: `npm run build`
Expected: 编译通过

- [ ] **Step 4: 每个 batch 完成后 commit**

```bash
git commit -m "feat(i18n): 前端组件 i18n 迁移 - batch N"
```

注意：此 Task 工作量大，建议使用 subagent 并行处理不同 batch。每个 batch 独立工作，不存在依赖关系。

---

## Task 13: 端到端验证

**Files:** 无新文件

- [ ] **Step 1: Rust 全量测试**

Run: `cd src-tauri && cargo test`
Expected: 所有测试通过

- [ ] **Step 2: 前端编译**

Run: `npm run build`
Expected: 编译通过

- [ ] **Step 3: 开发模式启动测试**

Run: `cargo tauri dev`
Expected:
1. 应用正常启动
2. 默认跟随系统语言
3. 语言切换器可见并可切换
4. 切换到英文后，界面文本变为英文
5. 导入文件后，英文模式识别英文敏感信息（SSN、US Phone 等）
6. 脱敏功能正常工作

- [ ] **Step 4: 最终 commit**

```bash
git add -A
git commit -m "feat(i18n): 多语言支持一期完成 — 英文界面 + 英文识别引擎"
```
