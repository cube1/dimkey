# E2E Skill 升级 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 升级 dimkey-e2e skill，实现自然语言描述场景 → 自动写入 Excel → 生成测试代码 → 执行 → 回报结果的完整流程。

**Architecture:** excel_manager 负责 Excel 读写，baseline 负责基线对照断言，helpers 新增 get_detected_items，skill 编排完整 4 步流程。

**Tech Stack:** Python 3, openpyxl, Playwright, pytest

---

## File Structure

```
e2e/utils/
├── helpers.py          (Modify: 新增 get_detected_items)
├── excel_manager.py    (Create: Excel 读写)
└── baseline.py         (Create: 基线对照断言)

e2e/testcases.xlsx      (Modify: Sheet1 新增 3 列, Sheet2 新增 1 列)

skills/dimkey-e2e/
└── SKILL.md            (Modify: 升级为 4 步流程)
```

---

### Task 1: excel_manager.py — Excel 读写工具

**Files:**
- Create: `e2e/utils/excel_manager.py`

- [ ] **Step 1: 创建 excel_manager.py**

Create `e2e/utils/excel_manager.py`:

```python
"""Dimkey E2E 测试用例 Excel 管理器

读写 testcases.xlsx：
- Sheet1 "测试用例": 用例定义和执行结果
- Sheet2 "Fixture数据基线": fixture 文件的期望敏感值
"""

from datetime import datetime
from pathlib import Path
from openpyxl import load_workbook
from openpyxl.styles import PatternFill, Border, Side, Alignment

EXCEL_PATH = Path(__file__).resolve().parent.parent / "testcases.xlsx"

THIN_BORDER = Border(
    left=Side(style="thin"), right=Side(style="thin"),
    top=Side(style="thin"), bottom=Side(style="thin"),
)
WRAP = Alignment(wrap_text=True, vertical="top")

# Sheet1 列索引（1-based）
COL_ID = 1
COL_CATEGORY = 2
COL_SCENARIO = 3
COL_PRECONDITION = 4
COL_STEPS = 5
COL_EXPECTED = 6
COL_FIXTURE = 7
COL_PRIORITY = 8
COL_COVERAGE = 9
COL_TEST_FILE = 10
COL_NOTE = 11
COL_EXEC_RESULT = 12
COL_FAIL_REASON = 13
COL_EXEC_TIME = 14
COL_SCREENSHOT = 15

# Sheet2 列索引（1-based）
BL_COL_FIXTURE = 1
BL_COL_VALUE = 2
BL_COL_TYPE = 3
BL_COL_COUNT = 4
BL_COL_NOTE = 5
BL_COL_ASSERT_MODE = 6


def _load_wb():
    """加载工作簿"""
    return load_workbook(str(EXCEL_PATH))


def _save_wb(wb):
    """保存工作簿"""
    wb.save(str(EXCEL_PATH))


def _style_row(ws, row, col_count):
    """给新行添加边框和换行"""
    for col in range(1, col_count + 1):
        cell = ws.cell(row=row, column=col)
        cell.border = THIN_BORDER
        cell.alignment = WRAP


def read_testcases() -> list[dict]:
    """读取 Sheet1 所有用例"""
    wb = _load_wb()
    ws = wb["测试用例"]
    cases = []
    for row in range(2, ws.max_row + 1):
        case_id = ws.cell(row, COL_ID).value
        if not case_id:
            continue
        cases.append({
            "id": case_id,
            "category": ws.cell(row, COL_CATEGORY).value,
            "scenario": ws.cell(row, COL_SCENARIO).value,
            "precondition": ws.cell(row, COL_PRECONDITION).value,
            "steps": ws.cell(row, COL_STEPS).value,
            "expected": ws.cell(row, COL_EXPECTED).value,
            "fixture": ws.cell(row, COL_FIXTURE).value,
            "priority": ws.cell(row, COL_PRIORITY).value,
            "coverage": ws.cell(row, COL_COVERAGE).value,
            "test_file": ws.cell(row, COL_TEST_FILE).value,
            "note": ws.cell(row, COL_NOTE).value,
            "exec_result": ws.cell(row, COL_EXEC_RESULT).value,
            "row": row,
        })
    wb.close()
    return cases


def _next_id(ws, prefix: str) -> str:
    """根据前缀计算下一个用例 ID，如 C10, S07"""
    max_num = 0
    for row in range(2, ws.max_row + 1):
        cell_id = ws.cell(row, COL_ID).value
        if cell_id and cell_id.startswith(prefix):
            try:
                num = int(cell_id[len(prefix):])
                max_num = max(max_num, num)
            except ValueError:
                pass
    return f"{prefix}{max_num + 1:02d}"


# 分类 → ID 前缀映射
CATEGORY_PREFIX = {
    "核心管道": "C",
    "策略切换": "S",
    "类型过滤": "T",
    "字典/白名单": "D",
    "列级规则": "L",
    "一致性替换": "K",
    "还原": "R",
    "批量处理": "B",
    "工作区管理": "W",
    "UI交互": "U",
}


def add_testcase(case: dict) -> str:
    """写入新用例行，返回分配的用例 ID

    case keys: category, scenario, precondition, steps, expected,
               fixture, priority, test_file, note
    """
    wb = _load_wb()
    ws = wb["测试用例"]

    prefix = CATEGORY_PREFIX.get(case.get("category", ""), "X")
    case_id = _next_id(ws, prefix)

    row = ws.max_row + 1
    ws.cell(row, COL_ID, case_id)
    ws.cell(row, COL_CATEGORY, case.get("category", ""))
    ws.cell(row, COL_SCENARIO, case.get("scenario", ""))
    ws.cell(row, COL_PRECONDITION, case.get("precondition", ""))
    ws.cell(row, COL_STEPS, case.get("steps", ""))
    ws.cell(row, COL_EXPECTED, case.get("expected", ""))
    ws.cell(row, COL_FIXTURE, case.get("fixture", ""))
    ws.cell(row, COL_PRIORITY, case.get("priority", "P1"))
    ws.cell(row, COL_COVERAGE, "未覆盖")
    ws.cell(row, COL_TEST_FILE, case.get("test_file", ""))
    ws.cell(row, COL_NOTE, case.get("note", ""))
    ws.cell(row, COL_EXEC_RESULT, "未执行")

    _style_row(ws, row, 15)
    _save_wb(wb)
    return case_id


def read_baseline(fixture_file: str) -> list[dict]:
    """读取 Sheet2 中某 fixture 的所有期望敏感值

    返回: [{"value": "13800138000", "type": "Phone", "assert_mode": "hard"}, ...]
    """
    wb = _load_wb()
    ws = wb["Fixture数据基线"]
    items = []
    for row in range(2, ws.max_row + 1):
        if ws.cell(row, BL_COL_FIXTURE).value == fixture_file:
            value = ws.cell(row, BL_COL_VALUE).value
            if not value or value.startswith("("):
                continue
            note = ws.cell(row, BL_COL_NOTE).value or ""
            assert_mode_cell = ws.cell(row, BL_COL_ASSERT_MODE).value
            # 根据备注推断断言模式：NER 类用 soft，正则类用 hard
            if assert_mode_cell:
                mode = assert_mode_cell
            elif "NER" in note:
                mode = "soft"
            else:
                mode = "hard"
            items.append({
                "value": str(value),
                "type": ws.cell(row, BL_COL_TYPE).value or "",
                "assert_mode": mode,
            })
    wb.close()
    return items


def add_baseline(fixture_file: str, items: list[dict]):
    """写入新基线条目

    items: [{"value": "xxx", "type": "Phone", "count": 1, "note": "正则", "assert_mode": "hard"}, ...]
    """
    wb = _load_wb()
    ws = wb["Fixture数据基线"]
    for item in items:
        row = ws.max_row + 1
        ws.cell(row, BL_COL_FIXTURE, fixture_file)
        ws.cell(row, BL_COL_VALUE, item["value"])
        ws.cell(row, BL_COL_TYPE, item.get("type", ""))
        ws.cell(row, BL_COL_COUNT, item.get("count", 1))
        ws.cell(row, BL_COL_NOTE, item.get("note", ""))
        ws.cell(row, BL_COL_ASSERT_MODE, item.get("assert_mode", "hard"))
        _style_row(ws, row, 6)
    _save_wb(wb)


def update_result(case_id: str, result: dict):
    """回写执行结果到 Sheet1

    result keys: exec_result ("通过"/"失败"), fail_reason, screenshot, coverage
    """
    wb = _load_wb()
    ws = wb["测试用例"]
    for row in range(2, ws.max_row + 1):
        if ws.cell(row, COL_ID).value == case_id:
            ws.cell(row, COL_EXEC_RESULT, result.get("exec_result", ""))
            ws.cell(row, COL_FAIL_REASON, result.get("fail_reason", ""))
            ws.cell(row, COL_EXEC_TIME, datetime.now().strftime("%Y-%m-%d %H:%M"))
            ws.cell(row, COL_SCREENSHOT, result.get("screenshot", ""))
            if result.get("coverage"):
                ws.cell(row, COL_COVERAGE, result["coverage"])
                # 着色
                fill_map = {
                    "已覆盖": PatternFill(start_color="D3F9D8", end_color="D3F9D8", fill_type="solid"),
                    "部分覆盖": PatternFill(start_color="FFF3BF", end_color="FFF3BF", fill_type="solid"),
                    "未覆盖": PatternFill(start_color="FFE3E3", end_color="FFE3E3", fill_type="solid"),
                }
                if result["coverage"] in fill_map:
                    ws.cell(row, COL_COVERAGE).fill = fill_map[result["coverage"]]
            # 执行结果着色
            if result.get("exec_result") == "通过":
                ws.cell(row, COL_EXEC_RESULT).fill = PatternFill(
                    start_color="D3F9D8", end_color="D3F9D8", fill_type="solid")
            elif result.get("exec_result") == "失败":
                ws.cell(row, COL_EXEC_RESULT).fill = PatternFill(
                    start_color="FFE3E3", end_color="FFE3E3", fill_type="solid")
            break
    _save_wb(wb)
```

