---
date: 2026-04-25
status: draft
owner: tanzeshun
title: 英文假数据替换字典 — 按实体语言自动分发 + Mou/Ordinal 美式风格
---

# 英文假数据替换字典 — 按实体语言自动分发

## 背景

Dimkey 的 NER 已支持识别英文 `PersonName / OrgName / Address / Title`，但 `desensitizer/replace.rs` 写死加载 `resources/fake_data/zh/*.json`，导致英文实体被替换为中文假名（`John Smith` → `赵芳`），破坏可读性和合规性。

`resources/fake_data/en/` 目录下的英文词库已就位但**完全没人消费**。本设计补齐消费链路，并按"主打美国市场"的目标定制 `Mou` / `Ordinal` 两种 `ReplaceStyle` 在英文场景下的语义。

## 目标

1. 英文实体替换走英文词库，输出符合美国读者直觉的假名/假机构/假地址
2. 中英文计数器隔离，互不干扰，保证各自池内"不耗尽不重复"
3. `Mou` 风格在英文上对齐美国法律惯例（`John Doe / Jane Doe / Acme Corp.`）
4. `Ordinal` 风格在英文上**静默降级为 Fake**（裸 `Person A / Company A` 不是地道的美国写法，与其牵强不如让 Fake 接管）
5. 老工作区码本兼容：旧 `PersonName` 计数器迁移到 `PersonName_zh`

## 非目标

- 不区分 US/UK 英文（EN 词库里掺杂 `London/Manchester` 等 UK 地名是 OK 的，都算英文场景）
- 不支持用户手动覆盖语言判断（YAGNI；NER 单语言识别 + 字符自动判断已足够）
- 不修改 `Mask / Generalize` 策略（这两个策略本就语言无关）
- 不修改前端 UI（语言判断在后端完成，前端无感）

## 关键决策与依据

| 维度 | 决策 | 依据 |
|---|---|---|
| 语言判断方式 | 按实体原文字符自动判断（含任一汉字 → Zh，否则 En） | 用户文档常出现中英混排，按字符判断零配置；NER 单语言识别保证不会脏 |
| Mou/Ordinal 英文化 | Mou 实现，Ordinal 在英文上降级为 Fake | 主打美国市场。`John Doe` 是真实美国法律惯例；`Person A / Company A` 不是英文典型用法 |
| 计数器隔离 | 中英分离（`PersonName_zh` / `PersonName_en` 等独立 key） | unique 池消费语义干净，不会因计数器跳跃浪费英文池组合 |
| 边界规则 | 含任一汉字走中文，否则英文，空字符串兜底 Zh | 最坏退化为中文输出，不影响数据可读性 |

## 架构概览

新增枚举与函数（在 `desensitizer/replace.rs` 顶部）：

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Language { Zh, En }

fn detect_language(text: &str) -> Language {
    if text.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c)) {
        Language::Zh
    } else {
        Language::En
    }
}
```

`FakeData` 改为持有两套子结构（中英字段不同，不强行共享 schema）：

```rust
struct ZhFakeData { person_names, org_components, titles, address_components, patterns }
struct EnFakeData { person_names, org_components, titles, address_components }
struct FakeData { zh: ZhFakeData, en: EnFakeData }
```

`patterns.json`（手机号/身份证号等格式型实体）只有中文版需要，因为 EN 的格式型实体 (`Ssn / CreditCard / UsPhone / UkPhone / Passport / Iban / ZipCode / UkPostcode / DriversLicense`) 已经是独立的 `SensitiveType`，在 `apply_replace` 内分支里就近生成，不依赖词库文件。

`ReplaceState` 字段扩展为双套洗牌索引：

```rust
pub struct ReplaceState {
    seed: u64,
    counters: HashMap<String, usize>,
    name_indices_zh: Option<Vec<u32>>,
    name_indices_en: Option<Vec<u32>>,
    org_indices_zh: Option<Vec<u32>>,
    org_indices_en: Option<Vec<u32>>,
    address_indices_zh: Option<Vec<u32>>,
    address_indices_en: Option<Vec<u32>>,
    title_indices_zh: Option<Vec<u32>>,
    title_indices_en: Option<Vec<u32>>,
}
```

类型偏移量翻倍（zh / en 分开种子），保证两套池洗牌结果完全独立：

```rust
const NAME_SEED_OFFSET_ZH: u64 = 0;
const NAME_SEED_OFFSET_EN: u64 = 4;
const ORG_SEED_OFFSET_ZH: u64 = 1;
const ORG_SEED_OFFSET_EN: u64 = 5;
// ... 以此类推
```

## 数据流（以 PersonName 为例）

```
NER 识别 "John Smith" / "张三"
  ↓
