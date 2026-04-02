# 多语言支持设计（一期：英文）

## 概述

为Dimkey（Dimkey）添加英文支持，面向海外市场。分两期：

- **一期**：i18n 框架 + 英文界面 + 英文正则规则 + 英文假数据
- **二期**：英文 NER 模型调研与集成（Person Name / Org / Address）

## 总体方案

**方案 B：前端 i18next + 后端全局语言状态**

- 前端用 `react-i18next` 管理界面文本翻译
- 后端通过 Tauri State 维护当前语言，引擎根据语言加载对应规则集和假数据
- 前端切换语言时调用 `set_language` command，后续引擎调用自动读取全局状态

语言切换方式：默认跟随系统语言，允许手动覆盖。

---

## 模块一：前端 i18n

### 技术选型

- `react-i18next` + `i18next-browser-languagedetector`

### 翻译文件结构

```
src/locales/
├── zh.json
└── en.json
```

### Key 组织

```json
{
  "common": { "confirm": "确认", "cancel": "取消" },
  "sensitiveType": { "Phone": "手机号", "Ssn": "SSN" },
  "strategy": { "Mask": "掩码", "Replace": "替换", "Generalize": "泛化" },
  "replaceStyle": { "Fake": "假数据", "Mou": "某式", "Ordinal": "序号式" },
  "home": { "dropHint": "拖入文件开始脱敏" },
  "preview": { "scanning": "扫描识别中" },
  "result": { "exportBtn": "导出" }
}
```

### 语言切换 UI

在布局顶部或设置区域添加语言选择器。切换时：

1. 更新 i18next 语言
2. 调用后端 `set_language` command
3. 持久化到 localStorage

### 初始化逻辑

1. 读 localStorage 用户偏好
2. 无偏好则用 `i18next-browser-languagedetector` 检测系统语言
3. 将确定的语言同步到后端

### 需改造的现有文件

- `src/types/index.ts` — `SENSITIVE_TYPE_CONFIG`、`STRATEGY_LABELS` 等常量改为函数式获取
- `src/layouts/WorkspaceLayout.tsx` — 步骤标签
- 各页面和组件中的按钮、提示、placeholder 文本

---

## 模块二：后端语言状态与引擎路由

### 全局语言状态

```rust
pub struct AppLanguage(pub RwLock<Language>);

pub enum Language {
    Zh,
    En,
}
```

通过 Tauri `manage()` 注入。新增 command：

```rust
#[tauri::command]
fn set_language(lang: String, state: State<AppLanguage>) -> Result<(), String>
```

### 正则引擎改造

按语言拆分规则集：

```
engine/
├── regex_engine.rs          # 引擎主体，根据语言选择规则集
├── rules/
│   ├── mod.rs
│   ├── common.rs            # 通用规则（Email、IP）
│   ├── zh.rs                # 中文规则
│   └── en.rs                # 英文规则
```

### SensitiveType 枚举扩展

```rust
pub enum SensitiveType {
    // 通用
    Email, IpAddress,
    // 中文特有
    Phone, IdCard, BankCard, LandlinePhone, LicensePlate, CreditCode,
    // 英文特有
    Ssn, CreditCard, UsPhone, UkPhone, Passport, Iban, ZipCode, UkPostcode, DriversLicense,
    // NER 实体（中英通用，二期复用）
    PersonName, OrgName, Address, Title,
    // 自定义
    Custom(String),
}
```

### 假数据资源按语言分目录

```
resources/fake_data/
├── zh/
│   ├── patterns.json
│   ├── person_names.json
│   ├── org_components.json
│   ├── address_components.json
│   └── titles.json
└── en/
    ├── patterns.json
    ├── person_names.json
    ├── org_components.json
    ├── address_components.json
    └── titles.json
```

### 替换风格适配

| 风格 | 中文 | 英文 |
|------|------|------|
| Fake | 张伟 → 李明 | John Smith → Sarah Johnson |
| Mou（某式） | 张某某 | [REDACTED] |
| Ordinal | 人员-1 | Person-1 |

---

## 模块三：英文正则规则

### 规则清单

| 类型 | 正则模式 | 示例 | 默认策略 |
|------|---------|------|---------|
| SSN | `\d{3}-\d{2}-\d{4}` | 123-45-6789 | Mask（保留后4位） |
| US Phone | `(\+1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}` | (555) 123-4567 | Mask（保留后4位） |
| UK Phone | `(\+44[-.\s]?\|0)\d{2,4}[-.\s]?\d{3,4}[-.\s]?\d{3,4}` | +44 20 7946 0958 | Mask（保留后4位） |
| Credit Card | `\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}` + Luhn 校验 | 4111-1111-1111-1111 | Mask（保留后4位） |
| Passport | `[A-Z]{1,2}\d{6,9}` | AB1234567 | Mask（保留前2位） |
| IBAN | `[A-Z]{2}\d{2}[A-Z0-9]{4}\d{7}([A-Z0-9]?){0,16}` | GB29 NWBK 6016 1331 9268 19 | Mask（保留前4后4） |
| ZIP Code | `\d{5}(-\d{4})?` | 10001 | Generalize（保留前3位） |
| UK Postcode | `[A-Z]{1,2}\d[A-Z\d]?\s?\d[A-Z]{2}` | SW1A 1AA | Generalize（保留前半） |
| Driver's License | 宽松模式（各州差异大） | — | Mask |

### 注意事项

- ZIP Code、Passport 正则易误匹配普通数字，需加上下文约束（前后不能紧邻其他数字）
- Credit Card 加 Luhn 校验减少误报
- Driver's License 各州格式差异大，一期做宽松匹配，准确率偏低时提示用户手动确认

---

## 模块四：配置持久化与语言联动

### 语言设置持久化

`config.json` 新增 `language` 字段：

```json
{
  "language": "zh",
  "strategies": { ... }
}
```

应用启动流程：

1. Rust 读取 `config.json` 中的 `language`
2. 若无值，返回 `null`，前端走系统语言检测
3. 前端确定语言后调用 `set_language`，同时持久化

### 策略配置

中英文 SensitiveType 不同，策略按类型存储，天然隔离。切换语言时策略不清空，各语言独立保存。

### 自定义词典

`DictEntry` 新增可选 `language` 字段：

```rust
pub struct DictEntry {
    pub text: String,
    pub sensitive_type: SensitiveType,
    pub match_mode: MatchMode,
    pub replacement: Option<String>,
    pub language: Option<Language>,  // None = 所有语言生效
}
```

---

## 二期预留：英文 NER

- 当前 NER 引擎已支持可插拔后端，架构上预留多模型支持
- 二期需专项调研英文 NER 模型选择（轻量英文模型 vs 多语言模型）
- SensitiveType 中 PersonName / OrgName / Address / Title 中英通用，无需改动

---

## 一期不做

- 英文 NER 模型集成（二期）
- 其他语言支持（日文、韩文等）
- PDF/图片 OCR 多语言