- [ ] **Step 2: 验证语法**

Run: `python3 -m py_compile e2e/utils/excel_manager.py && echo "OK"`
Expected: `OK`

- [ ] **Step 3: Commit**

```bash
cd /Users/tanzs-mac-mini/workpath/personal/dimkey
git add e2e/utils/excel_manager.py
git commit -m "feat: 添加 Excel 用例管理器 — 读写测试用例和基线数据

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: 更新 testcases.xlsx — 新增列和断言模式

**Files:**
- Create: `e2e/update_excel_schema.py`（一次性脚本）
- Modify: `e2e/testcases.xlsx`

- [ ] **Step 1: 创建 schema 更新脚本**

Create `e2e/update_excel_schema.py`:

```python
#!/usr/bin/env python3
"""一次性脚本：为 testcases.xlsx 新增 Sheet1 执行结果列 + Sheet2 断言模式列"""

from openpyxl import load_workbook
from openpyxl.styles import Font, PatternFill, Alignment, Border, Side
from pathlib import Path

EXCEL_PATH = Path(__file__).parent / "testcases.xlsx"

HEADER_FONT = Font(bold=True, color="FFFFFF", size=11)
HEADER_FILL = PatternFill(start_color="4472C4", end_color="4472C4", fill_type="solid")
THIN_BORDER = Border(
    left=Side(style="thin"), right=Side(style="thin"),
    top=Side(style="thin"), bottom=Side(style="thin"),
)

