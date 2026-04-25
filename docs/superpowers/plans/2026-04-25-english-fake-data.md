# 英文假数据替换字典 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 `desensitizer/replace.rs` 在英文实体上输出英文假数据，并实现 `Mou` 风格的美式语义（`John Doe / Acme Corp.`），`Ordinal` 在英文实体上静默降级为 `Fake`。

**Architecture:** 在 `apply_replace()` 顶部用 `detect_language()` 按字符判断语言；`ReplaceState` 持有中英两套洗牌索引和独立计数器（key 后缀 `_zh` / `_en`）；老工作区计数器在加载时迁移。复用项目里已有的 `crate::models::language::Language` 枚举。

**Tech Stack:** Rust (Tauri v2 后端) — 改动仅在 `src-tauri/src/desensitizer/replace.rs`、`src-tauri/src/commands/workspace.rs` 和测试文件。前端零改动。

**Spec:** `docs/superpowers/specs/2026-04-25-english-fake-data-design.md`

---

## File Structure

**修改：**
- `src-tauri/src/desensitizer/replace.rs` — 主改动：拆 `FakeData`、加 `detect_language`、`ReplaceState` 双索引、method 加 `Language` 参数、`Mou` 英文实现、`Ordinal` 英文降级。约 +250 行。
- `src-tauri/src/commands/workspace.rs` — 加 `migrate_legacy_counters()` 并在 `read_workspace_data()` 后调用。约 +20 行。
- `src-tauri/tests/restore_roundtrip_english.rs` — 收紧英文输出断言（不含汉字）。
- `src-tauri/tests/strategy_switching_english.rs` — 收紧 SE06 风格断言到具体英文模式。

**不变：**
- `src-tauri/resources/fake_data/en/*.json` — 已就位
- `src-tauri/src/models/sensitive.rs` — `SensitiveType` 不动
- `src-tauri/src/models/language.rs` — `Language` 枚举已存在，直接复用
- 前端代码

---

## Task 1: 加 `detect_language()` 工具函数

**Files:**
- Modify: `src-tauri/src/desensitizer/replace.rs`（顶部加 use + 新增函数）
- Test: `src-tauri/src/desensitizer/replace.rs`（同文件 `mod tests`）

- [ ] **Step 1: 在 `mod tests` 末尾追加失败测试**

打开 `src-tauri/src/desensitizer/replace.rs`，在最末尾 `mod tests` 块（最后一个 `}` 之前）追加：

```rust
    // ========== detect_language 测试 ==========

    #[test]
    fn test_detect_language_chinese() {
        assert_eq!(detect_language("张三"), Language::Zh);
        assert_eq!(detect_language("北京市朝阳区"), Language::Zh);
        assert_eq!(detect_language("腾讯科技有限公司"), Language::Zh);
    }

    #[test]
    fn test_detect_language_english() {
        assert_eq!(detect_language("John Smith"), Language::En);
        assert_eq!(detect_language("Apple Inc."), Language::En);
        assert_eq!(detect_language("123 Main St, New York"), Language::En);
    }

    #[test]
    fn test_detect_language_mixed_falls_back_to_zh() {
        // 含任一汉字即视为中文
        assert_eq!(detect_language("John 张"), Language::Zh);
        assert_eq!(detect_language("Mr. 王"), Language::Zh);
        assert_eq!(detect_language("北京 Office"), Language::Zh);
    }

    #[test]
    fn test_detect_language_empty_falls_back_to_zh() {
        assert_eq!(detect_language(""), Language::Zh);
    }
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cd src-tauri && cargo test desensitizer::replace::tests::test_detect_language --lib
```

Expected: 编译失败 — `cannot find function detect_language` 和 `cannot find type Language`

- [ ] **Step 3: 在 `replace.rs` 顶部 `use` 区添加 import**

把 `replace.rs` 第 8 行 `use std::collections::HashMap;` 上方加一行：

```rust
use crate::models::language::Language;
```

最终 import 区变成：
```rust
use crate::models::sensitive::SensitiveType;
use crate::models::strategy::ReplaceStyle;
use crate::models::language::Language;
use rand::Rng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;
```

- [ ] **Step 4: 在 import 区下方加 `detect_language()` 函数**

在 `// 编译时嵌入 JSON 假数据` 这行之上插入：

```rust
/// 按实体原文字符自动判断语言：含任一汉字 → Zh，否则 En；空字符串兜底 Zh。
pub fn detect_language(text: &str) -> Language {
    if text.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c)) {
        Language::Zh
    } else if text.is_empty() {
        Language::Zh
    } else {
        Language::En
    }
}
```

- [ ] **Step 5: 跑测试确认通过**

```bash
cd src-tauri && cargo test desensitizer::replace::tests::test_detect_language --lib
```

Expected: 4 个测试全部 PASS。

- [ ] **Step 6: 跑全部 replace 模块测试，确认无回归**

```bash
cd src-tauri && cargo test desensitizer::replace --lib
```

Expected: 所有现有测试仍 PASS。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/desensitizer/replace.rs
git commit -m "feat(replace): 新增 detect_language 函数 — 按字符自动判断中英"
```

---

## Task 2: 拆 `FakeData` 为中英两套子结构

**Files:**
- Modify: `src-tauri/src/desensitizer/replace.rs`（重组 `FakeData`、新增 EN 子 struct + JSON 嵌入）

本任务**不改变行为**，只是把数据结构准备好；现有 `next_*` 全部继续读 zh 子结构。

- [ ] **Step 1: 在文件顶部 const JSON 区追加 EN JSON 嵌入**

把 `replace.rs` 第 12-16 行后追加 4 个 EN JSON 常量（注意不需要 patterns_en，因 EN 格式型实体不依赖词库）：

```rust
const EN_PERSON_NAMES_JSON: &str = include_str!("../../resources/fake_data/en/person_names.json");
const EN_ORG_COMPONENTS_JSON: &str = include_str!("../../resources/fake_data/en/org_components.json");
const EN_TITLES_JSON: &str = include_str!("../../resources/fake_data/en/titles.json");
const EN_ADDRESS_COMPONENTS_JSON: &str = include_str!("../../resources/fake_data/en/address_components.json");
```

- [ ] **Step 2: 在 `struct PersonNames {...}` 等 struct 之后新增 EN 版子 struct**

在 `struct Patterns {...}` 之后（约第 50 行后）追加：

```rust
#[derive(Deserialize)]
struct EnPersonNames {
    first_names: Vec<String>,
    last_names: Vec<String>,
}

#[derive(Deserialize)]
struct EnOrgComponents {
    prefixes: Vec<String>,
    industries: Vec<String>,
    suffixes: Vec<String>,
}

#[derive(Deserialize)]
struct EnAddressComponents {
    cities: Vec<String>,
    streets: Vec<String>,
    numbers: Vec<u32>,
}
```

- [ ] **Step 3: 把 `struct FakeData` 拆为 zh + en**

把 `struct FakeData {...}`（约第 53-59 行）整段替换为：

```rust
/// 中文假数据子集
struct ZhFakeData {
    person_names: PersonNames,
    org_components: OrgComponents,
    titles: Vec<String>,
    address_components: AddressComponents,
    patterns: Patterns,
}