SensitiveItem { text, sensitive_type: PersonName }
  ↓
apply_replace(text, PersonName, &mut state, style)
  ↓
let lang = detect_language(text);   // 唯一语言判断点
  ↓
match (sensitive_type, style):
    (PersonName, Fake)    → state.next_name(lang)
    (PersonName, Mou)     → state.next_mou_name(text, lang)
    (PersonName, Ordinal) if lang == En → state.next_name(lang)  // 静默降级
    (PersonName, Ordinal) if lang == Zh → state.next_ordinal_name()
  ↓
state.next_name(Language::En):
    counter_key = "PersonName_en"
    indices    = self.name_indices_en (lazy init from EN pool)
    pool       = en.first_names × en.last_names
    output     = "{first} {last}" + 末尾 wrap > 0 时加序号
  ↓
返回 "Robert Garcia"
```

**要点：**
- 所有语言判断集中在 `apply_replace` 顶部，下游显式传 `Language`
- `Ordinal` 在英文上降级，由 `apply_replace` 的 match 分支完成（非各 `next_*` 内部）
- 拼装规则不同：`zh = "{surname}{given}"` / `en = "{first} {last}"`；`zh = "{district}{street}{number}号"` / `en = "{number} {street}, {city}"`

## 风格映射总表

| 类型 / Style | Fake | Mou | Ordinal |
|---|---|---|---|
| **PersonName** zh | `赵芳` | `张某 / 张某二` | `当事人一` |
| **PersonName** en | `Robert Garcia` | `John Doe / Jane Doe / John Doe 2 / Jane Doe 2 ...`（性别轮换） | **降级为 Fake** |
| **OrgName** zh | `创新科技公司` | `某公司 / 某法院` | `甲公司` |
| **OrgName** en | `Apex Solutions Inc.` | `Acme Inc. / Acme LLC ...`（保留原文 suffix） | **降级为 Fake** |
| **Address** zh | `朝阳区建国路100号` | `某地 / 某地二` | `地址一` |
| **Address** en | `123 Main St, New York, NY` | `[REDACTED CITY] / [REDACTED CITY] 2` | **降级为 Fake** |
| **Title** zh | `主任工程师` | `某职务` | `职务一` |
| **Title** en | `Software Engineer` | `[REDACTED TITLE] / [REDACTED TITLE] 2` | **降级为 Fake** |

### 英文 Mou 实现细节

**PersonName Mou:**
- 单一 counter `mou_name_en`，无需按 first-letter 分计数
- 偶数序号 → `John Doe`，奇数 → `Jane Doe`
- 同一对再出现时加阿拉伯数字后缀：`John Doe / Jane Doe / John Doe 2 / Jane Doe 2 / John Doe 3 / Jane Doe 3 ...`
- 不保留原文首字母（与中文"保留姓"语义不同 —— 美国匿名化优先级是"完全不泄露"，保留首字母反而构成 PII 残留）

**OrgName Mou:**
- 后缀表 `EN_ORG_SUFFIXES = ["Inc.", "Corp.", "LLC", "Ltd.", "Group", "Holdings", "Partners", "Associates", "International", "Co."]`（与 Fake 池 `en/org_components.json` 的 suffixes 完全对齐，避免不一致）
- 按出现顺序匹配原文末尾；匹配到 → 输出 `Acme {suffix}`；匹配不到（如 `GitHub`）→ 兜底 `Acme Co.`
- 同 suffix 第 N 次出现 → `Acme Inc. 2`；counter key `mou_org_en_{suffix}`

**Address Mou:**
- 单一 counter `mou_address_en`
- 第 1 次 → `[REDACTED CITY]`，第 N 次 → `[REDACTED CITY] {N}`

**Title Mou:**
- 单一 counter `mou_title_en`
- 同上 → `[REDACTED TITLE]` / `[REDACTED TITLE] 2`

## API 变更

### `ReplaceState` method 签名

```rust
// 旧
pub fn next_name(&mut self) -> String;
pub fn next_org(&mut self) -> String;
pub fn next_address(&mut self) -> String;
pub fn next_title(&mut self) -> String;
pub fn next_mou_name(&mut self, original: &str) -> String;
pub fn next_mou_org(&mut self, original: &str) -> String;
pub fn next_mou_address(&mut self, _original: &str) -> String;
pub fn next_mou_title(&mut self, _original: &str) -> String;