wb = load_workbook(str(EXCEL_PATH))

# === Sheet1: 新增 4 列（执行结果、失败原因、执行时间、截图路径）===
ws1 = wb["测试用例"]
new_headers = {12: "执行结果", 13: "失败原因", 14: "执行时间", 15: "截图路径"}
for col, header in new_headers.items():
    cell = ws1.cell(1, col, header)
    cell.font = HEADER_FONT
    cell.fill = HEADER_FILL
    cell.alignment = Alignment(horizontal="center", vertical="center")
    cell.border = THIN_BORDER

# 给已有数据行填默认值
for row in range(2, ws1.max_row + 1):
    if ws1.cell(row, 1).value:
        ws1.cell(row, 12, "未执行")
        for col in range(12, 16):
            ws1.cell(row, col).border = THIN_BORDER
            ws1.cell(row, col).alignment = Alignment(wrap_text=True, vertical="top")

# 更新自动筛选范围
ws1.auto_filter.ref = f"A1:O{ws1.max_row}"

# === Sheet2: 新增"断言模式"列 ===
ws2 = wb["Fixture数据基线"]
cell = ws2.cell(1, 6, "断言模式")
cell.font = HEADER_FONT
cell.fill = HEADER_FILL
cell.alignment = Alignment(horizontal="center", vertical="center")
cell.border = THIN_BORDER

