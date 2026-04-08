# 端到端测试整合 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 消除"为了测试而测试"的冗余测试，让所有核心管道用例走 full_pipeline 三层引擎，baseline 断言全部 hard，测试结果如实反映识别能力。

**Architecture:** full_pipeline_*.rs 作为核心管道的唯一测试入口（Regex + NER + Dict 三层合并）。删除只跑单引擎的冗余场景测试文件。保留有独立价值的功能测试（策略/字典/类型过滤/还原/边界等）不动。

**Tech Stack:** Rust tests, Python excel_manager, baseline.json sidecar files

---

## 变更概览

### 要删除的冗余测试文件（9 个，约 66 个测试函数）

这些文件用同样的 fixture、只跑正则引擎、做更弱的断言，full_pipeline 已完全覆盖：

| 文件 | 测试数 | 替代者 |
|------|--------|--------|
| desensitize_scenario_csv.rs | 4 | full_pipeline_csv.rs |
| desensitize_scenario_docx.rs | 18 | full_pipeline_docx.rs |
| desensitize_scenario_xlsx.rs | 14 | full_pipeline_xlsx.rs |
| desensitize_executive.rs | 5 | full_pipeline_docx.rs (集团高管通讯录) |
| desensitize_uk.rs | 7 | full_pipeline_csv.rs (uk_customer_records) |
| desensitize_us_compliance.rs | 7 | full_pipeline_xlsx.rs (us_compliance_audit) |
| desensitize_english.rs | 5 | full_pipeline_csv/xlsx.rs |
| desensitize_international.rs | 9 | full_pipeline_docx.rs |
| desensitize_legal_english.rs | 9 | full_pipeline_basic/csv/docx/xlsx.rs |

### 要精简的灰色地带文件（2 个）

| 文件 | 保留 | 删除 |
|------|------|------|
| desensitize_csv.rs | test_csv_import_structure, test_csv_mask_phone, test_csv_mask_idcard, test_csv_replace_phone, test_csv_replace_email (5 个，测格式/策略) | test_csv_regex_detect_counts, test_csv_regex_detect_exact_first_row, test_csv_desensitize_summary (3 个，测单引擎检测) |
| desensitize_txt.rs | test_txt_meeting_import_structure (1 个，测导入结构) | 其余 14 个 detect_count + baseline 测试（full_pipeline_txt 已覆盖且更强） |

### 不动的文件

- full_pipeline_*.rs（5 个）— 这是目标，不改
- dict_integration.rs, dict_special.rs — 字典引擎逻辑
- strategy_switching.rs — 策略逻辑
- type_filtering.rs — 类型过滤逻辑
- restore_roundtrip.rs, restore_roundtrip_english.rs — 还原往返
- column_inference.rs, column_desensitize.rs — 列级逻辑
- consistency.rs, alias_group.rs — 一致性逻辑
- generalize_integration.rs — 泛化逻辑
- boundary.rs, encoding_boundary.rs — 编码边界
- desensitize_pdf.rs — PDF（full_pipeline 不覆盖 PDF）
- desensitize_excel.rs, desensitize_word.rs — 格式特有结构/策略测试
- batch_processing.rs — 批量处理
- audit_dump.rs — 审计输出
- common/mod.rs — 测试基础设施

### baseline.json 变更

所有 53 个 baseline.json 文件中 `"assert": "soft"` → `"assert": "hard"`。

### Excel 用例 test_file 重映射

