# 真正的端到端测试 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让测试走完"文件导入 → 三层引擎（正则+NER+词典）识别 → 结果合并 → 基线断言"的完整链路，任何引擎漏检都会导致测试失败。

**Architecture:** 两层方案并行：(1) Rust 全管道集成测试 — 在 Rust 侧初始化真实 NER 模型，调用三层引擎并合并结果，用 sidecar baseline 做 hard 断言。这是主要的 CI 测试手段，不需要 UI。(2) 保留现有 Pytest+Playwright 测试用于 UI 交互验证，`needs_backend` 测试待后续 WebDriver 方案落地时启用。

**Tech Stack:** Rust (ort/ONNX), cargo test, 现有 baseline.json sidecar 文件

**关键设计决策：** 不做 WebDriver/Selenium 方案（复杂且不稳定），改为在 Rust 测试中复现前端的三层合并逻辑。这样：
- NER 模型真实运行，漏检即 fail
- 合并去重逻辑与前端一致（overlap 检测）
- 无 UI 依赖，CI 友好，3 分钟内跑完
- baseline sidecar 是唯一数据源，47 个 fixture 全覆盖

---

### Task 1: 在 tests/common/mod.rs 中添加三层引擎全管道测试工具

**Files:**
- Modify: `src-tauri/tests/common/mod.rs`

这个 Task 的目标：提供一个 `detect_full_pipeline(content: &FileContent) -> Vec<SensitiveItem>` 函数，在测试中调用真实的三层引擎并合并结果，逻辑与 `useAutoDesensitize.ts:1058-1112` 一致。

- [ ] **Step 1: 添加 NER 引擎初始化帮助函数**

在 `tests/common/mod.rs` 顶部添加 NER 相关 import，然后添加一个惰性初始化的全局 NER 引擎：

```rust
use dimkey_lib::engine::ner_engine::NerEngine;
use dimkey_lib::engine::backends::onnx_backend::OnnxBackend;
use std::sync::Mutex;

static NER_ENGINE: std::sync::OnceLock<Mutex<NerEngine>> = std::sync::OnceLock::new();

/// 获取或初始化全局 NER 引擎（真实 ONNX 模型）
/// 模型路径: src-tauri/resources/ner/
/// 如果模型不存在，返回 degraded 模式（detect 返回空）
fn get_ner_engine() -> &'static Mutex<NerEngine> {
    NER_ENGINE.get_or_init(|| {
        let ner_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("ner");
        let engine = match OnnxBackend::try_load(&ner_dir) {
            Ok(Some(backend)) => {
                let label_map = backend.build_label_map();
                eprintln!("[test] NER 引擎已加载 (ONNX)");
                NerEngine::new(Box::new(backend), label_map)
            }
            Ok(None) => {
                eprintln!("[test] ⚠️ NER 模型文件不存在，降级运行");
                NerEngine::degraded()
            }
            Err(e) => {
                eprintln!("[test] ⚠️ NER 引擎加载失败: {}，降级运行", e);
                NerEngine::degraded()
            }
        };
        Mutex::new(engine)
    })
}
```

- [ ] **Step 2: 添加 detect_full_pipeline 函数**

此函数复现前端 `processFileStandalone` 中的三层合并逻辑（`useAutoDesensitize.ts:1101-1112`）：

