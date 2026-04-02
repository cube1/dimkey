# ReplaceStyle 替换风格 实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 Replace 策略下新增三种替换风格（假数据/某式/序号式），支持法律文书脱敏场景。

**Architecture:** 在 `Strategy::Replace` 枚举变体中嵌入 `ReplaceStyle` 子类型，通过 `ReplaceState` 的新方法实现某式和序号式生成逻辑，复用现有一致性映射机制。全局配置 `replace_style` 字段控制 UI 默认值。

**Tech Stack:** Rust (serde, Tauri commands), React + TypeScript + TailwindCSS, Zustand

---

### Task 1: 新增 ReplaceStyle 枚举并修改 Strategy 枚举

**Files:**
- Modify: `src-tauri/src/models/strategy.rs`

**Step 1: 编写反序列化兼容性测试**

在 `src-tauri/src/models/strategy.rs` 文件底部添加测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_style_default() {
        assert_eq!(ReplaceStyle::default(), ReplaceStyle::Fake);
    }

    #[test]
    fn test_deserialize_legacy_replace_string() {
        // 旧版 JSON 中 "Replace" 是纯字符串
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
        // 旧版 JSON 没有 replace_style 字段
        let json = r#"{"strategies":{"PersonName":"Replace"}}"#;
        let map: StrategyMap = serde_json::from_str(json).unwrap();
        assert_eq!(map.replace_style, ReplaceStyle::Fake);
    }
}
```

**Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test models::strategy::tests -- --nocapture`
Expected: 编译失败，`ReplaceStyle` 未定义

**Step 3: 实现 ReplaceStyle 枚举**

在 `src-tauri/src/models/strategy.rs` 的 `use` 语句后（`Strategy` 枚举之前）添加：

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ReplaceStyle {
    Fake,
    Mou,
    Ordinal,
}

impl Default for ReplaceStyle {
    fn default() -> Self {
        ReplaceStyle::Fake
    }
}
```

**Step 4: 修改 Strategy 枚举为带 style 字段的 Replace**

将 `Strategy` 枚举从：

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Strategy {
    Mask { keep_prefix: usize, keep_suffix: usize },
    Replace,
    Generalize,
}
```

改为（移除 `Deserialize` derive，保留 `Serialize`）：

```rust
#[derive(Clone, Debug, Serialize, PartialEq)]
pub enum Strategy {
    Mask { keep_prefix: usize, keep_suffix: usize },
    Replace { style: ReplaceStyle },
    Generalize,
}
```

然后在下方添加自定义 Deserialize 实现，处理旧版 `"Replace"` 字符串的兼容：

```rust
impl<'de> serde::Deserialize<'de> for Strategy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match &value {
            serde_json::Value::String(s) => match s.as_str() {
                "Replace" => Ok(Strategy::Replace {
                    style: ReplaceStyle::default(),
                }),
                "Generalize" => Ok(Strategy::Generalize),
                other => Err(serde::de::Error::unknown_variant(
                    other,
                    &["Replace", "Generalize"],
                )),
            },
            serde_json::Value::Object(map) => {
                if let Some(v) = map.get("Mask") {
                    #[derive(Deserialize)]
                    struct M {
                        keep_prefix: usize,
                        keep_suffix: usize,
                    }
                    let m: M =
                        serde_json::from_value(v.clone()).map_err(serde::de::Error::custom)?;
                    Ok(Strategy::Mask {
                        keep_prefix: m.keep_prefix,
                        keep_suffix: m.keep_suffix,
                    })
                } else if let Some(v) = map.get("Replace") {
                    #[derive(Deserialize)]
                    struct R {
                        style: ReplaceStyle,
                    }
                    let r: R =
                        serde_json::from_value(v.clone()).map_err(serde::de::Error::custom)?;
                    Ok(Strategy::Replace { style: r.style })
                } else {
                    Err(serde::de::Error::custom("unknown strategy variant"))
                }
            }
            _ => Err(serde::de::Error::custom("expected string or object")),
        }
    }
}
```

**Step 5: 更新 StrategyMap 添加 replace_style 字段**

将 `StrategyMap` 从：

```rust
pub struct StrategyMap {
    pub strategies: std::collections::HashMap<String, Strategy>,
}
```

改为：

