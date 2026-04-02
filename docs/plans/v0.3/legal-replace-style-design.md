# INK-017 法律场景脱敏替换格式（某式/序号式）

> 版本: v0.3.3 | 日期: 2026-02-28

## 背景

律师用户在脱敏法律文书时，当前 Replace 策略会生成随机假名（如"张三"→"赵云峰"），导致脱敏后文档不符合法律文书习惯。法律行业通用做法是使用"某式"（张某、某公司）或"序号式"（当事人一、甲公司）来替代敏感实体，便于阅读且一目了然哪些是脱敏内容。

## 需求目标

- 在现有 Replace 策略下新增**替换风格（ReplaceStyle）**子选项
- 支持三种风格：**假数据（现有默认）**、**某式**、**序号式**
- 适用所有文件类型（docx/txt/xlsx/csv）
- 同一实体全文一致替换（复用现有一致性映射机制）

## 替换风格定义

### 风格一：假数据（FakeName）— 现有默认行为

保持不变，使用 fake_data 生成随机假名。

| 敏感类型 | 替换示例 |
|---------|---------|
| PersonName | 张三 → 赵云峰 |
| OrgName | 腾讯科技 → 华盛信息技术有限公司 |

### 风格二：某式（Anonymous）

| 敏感类型 | 替换规则 | 示例 |
|---------|---------|------|
| PersonName | 取原文姓氏 + "某"；若姓氏提取失败则用"某某" | 张三 → 张某，李思明 → 李某 |
| OrgName | "某公司" + 中文序号（一、二、三…） | 腾讯科技 → 某公司一，阿里巴巴 → 某公司二 |
| Address | "某地" + 中文序号 | 北京市朝阳区 → 某地一 |
| Title | "某职务" | 技术总监 → 某职务 |

**注意**：PersonName 的某式需要从原文提取姓氏。提取逻辑：取原文第一个字符作为姓氏（覆盖单姓场景），复姓（欧阳、司马等）可在后续版本优化。若原文长度为 0 则用"某某"。

### 风格三：序号式（Ordinal）

| 敏感类型 | 替换规则 | 示例 |
|---------|---------|------|
| PersonName | "当事人" + 中文序号 | 张三 → 当事人一，李四 → 当事人二 |
| OrgName | 天干序号 + "公司" （甲乙丙丁戊己庚辛壬癸，超过10个续接"第十一公司"…） | 腾讯 → 甲公司，阿里 → 乙公司 |
| Address | "地址" + 中文序号 | 北京市朝阳区 → 地址一 |
| Title | "职务" + 中文序号 | 技术总监 → 职务一，产品经理 → 职务二 |

### 不受风格影响的类型

以下类型的 Replace 行为不受 ReplaceStyle 影响，保持现有假数据生成逻辑：

- Phone（手机号）
- IdCard（身份证号）
- BankCard（银行卡号）
- Email（邮箱）
- IpAddress（IP 地址）
- LandlinePhone（固定电话）
- LicensePlate（车牌号）
- CreditCode（统一社会信用代码）

**原因**：这些类型是结构化编号，法律文书中通常使用掩码而非某式/序号式处理，且当前默认策略已经是 Mask。用户如果对这些类型选了 Replace，仍然使用假数据替换，符合预期。

## Rust 后端改动

### 1. 数据模型 — `src-tauri/src/models/strategy.rs`