| 用例 ID | 当前 test_file | 新 test_file |
|---------|---------------|-------------|
| C10 | desensitize_excel.rs | desensitize_excel.rs（不变，测结构/策略） |
| C11 | desensitize_scenario_xlsx.rs | full_pipeline_xlsx.rs |
| C12 | desensitize_scenario_xlsx.rs | full_pipeline_xlsx.rs |
| C13 | desensitize_scenario_xlsx.rs | full_pipeline_xlsx.rs |
| C14 | desensitize_scenario_xlsx.rs | full_pipeline_xlsx.rs |
| C15 | desensitize_scenario_xlsx.rs | full_pipeline_xlsx.rs |
| C16 | desensitize_scenario_xlsx.rs | full_pipeline_xlsx.rs |
| C17 | desensitize_scenario_xlsx.rs | full_pipeline_xlsx.rs |
| C18 | desensitize_scenario_csv.rs | full_pipeline_csv.rs |
| C19 | desensitize_scenario_csv.rs | full_pipeline_csv.rs |
| C20 | desensitize_scenario_csv.rs | full_pipeline_csv.rs |
| C21 | desensitize_scenario_csv.rs | full_pipeline_csv.rs |
| C22 | desensitize_scenario_docx.rs | full_pipeline_docx.rs |
| C23 | desensitize_scenario_docx.rs | full_pipeline_docx.rs |
| C24 | desensitize_scenario_docx.rs | full_pipeline_docx.rs |
| C25 | desensitize_scenario_docx.rs | full_pipeline_docx.rs |
| C26 | desensitize_scenario_docx.rs | full_pipeline_docx.rs |
| C27 | desensitize_scenario_docx.rs | full_pipeline_docx.rs |
| C28 | desensitize_scenario_docx.rs | full_pipeline_docx.rs |
| C29 | desensitize_scenario_docx.rs | full_pipeline_docx.rs |
| C30 | desensitize_scenario_docx.rs | full_pipeline_docx.rs |
| C31 | desensitize_english.rs | full_pipeline_csv.rs |
| C32 | desensitize_english.rs | full_pipeline_xlsx.rs |
| C36 | desensitize_txt.rs | full_pipeline_txt.rs |
| C37 | desensitize_txt.rs | full_pipeline_txt.rs |
| C39 | desensitize_uk.rs | full_pipeline_csv.rs |
| C40 | desensitize_us_compliance.rs | full_pipeline_xlsx.rs |
| C41 | desensitize_international.rs | full_pipeline_docx.rs |
| C42 | desensitize_txt.rs | full_pipeline_txt.rs |
| C43 | desensitize_executive.rs | full_pipeline_docx.rs |
| C44 | desensitize_legal_english.rs | full_pipeline_xlsx.rs |
| C45 | desensitize_legal_english.rs | full_pipeline_csv.rs |
| C46 | desensitize_legal_english.rs | full_pipeline_docx.rs |
| C47 | desensitize_legal_english.rs | full_pipeline_docx.rs |
| C48 | desensitize_legal_english.rs | full_pipeline_csv.rs |
| C49 | desensitize_legal_english.rs | full_pipeline_basic.rs |

---

## Task 1: baseline.json soft → hard

**Files:**
- Modify: 所有 `e2e/fixtures/**/*.baseline.json` 中含 `"assert": "soft"` 的文件

- [ ] **Step 1: 批量替换 soft → hard**

```bash
find e2e/fixtures -name "*.baseline.json" -exec sed -i '' 's/"assert": "soft"/"assert": "hard"/g' {} +
```

- [ ] **Step 2: 验证无残留 soft**

```bash
grep -r '"assert": "soft"' e2e/fixtures/
# 预期: 无输出
```

- [ ] **Step 3: 抽查一个文件确认格式正确**

```bash
cat e2e/fixtures/scenarios/txt/会议纪要.txt.baseline.json | python3 -m json.tool | head -20
```

- [ ] **Step 4: Commit**

```bash
git add e2e/fixtures/
git commit -m "test: baseline 断言全部改为 hard，NER 条目漏检即失败"
```

---

## Task 2: 删除冗余测试文件

**Files:**
- Delete: `src-tauri/tests/desensitize_scenario_csv.rs`
- Delete: `src-tauri/tests/desensitize_scenario_docx.rs`
- Delete: `src-tauri/tests/desensitize_scenario_xlsx.rs`
- Delete: `src-tauri/tests/desensitize_executive.rs`
- Delete: `src-tauri/tests/desensitize_uk.rs`
- Delete: `src-tauri/tests/desensitize_us_compliance.rs`
- Delete: `src-tauri/tests/desensitize_english.rs`
- Delete: `src-tauri/tests/desensitize_international.rs`
- Delete: `src-tauri/tests/desensitize_legal_english.rs`

- [ ] **Step 1: 删除 9 个冗余文件**

```bash
cd src-tauri/tests
rm desensitize_scenario_csv.rs \
   desensitize_scenario_docx.rs \
   desensitize_scenario_xlsx.rs \
   desensitize_executive.rs \
   desensitize_uk.rs \
   desensitize_us_compliance.rs \
   desensitize_english.rs \
   desensitize_international.rs \
   desensitize_legal_english.rs
```

- [ ] **Step 2: 确认编译通过**

```bash
cd src-tauri && cargo test --no-run 2>&1
# 预期: 编译成功，无引用断裂
```

- [ ] **Step 3: Commit**

```bash
git add -u src-tauri/tests/
git commit -m "test: 删除 9 个单引擎冗余测试文件，核心管道统一走 full_pipeline"
```

---

## Task 3: 精简灰色地带测试文件

**Files:**
- Modify: `src-tauri/tests/desensitize_csv.rs`
- Modify: `src-tauri/tests/desensitize_txt.rs`

- [ ] **Step 1: 精简 desensitize_csv.rs**

保留 5 个有独立价值的测试（结构/Mask/Replace），删除 3 个冗余的检测计数/baseline 测试：
- 删除 `test_csv_regex_detect_counts`
- 删除 `test_csv_regex_detect_exact_first_row`
- 删除 `test_csv_desensitize_summary`

- [ ] **Step 2: 精简 desensitize_txt.rs**

保留 1 个导入结构测试，删除 14 个冗余的检测计数/baseline 测试：
- 保留 `test_txt_meeting_import_structure`
- 删除其余所有 `test_txt_*` 函数