```rust
pub struct StrategyMap {
    pub strategies: std::collections::HashMap<String, Strategy>,
    #[serde(default)]
    pub replace_style: ReplaceStyle,
}
```

**Step 6: 更新所有 `Strategy::Replace` 引用**

在同一文件中，`Default for AppConfig` 的 `default()` 方法里所有 `Strategy::Replace` 改为 `Strategy::Replace { style: ReplaceStyle::Fake }`。搜索整个文件中 `Strategy::Replace` 并统一替换（注意已经带 `{ style: ... }` 的不要重复）。

**Step 7: 运行测试确认全部通过**

Run: `cd src-tauri && cargo test models::strategy::tests -- --nocapture`
Expected: 全部 PASS

**Step 8: 修复其他文件的编译错误**

`Strategy::Replace` 的使用遍布多个文件，改为 struct variant 后需要全部更新：

- `src-tauri/src/commands/desensitize.rs`: 所有 `Strategy::Replace =>` 改为 `Strategy::Replace { style } =>`（暂时忽略 `style`，后续 Task 使用）
- `src-tauri/src/commands/workspace.rs`: `default_strategies()` 函数中所有 `Strategy::Replace` 改为 `Strategy::Replace { style: ReplaceStyle::Fake }`
- `src-tauri/src/commands/config.rs`: 如有使用也更新

**Step 9: 确认全项目编译通过**

Run: `cd src-tauri && cargo check`
Expected: 无错误

**Step 10: 提交**

```bash
git add src-tauri/src/models/strategy.rs src-tauri/src/commands/
git commit -m "feat: 新增 ReplaceStyle 枚举，修改 Strategy::Replace 为带 style 字段"
```

---

### Task 2: 添加辅助函数（姓氏提取、组织后缀提取、中文数字）

**Files:**
- Modify: `src-tauri/src/desensitizer/replace.rs`

**Step 1: 编写辅助函数的测试**

在 `src-tauri/src/desensitizer/replace.rs` 的 `#[cfg(test)] mod tests` 中添加：

```rust
    #[test]
    fn test_extract_surname_single() {
        assert_eq!(extract_surname("张三"), "张");
        assert_eq!(extract_surname("李明华"), "李");
        assert_eq!(extract_surname("王"), "王");
    }

    #[test]
    fn test_extract_surname_compound() {
        assert_eq!(extract_surname("欧阳修"), "欧阳");
        assert_eq!(extract_surname("司马迁"), "司马");
        assert_eq!(extract_surname("上官婉儿"), "上官");
        assert_eq!(extract_surname("诸葛亮"), "诸葛");
    }

    #[test]
    fn test_extract_org_suffix() {
        assert_eq!(extract_org_suffix("腾讯科技有限公司"), "公司");
        assert_eq!(extract_org_suffix("北京市朝阳区人民法院"), "法院");
        assert_eq!(extract_org_suffix("中国人民银行"), "银行");
        assert_eq!(extract_org_suffix("北京大学"), "大学");
        assert_eq!(extract_org_suffix("某某机构"), "单位"); // 未匹配到关键词
    }

    #[test]
    fn test_to_chinese_numeral() {
        assert_eq!(to_chinese_numeral(2), "二");
        assert_eq!(to_chinese_numeral(3), "三");
        assert_eq!(to_chinese_numeral(10), "十");
        assert_eq!(to_chinese_numeral(11), "十一");
        assert_eq!(to_chinese_numeral(20), "二十");
        assert_eq!(to_chinese_numeral(21), "二十一");
    }
```

**Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test desensitizer::replace::tests::test_extract_surname -- --nocapture`
Expected: 编译失败，函数未定义

**Step 3: 实现辅助函数**

在 `replace.rs` 中（`ReplaceState` impl 之前或之后）添加：

```rust
/// 复姓表
const COMPOUND_SURNAMES: &[&str] = &[
    "欧阳", "太史", "端木", "上官", "司马", "东方", "独孤", "南宫",
    "万俟", "闻人", "夏侯", "诸葛", "尉迟", "公羊", "赫连", "澹台",
    "皇甫", "宗政", "濮阳", "公冶", "太叔", "申屠", "公孙", "慕容",
    "仲孙", "钟离", "长孙", "宇文", "司徒", "鲜于", "司空", "闾丘",
    "令狐", "百里", "呼延", "东郭", "南门", "西门", "左丘", "第五",
];

