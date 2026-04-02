# Replace 替换风格（ReplaceStyle）设计文档

> 日期: 2026-02-28 | 版本: v0.3.3

## 背景

律师用户脱敏法律文书时，当前 Replace 策略生成随机假名（张三→赵云峰），不符合法律文书习惯。法律行业通用做法是使用"某式"（张某、某公司）或"序号式"（甲、甲公司）来替代敏感实体。

## 需求

- 在 Replace 策略下新增 **替换风格（ReplaceStyle）** 子选项
- 三种风格：**假数据（Fake，现有默认）**、**某式（Mou）**、**序号式（Ordinal）**
- 全局统一配置（所有 Replace 类型使用同一风格）
- 仅适用于文本型实体：PersonName、OrgName、Address、Title
- Phone/IdCard/Email 等格式型实体不受影响，继续用假数据
- 适用所有文件类型（docx/txt/xlsx/csv）
- 复用现有一致性映射机制

## 数据模型

### Rust 枚举

```rust
// models/strategy.rs

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ReplaceStyle {
    Fake,      // 假数据（现有默认）
    Mou,       // 某式
    Ordinal,   // 序号式
}

impl Default for ReplaceStyle {
    fn default() -> Self { ReplaceStyle::Fake }
}

// Strategy::Replace 从无字段变为带 style 字段
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Strategy {
    Mask { keep_prefix: usize, keep_suffix: usize },
    Replace { style: ReplaceStyle },
    Generalize,
}
```

### 向后兼容

旧 JSON 中的 `"Replace"` 需兼容反序列化为 `Replace { style: Fake }`。通过自定义 serde Deserialize 实现。

### 前端 TypeScript 类型

```typescript
type ReplaceStyle = "Fake" | "Mou" | "Ordinal";
type Strategy =
  | { Mask: { keep_prefix: number; keep_suffix: number } }
  | { Replace: { style: ReplaceStyle } }
  | "Generalize";
```

## 替换逻辑

### 某式（Mou）

| 类型 | 规则 | 示例 |
|------|------|------|
| PersonName | 提取姓氏 + "某"。同姓多人第二个起加中文数字序号 | 张三→张某，张四→张某二，李明→李某 |
| OrgName | 匹配尾部关键词 → "某"+关键词。未匹配→"某单位"。同类型多个加序号 | 腾讯科技→某公司，朝阳法院→某法院，中国银行→某银行 |
| Address | 统一→"某地"。多个加序号 | 北京朝阳区→某地，上海浦东→某地二 |
| Title | 统一→"某职务"。多个加序号 | 总经理→某职务 |

**姓氏提取**：优先匹配复姓表（欧阳/司马/上官等），否则取首字。

**组织后缀关键词表**：公司、法院、检察院、银行、医院、学校、大学、集团、局、委、所、中心、协会、基金会 等。

### 序号式（Ordinal）

| 类型 | 序列 | 示例 |
|------|------|------|
| PersonName | 甲乙丙丁戊己庚辛壬癸，超过10个用"当事人11"... | 张三→甲，李四→乙 |
| OrgName | 甲公司/乙公司...后缀提取规则同某式 | 腾讯→甲公司，百度→乙公司 |
| Address | A地址/B地址/C地址...(A-Z)，超过26个用"地址27"... | 北京路→A地址 |
| Title | 职务一/职务二... | 总经理→职务一 |

### 一致性映射

- 复用现有 `ConsistencyMapping` 机制
- 同一 `original_text` 始终映射到相同替换结果
- 序号按首次出现顺序分配
- 某式的同姓计数器和序号式的全局计数器记入 `workspace.replace_counters`

## 前端 UI

### 策略面板（RulesSection）

当有类型使用 Replace 策略时，在面板中显示全局"替换风格"选择器（三个 radio button / segmented control）：

```
替换风格: [假数据]  [某式]  [序号式]
```

全局设置，只显示一次。

### 策略配置弹窗（StrategyConfig）

同样在弹窗中增加全局"替换风格"选择。

## 配置持久化

### 全局 config.json

```json
{
  "strategies": { ... },
  "replace_style": "Fake"
}
```

### Workspace JSON

```json
{
  "strategies": { ... },
  "replace_style": "Mou",
  ...
}
```

字段缺失时默认为 `"Fake"`，向后兼容。

## 数据流

1. 前端调用 `apply_desensitize` 时传递 `replace_style` 参数
2. Rust 端根据 `replace_style` 选择生成逻辑分支（Fake 走现有逻辑，Mou/Ordinal 走新逻辑）
3. 对 PersonName/OrgName/Address/Title 应用风格化替换
4. 对 Phone/IdCard/Email 等类型忽略风格，继续用 Fake 假数据
5. 生成的映射存入 `ConsistencyMapping`

## 不做

- 不支持每个敏感类型单独配置风格
- 不对格式型实体（Phone/IdCard/Email 等）应用某式/序号式
- 不做自定义前缀/序列配置