- [ ] **Step 3: 编译验证**

```bash
cd src-tauri && cargo test --test desensitize_csv --test desensitize_txt --no-fail-fast 2>&1
# 预期: 保留的测试全部通过
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tests/desensitize_csv.rs src-tauri/tests/desensitize_txt.rs
git commit -m "test: 精简 csv/txt 测试，保留结构/策略验证，删除冗余检测计数"
```

---

## Task 4: 更新 Excel 用例 test_file 映射

**Files:**
- Modify: `e2e/testcases.xlsx`（通过 excel_manager）

- [ ] **Step 1: 批量更新 test_file**

```python
import sys; sys.path.insert(0, 'e2e')
from utils.excel_manager import update_result

# C11-C17: desensitize_scenario_xlsx.rs → full_pipeline_xlsx.rs
for cid in ['C11','C12','C13','C14','C15','C16','C17']:
    update_result(cid, {"test_file": "full_pipeline_xlsx.rs"})

# C18-C21: desensitize_scenario_csv.rs → full_pipeline_csv.rs
for cid in ['C18','C19','C20','C21']:
    update_result(cid, {"test_file": "full_pipeline_csv.rs"})

# C22-C30: desensitize_scenario_docx.rs → full_pipeline_docx.rs
for cid in ['C22','C23','C24','C25','C26','C27','C28','C29','C30']:
    update_result(cid, {"test_file": "full_pipeline_docx.rs"})

# 其余单文件映射
mapping = {
    'C31': 'full_pipeline_csv.rs',
    'C32': 'full_pipeline_xlsx.rs',
    'C36': 'full_pipeline_txt.rs',
    'C37': 'full_pipeline_txt.rs',
    'C39': 'full_pipeline_csv.rs',
    'C40': 'full_pipeline_xlsx.rs',
    'C41': 'full_pipeline_docx.rs',
    'C42': 'full_pipeline_txt.rs',
    'C43': 'full_pipeline_docx.rs',
    'C44': 'full_pipeline_xlsx.rs',
    'C45': 'full_pipeline_csv.rs',
    'C46': 'full_pipeline_docx.rs',
    'C47': 'full_pipeline_docx.rs',
    'C48': 'full_pipeline_csv.rs',
    'C49': 'full_pipeline_basic.rs',
}
for cid, tf in mapping.items():
    update_result(cid, {"test_file": tf})
```

- [ ] **Step 2: 清空之前的执行结果（因为引擎变了，旧结果无效）**

对所有重映射的用例清空 exec_result：

```python
all_remapped = [f'C{i}' for i in range(11,50)] + ['C31','C32','C36','C37','C39','C40','C41','C42','C43']
for cid in set(all_remapped):
    update_result(cid, {"exec_result": "", "fail_reason": ""})
```

- [ ] **Step 3: Commit**

```bash
git add e2e/testcases.xlsx
git commit -m "test: Excel 用例 test_file 重映射到 full_pipeline，清空旧结果"
```

---

## Task 5: 执行 full_pipeline 测试并回写结果

**Files:**
- Run: `src-tauri/tests/full_pipeline_*.rs`
- Modify: `e2e/testcases.xlsx`
- Modify: `e2e/bug-list.md`

- [ ] **Step 1: 执行全部 full_pipeline 测试**

```bash
cd src-tauri && cargo test --test full_pipeline_basic --test full_pipeline_csv --test full_pipeline_xlsx --test full_pipeline_docx --test full_pipeline_txt --no-fail-fast 2>&1
```

- [ ] **Step 2: 解析结果，逐用例判断通过/失败**

根据测试输出中每个 test function 的 ok/FAILED 状态，映射回 Excel 用例 ID。

- [ ] **Step 3: 回写 Excel**

对每个用例调用 `update_result(case_id, {"exec_result": "通过/失败", "fail_reason": "...", "coverage": "已覆盖"})`。

- [ ] **Step 4: 更新 bug-list.md**

- 已修复的 Bug → 关闭
- 仍失败的 Bug → 保留
- 新增失败 → 追加新 Bug 条目

- [ ] **Step 5: Commit**

```bash
git add e2e/testcases.xlsx e2e/bug-list.md
git commit -m "test: full_pipeline 端到端测试结果回写"
```

---

## Task 6: 汇报结果

- [ ] **Step 1: 输出测试汇报**

```
## 测试结果

执行范围: full_pipeline 端到端（三层引擎，baseline 全 hard）
full_pipeline_basic: X 通过 / Y 失败
full_pipeline_csv: X 通过 / Y 失败
full_pipeline_xlsx: X 通过 / Y 失败
full_pipeline_docx: X 通过 / Y 失败
full_pipeline_txt: X 通过 / Y 失败

### 失败用例
| ID | 场景 | 错误摘要 |

### Bug 清单变更
新增: N 个 / 关闭: M 个 / 保持: K 个

已回写 Excel ✓
```