/// 组织后缀关键词表（从长到短排列，优先匹配更长的后缀）
const ORG_SUFFIXES: &[&str] = &[
    "人民检察院", "人民法院", "检察院", "基金会", "有限公司",
    "法院", "银行", "医院", "学校", "大学", "集团", "协会",
    "中心", "公司", "局", "委", "所", "院", "厂",
];

/// 从人名中提取姓氏（优先匹配复姓）
fn extract_surname(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() >= 2 {
        let two: String = chars[..2].iter().collect();
        if COMPOUND_SURNAMES.contains(&two.as_str()) {
            return two;
        }
    }
    if chars.is_empty() {
        return String::new();
    }
    chars[0].to_string()
}

/// 从组织名中提取后缀关键词（如 "公司"、"法院"）
fn extract_org_suffix(org: &str) -> String {
    for suffix in ORG_SUFFIXES {
        if org.ends_with(suffix) {
            return suffix.to_string();
        }
    }
    "单位".to_string()
}

/// 数字转中文数词（用于某式序号：二、三、...、十一、...）
fn to_chinese_numeral(n: usize) -> String {
    const DIGITS: &[&str] = &["零", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
    if n <= 10 {
        return DIGITS[n].to_string();
    }
    if n < 20 {
        return format!("十{}", if n % 10 == 0 { "" } else { DIGITS[n % 10] });
    }
    let tens = n / 10;
    let ones = n % 10;
    if ones == 0 {
        format!("{}十", DIGITS[tens])
    } else {
        format!("{}十{}", DIGITS[tens], DIGITS[ones])
    }
}
```

**Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test desensitizer::replace::tests::test_extract -- --nocapture`
Run: `cd src-tauri && cargo test desensitizer::replace::tests::test_to_chinese -- --nocapture`
Expected: 全部 PASS

**Step 5: 提交**

```bash
git add src-tauri/src/desensitizer/replace.rs
git commit -m "feat: 添加姓氏提取、组织后缀提取、中文数字转换辅助函数"
```

---

### Task 3: 实现某式（Mou）替换生成逻辑

**Files:**
- Modify: `src-tauri/src/desensitizer/replace.rs`

**Step 1: 编写某式替换测试**

在 tests 模块中添加：

```rust
    use crate::models::strategy::ReplaceStyle;

    #[test]
    fn test_mou_person_name() {
        let mut state = ReplaceState::new(42, HashMap::new());
        // 不同姓：各自生成无序号
        let r1 = state.next_mou_name("张三");
        assert_eq!(r1, "张某");
        let r2 = state.next_mou_name("李四");
        assert_eq!(r2, "李某");
        // 同姓第二个：加序号
        let r3 = state.next_mou_name("张四");
        assert_eq!(r3, "张某二");
        // 复姓
        let r4 = state.next_mou_name("欧阳修");
        assert_eq!(r4, "欧阳某");
    }

    #[test]
    fn test_mou_org_name() {
        let mut state = ReplaceState::new(42, HashMap::new());
        let r1 = state.next_mou_org("腾讯科技有限公司");
        assert_eq!(r1, "某公司");
        let r2 = state.next_mou_org("北京市朝阳区人民法院");
        assert_eq!(r2, "某法院");
        // 同后缀第二个
        let r3 = state.next_mou_org("百度在线网络技术有限公司");
        assert_eq!(r3, "某公司二");
    }

    #[test]
    fn test_mou_address() {
        let mut state = ReplaceState::new(42, HashMap::new());
        let r1 = state.next_mou_address("北京市朝阳区建国路100号");
        assert_eq!(r1, "某地");
        let r2 = state.next_mou_address("上海市浦东新区陆家嘴");
        assert_eq!(r2, "某地二");
    }

    #[test]
    fn test_mou_title() {
        let mut state = ReplaceState::new(42, HashMap::new());
        let r1 = state.next_mou_title("总经理");
        assert_eq!(r1, "某职务");
        let r2 = state.next_mou_title("副总裁");
        assert_eq!(r2, "某职务二");
    }
```

**Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test desensitizer::replace::tests::test_mou -- --nocapture`
Expected: 编译失败，方法未定义

**Step 3: 实现某式生成方法**

在 `ReplaceState` 的 `impl` 块中添加：

```rust
    /// 某式：人名替换（张某、张某二、李某...）
    pub fn next_mou_name(&mut self, original: &str) -> String {
        let surname = extract_surname(original);
        let key = format!("mou_surname_{}", surname);
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        if *count == 1 {
            format!("{}某", surname)
        } else {
            format!("{}某{}", surname, to_chinese_numeral(*count))
        }
    }

    /// 某式：组织名替换（某公司、某法院、某公司二...）
    pub fn next_mou_org(&mut self, original: &str) -> String {
        let suffix = extract_org_suffix(original);
        let key = format!("mou_org_{}", suffix);
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        if *count == 1 {
            format!("某{}", suffix)
        } else {
            format!("某{}{}", suffix, to_chinese_numeral(*count))
        }
    }

    /// 某式：地址替换（某地、某地二...）
    pub fn next_mou_address(&mut self, _original: &str) -> String {
        let key = "mou_address".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        if *count == 1 {
            "某地".to_string()
        } else {
            format!("某地{}", to_chinese_numeral(*count))
        }
    }

    /// 某式：职务替换（某职务、某职务二...）
    pub fn next_mou_title(&mut self, _original: &str) -> String {
        let key = "mou_title".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        if *count == 1 {
            "某职务".to_string()
        } else {
            format!("某职务{}", to_chinese_numeral(*count))
        }
    }
```

**Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test desensitizer::replace::tests::test_mou -- --nocapture`
Expected: 全部 PASS

**Step 5: 提交**

```bash
git add src-tauri/src/desensitizer/replace.rs
git commit -m "feat: 实现某式（Mou）替换生成逻辑"
```

---

### Task 4: 实现序号式（Ordinal）替换生成逻辑

**Files:**
- Modify: `src-tauri/src/desensitizer/replace.rs`

**Step 1: 编写序号式替换测试**

在 tests 模块中添加：

```rust
    #[test]
    fn test_ordinal_person_name() {
        let mut state = ReplaceState::new(42, HashMap::new());
        assert_eq!(state.next_ordinal_name(), "甲");
        assert_eq!(state.next_ordinal_name(), "乙");
        assert_eq!(state.next_ordinal_name(), "丙");
        // 生成 10 个用完天干后
        for _ in 3..10 {
            state.next_ordinal_name();
        }
        assert_eq!(state.next_ordinal_name(), "当事人11");
        assert_eq!(state.next_ordinal_name(), "当事人12");
    }

    #[test]
    fn test_ordinal_org_name() {
        let mut state = ReplaceState::new(42, HashMap::new());
        let r1 = state.next_ordinal_org("腾讯科技有限公司");
        assert_eq!(r1, "甲公司");
        let r2 = state.next_ordinal_org("百度集团");
        assert_eq!(r2, "乙集团");
        let r3 = state.next_ordinal_org("阿里巴巴有限公司");
        assert_eq!(r3, "丙公司");
    }

    #[test]
    fn test_ordinal_address() {
        let mut state = ReplaceState::new(42, HashMap::new());
        assert_eq!(state.next_ordinal_address(), "A地址");
        assert_eq!(state.next_ordinal_address(), "B地址");
        // 生成 26 个用完字母后
        for _ in 2..26 {
            state.next_ordinal_address();
        }
        assert_eq!(state.next_ordinal_address(), "地址27");
    }

    #[test]
    fn test_ordinal_title() {
        let mut state = ReplaceState::new(42, HashMap::new());
        assert_eq!(state.next_ordinal_title(), "职务一");
        assert_eq!(state.next_ordinal_title(), "职务二");
        assert_eq!(state.next_ordinal_title(), "职务三");
    }

    #[test]
    fn test_style_does_not_affect_phone() {
        // 格式型实体不受风格影响，始终用假数据
        let mut state = ReplaceState::new(42, HashMap::new());
        let r = apply_replace("13812345678", &SensitiveType::Phone, &mut state, &ReplaceStyle::Mou);
        // 应该是 11 位手机号，不是 "某手机号"
        assert_eq!(r.len(), 11);
        assert!(r.chars().all(|c| c.is_ascii_digit()));
    }
```

**Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test desensitizer::replace::tests::test_ordinal -- --nocapture`
Expected: 编译失败，方法未定义

**Step 3: 实现序号式生成方法**

在 `ReplaceState` 的 `impl` 块中添加：

```rust
    /// 天干序列，用于序号式人名
    const TIANGAN: &'static [&'static str] = &[
        "甲", "乙", "丙", "丁", "戊", "己", "庚", "辛", "壬", "癸",
    ];

    /// 序号式：人名替换（甲、乙、丙...超过10个用 当事人11）
    pub fn next_ordinal_name(&mut self) -> String {
        let key = "ordinal_name".to_string();
        let count = self.counters.entry(key).or_insert(0);
        let result = if *count < Self::TIANGAN.len() {
            Self::TIANGAN[*count].to_string()
        } else {
            format!("当事人{}", *count + 1)
        };
        *count += 1;
        result
    }

    /// 序号式：组织名替换（甲公司、乙集团...）
    pub fn next_ordinal_org(&mut self, original: &str) -> String {
        let suffix = extract_org_suffix(original);
        let key = format!("ordinal_org_{}", suffix);
        let count = self.counters.entry(key).or_insert(0);
        let prefix = if *count < Self::TIANGAN.len() {
            Self::TIANGAN[*count].to_string()
        } else {
            format!("{}", *count + 1)
        };
        *count += 1;
        format!("{}{}", prefix, suffix)
    }

    /// 序号式：地址替换（A地址、B地址...超过26个用 地址27）
    pub fn next_ordinal_address(&mut self) -> String {
        let key = "ordinal_address".to_string();
        let count = self.counters.entry(key).or_insert(0);
        let result = if *count < 26 {
            let letter = (b'A' + *count as u8) as char;
            format!("{}地址", letter)
        } else {
            format!("地址{}", *count + 1)
        };
        *count += 1;
        result
    }

    /// 序号式：职务替换（职务一、职务二...）
    pub fn next_ordinal_title(&mut self) -> String {
        let key = "ordinal_title".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        format!("职务{}", to_chinese_numeral(*count))
    }