# 根据备注自动填充断言模式
for row in range(2, ws2.max_row + 1):
    note = ws2.cell(row, 5).value or ""
    mode = "soft" if "NER" in note else "hard"
    ws2.cell(row, 6, mode)
    ws2.cell(row, 6).border = THIN_BORDER
    ws2.cell(row, 6).alignment = Alignment(vertical="top")

# 更新自动筛选范围
ws2.auto_filter.ref = f"A1:F{ws2.max_row}"

wb.save(str(EXCEL_PATH))
print(f"Excel schema 已更新: {EXCEL_PATH}")
```

- [ ] **Step 2: 运行脚本**

Run: `python3 e2e/update_excel_schema.py`
Expected: `Excel schema 已更新: ...`

- [ ] **Step 3: 验证新增列**

Run:
```bash
python3 -c "
from openpyxl import load_workbook
wb = load_workbook('e2e/testcases.xlsx')
ws1 = wb['测试用例']
print('Sheet1 headers:', [ws1.cell(1,c).value for c in range(1,16)])
ws2 = wb['Fixture数据基线']
print('Sheet2 headers:', [ws2.cell(1,c).value for c in range(1,7)])
print('Sheet2 row2 assert_mode:', ws2.cell(2,6).value)
"
```
Expected: Sheet1 含 15 列（含执行结果/失败原因/执行时间/截图路径），Sheet2 含 6 列（含断言模式）

- [ ] **Step 4: Commit**

```bash
cd /Users/tanzs-mac-mini/workpath/personal/dimkey
git add e2e/update_excel_schema.py e2e/testcases.xlsx
git commit -m "feat: 更新 Excel schema — 新增执行结果列和断言模式列

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: baseline.py — 基线对照断言

**Files:**
- Create: `e2e/utils/baseline.py`
- Modify: `e2e/utils/helpers.py`

- [ ] **Step 1: 创建 baseline.py**

Create `e2e/utils/baseline.py`:

```python
"""Fixture 数据基线对照 — 验证检测结果是否匹配期望"""

from utils.excel_manager import read_baseline


def assert_baseline(detected_texts: list[str], fixture_file: str) -> dict:
    """对照 Excel 基线验证检测结果

    Args:
        detected_texts: 页面上所有敏感高亮项的文本列表
        fixture_file: fixture 文件名（如 "sample.txt"）

    Returns:
        {
            "passed": bool,
            "hard_missing": [(value, type), ...],  # 正则类必须命中但未命中
            "soft_missing": [(value, type), ...],  # NER 类未命中（warning）
            "hard_found": [(value, type), ...],    # 正则类命中
            "soft_found": [(value, type), ...],    # NER 类命中
            "total_expected": int,
            "total_found": int,
        }
    """
    baseline = read_baseline(fixture_file)
    if not baseline:
        return {
            "passed": True,
            "hard_missing": [],
            "soft_missing": [],
            "hard_found": [],
            "soft_found": [],
            "total_expected": 0,
            "total_found": 0,
        }

    detected_set = set(detected_texts)

    hard_missing = []
    soft_missing = []
    hard_found = []
    soft_found = []

    for item in baseline:
        value = item["value"]
        type_name = item["type"]
        mode = item["assert_mode"]

        found = value in detected_set
        if mode == "hard":
            if found:
                hard_found.append((value, type_name))
            else:
                hard_missing.append((value, type_name))
        else:  # soft
            if found:
                soft_found.append((value, type_name))
            else:
                soft_missing.append((value, type_name))

    return {
        "passed": len(hard_missing) == 0,
        "hard_missing": hard_missing,
        "soft_missing": soft_missing,
        "hard_found": hard_found,
        "soft_found": soft_found,
        "total_expected": len(baseline),
        "total_found": len(hard_found) + len(soft_found),
    }


def format_baseline_report(result: dict) -> str:
    """格式化基线对照报告"""
    lines = []
    total = result["total_expected"]
    found = result["total_found"]
    lines.append(f"基线对照: {found}/{total} 命中")

    if result["hard_missing"]:
        lines.append(f"  ❌ 正则类未命中 ({len(result['hard_missing'])}):")
        for value, type_name in result["hard_missing"]:
            lines.append(f"     - {type_name}: {value}")

    if result["soft_missing"]:
        lines.append(f"  ⚠️  NER 类未命中 ({len(result['soft_missing'])}):")
        for value, type_name in result["soft_missing"]:
            lines.append(f"     - {type_name}: {value}")

    if result["passed"]:
        lines.append("  ✅ 正则类全部命中")
    else:
        lines.append("  ❌ 存在正则类未命中项，测试失败")

    return "\n".join(lines)
```

