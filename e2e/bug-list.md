# Bug 清单

> 更新时间: 2026-04-07（第 10 次执行）
> 来源: 英文模型验证 — 跑全量非中文测试（11 个 Rust 测试文件，111 个用例，77 通过/34 失败）。新增 desensitize_legal_english.rs（12 用例，9 通过/3 失败）、restore_roundtrip_english.rs（6 用例，全通过）。已有英文 Bug 基本不变，新增 IBAN docx 场景漏检。

## 重要说明：全管道测试（2026-04-05 新增）

之前的 Rust 集成测试只调用 `RegexEngine::detect()` 单个引擎，从未触发 NER 模型，导致 baseline 中所有 NER 类（PersonName/OrgName/Address/Title）的 soft 断言即便全部漏检也不报错。本次新增的 `full_pipeline_*.rs` 5 个测试文件复现了前端 `useAutoDesensitize.ts:1058-1112` 的三层引擎合并逻辑，NER 引擎真实加载 ONNX 模型并参与检测，soft/hard 断言一视同仁。

全管道测试结果（42 用例）：
- full_pipeline_basic: 2 通过 / 8 失败（sample.* 和 batch 基线数据问题占多数）
- full_pipeline_txt: 0 通过 / 3 失败（NER 短名字 + Title 不支持）
- full_pipeline_csv: 5 通过 / 2 失败（UkPhone 空格格式 + NER 短名字）
- full_pipeline_xlsx: 8 通过 / 3 失败（NER 短名字 + Title 英文不支持 + 语言切换）
- full_pipeline_docx: 9 通过 / 2 失败（国际护照/IBAN + NER Title）

**核心发现**：NER 引擎加载成功，对"长"中文人名（3-4 字且不在常见姓氏范围的）和复杂地址/组织名识别能力不足；Title 类型完全不支持（需要其他方案）。这是之前 soft 模式掩盖的真实问题，现在暴露出来由产品决定如何应对。

## 活跃 Bug