```

**Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test desensitizer::replace::tests::test_ordinal -- --nocapture`
Expected: 全部 PASS

**Step 5: 提交**

```bash
git add src-tauri/src/desensitizer/replace.rs
git commit -m "feat: 实现序号式（Ordinal）替换生成逻辑"
```

---

### Task 5: 更新 apply_replace 函数，按风格分派

**Files:**
- Modify: `src-tauri/src/desensitizer/replace.rs`

**Step 1: 修改 apply_replace 函数签名**

将 `apply_replace` 的签名从：

```rust
pub fn apply_replace(
    text: &str,
    sensitive_type: &SensitiveType,
    state: &mut ReplaceState,
) -> String
```

改为：

```rust
pub fn apply_replace(
    text: &str,
    sensitive_type: &SensitiveType,
    state: &mut ReplaceState,
    style: &ReplaceStyle,
) -> String
```

需要在文件顶部添加引用：

```rust
use crate::models::strategy::ReplaceStyle;
```

**Step 2: 在函数体中按风格分派**

对于文本型实体（PersonName、OrgName、Address、Title），按风格选择生成方式。将当前 `apply_replace` 函数体中对这四种类型的匹配分支改为：

```rust
    match sensitive_type {
        SensitiveType::PersonName => match style {
            ReplaceStyle::Fake => state.next_name(),
            ReplaceStyle::Mou => state.next_mou_name(text),
            ReplaceStyle::Ordinal => state.next_ordinal_name(),
        },
        SensitiveType::OrgName => match style {
            ReplaceStyle::Fake => state.next_org(),
            ReplaceStyle::Mou => state.next_mou_org(text),
            ReplaceStyle::Ordinal => state.next_ordinal_org(text),
        },
        SensitiveType::Address => match style {
            ReplaceStyle::Fake => state.next_address(),
            ReplaceStyle::Mou => state.next_mou_address(text),
            ReplaceStyle::Ordinal => state.next_ordinal_address(),
        },
        SensitiveType::Title => match style {
            ReplaceStyle::Fake => state.next_title(),
            ReplaceStyle::Mou => state.next_mou_title(text),
            ReplaceStyle::Ordinal => state.next_ordinal_title(),
        },
        // 其余格式型实体不受风格影响，保持现有逻辑不变
        SensitiveType::Phone => { /* 现有手机号生成逻辑不变 */ },
        // ... 其他类型
    }
```