/// 英文假数据子集
struct EnFakeData {
    person_names: EnPersonNames,
    org_components: EnOrgComponents,
    titles: Vec<String>,
    address_components: EnAddressComponents,
}

/// 所有假数据，启动时解析一次
struct FakeData {
    zh: ZhFakeData,
    en: EnFakeData,
}
```

- [ ] **Step 4: 改 `get_fake_data()` 加载两套**

把 `get_fake_data()`（约第 63-71 行）整段替换为：

```rust
fn get_fake_data() -> &'static FakeData {
    FAKE_DATA.get_or_init(|| FakeData {
        zh: ZhFakeData {
            person_names: serde_json::from_str(PERSON_NAMES_JSON)
                .expect("解析 zh/person_names.json 失败"),
            org_components: serde_json::from_str(ORG_COMPONENTS_JSON)
                .expect("解析 zh/org_components.json 失败"),
            titles: serde_json::from_str(TITLES_JSON).expect("解析 zh/titles.json 失败"),
            address_components: serde_json::from_str(ADDRESS_COMPONENTS_JSON)
                .expect("解析 zh/address_components.json 失败"),
            patterns: serde_json::from_str(PATTERNS_JSON).expect("解析 zh/patterns.json 失败"),
        },
        en: EnFakeData {
            person_names: serde_json::from_str(EN_PERSON_NAMES_JSON)
                .expect("解析 en/person_names.json 失败"),
            org_components: serde_json::from_str(EN_ORG_COMPONENTS_JSON)
                .expect("解析 en/org_components.json 失败"),
            titles: serde_json::from_str(EN_TITLES_JSON).expect("解析 en/titles.json 失败"),
            address_components: serde_json::from_str(EN_ADDRESS_COMPONENTS_JSON)
                .expect("解析 en/address_components.json 失败"),
        },
    })
}
```

- [ ] **Step 5: 临时把所有 `data.field` 改为 `data.zh.field`**

`replace.rs` 内现有的所有 `next_*` 和 `apply_replace` 都通过 `let data = get_fake_data();` 然后访问 `data.person_names / data.org_components / data.titles / data.address_components / data.patterns`。

打开 `replace.rs`，全文替换（精确匹配）：

| 原文 | 改为 |
|---|---|
| `data.person_names` | `data.zh.person_names` |
| `data.org_components` | `data.zh.org_components` |
| `data.titles` | `data.zh.titles` |
| `data.address_components` | `data.zh.address_components` |
| `data.patterns` | `data.zh.patterns` |

可用 `sed -i '' 's/data\.person_names/data.zh.person_names/g' src-tauri/src/desensitizer/replace.rs` 等命令批量替换；或用 IDE 重构。共 4 个 next_* method + apply_replace 内多个分支，约 15-20 处。

- [ ] **Step 6: 跑全部 replace 测试，确认零回归**

```bash
cd src-tauri && cargo test desensitizer::replace --lib
```

Expected: 全部 PASS（行为不变，只是数据访问路径加了一层 `.zh`）。

- [ ] **Step 7: 跑全量 lib 测试**

```bash
cd src-tauri && cargo test --lib
```

Expected: 全部 PASS。

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/desensitizer/replace.rs
git commit -m "refactor(replace): 拆 FakeData 为 zh/en 子结构 — 加载 EN JSON 但暂不消费"
```

---

## Task 3: `ReplaceState` 加双索引字段 + Fake 系列 method 签名加 `Language` 参数

**Files:**
- Modify: `src-tauri/src/desensitizer/replace.rs`

本任务把 `next_name / next_org / next_address / next_title` 改为接受 `Language` 参数。`apply_replace` 内部分发时统一传 `Language::Zh`（和现状等效），后续 task 再加 En 实现。所有上游不变，但单元测试中直接调用这 4 个 method 的需要改签名（在 Step 7）。

- [ ] **Step 1: 改 `ReplaceState` 字段**

把 `pub struct ReplaceState {...}`（约第 152-160 行）整段替换为：

```rust
pub struct ReplaceState {
    seed: u64,
    counters: HashMap<String, usize>,
    // zh 洗牌索引
    name_indices_zh: Option<Vec<u32>>,
    org_indices_zh: Option<Vec<u32>>,
    address_indices_zh: Option<Vec<u32>>,
    title_indices_zh: Option<Vec<u32>>,
    // en 洗牌索引
    name_indices_en: Option<Vec<u32>>,
    org_indices_en: Option<Vec<u32>>,
    address_indices_en: Option<Vec<u32>>,
    title_indices_en: Option<Vec<u32>>,
}
```

- [ ] **Step 2: 改种子偏移量常量**

把（约第 163-166 行）：

```rust
const NAME_SEED_OFFSET: u64 = 0;
const ORG_SEED_OFFSET: u64 = 1;
const ADDRESS_SEED_OFFSET: u64 = 2;
const TITLE_SEED_OFFSET: u64 = 3;
```

替换为：

```rust
const NAME_SEED_OFFSET_ZH: u64 = 0;
const ORG_SEED_OFFSET_ZH: u64 = 1;
const ADDRESS_SEED_OFFSET_ZH: u64 = 2;
const TITLE_SEED_OFFSET_ZH: u64 = 3;
const NAME_SEED_OFFSET_EN: u64 = 4;
const ORG_SEED_OFFSET_EN: u64 = 5;
const ADDRESS_SEED_OFFSET_EN: u64 = 6;
const TITLE_SEED_OFFSET_EN: u64 = 7;
```

- [ ] **Step 3: 改 `ReplaceState::new()` 初始化所有新字段**

把 `pub fn new(seed: u64, counters: HashMap<String, usize>) -> Self {...}` 整段替换为：

```rust
pub fn new(seed: u64, counters: HashMap<String, usize>) -> Self {
    Self {
        seed,
        counters,
        name_indices_zh: None,
        org_indices_zh: None,
        address_indices_zh: None,
        title_indices_zh: None,
        name_indices_en: None,
        org_indices_en: None,
        address_indices_en: None,
        title_indices_en: None,
    }
}
```

- [ ] **Step 4: 改 `next_name()` 加 `lang` 参数 + 内部按 lang 分发**

把 `pub fn next_name(&mut self) -> String {...}` 整段替换为：

```rust
/// 取下一个唯一姓名
pub fn next_name(&mut self, lang: Language) -> String {
    let data = get_fake_data();
    match lang {
        Language::Zh => {
            let surname_count = data.zh.person_names.surnames.len() as u32;
            let given_count = data.zh.person_names.given_names.len() as u32;
            let pool_size = surname_count * given_count;

            let indices = self.name_indices_zh.get_or_insert_with(|| {
                Self::init_shuffled_indices(self.seed, NAME_SEED_OFFSET_ZH, pool_size)
            });

            let counter = self.counters.entry("PersonName_zh".to_string()).or_insert(0);
            let idx = indices[*counter % indices.len()] as usize;
            let wrap = *counter / indices.len();
            *counter += 1;

            let surname_idx = idx / given_count as usize;
            let given_idx = idx % given_count as usize;

            let name = format!(
                "{}{}",
                data.zh.person_names.surnames[surname_idx],
                data.zh.person_names.given_names[given_idx]
            );

            if wrap > 0 {
                format!("{}{}", name, wrap)
            } else {
                name
            }
        }
        Language::En => {
            // 实现见 Task 5
            unimplemented!("EN next_name 在 Task 5 实现")
        }
    }
}
```