| # | 优先级 | 模块 | 失败测试 | 根因描述 | 分类 |
|---|--------|------|----------|----------|------|
| BUG-014 | P1 | parser::csv | `test_gbk_csv_*` ×4 (encoding_boundary.rs) | GBK 编码 CSV 解析失败，csv crate 仅支持 UTF-8，未做编码自动检测/转换 | 功能缺失 |
| BUG-015 | P2 | regex_engine | `test_uk_csv_baseline_coverage` (desensitize_uk.rs) | UkPhone `+44 161 496 0000`（含空格格式）未被正则识别，正则要求连续数字或 `-` 分隔 | 识别遗漏 |
| BUG-016 | P2 | regex_engine | `test_us_compliance_baseline_coverage`, `test_us_compliance_detect_drivers_license` (desensitize_us_compliance.rs) | US DriversLicense `S012-3456-7890` 未识别（首字母 S 不在正则范围？），识别 9/10 | 识别遗漏 |
| BUG-017 | P1 | regex_engine | `test_intl_docx_detect_passport`, `test_intl_docx_detect_drivers_license`, `test_intl_docx_baseline` (desensitize_international.rs) | Passport 号码完全未识别（0/3），国际驾照格式缺失，baseline 覆盖不足 | 功能缺失 |
| BUG-018 | P2 | E2E | `test_enabled_types_roundtrip` (test_type_persistence.py) | IPC mock override get_workspace 后 selectWorkspace 未正确触发 store 更新，wsData.workspace.enabled_types 仍为 null | 测试代码 |
| BUG-019 | P2 | E2E | `test_workspace_list_has_multiple` (test_workspace_advanced.py) | IPC mock 返回 2 个工作区但列表只显示 1 个，竞态或 store 渲染时序问题 | 测试代码 |
| BUG-020 | P2 | parser::xlsx | `test_encrypted_xlsx_wrong_password` (boundary.rs) | 错误密码解密后文件解析返回 "Cannot detect file format" 而非密码错误提示，错误分类不够精确 | 逻辑错误 |
| BUG-021 | **P0** | ner_engine | `test_fp_sample_*` ×4, `test_fp_batch_*` ×3, `test_fp_会议纪要`, `test_fp_通知公告`, `test_fp_跨文件一致性_*` ×2 | NER 模型对 2-3 字中文人名漏检（张三、周杰、马超、吴凡、郑华、冯磊、刘伟、陈静、赵明远、孙浩然、张美玲、陈志强、吴志远 等）。模型加载正常，但短人名识别率不足，可能是 `shibing624/bert4ner-base-chinese` 训练数据偏向长名 | NER 能力不足 |
| BUG-022 | **P0** | ner_engine | `test_fp_IT运维事件报告`, `test_fp_us_compliance_audit`, `test_fp_集团高管通讯录` | NER 模型完全不支持 Title（职位）类型识别。当前模型只有 PER/LOC/ORG 三个标签，没有 Title label。涉及 30 个 Title 漏检（15 中文 + 10 英文 + 5 其他）。**解决方向：(1) 正则规则枚举常见职位后缀（总监/经理/CFO/VP 等），(2) 自定义词典兜底，(3) 换支持 TITLE label 的模型** | NER 能力缺失 |
| BUG-023 | P1 | ner_engine | `test_fp_sample_*` ×4, `test_fp_batch_02_csv`, `test_fp_batch_03_docx` | NER 模型对完整的中文地址漏检（"北京市朝阳区建国路88号"、"上海市浦东新区陆家嘴环路1000号"、"南京市玄武区中山路18号" 等）。可能是地址含数字段导致 tokenizer 切分异常，或模型未覆盖门牌号格式 | NER 能力不足 |
| BUG-024 | P1 | ner_engine | `test_fp_sample_docx`, `test_fp_sample_txt` | NER 模型对长组织名"阿里巴巴集团控股有限公司"漏检。可能是"集团控股有限公司"后缀组合未在训练集出现 | NER 能力不足 |
| BUG-025 | P2 | baseline_data | `test_fp_sample_csv`, `test_fp_sample_xlsx` | sample.csv/xlsx 基线 sidecar 中 PersonName 和 Address 类型的第 2-N 条被错误标记为 `assert: "hard"`（仅第 1 条为 `soft`），自动迁移脚本 bug。PersonName/Address 属于 NER 类型，不应有 hard 断言 | 基线数据错误 |
| BUG-026 | P2 | baseline_data | `test_fp_boundary_fullwidth` | `boundary/fullwidth_digits.csv` 基线中全角手机号被标为 Phone（正则类），但正则引擎只支持半角数字。应改为 soft 或先做全角→半角归一化再匹配 | 基线数据错误 |
| BUG-027 | P3 | test_infra | `test_fp_投诉工单记录`, `test_fp_医院患者登记表`, `test_fp_律所案件分析备忘录_劳动争议`, `test_fp_门诊病历摘要`, `test_fp_会议纪要` | `common/mod.rs::parse_sensitive_type` 不识别 `MedicalInsurance`、`IP` 等类型字符串，导致基线条目被跳过（"跳过未知类型" warning）。不影响测试通过但降低基线覆盖率 | 测试基础设施 |
| BUG-028 | P1 | regex_engine | `test_fp_mixed_bilingual` | 中英混合 xlsx 中 SSN（`523-45-6789` 等 4 个）和 UkPostcode（`W1U 3BW`, `M1 5QA`, `SW1A 2AA`）未识别。可能是语言检测将文件判为 Zh，只加载中文正则引擎未加载英文规则 | 识别遗漏 |
| BUG-029 | P1 | regex_engine | `test_docx_attorney_engagement_letter_*` ×2, `test_docx_litigation_discovery_memo_baseline` (desensitize_legal_english.rs) | docx 中 IBAN 未识别：`GB29 NWBK 6016 1331 9268 19`、`GB82 WEST 1234 5698 7654 32`。可能是 docx 解析后 IBAN 跨段落/空格被吞，或正则未覆盖 GB 开头的 IBAN 带空格格式 | 识别遗漏 |

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