**Step 3: 更新现有测试中的 apply_replace 调用**

所有现有测试（`test_replace_person_name`、`test_replace_phone` 等）调用 `apply_replace` 时增加 `&ReplaceStyle::Fake` 参数：

```rust
// 原来:
let result = apply_replace("张三", &SensitiveType::PersonName, &mut state);
// 改为:
let result = apply_replace("张三", &SensitiveType::PersonName, &mut state, &ReplaceStyle::Fake);
```

**Step 4: 运行全部替换相关测试**

Run: `cd src-tauri && cargo test desensitizer::replace -- --nocapture`
Expected: 全部 PASS（包括新测试和旧测试）

**Step 5: 提交**

```bash
git add src-tauri/src/desensitizer/replace.rs
git commit -m "feat: apply_replace 支持按 ReplaceStyle 分派生成逻辑"
```

---

### Task 6: 更新命令层和持久化

**Files:**
- Modify: `src-tauri/src/commands/desensitize.rs`
- Modify: `src-tauri/src/commands/workspace.rs`
- Modify: `src-tauri/src/models/workspace.rs`

**Step 1: 更新 Workspace 结构体**

在 `src-tauri/src/models/workspace.rs` 的 `Workspace` 结构体中添加 `replace_style` 字段：

```rust
use crate::models::strategy::ReplaceStyle;

pub struct Workspace {
    // ... 现有字段 ...
    #[serde(default)]
    pub replace_style: ReplaceStyle,
    pub replace_seed: u64,
    pub replace_counters: HashMap<String, usize>,
}
```