- [ ] **Step 5: 同样改 `next_org / next_address / next_title`**

按同一模式改剩余 3 个 method（`next_org` 约第 227 行、`next_address` 约第 263 行、`next_title` 约第 299 行）。每个都是：
- 增加 `lang: Language` 参数
- 函数体改为 `match lang { Zh => 现有逻辑（key 后缀加 _zh，indices 字段加 _zh，offset 后缀加 _ZH），En => unimplemented!() }`

具体替换示例（next_org）：

```rust
/// 取下一个唯一机构名
pub fn next_org(&mut self, lang: Language) -> String {
    let data = get_fake_data();
    match lang {
        Language::Zh => {
            let prefix_count = data.zh.org_components.prefixes.len() as u32;
            let industry_count = data.zh.org_components.industries.len() as u32;
            let suffix_count = data.zh.org_components.suffixes.len() as u32;
            let pool_size = prefix_count * industry_count * suffix_count;

            let indices = self.org_indices_zh.get_or_insert_with(|| {
                Self::init_shuffled_indices(self.seed, ORG_SEED_OFFSET_ZH, pool_size)
            });

            let counter = self.counters.entry("OrgName_zh".to_string()).or_insert(0);
            let idx = indices[*counter % indices.len()] as usize;
            let wrap = *counter / indices.len();
            *counter += 1;

            let suffix_idx = idx % suffix_count as usize;
            let remaining = idx / suffix_count as usize;
            let industry_idx = remaining % industry_count as usize;
            let prefix_idx = remaining / industry_count as usize;

            let org = format!(
                "{}{}{}",
                data.zh.org_components.prefixes[prefix_idx],
                data.zh.org_components.industries[industry_idx],
                data.zh.org_components.suffixes[suffix_idx]
            );

            if wrap > 0 {
                format!("{}{}", org, wrap)
            } else {
                org
            }
        }
        Language::En => {
            unimplemented!("EN next_org 在 Task 5 实现")
        }
    }
}
```

`next_address` 同理：counter key `Address_zh`、indices `address_indices_zh`、offset `ADDRESS_SEED_OFFSET_ZH`、En 分支 `unimplemented!("EN next_address 在 Task 5 实现")`。

`next_title`：counter key `Title_zh`、indices `title_indices_zh`、offset `TITLE_SEED_OFFSET_ZH`、En 分支 `unimplemented!("EN next_title 在 Task 5 实现")`。

- [ ] **Step 6: 改 `apply_replace()` 内分发，传入 `lang`**

把 `pub fn apply_replace(...)` 函数体里 `let data = get_fake_data();` 这行下面（约第 425 行）插入：

```rust
let lang = detect_language(text);
```

然后把 `Fake` 分支改为传 `lang`：

```rust
SensitiveType::PersonName => match style {
    ReplaceStyle::Fake => state.next_name(lang),
    ReplaceStyle::Mou => state.next_mou_name(text),       // Task 4 改签名
    ReplaceStyle::Ordinal => state.next_ordinal_name(),   // Task 8 处理 En 降级
},
SensitiveType::OrgName => match style {
    ReplaceStyle::Fake => state.next_org(lang),
    ReplaceStyle::Mou => state.next_mou_org(text),
    ReplaceStyle::Ordinal => state.next_ordinal_org(text),
},
SensitiveType::Title => match style {
    ReplaceStyle::Fake => state.next_title(lang),
    ReplaceStyle::Mou => state.next_mou_title(text),
    ReplaceStyle::Ordinal => state.next_ordinal_title(),
},
SensitiveType::Address => match style {
    ReplaceStyle::Fake => state.next_address(lang),
    ReplaceStyle::Mou => state.next_mou_address(text),
    ReplaceStyle::Ordinal => state.next_ordinal_address(),
},
```

- [ ] **Step 7: 修复编译 — 更新内联单元测试中的直接调用**

5 个测试直接调用了改了签名的 method，全部加 `Language::Zh`：

替换 `state.next_name()` → `state.next_name(Language::Zh)`
替换 `state.next_org()` → `state.next_org(Language::Zh)`
替换 `state.next_address()` → `state.next_address(Language::Zh)`
替换 `state.next_title()` → `state.next_title(Language::Zh)`

具体涉及测试（在 `mod tests` 内）：
- `test_uniqueness_names`：`names.push(state.next_name())` → `names.push(state.next_name(Language::Zh))`
- `test_uniqueness_orgs`：`orgs.push(state.next_org())` → `orgs.push(state.next_org(Language::Zh))`
- `test_uniqueness_addresses`：`addrs.push(state.next_address())` → `addrs.push(state.next_address(Language::Zh))`
- `test_deterministic_with_seed`：6 处 `next_name() / next_org() / next_address()` 全部加 `Language::Zh` 参数
- `test_counter_resume`：3 处：
  - `state_fresh.next_name()` → `state_fresh.next_name(Language::Zh)`
  - `state_resumed.next_name()` → `state_resumed.next_name(Language::Zh)`
  - `counters.insert("PersonName".to_string(), 5);` → `counters.insert("PersonName_zh".to_string(), 5);`

- [ ] **Step 8: 跑全量 replace 测试**

```bash
cd src-tauri && cargo test desensitizer::replace --lib
```

Expected: 全部 PASS（中文行为零回归；En 路径无人调用，`unimplemented!` 不会被触发）。

- [ ] **Step 9: 跑全量 lib + 集成测试**

```bash
cd src-tauri && cargo test
```

Expected: 全部 PASS。注意 `restore_roundtrip_english.rs` 和 `strategy_switching_english.rs` 的英文用例此时仍走"中文 fake 输出"，但断言较弱（只断言"被替换"），所以会通过 — 它们将在 Task 10 收紧。

- [ ] **Step 10: Commit**

```bash
git add src-tauri/src/desensitizer/replace.rs
git commit -m "refactor(replace): Fake 系列 method 加 Language 参数 + counter key 加 _zh 后缀"
```

---

## Task 4: Mou 系列 method 签名加 `Language` 参数

**Files:**
- Modify: `src-tauri/src/desensitizer/replace.rs`

本任务只改签名，En 分支用 `unimplemented!()` 占位（Task 7 实现）。`apply_replace` 调用方传 `lang`。

- [ ] **Step 1: 改 4 个 mou_* method 签名**

把 `next_mou_name / next_mou_org / next_mou_address / next_mou_title`（约第 322-372 行）每个的签名最后追加 `lang: Language` 参数，并把函数体包成 `match lang { Zh => 现有逻辑, En => unimplemented!("Task 7") }`。