新增 `ReplaceStyle` 枚举，修改 `Strategy::Replace` 携带风格参数：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReplaceStyle {
    /// 假数据替换（现有默认行为）
    FakeName,
    /// 某式：张某、某公司一
    Anonymous,
    /// 序号式：当事人一、甲公司
    Ordinal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Strategy {
    Mask { keep_prefix: usize, keep_suffix: usize },
    Replace { style: ReplaceStyle },   // ← 从无参改为携带 style
    Generalize,
}
```

**兼容性**：现有工作区 JSON 中 `"Replace"` 是无参字符串。需要在反序列化时兼容：遇到旧的 `"Replace"` 字符串反序列化为 `Replace { style: FakeName }`。可通过 `#[serde(deserialize_with = "...")]` 自定义反序列化或使用 `#[serde(untagged)]` 配合额外变体实现。

### 2. 替换引擎 — `src-tauri/src/desensitizer/replace.rs`

#### 2.1 修改 `apply_replace` 函数签名

```rust
pub fn apply_replace(
    text: &str,
    sensitive_type: &SensitiveType,
    state: &mut ReplaceState,
    style: &ReplaceStyle,        // ← 新增参数
) -> String
```

#### 2.2 新增某式生成逻辑

```rust
fn apply_anonymous(
    text: &str,
    sensitive_type: &SensitiveType,
    state: &mut ReplaceState,
) -> String
```

- PersonName：提取首字符作为姓氏，返回 `"{姓}某"`
- OrgName：state 中维护 anonymous_org_counter，返回 `"某公司{中文序号}"`
- Address：state 中维护 anonymous_addr_counter，返回 `"某地{中文序号}"`
- Title：返回 `"某职务"`
- 其他类型：fallback 到 FakeName 逻辑

#### 2.3 新增序号式生成逻辑

```rust
fn apply_ordinal(
    sensitive_type: &SensitiveType,
    state: &mut ReplaceState,
) -> String
```

- PersonName：state 中维护 ordinal_person_counter，返回 `"当事人{中文序号}"`
- OrgName：state 中维护 ordinal_org_counter，返回 `"{天干}公司"` （甲乙丙丁…）
- Address：state 中维护 ordinal_addr_counter，返回 `"地址{中文序号}"`
- Title：state 中维护 ordinal_title_counter，返回 `"职务{中文序号}"`
- 其他类型：fallback 到 FakeName 逻辑

#### 2.4 中文序号工具函数

```rust
fn to_chinese_ordinal(n: usize) -> String
// 1→"一", 2→"二", ... 10→"十", 11→"十一", 20→"二十", 21→"二十一"

fn to_tiangan(n: usize) -> String
// 0→"甲", 1→"乙", ..., 9→"癸", 10→"第十一", 11→"第十二"
```

#### 2.5 ReplaceState 扩展

在 `ReplaceState` 中新增各风格的计数器：

```rust
pub struct ReplaceState {
    // ... 现有字段 ...

    // 某式计数器
    anonymous_org_counter: usize,
    anonymous_addr_counter: usize,

    // 序号式计数器
    ordinal_person_counter: usize,
    ordinal_org_counter: usize,
    ordinal_addr_counter: usize,
    ordinal_title_counter: usize,
}
```

这些计数器同样需要通过 `export_counters` / 构造函数实现跨会话持久化，存入工作区 JSON 的 `replace_counters` 字段。

### 3. 脱敏命令调用链适配

#### 3.1 `commands/desensitize.rs` — `apply_desensitize`

在匹配 `Strategy::Replace` 时传入 style：

```rust
Strategy::Replace { style } => {
    (replace::apply_replace(&item.text, &item.sensitive_type, &mut replace_state, &style), StrategyType::Replace)
}
```

#### 3.2 `commands/desensitize.rs` — `apply_desensitize_by_columns`

列级脱敏同理，从 `ColumnRule.strategy` 中获取 style 传递。

### 4. 一致性映射

**无需额外改动**。现有一致性映射机制以 `(original_text, sensitive_type)` 为 key，与 ReplaceStyle 无关。同一个原文在同一工作区内只会被脱敏一次，后续复用映射结果，天然保证风格一致。

**但需注意**：如果用户在同一工作区内切换了 ReplaceStyle，已有的映射结果不会自动更新。建议在用户切换风格时提示"已有脱敏结果将保留，如需重新脱敏请重置工作区"，或提供"重新脱敏"按钮清空映射后重跑。

### 5. 反向还原

现有反向还原逻辑基于一致性映射表做反向查找，与 ReplaceStyle 无关，**无需改动**。

## 前端改动

### 1. TypeScript 类型定义 — `src/types/index.ts`

```typescript
export type ReplaceStyle = "FakeName" | "Anonymous" | "Ordinal";

export type Strategy =
  | { Mask: { keep_prefix: number; keep_suffix: number } }
  | { Replace: { style: ReplaceStyle } }   // ← 从 "Replace" 字符串改为对象
  | "Generalize";
```

**兼容性**：前端代码中所有判断 `strategy === "Replace"` 的地方需要改为判断 `typeof strategy === "object" && "Replace" in strategy`。

### 2. 策略配置面板 — `src/components/StrategyPanel/RulesSection.tsx`

当用户选择 Replace 策略时，下方显示**替换风格**下拉选择：

- 假数据（默认）
- 某式（张某、某公司）
- 序号式（当事人一、甲公司）

交互逻辑：
1. 用户选择策略为 Replace → 显示风格下拉
2. 用户选择策略为 Mask 或 Generalize → 隐藏风格下拉
3. 风格变更时同样通过防抖保存到工作区

### 3. 列头策略配置弹窗 — `src/components/ColumnRulePopover/index.tsx`

在弹窗中选择 Replace 时，同样展示风格子选项。逻辑与 RulesSection 一致。

### 4. 策略默认值

`getAllowedStrategies` 函数无需修改（它控制的是策略类型选择，不涉及风格子选项）。

新建工作区时 Replace 策略默认风格为 `FakeName`，保持向后兼容。

## 文件改动清单

```
src-tauri/src/models/strategy.rs        — 新增 ReplaceStyle 枚举，修改 Strategy::Replace
src-tauri/src/desensitizer/replace.rs   — 新增 apply_anonymous / apply_ordinal，扩展 ReplaceState
src-tauri/src/commands/desensitize.rs   — apply_desensitize 和 apply_desensitize_by_columns 传入 style
src/types/index.ts                      — Strategy 类型适配
src/components/StrategyPanel/RulesSection.tsx  — Replace 策略下新增风格下拉
src/components/ColumnRulePopover/index.tsx     — 同上
```

## 不需要改动

- 三层检测引擎（regex/ner/dict）— 与替换策略无关
- Mask 和 Generalize 脱敏器 — 不受影响
- 一致性映射机制 — 自然兼容
- 反向还原逻辑 — 基于映射表，与风格无关
- 文件解析器（parser/*）— 与替换策略无关
- 导出逻辑 — 与替换策略无关

## 边界情况

- **姓氏提取失败**（某式 PersonName）：原文为空或非中文名时，使用"某某"兜底
- **序号超出天干范围**（序号式 OrgName）：超过10个机构时，使用"第十一公司"、"第十二公司"格式
- **旧工作区 JSON 兼容**：反序列化 `"Replace"` 字符串时自动映射为 `Replace { style: FakeName }`
- **工作区内切换风格**：已有映射不自动更新，需用户手动触发重新脱敏
- **非中文人名**：某式下提取首字符可能不是姓氏，此场景 fallback 为"某某"