**Step 2: 更新 apply_desensitize 命令**

在 `src-tauri/src/commands/desensitize.rs` 中：

1. 添加引用：
```rust
use crate::models::strategy::ReplaceStyle;
```

2. 将 `Strategy::Replace =>` 匹配分支改为 `Strategy::Replace { style } =>`，并传递 style 到 `apply_replace`：

```rust
Strategy::Replace { style } => {
    let r = replace::apply_replace(
        &item.text,
        &item.sensitive_type,
        &mut replace_state,
        style,
    );
    (r, StrategyType::Replace)
}
```

3. 对 `apply_desensitize_by_columns` 命令做同样的修改，将所有 `Strategy::Replace =>` 改为 `Strategy::Replace { style } =>`，传递 `&style`。

**Step 3: 更新 workspace 创建**

在 `src-tauri/src/commands/workspace.rs` 中：

1. `default_strategies()` 中所有 `Strategy::Replace` 改为 `Strategy::Replace { style: ReplaceStyle::Fake }`
2. `create_workspace` 和 `create_clipboard_workspace` 中 Workspace 初始化添加 `replace_style: ReplaceStyle::Fake`

**Step 4: 确认全项目编译通过**

Run: `cd src-tauri && cargo check`
Expected: 无错误

**Step 5: 运行全部测试**

Run: `cd src-tauri && cargo test`
Expected: 全部 PASS

**Step 6: 提交**

```bash
git add src-tauri/src/commands/ src-tauri/src/models/workspace.rs
git commit -m "feat: 命令层和持久化支持 ReplaceStyle"
```

---

### Task 7: 更新前端 TypeScript 类型和 Store

**Files:**
- Modify: `src/types/index.ts`
- Modify: `src/stores/configStore.ts`
- Modify: `src/stores/workspaceStore.ts`

**Step 1: 更新 TypeScript 类型定义**

在 `src/types/index.ts` 中：

1. 添加 `ReplaceStyle` 类型（在 `Strategy` 类型之前）：

```typescript
export type ReplaceStyle = "Fake" | "Mou" | "Ordinal";
```

2. 修改 `Strategy` 类型：

```typescript
// 原来:
export type Strategy =
  | { Mask: { keep_prefix: number; keep_suffix: number } }
  | "Replace"
  | "Generalize";

// 改为:
export type Strategy =
  | { Mask: { keep_prefix: number; keep_suffix: number } }
  | { Replace: { style: ReplaceStyle } }
  | "Generalize";
```

3. 添加 `REPLACE_STYLE_LABELS` 配置：

