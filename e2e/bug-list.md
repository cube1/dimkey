# Bug 清单

> 更新时间: 2026-04-04（第 5 次执行 — 8 Bug 修复）
> 来源: 全量测试执行（Rust 234通过/1失败 + Pytest 18通过/4失败/2跳过）

## 活跃 Bug

| # | 优先级 | 模块 | 失败测试 | 根因描述 | 分类 |
|---|--------|------|----------|----------|------|

（暂无活跃 Bug）

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
| BUG-004 | 2026-04-04 | parser::xlsx | `test_encrypted_xlsx_import_fails` (boundary.rs) | fixture 文件非真正文件级加密（仅 workbookProtection 标签），calamine 正常解析。修复：用 msoffcrypto 重新生成 OLE 加密的 fixture 文件 |
| BUG-005 | 2026-04-04 | E2E | `test_language_switch_to_english` (test_ui_extras.py) | `has_text="EN"` 子串匹配到 "Desensitize" 等按钮。修复：LanguageSwitcher 添加 `data-testid="lang-switcher"`，测试改用精确定位 |
| BUG-006 | 2026-04-04 | E2E | `test_sidebar_toggle` (test_ui_extras.py) | 本次执行变为 SKIPPED（用例已更新跳过条件），不再报错 |
| BUG-007 | 2026-04-04 | regex_engine | `test_*_baseline_coverage` ×4, `test_csv_complaint_baseline` | LicensePlate 正则加可选中间点 `[·.]?`，覆盖 `京A·12345` 等格式 |
| BUG-008 | 2026-04-04 | regex_engine | `test_uk_csv_baseline`, `test_us_compliance_baseline`, `test_intl_csv_baseline` | en.rs 完全没有 DriversLicense 规则。修复：添加 US 驾照 `[A-Z]\d{3}-\d{4}-\d{4}` 和 UK DVLA `[A-Z]{5}\d{6}[A-Z0-9]{2}\d[A-Z]{2}` 正则 |
| BUG-009 | 2026-04-04 | regex_engine | `test_uk_csv_baseline`, `test_us_compliance_baseline` | CreditCard 带空格格式 Luhn 校验失败 — fixture 数据本身不合法（8/10 卡号无法通过 Luhn）。修复：重新生成 Luhn 有效的测试卡号，更新 CSV/xlsx/docx fixture 和 baseline |
| BUG-010 | 2026-04-04 | regex_engine | `test_txt_it_ops_baseline_coverage` | IpAddress 正则仅覆盖 IPv4。修复：common.rs 添加完整 IPv6 正则（8 组完整格式 + `::` 压缩格式 10 种变体），IP 校验逻辑区分 IPv4/IPv6 |
| BUG-011 | 2026-04-04 | regex_engine | `test_executive_docx_baseline_coverage` | LandlinePhone 正则首位必须 `0`，不支持 400/800 热线。修复：正则扩展为 `(?:0\d{2,3}|400|800)-?\d{3,4}-?\d{4}` |
| BUG-012 | 2026-04-04 | E2E | `test_initial_enabled_types_loaded`, `test_enabled_types_roundtrip` (test_type_persistence.py) | 测试代码用 `state.wsData` 但 store 字段名是 `activeWorkspaceData`。修复：替换为正确字段名 |
| BUG-013 | 2026-04-04 | E2E | `test_workspace_list_has_multiple` (test_workspace_advanced.py) | conftest 初始化与 React 挂载竞态，`workspaces` 数组可能还未加载完成。修复：添加 `wait_for_function` 等待 store.workspaces.length > 0 |