```rust
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::engine::dict_engine::DictEngine;
use dimkey_lib::models::language::Language;

/// 全管道检测：正则 + NER + 词典，合并去重
/// 复现 useAutoDesensitize.ts:1058-1112 的完整逻辑
pub fn detect_full_pipeline(content: &FileContent, lang: Language) -> Vec<SensitiveItem> {
    // 1. 正则引擎
    let regex_engine = RegexEngine::for_language(lang);
    let regex_items = regex_engine.detect(content);

    // 2. NER 引擎
    let ner_items = {
        let engine_mutex = get_ner_engine();
        let mut engine = engine_mutex.lock().expect("NER 引擎锁获取失败");
        engine.detect(content).unwrap_or_default()
    };

    // 3. 词典引擎（内置词典）
    let builtin_dict_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("builtin_dict");
    let dict_json = match lang {
        Language::Zh => std::fs::read_to_string(builtin_dict_path.join("zh.json")).unwrap_or_default(),
        Language::En => std::fs::read_to_string(builtin_dict_path.join("en.json")).unwrap_or_default(),
    };

    #[derive(serde::Deserialize)]
    struct BuiltinDictItem {
        text: String,
        sensitive_type: String,
        match_mode: dimkey_lib::models::strategy::MatchMode,
    }

    let dict_entries: Vec<dimkey_lib::models::strategy::DictEntry> =
        serde_json::from_str::<Vec<BuiltinDictItem>>(&dict_json)
            .unwrap_or_default()
            .into_iter()
            .map(|item| dimkey_lib::models::strategy::DictEntry {
                text: item.text,
                sensitive_type: dimkey_lib::commands::desensitize::string_to_sensitive_type(&item.sensitive_type),
                match_mode: item.match_mode,
                replacement: None,
                language: None,
                builtin: true,
            })
            .collect();

    let dict_items = if dict_entries.is_empty() {
        vec![]
    } else {
        DictEngine::new(dict_entries).detect(content)
    };

    // 4. 合并去重（与前端 useAutoDesensitize.ts:1101-1112 一致）
    // 正则优先，词典和 NER 只补充非重叠区域
    let mut merged = regex_items;
    for di in dict_items.into_iter().chain(ner_items.into_iter()) {
        let overlap = merged.iter().any(|ex| {
            ex.sheet_index == di.sheet_index
                && ex.row == di.row
                && ex.col == di.col
                && ex.start < di.end
                && di.start < ex.end
        });
        if !overlap {
            merged.push(di);
        }
    }

    merged
}
```

- [ ] **Step 3: 添加全管道 baseline 断言函数**

```rust
/// 全管道基线断言：用 detect_full_pipeline 检测后，与 sidecar baseline 对照
/// NER 不存在时（降级模式）：soft 项标注 [NER degraded] 但仍然 fail
pub fn assert_full_pipeline_baseline(fixture_abs_path: &str, lang: Language) {
    let path = std::path::Path::new(fixture_abs_path);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let content = match ext {
        "xlsx" | "xls" => dimkey_lib::parser::excel::parse_excel(path)
            .unwrap_or_else(|e| panic!("Excel 导入失败: {}", e)),
        "csv" => dimkey_lib::parser::csv::parse_csv(path)
            .unwrap_or_else(|e| panic!("CSV 导入失败: {}", e)),
        "docx" => dimkey_lib::parser::docx::parse_docx(path)
            .unwrap_or_else(|e| panic!("Docx 导入失败: {}", e)),
        "txt" => dimkey_lib::parser::txt::parse_txt(path)
            .unwrap_or_else(|e| panic!("TXT 导入失败: {}", e)),
        _ => panic!("不支持的格式: {}", ext),
    };

    let items = detect_full_pipeline(&content, lang);

    // 检查 NER 是否降级
    let ner_loaded = get_ner_engine().lock().map(|e| e.is_loaded()).unwrap_or(false);
    if !ner_loaded {
        eprintln!("[test] ⚠️ NER 引擎降级中 — soft 项仍然断言但会标注 [NER degraded]");
    }

    assert_baseline_from_sidecar_filtered(&items, fixture_abs_path, None);
}
```

- [ ] **Step 4: 验证编译通过**

Run: `cd src-tauri && cargo test --no-run 2>&1 | tail -5`
Expected: 编译成功，无错误

- [ ] **Step 5: Commit**

```bash
git add src-tauri/tests/common/mod.rs
git commit -m "test: 添加三层引擎全管道测试工具（正则+NER+词典合并）"
```

---

### Task 2: 创建全管道集成测试文件 — 场景 XLSX

**Files:**
- Create: `src-tauri/tests/full_pipeline_xlsx.rs`

这个 Task 测试所有 xlsx 场景 fixture 的全管道检测。每个 fixture 一个测试函数，调用 `assert_full_pipeline_baseline`。

- [ ] **Step 1: 创建测试文件**

