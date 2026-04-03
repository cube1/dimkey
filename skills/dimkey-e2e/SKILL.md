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