```typescript
export const REPLACE_STYLE_LABELS: Record<ReplaceStyle, string> = {
  Fake: "假数据",
  Mou: "某式",
  Ordinal: "序号式",
};
```

4. 更新 `getAllowedStrategies` 和其他引用 `"Replace"` 字符串的地方。

5. 更新所有检查 `strategy === "Replace"` 的逻辑为检查 `typeof strategy === "object" && "Replace" in strategy`。

**Step 2: 添加辅助函数**

在 `src/types/index.ts` 中添加或更新：

```typescript
// 获取策略类型名称
export function getStrategyType(strategy: Strategy): StrategyType {
  if (typeof strategy === "string") return strategy as StrategyType; // "Generalize"
  if ("Mask" in strategy) return "Mask";
  if ("Replace" in strategy) return "Replace";
  return "Mask";
}

// 创建 Replace 策略
export function createReplaceStrategy(style: ReplaceStyle = "Fake"): Strategy {
  return { Replace: { style } };
}

// 获取 Replace 策略的风格
export function getReplaceStyle(strategy: Strategy): ReplaceStyle | null {
  if (typeof strategy === "object" && "Replace" in strategy) {
    return strategy.Replace.style;
  }
  return null;
}
```

**Step 3: 更新 configStore**

在 `src/stores/configStore.ts` 中：

1. 更新 `DEFAULT_STRATEGIES` 中所有 `"Replace"` 为 `{ Replace: { style: "Fake" } }`：

```typescript
const DEFAULT_STRATEGIES: Record<string, Strategy> = {
  Phone:         { Mask: { keep_prefix: 3, keep_suffix: 4 } },
  PersonName:    { Replace: { style: "Fake" } },
  OrgName:       { Replace: { style: "Fake" } },
  Title:         { Replace: { style: "Fake" } },
  Address:       { Replace: { style: "Fake" } },
  // ... 其余保持不变
};
```

2. 添加 `replaceStyle` 状态和 `updateReplaceStyle` 方法：

```typescript
interface ConfigState {
  strategies: Record<string, Strategy>;
  replaceStyle: ReplaceStyle;
  // ... 其他现有字段
  updateReplaceStyle: (style: ReplaceStyle) => void;
}
```

`updateReplaceStyle` 实现：更新全局 `replaceStyle` 并将所有 Replace 策略的 style 同步更新：

```typescript
updateReplaceStyle: (style: ReplaceStyle) => {
  set((state) => {
    const newStrategies = { ...state.strategies };
    for (const key in newStrategies) {
      const s = newStrategies[key];
      if (typeof s === "object" && "Replace" in s) {
        newStrategies[key] = { Replace: { style } };
      }
    }
    return { replaceStyle: style, strategies: newStrategies };
  });
  get().saveConfig();
},
```

3. 更新 `loadConfig` 从后端读取 `replace_style` 字段。

**Step 4: 更新 workspaceStore**

在 `src/stores/workspaceStore.ts` 中做类似的更新：
- 添加 `replaceStyle` 到 workspace 数据中
- 更新 `updateReplaceStyle` 方法（workspace 级别）
- 更新所有 `"Replace"` 字符串比较逻辑

**Step 5: 确认前端编译通过**

Run: `npm run build`
Expected: 无 TypeScript 错误

**Step 6: 提交**

```bash
git add src/types/index.ts src/stores/configStore.ts src/stores/workspaceStore.ts
git commit -m "feat: 前端类型和 Store 支持 ReplaceStyle"
```

---

### Task 8: 更新前端 UI 组件

**Files:**
- Modify: `src/components/StrategyConfig/index.tsx`
- Modify: `src/components/StrategyPanel/RulesSection.tsx`

**Step 1: 更新 StrategyConfig 组件**

在 `src/components/StrategyConfig/index.tsx` 中：

1. 更新 `getStrategyName` 辅助函数：

```typescript
function getStrategyName(strategy: Strategy): string {
  if (typeof strategy === "string") return strategy;
  if ("Mask" in strategy) return "Mask";
  if ("Replace" in strategy) return "Replace";
  return "Mask";
}
```

2. 更新 `handleStrategyChange`，创建 Replace 时使用当前全局风格：

