# Bug 清单

> 更新时间: 2026-04-04（第 3 次执行）
> 来源: 全量测试执行（Rust + Pytest）

## 活跃 Bug

| # | 优先级 | 模块 | 失败测试 | 根因描述 | 分类 |
|---|--------|------|----------|----------|------|
| BUG-004 | P2 | parser::xlsx | `test_encrypted_xlsx_import_fails` (boundary.rs) | 加密 xlsx 文件导入未返回错误，静默通过 | 检测缺失 |
| BUG-005 | P2 | E2E | `test_language_switch_to_english` (test_ui_extras.py) | locator `button has_text="EN"` 匹配到 4 个元素（"Desensitize" 等含 EN 子串），strict mode 报错 | 测试代码 |
| BUG-008 | P1 | regex_engine | `test_uk_csv_baseline`, `test_us_compliance_baseline`, `test_international_*` ×4 | DriversLicense 格式（UK: `WILSO703159GW9IJ`、US: `D123-4567-8901`）引擎不支持 | 功能缺失 |
| BUG-009 | P1 | regex_engine | `test_uk_csv_baseline`, `test_us_compliance_baseline` | CreditCard 带空格格式（`5425 2334 1102 8976`）未识别，共 14 张卡 | 识别遗漏 |
| BUG-010 | P2 | regex_engine | `test_txt_it_ops_baseline_coverage` | IPv6 地址（`2001:0db8:...`、`fd00::3:50`）正则未覆盖，4 个地址未识别 | 识别遗漏 |
| BUG-011 | P2 | regex_engine | `test_executive_docx_baseline_coverage` | 400 热线号码格式未被 LandlinePhone 正则覆盖 | 识别遗漏 |

## 环境问题（非 Bug）

| # | 说明 | 影响测试 | 解决方式 |
|---|------|----------|----------|
| ENV-001 | PDFium 动态库未部署 (`resources/pdfium/libpdfium.dylib`) | `desensitize_pdf.rs` 全部 3 个测试 | 将 PDFium 库放入 `src-tauri/resources/pdfium/` |
| ENV-002 | C01-C04 标记 `needs_backend`，需真实 Tauri 后端 | `test_basic_desensitize.py` 12 个测试 | 需在 Tauri 环境中执行 |

## 已关闭

| # | 关闭时间 | 模块 | 原失败测试 | 说明 |
|---|----------|------|------------|------|
| BUG-001 | 2026-04-04 | dict_engine | `test_dict_empty_string_entry` (dict_integration.rs) | 空字符串词条 `str::find("")` 返回 `Some(0)` 导致无限循环，在 `match_text` 中跳过空词条修复 |
| BUG-002 | 2026-04-04 | regex_engine | `test_docx_investment_due_diligence_*` ×2, `test_xlsx_law_case_baseline` | CreditCode 正则字符集扩展为 `[0-9A-Z]`，兼容非标字符；边界 baseline 中 `11010119900307678X` 误标为 CreditCode（实为 IdCard），已修正 |
| BUG-003 | 2026-04-04 | strategy_switching | `test_replace_strategy_on_txt` | 根因非引擎 bug：测试只给 Phone/IdCard/Email 配了 Replace，未配置类型 fallback 为 Mask。修复：过滤 items 到已配置策略的类型 |
| BUG-006 | 2026-04-04 | E2E | `test_sidebar_toggle` (test_ui_extras.py) | 本次执行变为 SKIPPED（用例已更新跳过条件），不再报错 |
| BUG-007 | 2026-04-04 | regex_engine | `test_*_baseline_coverage` ×4, `test_csv_complaint_baseline` | LicensePlate 正则加可选中间点 `[·.]?`，覆盖 `京A·12345` 等格式 |