示例（`next_mou_name`）：

```rust
/// 某式：人名替换（张某、张某二、李某 ... / English 在 Task 6 实现）
pub fn next_mou_name(&mut self, original: &str, lang: Language) -> String {
    match lang {
        Language::Zh => {
            if original.is_empty() {
                return "某某".to_string();
            }
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
        Language::En => {
            unimplemented!("EN mou_name 在 Task 6 实现")
        }
    }
}
```

`next_mou_org` / `next_mou_address` / `next_mou_title` 用同一模式：原中文逻辑放进 `Zh` 分支，`En` 分支 `unimplemented!("EN mou_<xxx> 在 Task 6 实现")`。

- [ ] **Step 2: 更新 `apply_replace` 中的调用**

把 `apply_replace` 里 4 处 `state.next_mou_*(text)` 全部加 `lang` 参数：

```rust
ReplaceStyle::Mou => state.next_mou_name(text, lang),
ReplaceStyle::Mou => state.next_mou_org(text, lang),
ReplaceStyle::Mou => state.next_mou_title(text, lang),
ReplaceStyle::Mou => state.next_mou_address(text, lang),
```

- [ ] **Step 3: 修复内联测试中的 mou 直接调用**

`mod tests` 内涉及（约第 768-799 行）：
- `test_mou_person_name`：`state.next_mou_name("张三")` → `state.next_mou_name("张三", Language::Zh)`，全部 5 处
- `test_mou_org_name`：`state.next_mou_org(...)` → 加 `Language::Zh`，全部 3 处
- `test_mou_address`：`state.next_mou_address(...)` → 加 `Language::Zh`，全部 2 处
- `test_mou_title`：`state.next_mou_title(...)` → 加 `Language::Zh`，全部 2 处

- [ ] **Step 4: 跑全量测试**

```bash
cd src-tauri && cargo test
```

Expected: 全部 PASS（中文 mou 逻辑零回归；英文 mou 无人调用）。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/desensitizer/replace.rs
git commit -m "refactor(replace): Mou 系列 method 加 Language 参数 — En 分支待实现"
```

---

## Task 5: 实现 EN Fake — `next_name / next_org / next_address / next_title`

**Files:**
- Modify: `src-tauri/src/desensitizer/replace.rs`（替换 4 个 `unimplemented!()`）
- Test: `src-tauri/src/desensitizer/replace.rs`（同文件 `mod tests`）

- [ ] **Step 1: 写 4 个失败测试**

在 `mod tests` 末尾追加：

```rust
    // ========== EN Fake 测试 ==========

    #[test]
    fn test_replace_person_name_en() {
        let mut state = test_state();
        let result = apply_replace(
            "John Smith",
            &SensitiveType::PersonName,
            &mut state,
            &ReplaceStyle::Fake,
        );
        // 不含汉字
        assert!(
            result.chars().all(|c| !('\u{4E00}'..='\u{9FFF}').contains(&c)),
            "EN 假名不应含汉字: {}",
            result
        );
        // 含一个空格分隔 first/last
        assert!(result.contains(' '), "EN 假名应含空格: {}", result);
        // 不等于原文
        assert_ne!(result, "John Smith");
    }

    #[test]
    fn test_replace_org_en() {
        let mut state = test_state();
        let result = apply_replace(
            "Apple Inc.",
            &SensitiveType::OrgName,
            &mut state,
            &ReplaceStyle::Fake,
        );
        assert!(
            result.chars().all(|c| !('\u{4E00}'..='\u{9FFF}').contains(&c)),
            "EN 假机构不应含汉字: {}",
            result
        );
        // 应含某个 EN suffix
        let en_suffixes = ["Inc.", "Corp.", "LLC", "Ltd.", "Group", "Holdings",
                           "Partners", "Associates", "International", "Co."];
        assert!(
            en_suffixes.iter().any(|s| result.ends_with(s)),
            "EN 假机构应以已知 suffix 结尾: {}",
            result
        );
    }

    #[test]
    fn test_replace_address_en() {
        let mut state = test_state();
        let result = apply_replace(
            "123 Main St, NY",
            &SensitiveType::Address,
            &mut state,
            &ReplaceStyle::Fake,
        );
        assert!(
            result.chars().all(|c| !('\u{4E00}'..='\u{9FFF}').contains(&c)),
            "EN 假地址不应含汉字: {}",
            result
        );
        // 形如 "123 Main St, New York, NY"，含逗号
        assert!(result.contains(','), "EN 假地址应含逗号: {}", result);
    }

    #[test]
    fn test_replace_title_en() {
        let mut state = test_state();
        let result = apply_replace(
            "Software Engineer",
            &SensitiveType::Title,
            &mut state,
            &ReplaceStyle::Fake,
        );
        assert!(
            result.chars().all(|c| !('\u{4E00}'..='\u{9FFF}').contains(&c)),
            "EN 假职位不应含汉字: {}",
            result
        );
    }

    #[test]
    fn test_uniqueness_names_en() {
        let mut state = test_state();
        let mut names: Vec<String> = Vec::new();
        for _ in 0..100 {
            names.push(state.next_name(Language::En));
        }
        let unique: std::collections::HashSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), 100, "100 个 EN 姓名应全部唯一");
    }

    #[test]
    fn test_uniqueness_orgs_en() {
        let mut state = test_state();
        let mut orgs: Vec<String> = Vec::new();
        for _ in 0..100 {
            orgs.push(state.next_org(Language::En));
        }
        let unique: std::collections::HashSet<&String> = orgs.iter().collect();
        assert_eq!(unique.len(), 100, "100 个 EN 机构应全部唯一");
    }

    #[test]
    fn test_counters_isolated() {
        // 中英 counter 独立
        let mut state = test_state();
        for _ in 0..5 {
            state.next_name(Language::Zh);
        }
        for _ in 0..3 {
            state.next_name(Language::En);
        }
        let counters = state.export_counters();
        assert_eq!(counters.get("PersonName_zh"), Some(&5));
        assert_eq!(counters.get("PersonName_en"), Some(&3));
    }