```typescript
const handleStrategyChange = (typeKey: string, value: string) => {
  if (value === "Mask") {
    updateStrategy(typeKey, { Mask: { keep_prefix: 3, keep_suffix: 4 } });
  } else if (value === "Generalize") {
    updateStrategy(typeKey, "Generalize");
  } else {
    updateStrategy(typeKey, { Replace: { style: replaceStyle } });
  }
};
```

3. 在策略列表上方添加全局替换风格选择器：

```tsx
{/* 替换风格选择器 - 仅当有类型使用 Replace 时显示 */}
{Object.values(strategies).some(
  (s) => typeof s === "object" && "Replace" in s
) && (
  <div className="mb-4 flex items-center gap-3">
    <span className="text-sm text-gray-500">替换风格</span>
    <div className="flex gap-1 rounded-lg bg-gray-100 p-1">
      {(["Fake", "Mou", "Ordinal"] as ReplaceStyle[]).map((style) => (
        <button
          key={style}
          onClick={() => updateReplaceStyle(style)}
          className={`rounded-md px-3 py-1 text-sm transition-colors ${
            replaceStyle === style
              ? "bg-white text-gray-900 shadow-sm"
              : "text-gray-500 hover:text-gray-700"
          }`}
        >
          {REPLACE_STYLE_LABELS[style]}
        </button>
      ))}
    </div>
  </div>
)}
```

**Step 2: 更新 RulesSection 组件**

在 `src/components/StrategyPanel/RulesSection.tsx` 中做类似修改：

1. 更新策略名称获取逻辑
2. 更新 `handleStrategyChange` 创建 Replace 时使用当前风格
3. 在规则列表上方添加替换风格选择器（样式与 StrategyConfig 一致）
4. 当用户切换风格时，调用 workspace 的 `clearConsistencyMappings`（清除旧映射，避免风格混用）

```typescript
const handleReplaceStyleChange = async (style: ReplaceStyle) => {
  // 更新所有 Replace 策略的 style
  const newStrategies = { ...strategies };
  for (const key in newStrategies) {
    const s = newStrategies[key];
    if (typeof s === "object" && "Replace" in s) {
      newStrategies[key] = { Replace: { style } };
    }
  }
  updateStrategies(newStrategies);
  // 清除一致性映射（风格变更后旧映射不再适用）
  await clearConsistencyMappings();
};
```

**Step 3: 确认前端编译通过**

Run: `npm run build`
Expected: 无错误

**Step 4: 手动功能验证**

Run: `cargo tauri dev`

验证清单：
- [ ] 打开策略配置弹窗，能看到替换风格选择器（假数据/某式/序号式）
- [ ] 默认选中"假数据"
- [ ] 切换到"某式"后，脱敏人名显示为"张某"格式
- [ ] 切换到"序号式"后，脱敏人名显示为"甲""乙"格式
- [ ] 手机号等格式型实体不受风格影响
- [ ] 同一实体全文一致替换
- [ ] 风格设置在关闭重开后保持

**Step 5: 提交**

```bash
git add src/components/StrategyConfig/ src/components/StrategyPanel/
git commit -m "feat: 前端 UI 支持替换风格选择（假数据/某式/序号式）"
```

---

### Task 9: 更新 CHANGELOG 并整体验收

**Files:**
- Modify: `CHANGELOG.md`

**Step 1: 更新 CHANGELOG**

在 `CHANGELOG.md` 顶部添加 v0.3.3 记录：

```markdown
## [v0.3.3] — 2026-02-28

### 新增
- Replace 策略新增**替换风格**子选项：假数据（默认）、某式（张某/某公司）、序号式（甲/甲公司）
- 某式支持复姓识别（欧阳、司马等）、组织后缀智能提取（公司/法院/银行等）
- 序号式使用天干（甲乙丙丁）命名人物、字母（A-Z）标注地址
- 风格全局统一设置，适用于所有文件类型
```

**Step 2: 运行全部 Rust 测试**

Run: `cd src-tauri && cargo test`
Expected: 全部 PASS

**Step 3: 构建验证**

Run: `npm run build && cd src-tauri && cargo check`
Expected: 前后端均无错误

**Step 4: 提交**

```bash
git add CHANGELOG.md
git commit -m "docs: 更新 CHANGELOG v0.3.3 添加替换风格功能说明"
```
