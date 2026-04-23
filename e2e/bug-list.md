# Bug 清单

> 更新时间: 2026-04-08（第 13 次执行）
> 来源: 全量测试（用户优化测试代码后）。Rust 236 通过 / 57 失败（10 个模块），Pytest 0 通过 / 23 ERROR（环境问题）。相比上次：desensitize_uk、desensitize_us_compliance 模块已修复通过；type_filtering 出现新失败。

## 活跃 Bug

| # | 优先级 | 模块 | 失败测试 | 根因描述 | 分类 |
|---|--------|------|----------|----------|------|
| BUG-014 | P1 | parser::csv | `test_gbk_csv_*` ×4 (encoding_boundary.rs) | GBK 编码 CSV 解析失败，csv crate 仅支持 UTF-8，未做编码自动检测/转换 | 功能缺失 |
| BUG-015 | P2 | regex_engine | `test_fp_uk_customer_records` (full_pipeline_csv.rs) | UkPhone `+44 161 496 0000`（含空格格式）未被正则识别，正则要求连续数字或 `-` 分隔 | 识别遗漏 |
| BUG-016 | P2 | regex_engine | `test_fp_us_compliance_audit` (full_pipeline_xlsx.rs) | US DriversLicense `S012-3456-7890` 未识别（首字母 S 不在正则范围），识别 9/10 | 识别遗漏 |
| BUG-017 | P1 | regex_engine | `test_fp_international_vendor_contacts` (full_pipeline_docx.rs) | Passport 号码完全未识别（0/3），国际驾照格式缺失。**部分修复**：已扩展正则支持 `GA 1234567`（字母+空格+数字）与 `12AB34567`（数字字母混合）两种格式；纯 9 位数字格式 `533410987` 仍不识别（误报风险太高需要上下文） | 识别遗漏（部分修复） |
| BUG-020 | P2 | parser::xlsx | `test_encrypted_xlsx_wrong_password` (boundary.rs) | 错误密码解密后文件解析返回 "Cannot detect file format" 而非密码错误提示 | 逻辑错误 |
| BUG-021 | **P0** | ner_engine | 46 个 full_pipeline 测试 | **英文 distilbert-ner 模型对中文人名全面漏检**。2-4 字中文人名（张三、王建国、刘晓燕等）识别率接近 0%。这不是 bug 而是模型能力边界——英文模型不认中文 | NER 模型能力 |
| BUG-022 | **P0** | ner_engine | `test_fp_IT运维事件报告`, `test_fp_集团高管通讯录`, `test_fp_会议纪要` 等 | NER 模型完全不支持 Title（职位）类型识别。当前模型只有 PER/LOC/ORG 标签，没有 Title label。涉及 50+ 个 Title 漏检 | NER 能力缺失 |
| BUG-023 | P1 | ner_engine | 全部中文场景 | NER 模型对中文地址全面漏检（"北京市朝阳区建国路88号" 等）。英文模型不支持中文地址格式 | NER 模型能力 |
| BUG-024 | P1 | ner_engine | 全部中文场景 | NER 模型对中文组织名全面漏检（"北京星辰科技有限公司"、"浙江大学医学院附属第一医院" 等）。英文模型不支持中文组织名格式 | NER 模型能力 |
| BUG-026 | P2 | regex_engine | `test_fp_boundary_fullwidth` (full_pipeline_basic.rs) | 全角手机号（１３９１２３４５６７８）未识别，正则引擎只支持半角数字，需全角→半角归一化 | 识别遗漏 |
| BUG-027 | P3 | test_infra | `test_fp_门诊病历摘要`, `test_fp_会议纪要` 等 | `common/mod.rs::parse_sensitive_type` 不识别 `MedicalInsurance`、`IP` 等类型字符串，导致基线条目被跳过 | 测试基础设施 |
| BUG-028 | P1 | regex_engine | `test_fp_mixed_bilingual` (full_pipeline_xlsx.rs) | 中英混合 xlsx 中 SSN 和 UkPostcode 未识别，可能是语言检测将文件判为 Zh 只加载中文正则 | 识别遗漏 |
| BUG-029 | P1 | parser::docx | `test_fp_attorney_engagement_letter`, `test_fp_litigation_discovery_memo` (full_pipeline_docx.rs) | docx 中 IBAN 未识别（GB29 NWBK... 带空格格式）。**根因调整**：孤立正则测试证明 `GB29 NWBK 6016 1331 9268 19` 等带空格 IBAN 当前正则能正确匹配；docx 场景失败应非正则 bug，疑为 docx 解析器产生了非标准空白（如 U+00A0 不间断空格）或 NER 降级掩盖。待 NER 模型可用后复现定位 | 待复现 |
| BUG-030 | P1 | ner_engine | `test_fp_english_employee`, `test_fp_english_legal_edge_cases` 等英文场景 | **英文模型对英文 PersonName/OrgName/Address 也部分漏检**。如 'Pacific Coast Medical Center'、'Mitchell, Chen & Park LLP'、'1420 Market Street...' 未识别。可能是 distilbert-ner 模型精度不足或 tokenizer 对长实体切分问题 | NER 能力不足 |
| BUG-031 | P1 | full_pipeline | `test_all_types_enabled_detects_all` (type_filtering.rs) | **新增**：全类型启用检测用例失败，24 个硬断言项未命中（全部为 NER 类型：PersonName/OrgName/Address），与 BUG-021/023/024 同根因 | NER 模型能力 |
| BUG-034  | P2 | E2E | `test_batch_auto_retry_failed` (test_batch_auto.py:120) | **新增**：原测试（fa07796 引入）的 mock 让 `detect_by_regex/ner/dict` 全返空数组，`runDesensitizePipeline` 在 `mergedItems.length===0` 处短路返回 `record=null`，worker 走 0 敏感项 silent confirm 分支，`apply_desensitize` 永远不被调用 → `__E2E_RETRY_COUNT__` 永不递增 → 期望的 2 个 row-failed 永远不会出现。修复：mock 让 detect_by_* 返 1 项敏感，使 pipeline 走到 apply_desensitize 才能触发 throw | 测试逻辑错误 |

