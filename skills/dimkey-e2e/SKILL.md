---
name: dimkey-e2e
description: Dimkey 脱敏工具的自动化测试。自然语言描述场景 → Excel 用例管理 → 生成 Rust/Playwright 测试 → 执行 → 回写结果。触发词："跑测试"、"测一下"、"帮我测"、"E2E"、"验证脱敏"、"检查识别"、"回归测试"、"覆盖率"。不要在用户讨论通用测试概念时触发，仅在涉及 Dimkey 应用测试时使用。
---

# Dimkey E2E 测试

## 双层测试选择

| 用户意图 | 测试层 | 工具 |
|---------|--------|------|
| 文件检测、脱敏逻辑、识别准确率 | Rust 集成测试 | `cargo test` |
| 界面面板、按钮交互、视图切换 | UI 测试 | Playwright + mock |
| "跑测试" / "回归" | 两层都跑 | 两者 |

## 4 步工作流

### Step 1: Excel 用例管理

读写 `e2e/testcases.xlsx`。详见 [references/excel-manager.md](references/excel-manager.md)。

### Step 2: 生成测试代码

- **Rust 测试**: 在 `src-tauri/tests/` 下编写。详见 [references/rust-test-patterns.md](references/rust-test-patterns.md)。
- **UI 测试**: 在 `e2e/tests/` 下编写 pytest。详见 [references/ui-test-patterns.md](references/ui-test-patterns.md)。

### Step 3: 执行

```bash
# Rust（核心逻辑，165 个测试）
cd src-tauri && cargo test

# UI（面板/按钮，5 个测试，需先 TAURI_DEV_HOST=127.0.0.1 npm run dev）
DIMKEY_E2E=1 DIMKEY_TEST_URL=http://127.0.0.1:1420 \
  e2e/.venv/bin/pytest e2e/tests/ -v -m "not needs_backend"
```

### Step 4: 回报 + 回写 Excel

对话中汇报通过/失败和关键数据。用 `update_result()` 回写 Excel。

## Fixture 文件

```
e2e/fixtures/
├── sample.*           # 基础样本
├── batch/             # 批量测试
└── scenarios/         # 场景文档（21 个文件）
    ├── xlsx/ (8)      # 员工花名册、银行贷款…
    ├── csv/  (4)      # 员工信息表、客户通讯录…
    └── docx/ (9)      # 合同、病历、判决书…
```