// 新
pub fn next_name(&mut self, lang: Language) -> String;
pub fn next_org(&mut self, lang: Language) -> String;
pub fn next_address(&mut self, lang: Language) -> String;
pub fn next_title(&mut self, lang: Language) -> String;
pub fn next_mou_name(&mut self, original: &str, lang: Language) -> String;
pub fn next_mou_org(&mut self, original: &str, lang: Language) -> String;
pub fn next_mou_address(&mut self, original: &str, lang: Language) -> String;
pub fn next_mou_title(&mut self, original: &str, lang: Language) -> String;

// Ordinal 系列保持原签名（中文专属，无需 lang 参数）
pub fn next_ordinal_name(&mut self) -> String;
pub fn next_ordinal_org(&mut self, original: &str) -> String;
pub fn next_ordinal_address(&mut self) -> String;
pub fn next_ordinal_title(&mut self) -> String;
```

`apply_replace` 公共签名不变。

### Counter key 命名

| 旧 key | 新 key |
|---|---|
| `PersonName` | `PersonName_zh` / `PersonName_en` |
| `OrgName` | `OrgName_zh` / `OrgName_en` |
| `Address` | `Address_zh` / `Address_en` |
| `Title` | `Title_zh` / `Title_en` |
| `mou_surname_{X}` | 中文保持 / 英文新增 `mou_name_en` |
| `mou_org_{suffix}` | 中文保持 / 英文新增 `mou_org_en_{suffix}` |
| `mou_address` | 中文保持 / 英文新增 `mou_address_en` |
| `mou_title` | 中文保持 / 英文新增 `mou_title_en` |
| `ordinal_*` | 不变（中文专属） |

## 错误处理与边界

1. **空字符串**：`detect_language("")` 兜底 `Zh`；上游已过滤空文本不走 replace，此分支基本不触发
2. **EN PersonName / OrgName / Address 池耗尽**：和中文一致，靠 `wrap` 加序号 → `John Doe 1 / Robert Garcia 1` 等
3. **EN Address 数字池**：`numbers` 共 15 个 × 24 streets × 25 cities = 9000 组合，远超日常使用
4. **Mou EN OrgName 找不到 suffix**：兜底 `Acme Co.`（已在 suffix 表中）
5. **detect_language 性能**：`text.chars().any()` 在长字符串下短路，最坏 O(n)，n 通常 ≤ 100 字符；可接受

## 工作区码本迁移

加载工作区时（`commands/workspace.rs` 的 workspace 反序列化后），插入一段迁移：

```rust
fn migrate_legacy_counters(counters: &mut HashMap<String, usize>) {
    for legacy_key in ["PersonName", "OrgName", "Address", "Title"] {
        if let Some(v) = counters.remove(legacy_key) {
            counters.entry(format!("{}_zh", legacy_key)).or_insert(v);
        }
    }
}
```

**理由：**
- 老用户绝大多数是中文场景，旧 `PersonName` 计数器实际全部对应中文池消费记录，迁移到 `PersonName_zh` 保持序号连续性
- 旧 mou/ordinal counter key 不变，无需迁移
- `or_insert(v)` 而非 `insert(v)`：防御老 `PersonName` 和新 `PersonName_zh` 共存时（理论上不会发生，但保险）

## 测试策略

### Excel 测试用例表（源头）

用 `dimkey-test-design` skill 新增以下用例，分类标签 `english_replace`：

| 用例 ID | 场景 | fixture 类型 |
|---|---|---|
| `EN-REPLACE-FAKE-PERSON` | 英文 PersonName + Fake → 含空格、纯 ASCII 输出 | docx + xlsx |
| `EN-REPLACE-FAKE-ORG` | 英文 OrgName + Fake → 含 `Inc./Corp./LLC` 之一 | docx + xlsx |
| `EN-REPLACE-FAKE-ADDR` | 英文 Address + Fake → 形如 `数字 + 街 + 城市` | docx + xlsx |
| `EN-REPLACE-FAKE-TITLE` | 英文 Title + Fake → 在英文 titles 池内 | docx + xlsx |
| `EN-REPLACE-MOU-PERSON` | Mou: 第 1 → John Doe, 第 2 → Jane Doe, 第 3 → John Doe 2 | docx |
| `EN-REPLACE-MOU-ORG` | Mou: `Apple Inc.` → `Acme Inc.`；`GitHub` → `Acme Co.`（兜底） | docx |
| `EN-REPLACE-MOU-ADDR` | Mou: → `[REDACTED CITY]` / `[REDACTED CITY] 2` | docx |
| `EN-REPLACE-MOU-TITLE` | Mou: → `[REDACTED TITLE]` | docx |
| `EN-REPLACE-ORDINAL-FALLBACK` | Ordinal 在英文实体上输出与 Fake 一致（不含 `Person A`） | docx |
| `MIXED-CONSISTENCY-COUNTER-ISOLATION` | 文档含中英 PersonName，counters 应有 `PersonName_zh=N` 和 `PersonName_en=M`，N+M=总命中数 | full_pipeline xlsx |

更新现有英文用例基线期望值：
- `restore_roundtrip_english.rs`：基线由"含汉字"改为"不含汉字"
- `strategy_switching_english.rs`：PersonName/OrgName/Address/Title 的 Fake/Mou 断言改为对应英文模式

### Rust 单元测试（在 `replace.rs` 内）

新增（13 个，按上面用例的最小复现）：
```
test_detect_language
test_replace_person_name_en
test_replace_org_en
test_replace_address_en
test_replace_title_en
test_mou_person_name_en
test_mou_org_en
test_mou_org_en_fallback
test_mou_address_en
test_mou_title_en
test_ordinal_en_fallback_to_fake
test_uniqueness_names_en
test_uniqueness_orgs_en
test_counters_isolated     // 中英计数器独立
```

修改现有（5 个）：
- `test_uniqueness_names / orgs / addresses` → 补 `Language::Zh` 参数
- `test_deterministic_with_seed` → 补 `Language::Zh` 参数
- `test_counter_resume` → counter key 改为 `PersonName_zh`

### Rust 集成测试（`tests/`）

- `restore_roundtrip_english.rs`：断言收紧为"假名输出**不含汉字**"
- `strategy_switching_english.rs`：调整断言匹配新风格映射表
- 新增 `tests/counter_isolation.rs`（可选）：full_pipeline 验证中英 counter 独立 —— 由 `MIXED-CONSISTENCY-COUNTER-ISOLATION` 用例驱动

### 工具链

- 用 `dimkey-test-design` 写 Excel + fixture
- 用 `dimkey-test-codegen` 从 Excel 生成 Rust 测试代码
- 用 `dimkey-test-run` 跑 `english_replace` 标签子集 + 全量回归确认无中文用例 regress；结果回写 Excel

## 文件影响清单

修改：
- `src-tauri/src/desensitizer/replace.rs` — 主要改动（新增 enum + 拆 FakeData + 改 method 签名 + Mou/Ordinal 英文分支）
- `src-tauri/src/commands/workspace.rs` — 加 `migrate_legacy_counters`
- `src-tauri/tests/restore_roundtrip_english.rs` — 收紧断言
- `src-tauri/tests/strategy_switching_english.rs` — 调整风格断言

新增：
- 测试用例 Excel（增量）
- fixture 文件（按上面用例清单）

不变：
- `src-tauri/resources/fake_data/en/*.json` — 已就位
- `src-tauri/src/models/sensitive.rs` — `SensitiveType` 不动
- 前端 — 完全无感

## 实施顺序建议（留给 writing-plans）

1. 先扩 `FakeData` 结构 + 加 `Language` enum + `detect_language`
2. 改 `ReplaceState` 字段 + method 签名（中文路径不动行为）→ 跑现有测试确认中文不 regress
3. 加英文 Fake 分支
4. 加英文 Mou 分支
5. 加 Ordinal 英文降级
6. 加 `migrate_legacy_counters`
7. 用 `dimkey-test-design` / `-codegen` / `-run` 三件套补测试

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 计数器 key 改名导致老工作区序号重置 | `migrate_legacy_counters` 一行迁移 |
| 英文 mou OrgName 的 suffix 表覆盖不全 | 兜底 `Acme Co.`，且后续可扩 |
| EN 词库当前规模有限（68 first × 64 last = 4352 名字） | 与中文池量级相仿；若实际不够，后续扩词库即可（不影响代码） |
| Mou English `John Doe` 的性别二选一可能不被部分用户接受 | 文档化此行为；如有反馈可改为单一 `John Doe + 序号` |