## 环境问题（非 Bug）

| # | 说明 | 影响测试 | 解决方式 |
|---|------|----------|----------|
| ENV-001 | PDFium 动态库未部署 (`resources/pdfium/libpdfium.dylib`) | `desensitize_pdf.rs` 全部 3 个测试 | 将 PDFium 库放入 `src-tauri/resources/pdfium/` |
| ENV-002 | C01-C04 标记 `needs_backend`，需真实 Tauri 后端 | `test_basic_desensitize.py` 12 个测试 | 需在 Tauri 环境中执行 |
| ENV-003 | Playwright UI 测试全部超时（23 个 ERROR） | 全部 Pytest 测试 | Vite dev server 可达但 IPC mock 未正确注入，Playwright 等待 UI 元素超时 |

## 已关闭

| # | 关闭时间 | 模块 | 原失败测试 | 说明 |
|---|----------|------|------------|------|
| BUG-001 | 2026-04-04 | dict_engine | `test_dict_empty_string_entry` | 空字符串词条导致无限循环，已修复 |
| BUG-002 | 2026-04-04 | regex_engine | `test_docx_investment_due_diligence_*` ×2 | CreditCode 正则字符集扩展修复 |
| BUG-003 | 2026-04-04 | strategy_switching | `test_replace_strategy_on_txt` | 未配置类型 fallback 问题修复 |
| BUG-004 | 2026-04-04 | parser::xlsx | `test_encrypted_xlsx_import_fails` | fixture 文件非真正加密，已重新生成 |
| BUG-005 | 2026-04-04 | E2E | `test_language_switch_to_english` | 子串匹配误命中修复 |
| BUG-006 | 2026-04-04 | E2E | `test_sidebar_toggle` | 用例已更新跳过条件 |
| BUG-007 | 2026-04-04 | regex_engine | `test_*_baseline_coverage` ×4 | 车牌号正则加可选中间点 |
| BUG-008 | 2026-04-04 | regex_engine | `test_uk_csv_baseline` 等 | 添加 US/UK DriversLicense 正则 |
| BUG-009 | 2026-04-04 | regex_engine | `test_uk_csv_baseline` 等 | Luhn 校验数据修复 |
| BUG-010 | 2026-04-04 | regex_engine | `test_txt_it_ops_baseline_coverage` | 添加 IPv6 正则 |
| BUG-011 | 2026-04-04 | regex_engine | `test_executive_docx_baseline_coverage` | 400/800 热线正则扩展 |
| BUG-012 | 2026-04-04 | E2E | `test_initial_enabled_types_loaded` 等 | store 字段名修复 |
| BUG-013 | 2026-04-04 | E2E | `test_workspace_list_has_multiple` | 竞态修复 |
| BUG-018 | 2026-04-08 | E2E | `test_enabled_types_roundtrip` | 降级为 ENV-003 环境问题，非代码 Bug |
| BUG-019 | 2026-04-08 | E2E | `test_workspace_list_has_multiple` | 降级为 ENV-003 环境问题，非代码 Bug |
| BUG-025 | 2026-04-08 | baseline_data | `test_fp_sample_csv/xlsx` | 已通过全量 soft→hard 统一解决，不再区分 soft/hard |
| BUG-032 | 2026-04-17 | desensitizer::generalize | `test_en_generalize_title_director_not_executive`, `test_en_generalize_title_no_substring_pollution` | `generalize_en_title` 改用 token 精确匹配（按非字母数字分割）+ 短语匹配优先级高于单词，消除 "cto" 子串污染；新增 2 个回归测试通过 |
| BUG-033 | 2026-04-23 | E2E | `test_batch_auto.py` 11 个测试（全部解除阻塞） | 三处测试基础设施 bug 组合修复：(1) `_wait_workspace_ready` 顺序错误——首个 wait 改为 `workspace-list`；(2) `conftest.py` 未设 i18n 语言——追加 `localStorage.setItem('dimkey-lang', 'zh')` 避免回退到 navigator.language=en；(3) 新增 B10/B13 两处 `get_by_text` 命中多元素，换成 `viewonly-badge` testid 和 `batch-result-report` 容器 contains_text。修复后 8 个新用例 B06-B13 全部通过，10/11 绿（剩 retry_failed 是 BUG-034） |
