# Bug 清单

> 生成时间: 2026-04-04
> 来源: 全量测试执行（Rust + Pytest）

## 活跃 Bug

| # | 优先级 | 模块 | 失败测试 | 根因描述 | 分类 |
|---|--------|------|----------|----------|------|
| BUG-001 | P0 | dict_engine | `test_dict_empty_string_entry` (dict_integration.rs) | 空字符串作为字典条目时疑似触发无限循环，运行超 60s 被 SIGKILL | 死循环 |
| BUG-002 | P1 | regex_engine | `test_docx_investment_due_diligence_*` ×2, `test_xlsx_boundary_*` ×2, `test_xlsx_law_case_baseline` | CreditCode 正则对含字母的统一社会信用代码（如 `91320500MA1WXYZ123`、`11010119900307678X`）匹配失败 | 识别遗漏 |
| BUG-003 | P1 | strategy_switching | `test_replace_strategy_on_txt` (strategy_switching.rs) | TXT 格式下策略切换为 Replace 后，实际仍为 Mask，策略未正确应用 | 逻辑错误 |
| BUG-004 | P2 | parser::xlsx | `test_encrypted_xlsx_import_fails` (boundary.rs) | 加密 xlsx 文件导入未返回错误，静默通过 | 检测缺失 |
| BUG-005 | P2 | E2E | `test_language_switch_to_english` (test_ui_extras.py) | locator `button has_text="EN"` 匹配到 4 个元素（"Desensitize" 等含 EN 子串），strict mode 报错 | 测试代码 |
| BUG-006 | P2 | E2E | `test_sidebar_toggle` (test_ui_extras.py) | Page.goto 超时 30s，页面导航失败 | 环境/时序 |

## 环境问题（非 Bug）

| # | 说明 | 影响测试 | 解决方式 |
|---|------|----------|----------|
| ENV-001 | PDFium 动态库未部署 (`resources/pdfium/libpdfium.dylib`) | `desensitize_pdf.rs` 全部 3 个测试 | 将 PDFium 库放入 `src-tauri/resources/pdfium/` |
| ENV-002 | C01-C04 标记 `needs_backend`，需真实 Tauri 后端 | `test_basic_desensitize.py` 12 个测试 | 需在 Tauri 环境中执行 |

## 待确认

`test_xlsx_boundary_detect_counts`: 期望 ≥4 个银行卡号，实际识别 3 个。可能是 Luhn 校验误排了一个有效卡号，也可能是测试基线数据需要修正。归入 BUG-002 一并排查。

## 已关闭

| # | 关闭时间 | 模块 | 原失败测试 | 说明 |
|---|----------|------|------------|------|
| （暂无） | | | | |
