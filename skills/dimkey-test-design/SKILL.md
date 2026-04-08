---
name: dimkey-test-design
description: Dimkey 测试用例设计。自然语言描述场景 → 生成 fixture 文件 → 写 Excel 用例 + 基线数据。触发词："添加用例"、"设计测试"、"补充场景"、"新增测试"。适用场景：为 Dimkey 脱敏工具设计新的测试用例、补充边界场景、生成测试数据文件。
---

# Dimkey 测试用例设计

## 定位

测试工作流**第一步** — 设计测试素材。只负责"测什么"，不负责"怎么测"和"跑测试"。

**AI 只做生成时工作，所有输出需用户审核确认。**

不在用户讨论通用测试概念时触发，仅在涉及 Dimkey 应用测试用例设计时使用。

## 工作流

```
用户自然语言描述场景
    │
    ▼
Step 1: 检查 fixture 是否存在
    ├─ 存在 → 跳到 Step 3
    └─ 不存在 → Step 2
    │
    ▼
Step 2: 生成 fixture 文件
    写入 e2e/fixtures/scenarios/{ext}/ 下
    详见 [references/fixture-patterns.md](references/fixture-patterns.md)
    │
    ▼
Step 3: 读取 fixture 内容，AI 分析敏感值
    识别其中的手机号、身份证、邮箱、姓名等
    区分 hard（正则类）和 soft（NER 类）断言模式
    │
    ▼
Step 3.5: 生成 .baseline.json sidecar 文件
    与 fixture 同目录同名，如 sample.csv → sample.csv.baseline.json
    这是基线数据的**单一数据源**，测试代码运行时直接读取
    │
    ▼
Step 4: 写 Excel
    Sheet1: 新增用例行（add_testcase），不传 test_file（由 codegen 阶段回写）
    Sheet2: 从 .baseline.json 聚合写入（非数据源，仅展示用）
    详见 [references/excel-manager.md](references/excel-manager.md)
    │
    ▼
Step 5: AI 发散 — 补充边界用例
    基于已有场景，联想边界值、异常路径
    如：全角数字、带前缀号码、空值、超长文本等
    │
    ▼
Step 6: 输出汇报，等待用户审核
    列出所有新增的用例 ID、基线条目数
    用户可删除/修改后确认
```

## 基线断言模式

| 模式 | 适用类型 | 含义 |
|------|----------|------|
| hard | Phone, IdCard, Email, BankCard, CreditCode, Landline, LicensePlate, IpAddress, SSN, CreditCard, UsPhone, UkPhone, Passport, IBAN, ZipCode, UkPostcode, DriversLicense | 正则类 |
| soft | PersonName, OrgName, Address, Title | NER 类 |

**注意**: generator 和 sidecar 中保留 soft/hard 标记用于区分敏感类型来源（正则 vs NER），但 **full_pipeline 测试中所有断言统一按 hard 处理**——无论标记为 soft 还是 hard，未命中即失败。soft 标记仅作为元数据保留，不影响测试行为。

## Fixture 文件

### 目录结构

```
e2e/fixtures/scenarios/
├── xlsx/       # Excel 场景（含中英文）
├── csv/        # CSV 场景（含中英文）
├── docx/       # Word 场景（含中英文）
├── txt/        # 纯文本场景
└── pdf/        # PDF 场景（预留）

e2e/fixtures/boundary/  # 边界/编码/异常场景（不按格式归类）
```

**分类规则**: 按文件格式归目录，英文/双语文件用英文命名放入对应格式目录（如 `csv/english_employee.csv`）。边界和编码测试统一放 `boundary/`。

### .baseline.json Sidecar 文件

每个 fixture 旁边放一个 `.baseline.json`，记录该 fixture 中所有预期敏感值：

```json
{
  "fixture": "scenarios/csv/uk_customer_records.csv",
  "generated_by": "dimkey-test-design",
  "generated_at": "2026-04-04",
  "expected": [
    {"value": "+44 7911 123456", "type": "UkPhone", "count": 1, "note": "移动号码", "assert": "hard"},
    {"value": "Oliver Thompson", "type": "PersonName", "count": 1, "note": "NER", "assert": "soft"}
  ]
}
```

**关键原则**：
- Sidecar 是基线数据的**单一数据源**，fixture 和 sidecar 必须同时生成
- `assert: "hard"` → 正则类
- `assert: "soft"` → NER 类
- **测试行为**: full_pipeline 测试中 soft 和 hard 均为必须命中，未命中即 fail。soft/hard 仅标记来源类型
- `count` → 该值在文件中出现的次数
- Sheet2 从 sidecar 聚合生成，不再是数据源

### 生成规范

详见 [references/fixture-patterns.md](references/fixture-patterns.md)。

核心原则：
- 贴近真实业务，多种敏感类型混合
- 每个 fixture 至少 3 种敏感类型
- 表格类 10-30 行，文档类 1-3 页
- 英文/双语文件用英文命名，放入对应格式目录

## 不做的事

- **不写测试代码** → 交给 `dimkey-test-codegen`
- **不执行测试** → 交给 `dimkey-test-run`
- **不修改已有测试文件**