- [ ] **Step 2: 在 helpers.py 中新增 get_detected_items**

在 `e2e/utils/helpers.py` 文件末尾追加：

```python


def get_detected_items(page: Page) -> list[str]:
    """提取页面上所有敏感高亮项的原始文本"""
    elements = page.locator('[data-testid="sensitive-highlight"]').all()
    return [el.text_content().strip() for el in elements if el.text_content()]
```

- [ ] **Step 3: 验证语法**

Run:
```bash
python3 -m py_compile e2e/utils/baseline.py && \
python3 -m py_compile e2e/utils/helpers.py && \
echo "OK"
```
Expected: `OK`

- [ ] **Step 4: Commit**

```bash
cd /Users/tanzs-mac-mini/workpath/personal/dimkey
git add e2e/utils/baseline.py e2e/utils/helpers.py
git commit -m "feat: 添加基线对照断言 — hard/soft 两级验证检测结果

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: 升级 dimkey-e2e Skill

**Files:**
- Modify: `skills/dimkey-e2e/SKILL.md`

- [ ] **Step 1: 重写 SKILL.md**

Read current `skills/dimkey-e2e/SKILL.md`, then overwrite with:

```markdown
---
name: dimkey-e2e
description: Dimkey 应用 E2E 自动化测试工具。自然语言描述场景 → 自动写入 Excel → 生成测试代码 → 执行 → 回报结果。当用户提到"测试"、"跑测试"、"E2E"、"验证功能"、"测一下"、"帮我测"、"写测试用例"、"界面测试"、"自动化测试"、"回归测试"、"覆盖率"时触发。
---

# Dimkey E2E 测试

## 核心工作流

当用户用自然语言描述测试场景时，执行以下 4 步：

### Step 1: 写入 Excel

用 `e2e/utils/excel_manager.py` 操作 `e2e/testcases.xlsx`：

```python
from e2e.utils.excel_manager import add_testcase, add_baseline

# 新增用例到 Sheet1
case_id = add_testcase({
    "category": "核心管道",        # 分类（决定 ID 前缀）
    "scenario": "用户描述的场景",
    "precondition": "前置条件",
    "steps": "1. 操作步骤\n2. ...",
    "expected": "1. 期望结果\n2. ...",
    "fixture": "sample.xlsx",
    "priority": "P1",
    "test_file": "test_xxx.py",
})

# 如果涉及新的敏感数据基线，补到 Sheet2
add_baseline("sample.txt", [
    {"value": "13800138000", "type": "Phone", "assert_mode": "hard"},
    {"value": "张三", "type": "PersonName", "assert_mode": "soft"},
])
```

分类 → ID 前缀映射：
- 核心管道→C, 策略切换→S, 类型过滤→T, 字典/白名单→D
- 列级规则→L, 一致性替换→K, 还原→R, 批量处理→B
- 工作区管理→W, UI交互→U

### Step 2: 生成测试代码

在 `e2e/tests/` 下创建或追加测试函数。使用基线对照断言：

```python
from utils.helpers import (
    wait_for_view, wait_for_processing_done,
    import_file_via_ipc, get_fixture_path,
    get_detected_items, take_diagnostic,
)
from utils.baseline import assert_baseline, format_baseline_report

def test_case_id_scenario(page):
    """C01: 基础脱敏 - xlsx"""
    fixture_path = get_fixture_path("sample.xlsx")
    wait_for_view(page, "dropzone", timeout=10_000)
    import_file_via_ipc(page, fixture_path)
    wait_for_processing_done(page)

    # 基线对照
    detected = get_detected_items(page)
    result = assert_baseline(detected, "sample.xlsx")
    print(format_baseline_report(result))
    take_diagnostic(page, "C01_basic_xlsx")

    assert result["passed"], format_baseline_report(result)
```