```rust
//! 全管道集成测试 — XLSX 场景
//! 三层引擎（正则+NER+词典）合并检测，与 baseline sidecar 对照

mod common;

use common::assert_full_pipeline_baseline;
use dimkey_lib::models::language::Language;

fn xlsx_fixture(name: &str) -> String {
    common::test_data_path(name)
}

#[test]
fn test_fp_员工花名册() {
    assert_full_pipeline_baseline(&xlsx_fixture("员工花名册.xlsx"), Language::Zh);
}

#[test]
fn test_fp_边界测试用例() {
    assert_full_pipeline_baseline(&xlsx_fixture("边界测试用例.xlsx"), Language::Zh);
}

#[test]
fn test_fp_混合敏感信息场景() {
    assert_full_pipeline_baseline(&xlsx_fixture("混合敏感信息场景.xlsx"), Language::Zh);
}

#[test]
fn test_fp_律所案件登记表() {
    assert_full_pipeline_baseline(&xlsx_fixture("律所案件登记表.xlsx"), Language::Zh);
}

#[test]
fn test_fp_物业业主信息表() {
    assert_full_pipeline_baseline(&xlsx_fixture("物业业主信息表.xlsx"), Language::Zh);
}

#[test]
fn test_fp_学校学生信息登记表() {
    assert_full_pipeline_baseline(&xlsx_fixture("学校学生信息登记表.xlsx"), Language::Zh);
}

#[test]
fn test_fp_医院患者登记表() {
    assert_full_pipeline_baseline(&xlsx_fixture("医院患者登记表.xlsx"), Language::Zh);
}

#[test]
fn test_fp_银行贷款申请表() {
    assert_full_pipeline_baseline(&xlsx_fixture("银行贷款申请表.xlsx"), Language::Zh);
}

#[test]
fn test_fp_mixed_bilingual() {
    assert_full_pipeline_baseline(&xlsx_fixture("mixed_bilingual.xlsx"), Language::Zh);
}

#[test]
fn test_fp_us_compliance_audit() {
    assert_full_pipeline_baseline(&xlsx_fixture("us_compliance_audit.xlsx"), Language::En);
}

#[test]
fn test_fp_跨文件一致性_入职信息() {
    assert_full_pipeline_baseline(&xlsx_fixture("跨文件一致性_入职信息.xlsx"), Language::Zh);
}
```

- [ ] **Step 2: 运行测试**

Run: `cd src-tauri && cargo test --test full_pipeline_xlsx --no-fail-fast -- --nocapture 2>&1`
Expected: 如果 NER 模型存在，NER 类型（PersonName/Title/Address/OrgName）应被识别到。观察输出中是否有 "NER 引擎已加载" 字样。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/full_pipeline_xlsx.rs
git commit -m "test: 添加 XLSX 全管道集成测试（11 个场景）"
```

---

### Task 3: 创建全管道集成测试文件 — 场景 CSV

**Files:**
- Create: `src-tauri/tests/full_pipeline_csv.rs`

- [ ] **Step 1: 创建测试文件**

```rust
//! 全管道集成测试 — CSV 场景

mod common;

use common::assert_full_pipeline_baseline;
use dimkey_lib::models::language::Language;

fn csv_fixture(name: &str) -> String {
    common::test_data_path(name)
}

#[test]
fn test_fp_员工信息表() {
    assert_full_pipeline_baseline(&csv_fixture("员工信息表.csv"), Language::Zh);
}

#[test]
fn test_fp_客户通讯录() {
    assert_full_pipeline_baseline(&csv_fixture("客户通讯录.csv"), Language::Zh);
}

#[test]
fn test_fp_会议纪要记录() {
    assert_full_pipeline_baseline(&csv_fixture("会议纪要记录.csv"), Language::Zh);
}

#[test]
fn test_fp_投诉工单记录() {
    assert_full_pipeline_baseline(&csv_fixture("投诉工单记录.csv"), Language::Zh);
}

#[test]
fn test_fp_english_employee() {
    assert_full_pipeline_baseline(&csv_fixture("english_employee.csv"), Language::En);
}

#[test]
fn test_fp_uk_customer_records() {
    assert_full_pipeline_baseline(&csv_fixture("uk_customer_records.csv"), Language::En);
}
```

- [ ] **Step 2: 运行测试**

Run: `cd src-tauri && cargo test --test full_pipeline_csv --no-fail-fast -- --nocapture 2>&1`

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/full_pipeline_csv.rs
git commit -m "test: 添加 CSV 全管道集成测试（6 个场景）"
```

---

### Task 4: 创建全管道集成测试文件 — 场景 DOCX

**Files:**
- Create: `src-tauri/tests/full_pipeline_docx.rs`

- [ ] **Step 1: 创建测试文件**