```

- [ ] **Step 2: 跑测试确认失败（panic 在 unimplemented!）**

```bash
cd src-tauri && cargo test desensitizer::replace::tests::test_replace_person_name_en --lib
```

Expected: PANIC `not implemented: EN next_name 在 Task 5 实现`。

- [ ] **Step 3: 实现 `next_name` 的 En 分支**

把 `next_name` 内 `Language::En => unimplemented!(...)` 替换为：

```rust
Language::En => {
    let first_count = data.en.person_names.first_names.len() as u32;
    let last_count = data.en.person_names.last_names.len() as u32;
    let pool_size = first_count * last_count;

    let indices = self.name_indices_en.get_or_insert_with(|| {
        Self::init_shuffled_indices(self.seed, NAME_SEED_OFFSET_EN, pool_size)
    });

    let counter = self.counters.entry("PersonName_en".to_string()).or_insert(0);
    let idx = indices[*counter % indices.len()] as usize;
    let wrap = *counter / indices.len();
    *counter += 1;

    let first_idx = idx / last_count as usize;
    let last_idx = idx % last_count as usize;

    let name = format!(
        "{} {}",
        data.en.person_names.first_names[first_idx],
        data.en.person_names.last_names[last_idx]
    );

    if wrap > 0 {
        format!("{} {}", name, wrap)
    } else {
        name
    }
}
```

- [ ] **Step 4: 实现 `next_org` 的 En 分支**

```rust
Language::En => {
    let prefix_count = data.en.org_components.prefixes.len() as u32;
    let industry_count = data.en.org_components.industries.len() as u32;
    let suffix_count = data.en.org_components.suffixes.len() as u32;
    let pool_size = prefix_count * industry_count * suffix_count;

    let indices = self.org_indices_en.get_or_insert_with(|| {
        Self::init_shuffled_indices(self.seed, ORG_SEED_OFFSET_EN, pool_size)
    });

    let counter = self.counters.entry("OrgName_en".to_string()).or_insert(0);
    let idx = indices[*counter % indices.len()] as usize;
    let wrap = *counter / indices.len();
    *counter += 1;

    let suffix_idx = idx % suffix_count as usize;
    let remaining = idx / suffix_count as usize;
    let industry_idx = remaining % industry_count as usize;
    let prefix_idx = remaining / industry_count as usize;

    let org = format!(
        "{} {} {}",
        data.en.org_components.prefixes[prefix_idx],
        data.en.org_components.industries[industry_idx],
        data.en.org_components.suffixes[suffix_idx]
    );

    if wrap > 0 {
        format!("{} {}", org, wrap)
    } else {
        org
    }
}
```

- [ ] **Step 5: 实现 `next_address` 的 En 分支**

```rust
Language::En => {
    let city_count = data.en.address_components.cities.len() as u32;
    let street_count = data.en.address_components.streets.len() as u32;
    let number_count = data.en.address_components.numbers.len() as u32;
    let pool_size = city_count * street_count * number_count;

    let indices = self.address_indices_en.get_or_insert_with(|| {
        Self::init_shuffled_indices(self.seed, ADDRESS_SEED_OFFSET_EN, pool_size)
    });

    let counter = self.counters.entry("Address_en".to_string()).or_insert(0);
    let idx = indices[*counter % indices.len()] as usize;
    let wrap = *counter / indices.len();
    *counter += 1;

    let number_idx = idx % number_count as usize;
    let remaining = idx / number_count as usize;
    let street_idx = remaining % street_count as usize;
    let city_idx = remaining / street_count as usize;

    let addr = format!(
        "{} {}, {}",
        data.en.address_components.numbers[number_idx],
        data.en.address_components.streets[street_idx],
        data.en.address_components.cities[city_idx]
    );

    if wrap > 0 {
        format!("{} {}", addr, wrap)
    } else {
        addr
    }
}
```

- [ ] **Step 6: 实现 `next_title` 的 En 分支**

```rust
Language::En => {
    let pool_size = data.en.titles.len() as u32;

    let indices = self.title_indices_en.get_or_insert_with(|| {
        Self::init_shuffled_indices(self.seed, TITLE_SEED_OFFSET_EN, pool_size)
    });

    let counter = self.counters.entry("Title_en".to_string()).or_insert(0);
    let idx = indices[*counter % indices.len()] as usize;
    let wrap = *counter / indices.len();
    *counter += 1;

    let title = data.en.titles[idx].clone();

    if wrap > 0 {
        format!("{} {}", title, wrap)
    } else {
        title
    }
}
```

- [ ] **Step 7: 跑测试确认通过**

```bash
cd src-tauri && cargo test desensitizer::replace --lib
```

Expected: 7 个新增测试 PASS，所有现有测试仍 PASS。

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/desensitizer/replace.rs
git commit -m "feat(replace): 实现 EN Fake 风格 — PersonName/OrgName/Address/Title"
```

---

## Task 6: 实现 EN Mou — `John Doe / Acme Corp. / [REDACTED CITY] / [REDACTED TITLE]`

**Files:**
- Modify: `src-tauri/src/desensitizer/replace.rs`（替换 4 个 mou En `unimplemented!()`，加 EN suffix 表）
- Test: `src-tauri/src/desensitizer/replace.rs`

- [ ] **Step 1: 在文件顶部 const 区追加 EN suffix 表**

在 `const ORG_SUFFIXES: &[&str] = &[...]`（约第 88-91 行）之下追加：

```rust
/// 英文组织后缀表（与 fake_data/en/org_components.json 的 suffixes 完全对齐）
const EN_ORG_SUFFIXES: &[&str] = &[
    "Inc.", "Corp.", "LLC", "Ltd.", "Group", "Holdings",
    "Partners", "Associates", "International", "Co.",
];

/// 从英文组织名末尾提取 suffix（按 EN_ORG_SUFFIXES 顺序匹配）；找不到则返回 "Co." 兜底
fn extract_en_org_suffix(org: &str) -> &'static str {
    for suffix in EN_ORG_SUFFIXES {
        if org.ends_with(suffix) {
            return suffix;
        }
    }
    "Co."
}
```

- [ ] **Step 2: 写失败测试**

在 `mod tests` 末尾追加：

```rust
    // ========== EN Mou 测试 ==========

    #[test]
    fn test_extract_en_org_suffix() {
        assert_eq!(extract_en_org_suffix("Apple Inc."), "Inc.");
        assert_eq!(extract_en_org_suffix("Acme LLC"), "LLC");
        assert_eq!(extract_en_org_suffix("Smith & Partners"), "Partners");
        assert_eq!(extract_en_org_suffix("GitHub"), "Co."); // 兜底
    }

    #[test]
    fn test_mou_person_name_en() {
        let mut state = test_state();
        // 性别轮换 + 序号
        assert_eq!(
            state.next_mou_name("John Smith", Language::En),
            "John Doe"
        );
        assert_eq!(
            state.next_mou_name("Jane Doe", Language::En),
            "Jane Doe"
        );
        assert_eq!(
            state.next_mou_name("Robert Garcia", Language::En),
            "John Doe 2"
        );
        assert_eq!(
            state.next_mou_name("Linda Wilson", Language::En),
            "Jane Doe 2"
        );
        assert_eq!(
            state.next_mou_name("Edward Lee", Language::En),
            "John Doe 3"
        );
    }

    #[test]
    fn test_mou_org_en_with_suffix() {
        let mut state = test_state();
        assert_eq!(
            state.next_mou_org("Apple Inc.", Language::En),
            "Acme Inc."
        );
        // 同 suffix 第 2 次出现 → 加序号
        assert_eq!(
            state.next_mou_org("Microsoft Inc.", Language::En),
            "Acme Inc. 2"
        );
        // 不同 suffix 独立计数
        assert_eq!(
            state.next_mou_org("Tesla LLC", Language::En),
            "Acme LLC"
        );
    }

    #[test]
    fn test_mou_org_en_fallback_no_suffix() {
        let mut state = test_state();
        // GitHub 没有标准 suffix → 兜底为 Acme Co.
        assert_eq!(
            state.next_mou_org("GitHub", Language::En),
            "Acme Co."
        );
        assert_eq!(
            state.next_mou_org("Zoom", Language::En),
            "Acme Co. 2"
        );
    }

    #[test]
    fn test_mou_address_en() {
        let mut state = test_state();
        assert_eq!(
            state.next_mou_address("123 Main St, NY", Language::En),
            "[REDACTED CITY]"
        );
        assert_eq!(
            state.next_mou_address("456 Oak Ave, LA", Language::En),
            "[REDACTED CITY] 2"
        );
    }

    #[test]
    fn test_mou_title_en() {
        let mut state = test_state();
        assert_eq!(
            state.next_mou_title("Software Engineer", Language::En),
            "[REDACTED TITLE]"
        );
        assert_eq!(
            state.next_mou_title("Product Manager", Language::En),
            "[REDACTED TITLE] 2"
        );
    }
```