### Step 3: 执行测试

前提：dev server 已启动（`npm run dev` 或 `cargo tauri dev`）

```bash
# 执行单个测试
cd e2e && python3 -m pytest tests/test_xxx.py::test_name -v

# 执行某优先级
cd e2e && python3 -m pytest -v -m p0

# 完整执行
python3 e2e/scripts/with_tauri.py -- pytest e2e/tests/ -v
```

### Step 4: 回报结果

1. **对话中** — 告诉用户：
   - 通过/失败
   - 基线对照摘要（命中数/总数，未命中项）
   - 截图路径
   
2. **Excel 中** — 回写执行结果：

```python
from e2e.utils.excel_manager import update_result

update_result("C01", {
    "exec_result": "通过",       # 或 "失败"
    "fail_reason": "",           # 失败时填具体原因
    "screenshot": "C01_basic_xlsx.png",
    "coverage": "已覆盖",        # 或 "部分覆盖"
})
```

---

## 用户说"跑测试"时

执行指定范围的测试，收集结果批量回写 Excel。

## 用户说"看覆盖率"时

读取 Excel 数据统计：
```python
from e2e.utils.excel_manager import read_testcases
cases = read_testcases()
# 统计各分类的覆盖和执行情况
```

---

## 基线对照机制

### 断言模式

- **hard**（正则类：Phone, IdCard, Email, CreditCode 等）— 必须命中，未命中则测试失败
- **soft**（NER 类：PersonName, OrgName, Address）— 未命中记录 warning，不导致测试失败

### 数据来源

`e2e/testcases.xlsx` → Sheet2 "Fixture数据基线"

每行记录一个期望的敏感值：fixture 文件、敏感值、类型、断言模式。

### 验证流程

```
页面 DOM → get_detected_items() → ["13800138000", "张三", ...]
                                          ↓ 对比
Excel 基线 → read_baseline()    → [("13800138000", "Phone", "hard"), ...]
                                          ↓
                                   assert_baseline() → passed/failed + 详细报告
```

---

## 可用的 helpers

| 函数 | 用途 |
|------|------|
| `wait_for_view(page, name)` | 等待视图切换 |
| `wait_for_processing_done(page)` | 等待处理完成 |
| `count_highlights(page)` | 统计高亮数量 |
| `get_detected_items(page)` | 提取所有高亮项文本 |
| `import_file_via_ipc(page, path)` | 导入文件 |
| `click_export/back/restore_*` | 按钮操作 |
| `take_diagnostic(page, name)` | 截图 |
| `get_fixture_path(filename)` | 获取 fixture 路径 |

## data-testid 选择器

```
视图:   view-empty, view-dropzone, view-processing, view-comparison, view-restore
按钮:   btn-export, btn-export-next, btn-back, btn-restore-ai, btn-restore-workspace
面板:   panel-strategy, panel-dict, panel-whitelist
列表:   workspace-list, file-queue
高亮:   sensitive-highlight
```

## 视图状态机

```
empty ──(选择工作区)──→ dropzone
dropzone ──(导入文件)──→ processing ──(完成)──→ comparison
comparison ──(返回)──→ dropzone
dropzone ──(还原)──→ restore ──(返回)──→ dropzone
```
```

- [ ] **Step 2: Commit**

```bash
cd /Users/tanzs-mac-mini/workpath/personal/dimkey
git add skills/dimkey-e2e/SKILL.md
git commit -m "feat: 升级 dimkey-e2e skill — 4 步自然语言驱动测试流程

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: 提交设计文档

**Files:**
- `docs/superpowers/specs/2026-04-03-e2e-skill-upgrade-design.md`
- `docs/superpowers/plans/2026-04-03-e2e-skill-upgrade.md`

- [ ] **Step 1: Commit**

```bash
cd /Users/tanzs-mac-mini/workpath/personal/dimkey
git add docs/superpowers/specs/2026-04-03-e2e-skill-upgrade-design.md \
        docs/superpowers/plans/2026-04-03-e2e-skill-upgrade.md
git commit -m "docs: 添加 E2E skill 升级设计文档和实施计划

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```