```rust
//! 全管道集成测试 — DOCX 场景

mod common;

use common::assert_full_pipeline_baseline;
use dimkey_lib::models::language::Language;

fn docx_fixture(name: &str) -> String {
    common::test_data_path(name)
}

#[test]
fn test_fp_客户调研报告() {
    assert_full_pipeline_baseline(&docx_fixture("客户调研报告.docx"), Language::Zh);
}

#[test]
fn test_fp_人事变动通知() {
    assert_full_pipeline_baseline(&docx_fixture("人事变动通知.docx"), Language::Zh);
}

#[test]
fn test_fp_房屋租赁合同() {
    assert_full_pipeline_baseline(&docx_fixture("房屋租赁合同.docx"), Language::Zh);
}

#[test]
fn test_fp_保险理赔案件记录() {
    assert_full_pipeline_baseline(&docx_fixture("保险理赔案件记录.docx"), Language::Zh);
}

#[test]
fn test_fp_律师函_延期交房() {
    assert_full_pipeline_baseline(&docx_fixture("律师函-延期交房.docx"), Language::Zh);
}

#[test]
fn test_fp_律所案件分析备忘录_劳动争议() {
    assert_full_pipeline_baseline(&docx_fixture("律所案件分析备忘录-劳动争议.docx"), Language::Zh);
}

#[test]
fn test_fp_门诊病历摘要() {
    assert_full_pipeline_baseline(&docx_fixture("门诊病历摘要.docx"), Language::Zh);
}

#[test]
fn test_fp_民事判决书_商品房买卖纠纷() {
    assert_full_pipeline_baseline(&docx_fixture("民事判决书-商品房买卖纠纷.docx"), Language::Zh);
}

#[test]
fn test_fp_投资尽调报告() {
    assert_full_pipeline_baseline(&docx_fixture("投资尽调报告.docx"), Language::Zh);
}

#[test]
fn test_fp_集团高管通讯录() {
    assert_full_pipeline_baseline(&docx_fixture("集团高管通讯录.docx"), Language::Zh);
}

#[test]
fn test_fp_international_vendor_contacts() {
    assert_full_pipeline_baseline(&docx_fixture("international_vendor_contacts.docx"), Language::En);
}
```

- [ ] **Step 2: 运行测试**

Run: `cd src-tauri && cargo test --test full_pipeline_docx --no-fail-fast -- --nocapture 2>&1`

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/full_pipeline_docx.rs
git commit -m "test: 添加 DOCX 全管道集成测试（11 个场景）"
```

---

### Task 5: 创建全管道集成测试文件 — 场景 TXT

**Files:**
- Create: `src-tauri/tests/full_pipeline_txt.rs`

- [ ] **Step 1: 创建测试文件**

```rust
//! 全管道集成测试 — TXT 场景

mod common;

use common::assert_full_pipeline_baseline;
use dimkey_lib::models::language::Language;

fn txt_fixture(name: &str) -> String {
    common::test_data_path(name)
}

#[test]
fn test_fp_会议纪要() {
    assert_full_pipeline_baseline(&txt_fixture("会议纪要.txt"), Language::Zh);
}

#[test]
fn test_fp_通知公告() {
    assert_full_pipeline_baseline(&txt_fixture("通知公告.txt"), Language::Zh);
}

#[test]
fn test_fp_IT运维事件报告() {
    assert_full_pipeline_baseline(&txt_fixture("IT运维事件报告.txt"), Language::Zh);
}
```

- [ ] **Step 2: 运行测试**

Run: `cd src-tauri && cargo test --test full_pipeline_txt --no-fail-fast -- --nocapture 2>&1`

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/full_pipeline_txt.rs
git commit -m "test: 添加 TXT 全管道集成测试（3 个场景）"
```

---

### Task 6: 创建全管道集成测试文件 — 基础 fixture（sample.* + boundary）

**Files:**
- Create: `src-tauri/tests/full_pipeline_basic.rs`

这个 Task 覆盖 `e2e/fixtures/` 根目录下的基础 fixture（sample.xlsx/csv/docx/txt）和 boundary 场景。

- [ ] **Step 1: 创建测试文件**