- [ ] **Step 3: 跑测试确认失败**

```bash
cd src-tauri && cargo test desensitizer::replace::tests::test_mou --lib
```

Expected: PANIC（unimplemented）或断言失败。

- [ ] **Step 4: 实现 `next_mou_name` 的 En 分支**

替换原 `Language::En => unimplemented!(...)` 为：

```rust
Language::En => {
    let counter = self.counters.entry("mou_name_en".to_string()).or_insert(0);
    let n = *counter;
    *counter += 1;
    let base = if n % 2 == 0 { "John Doe" } else { "Jane Doe" };
    // 第 0/1 次：John Doe / Jane Doe；第 2/3 次：John Doe 2 / Jane Doe 2 ...
    let cycle = n / 2 + 1;
    if cycle == 1 {
        base.to_string()
    } else {
        format!("{} {}", base, cycle)
    }
}
```

- [ ] **Step 5: 实现 `next_mou_org` 的 En 分支**

替换为：

```rust
Language::En => {
    let suffix = extract_en_org_suffix(original);
    let key = format!("mou_org_en_{}", suffix);
    let count = self.counters.entry(key).or_insert(0);
    *count += 1;
    if *count == 1 {
        format!("Acme {}", suffix)
    } else {
        format!("Acme {} {}", suffix, *count)
    }
}
```

- [ ] **Step 6: 实现 `next_mou_address` 的 En 分支**

替换为：

```rust
Language::En => {
    let key = "mou_address_en".to_string();
    let count = self.counters.entry(key).or_insert(0);
    *count += 1;
    if *count == 1 {
        "[REDACTED CITY]".to_string()
    } else {
        format!("[REDACTED CITY] {}", *count)
    }
}
```

- [ ] **Step 7: 实现 `next_mou_title` 的 En 分支**

替换为：

```rust
Language::En => {
    let key = "mou_title_en".to_string();
    let count = self.counters.entry(key).or_insert(0);
    *count += 1;
    if *count == 1 {
        "[REDACTED TITLE]".to_string()
    } else {
        format!("[REDACTED TITLE] {}", *count)
    }
}
```

- [ ] **Step 8: 跑全量 replace 测试**

```bash
cd src-tauri && cargo test desensitizer::replace --lib
```

Expected: 全部 PASS（5 + 1 个新测试通过；现有测试零回归）。

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/desensitizer/replace.rs
git commit -m "feat(replace): 实现 EN Mou 风格 — John Doe/Acme suffix/[REDACTED CITY|TITLE]"
```

---

## Task 7: Ordinal 在英文实体上静默降级为 Fake

**Files:**
- Modify: `src-tauri/src/desensitizer/replace.rs`（仅改 `apply_replace` 的 Ordinal 分支）
- Test: `src-tauri/src/desensitizer/replace.rs`

- [ ] **Step 1: 写失败测试**

在 `mod tests` 末尾追加：

```rust
    // ========== Ordinal 英文降级测试 ==========

    #[test]
    fn test_ordinal_en_person_falls_back_to_fake() {
        let mut state = test_state();
        let result = apply_replace(
            "John Smith",
            &SensitiveType::PersonName,
            &mut state,
            &ReplaceStyle::Ordinal,
        );
        // 不应输出 "Person A"（中文 ordinal 在英文上的牵强映射）
        assert!(!result.starts_with("Person "), "EN Ordinal 应降级为 Fake，不应是 Person A: {}", result);
        // 应输出英文假名（含空格、纯 ASCII）
        assert!(result.contains(' '), "应是英文假名: {}", result);
        assert!(
            result.chars().all(|c| !('\u{4E00}'..='\u{9FFF}').contains(&c)),
            "应不含汉字: {}",
            result
        );
    }

    #[test]
    fn test_ordinal_en_org_falls_back_to_fake() {
        let mut state = test_state();
        let result = apply_replace(
            "Apple Inc.",
            &SensitiveType::OrgName,
            &mut state,
            &ReplaceStyle::Ordinal,
        );
        assert!(!result.starts_with("Company "), "EN Ordinal 应降级为 Fake: {}", result);
        let en_suffixes = ["Inc.", "Corp.", "LLC", "Ltd.", "Group", "Holdings",
                           "Partners", "Associates", "International", "Co."];
        assert!(
            en_suffixes.iter().any(|s| result.ends_with(s)),
            "应是 EN Fake 输出（带 EN suffix）: {}",
            result
        );
    }

    #[test]
    fn test_ordinal_en_address_falls_back_to_fake() {
        let mut state = test_state();
        let result = apply_replace(
            "123 Main St",
            &SensitiveType::Address,
            &mut state,
            &ReplaceStyle::Ordinal,
        );
        assert!(!result.starts_with("Address "), "EN Ordinal 应降级为 Fake: {}", result);
        assert!(result.contains(','), "应是 EN Fake 输出: {}", result);
    }

    #[test]
    fn test_ordinal_en_title_falls_back_to_fake() {
        let mut state = test_state();
        let result = apply_replace(
            "Software Engineer",
            &SensitiveType::Title,
            &mut state,
            &ReplaceStyle::Ordinal,
        );
        assert!(!result.starts_with("Title "), "EN Ordinal 应降级为 Fake: {}", result);
    }

    #[test]
    fn test_ordinal_zh_still_works() {
        // 中文 Ordinal 行为不变
        let mut state = test_state();
        let result = apply_replace(
            "张三",
            &SensitiveType::PersonName,
            &mut state,
            &ReplaceStyle::Ordinal,
        );
        assert_eq!(result, "当事人一");
    }
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cd src-tauri && cargo test desensitizer::replace::tests::test_ordinal_en --lib
```

Expected: 4 个 FAIL（当前 EN Ordinal 走中文 ordinal，输出"当事人一"等汉字，不符合断言）。

- [ ] **Step 3: 修改 `apply_replace` 中 4 个 Ordinal 分支**

把 `apply_replace` 内的 Ordinal 分发改为按语言分发：

```rust
SensitiveType::PersonName => match style {
    ReplaceStyle::Fake => state.next_name(lang),
    ReplaceStyle::Mou => state.next_mou_name(text, lang),
    ReplaceStyle::Ordinal => match lang {
        Language::Zh => state.next_ordinal_name(),
        Language::En => state.next_name(lang),  // 降级为 Fake
    },
},
SensitiveType::OrgName => match style {
    ReplaceStyle::Fake => state.next_org(lang),
    ReplaceStyle::Mou => state.next_mou_org(text, lang),
    ReplaceStyle::Ordinal => match lang {
        Language::Zh => state.next_ordinal_org(text),
        Language::En => state.next_org(lang),
    },
},
SensitiveType::Title => match style {
    ReplaceStyle::Fake => state.next_title(lang),
    ReplaceStyle::Mou => state.next_mou_title(text, lang),
    ReplaceStyle::Ordinal => match lang {
        Language::Zh => state.next_ordinal_title(),
        Language::En => state.next_title(lang),
    },
},
SensitiveType::Address => match style {
    ReplaceStyle::Fake => state.next_address(lang),
    ReplaceStyle::Mou => state.next_mou_address(text, lang),
    ReplaceStyle::Ordinal => match lang {
        Language::Zh => state.next_ordinal_address(),
        Language::En => state.next_address(lang),
    },
},
```

- [ ] **Step 4: 跑测试确认通过**

```bash
cd src-tauri && cargo test desensitizer::replace --lib
```

Expected: 全部 PASS（5 个新降级测试 + 现有零回归）。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/desensitizer/replace.rs
git commit -m "feat(replace): Ordinal 在英文实体上静默降级为 Fake"
```

---

## Task 8: 老工作区码本迁移 `migrate_legacy_counters`

**Files:**
- Modify: `src-tauri/src/commands/workspace.rs`（新增函数 + 在 `read_workspace_data` 后调用）

- [ ] **Step 1: 在 `workspace.rs` 末尾追加迁移函数和单元测试**

打开 `src-tauri/src/commands/workspace.rs`，在文件末尾（最后一个 `}` 之后）追加：

```rust
/// 迁移老工作区计数器：旧 key（如 "PersonName"）→ 新 key（"PersonName_zh"）。
///
/// 老用户绝大多数是中文场景，旧 counter 实际记录的就是中文池消费记录。
/// 迁移到 `_zh` 后缀键，保持中文序号连续性；不影响英文池（从 0 开始）。
pub(crate) fn migrate_legacy_counters(counters: &mut HashMap<String, usize>) {
    for legacy_key in ["PersonName", "OrgName", "Address", "Title"] {
        if let Some(v) = counters.remove(legacy_key) {
            counters.entry(format!("{}_zh", legacy_key)).or_insert(v);
        }
    }
}

#[cfg(test)]
mod migrate_tests {
    use super::*;

    #[test]
    fn test_migrate_moves_legacy_keys() {
        let mut counters = HashMap::new();
        counters.insert("PersonName".to_string(), 5);
        counters.insert("OrgName".to_string(), 3);
        counters.insert("Address".to_string(), 1);
        counters.insert("Title".to_string(), 2);
        // 无关 key 不动
        counters.insert("mou_surname_张".to_string(), 1);

        migrate_legacy_counters(&mut counters);

        assert_eq!(counters.get("PersonName"), None);
        assert_eq!(counters.get("OrgName"), None);
        assert_eq!(counters.get("Address"), None);
        assert_eq!(counters.get("Title"), None);
        assert_eq!(counters.get("PersonName_zh"), Some(&5));
        assert_eq!(counters.get("OrgName_zh"), Some(&3));
        assert_eq!(counters.get("Address_zh"), Some(&1));
        assert_eq!(counters.get("Title_zh"), Some(&2));
        // 无关 key 保留
        assert_eq!(counters.get("mou_surname_张"), Some(&1));
    }

    #[test]
    fn test_migrate_idempotent_when_zh_already_set() {
        let mut counters = HashMap::new();
        counters.insert("PersonName".to_string(), 5);
        counters.insert("PersonName_zh".to_string(), 10); // 已迁移过
        migrate_legacy_counters(&mut counters);
        // 已存在的 _zh 不被覆盖
        assert_eq!(counters.get("PersonName_zh"), Some(&10));
        assert_eq!(counters.get("PersonName"), None);
    }

    #[test]
    fn test_migrate_no_legacy_keys_does_nothing() {
        let mut counters = HashMap::new();
        counters.insert("PersonName_zh".to_string(), 7);
        counters.insert("PersonName_en".to_string(), 3);
        let before = counters.clone();
        migrate_legacy_counters(&mut counters);
        assert_eq!(counters, before);
    }

    #[test]
    fn test_migrate_empty_counters_no_op() {
        let mut counters: HashMap<String, usize> = HashMap::new();
        migrate_legacy_counters(&mut counters);
        assert!(counters.is_empty());
    }
}
```

- [ ] **Step 2: 跑迁移单元测试确认通过**

```bash
cd src-tauri && cargo test commands::workspace::migrate_tests --lib
```

Expected: 4 个测试 PASS。

- [ ] **Step 3: 在 `read_workspace_data` 中接入迁移**

把 `read_workspace_data`（约第 31-36 行）改为：

```rust
pub(crate) fn read_workspace_data(path: &std::path::Path) -> Result<WorkspaceData, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("读取工作区文件失败: {}", e))?;
    let mut data: WorkspaceData = serde_json::from_str(&content)
        .map_err(|e| format!("解析工作区数据失败: {}", e))?;
    migrate_legacy_counters(&mut data.workspace.replace_counters);
    Ok(data)
}
```

- [ ] **Step 4: 跑全量 lib 测试确认零回归**

```bash
cd src-tauri && cargo test --lib
```

Expected: 全部 PASS。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/workspace.rs
git commit -m "feat(workspace): 加载工作区时迁移老 counter key 至 _zh 后缀"
```

---

## Task 9: 收紧英文集成测试断言

**Files:**
- Modify: `src-tauri/tests/restore_roundtrip_english.rs`
- Modify: `src-tauri/tests/strategy_switching_english.rs`

本任务把这两个文件里"只断言被替换"的弱断言收紧为"输出不含汉字 + 符合英文 Fake/Mou 形态"。

- [ ] **Step 1: 在 `restore_roundtrip_english.rs` 加输出格式断言**

打开 `src-tauri/tests/restore_roundtrip_english.rs`，在 `assert_spreadsheet_roundtrip` 函数体里 `// 脱敏后应有映射产生` 这段断言之后追加：

```rust
    // 脱敏输出不应含汉字（英文 Fake 应走英文池）
    let desensitized_text: String = match &result.content {
        FileContent::Spreadsheet { sheets, .. } => sheets
            .iter()
            .flat_map(|s| s.rows.iter().flat_map(|r| r.iter().map(|c| c.text.clone())))
            .collect::<Vec<_>>()
            .join(" "),
        FileContent::Document { paragraphs, .. } => {
            paragraphs.iter().map(|p| p.text.clone()).collect::<Vec<_>>().join(" ")
        }
    };
    assert!(
        desensitized_text.chars().all(|c| !('\u{4E00}'..='\u{9FFF}').contains(&c)),
        "英文文档脱敏后不应含汉字: {}",
        fixture_rel_path
    );
```