```rust
//! 全管道集成测试 — 基础 fixture + 边界场景

mod common;

use common::{assert_full_pipeline_baseline, fixture_path};
use dimkey_lib::models::language::Language;

// --- 基础 sample 文件 ---

#[test]
fn test_fp_sample_xlsx() {
    assert_full_pipeline_baseline(&fixture_path("sample.xlsx"), Language::Zh);
}

#[test]
fn test_fp_sample_csv() {
    assert_full_pipeline_baseline(&fixture_path("sample.csv"), Language::Zh);
}

#[test]
fn test_fp_sample_docx() {
    assert_full_pipeline_baseline(&fixture_path("sample.docx"), Language::Zh);
}

#[test]
fn test_fp_sample_txt() {
    assert_full_pipeline_baseline(&fixture_path("sample.txt"), Language::Zh);
}

// --- 边界场景 ---

#[test]
fn test_fp_boundary_utf8bom() {
    assert_full_pipeline_baseline(&fixture_path("boundary/utf8bom_sample.csv"), Language::Zh);
}

#[test]
fn test_fp_boundary_fullwidth() {
    assert_full_pipeline_baseline(&fixture_path("boundary/fullwidth_digits.csv"), Language::Zh);
}

#[test]
fn test_fp_boundary_large_cell() {
    assert_full_pipeline_baseline(&fixture_path("boundary/large_cell.xlsx"), Language::Zh);
}

// --- batch 文件 ---

#[test]
fn test_fp_batch_01_xlsx() {
    assert_full_pipeline_baseline(&fixture_path("batch/batch_01.xlsx"), Language::Zh);
}

#[test]
fn test_fp_batch_02_csv() {
    assert_full_pipeline_baseline(&fixture_path("batch/batch_02.csv"), Language::Zh);
}

#[test]
fn test_fp_batch_03_docx() {
    assert_full_pipeline_baseline(&fixture_path("batch/batch_03.docx"), Language::Zh);
}
```

- [ ] **Step 2: 运行测试**

Run: `cd src-tauri && cargo test --test full_pipeline_basic --no-fail-fast -- --nocapture 2>&1`

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/full_pipeline_basic.rs
git commit -m "test: 添加基础+边界全管道集成测试（10 个场景）"
```

---

### Task 7: 跑一次全量全管道测试，记录 NER 识别结果

**Files:**
- No new files

这个 Task 是验证性的 — 跑全部 `full_pipeline_*` 测试，收集结果，确认 NER 确实在工作。

- [ ] **Step 1: 编译全部新测试**

Run: `cd src-tauri && cargo test --no-run 2>&1 | grep "full_pipeline"`
Expected: 5 个新测试二进制编译成功

- [ ] **Step 2: 运行全部全管道测试**

Run: `cd src-tauri && cargo test full_pipeline --no-fail-fast -- --nocapture 2>&1`

观察输出中：
- `[test] NER 引擎已加载 (ONNX)` — 确认模型加载成功
- `NER 类未命中` — 如果 NER 模型正常但仍有漏检，记录到 bug-list
- 所有 hard 项（正则类）应全部通过

- [ ] **Step 3: 根据测试结果更新 bug-list.md**

对照测试输出：
- NER 模型正常加载但仍漏检的项 → 新增 BUG（NER 模型能力不足）
- NER 模型降级（模型文件问题）→ 标注为 ENV 问题
- 之前的 57 项 soft 漏检应显著减少（NER 引擎工作后能识别大部分 PersonName/Title）

- [ ] **Step 4: 回写 Excel 结果**

使用现有 `excel_manager.py` 的 `update_result()` 回写全管道测试结果。

---

### Task 8: 代码审查

**Files:**
- All files modified/created in Task 1-6

- [ ] **Step 1: 审查 common/mod.rs 中的 detect_full_pipeline**

检查点：
- 合并逻辑是否与 `useAutoDesensitize.ts:1101-1112` 完全一致（overlap 判断条件）
- NER 引擎锁获取是否安全（不会死锁）
- 内置词典加载路径是否正确
- Language 参数是否对每个 fixture 设置正确（中文 fixture 用 Zh，英文用 En）

- [ ] **Step 2: 审查各 full_pipeline_*.rs 文件**

检查点：
- 每个 fixture 文件路径是否正确存在
- Language 是否匹配（英文 fixture 如 `english_employee.csv` 应用 `Language::En`）
- 是否有遗漏的 fixture（对照 47 个 baseline sidecar 文件）

- [ ] **Step 3: 审查 baseline 断言逻辑**

确认 `assert_baseline_from_sidecar_filtered` 中 soft 和 hard 都会 panic（Task 1 已改），不存在任何"静默跳过"的路径。

---

## 不在此计划范围内

- WebDriver/Selenium 真实 UI E2E（需要更多基础设施，独立计划）
- PDF 格式（依赖 PDFium，ENV-001 问题未解决）
- GBK 编码（BUG-014，CSV parser 不支持）
- 现有 Rust 单引擎测试的修改（保留，互不影响）