如果文件里同时有文档类辅助函数（类似 `assert_document_roundtrip`），同样补一段。

- [ ] **Step 2: 跑该测试确认通过**

```bash
cd src-tauri && cargo test --test restore_roundtrip_english
```

Expected: 全部 PASS。

- [ ] **Step 3: 收紧 `strategy_switching_english.rs` 中 SE06 的 PersonName 断言**

打开 `src-tauri/tests/strategy_switching_english.rs`，找到 `test_en_replace_styles_on_ner_types`（约第 339 行），把当前的 3 个 `assert_ne!(..., name, ...)` 弱断言替换为：

```rust
    // 三种风格都应替换原文
    assert_ne!(fake, name, "Fake 应替换原文");
    assert_ne!(mou, name, "Mou 应替换原文");
    assert_ne!(ordinal, name, "Ordinal 应替换原文");

    // Fake：英文假名（含空格、不含汉字、不是 John Doe 占位）
    assert!(fake.contains(' '), "Fake 输出应含空格: {}", fake);
    assert!(
        fake.chars().all(|c| !('\u{4E00}'..='\u{9FFF}').contains(&c)),
        "Fake 输出不应含汉字: {}",
        fake
    );
    assert_ne!(fake, "John Doe", "Fake 不应输出 John Doe（那是 Mou）");
    assert_ne!(fake, "Jane Doe", "Fake 不应输出 Jane Doe（那是 Mou）");

    // Mou：第一个英文 PersonName 应是 "John Doe"
    assert_eq!(mou, "John Doe", "Mou 第一个英文人名应是 John Doe");

    // Ordinal 在英文上降级为 Fake：含空格、不含汉字、不是 Person A
    assert!(!ordinal.starts_with("Person "), "Ordinal 在英文上不应输出 Person A: {}", ordinal);
    assert!(
        ordinal.chars().all(|c| !('\u{4E00}'..='\u{9FFF}').contains(&c)),
        "Ordinal 在英文上不应含汉字: {}",
        ordinal
    );
```

- [ ] **Step 4: 跑该测试确认通过**

```bash
cd src-tauri && cargo test --test strategy_switching_english
```

Expected: 全部 PASS。

- [ ] **Step 5: 跑全量测试确认零回归**

```bash
cd src-tauri && cargo test
```

Expected: 全部 PASS。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/tests/restore_roundtrip_english.rs src-tauri/tests/strategy_switching_english.rs
git commit -m "test(en): 收紧英文集成测试断言 — 不含汉字 + 具体形态匹配"
```

---

## Task 10: 用 dimkey 三件套补 Excel 用例 + 全量回归

**Files:**
- 测试用例 Excel（增量）
- `src-tauri/tests/fixtures/` 下英文 fixture

本任务用项目已有的 skill 链路完成，**不直接写 Rust 代码**。

- [ ] **Step 1: 用 `dimkey-test-design` skill 写 Excel 用例 + fixture**

在主对话或子对话里调用 `dimkey-test-design`，按 spec §"Excel 测试用例表"列的 10 个 case 描述场景：

```
EN-REPLACE-FAKE-PERSON  英文 PersonName + Fake → 含空格、纯 ASCII；fixture: docx + xlsx
EN-REPLACE-FAKE-ORG     英文 OrgName + Fake → 含 EN 已知 suffix 之一
EN-REPLACE-FAKE-ADDR    英文 Address + Fake → 形如 "数字 + 街 + 城市"
EN-REPLACE-FAKE-TITLE   英文 Title + Fake → 在 EN titles 池内
EN-REPLACE-MOU-PERSON   Mou: 第 1 → John Doe，第 2 → Jane Doe，第 3 → John Doe 2（性别轮换）
EN-REPLACE-MOU-ORG      Mou: Apple Inc. → Acme Inc.；GitHub → Acme Co.（兜底）
EN-REPLACE-MOU-ADDR     Mou: → [REDACTED CITY] / [REDACTED CITY] 2
EN-REPLACE-MOU-TITLE    Mou: → [REDACTED TITLE] / [REDACTED TITLE] 2
EN-REPLACE-ORDINAL-FALLBACK  Ordinal 英文输出与 Fake 一致（断言不含 "Person A"）
MIXED-CONSISTENCY-COUNTER-ISOLATION  full_pipeline xlsx：含中英 PersonName，counters 应有 PersonName_zh=N 和 PersonName_en=M，N+M=总命中数
```

- [ ] **Step 2: 用 `dimkey-test-codegen` 从 Excel 生成 Rust 测试代码**

调用 `dimkey-test-codegen` skill，让它读取 Excel 的"未覆盖"用例，参考已有的 `tests/strategy_switching_english.rs / restore_roundtrip_english.rs` 模板生成 Rust 代码。

- [ ] **Step 3: 用 `dimkey-test-run` 跑 english_replace 标签子集**

```bash
# 由 skill 自动选择测试集；预期输出：所有 english_replace 用例 PASS
```

- [ ] **Step 4: 跑全量回归确认中文用例零 regress**

```bash
cd src-tauri && cargo test
```

Expected: 全部 PASS。

- [ ] **Step 5: 让 dimkey-test-run 把通过状态回写 Excel**

`dimkey-test-run` skill 会自动更新 Excel 中的覆盖/通过状态。

- [ ] **Step 6: Commit Excel 用例 + 新生成的测试代码**

```bash
git add src-tauri/tests/ <excel 路径> <fixture 路径>
git commit -m "test(en): 新增英文假数据替换 Excel 用例 + Rust 测试 — 全量回归通过"
```

---

## Self-Review 摘要

- ✅ Spec 覆盖：所有 §决策（语言判断、Mou/Ordinal 美式、计数器隔离、码本迁移、测试策略）都有对应 Task
- ✅ 无 placeholder（每步都有具体代码 + 命令 + 预期）
- ✅ 类型一致：`Language` 全程指 `crate::models::language::Language`；counter key `PersonName_zh / _en` 等命名贯穿一致；method 签名 `next_name(lang)` / `next_mou_name(text, lang)` 一致
- ✅ 测试 TDD：每个新功能 Task 都"先写测 → 跑失败 → 实现 → 跑通过 → commit"
- ✅ Bite-sized：每步 2-5 分钟可完成；多 Task 间有 commit 边界

## 风险提示

- Task 2 的"全文替换 `data.field` → `data.zh.field`"用 sed 时务必精确匹配，否则可能误改注释。建议用 IDE 的 "Replace in file with regex" 功能，模式 `\bdata\.(person_names|org_components|titles|address_components|patterns)\b` → `data.zh.$1`。
- Task 5 的 `test_mou_person_name_en` 假设 EN mou 用 single counter `mou_name_en` 而不是按 first-letter 分计数；若实现走偏，5 处断言会一起 fail。
- Task 8 的迁移函数对老用户场景影响最小化：先 remove 旧 key，再用 `or_insert` 防覆盖新 key。
