# Excel 用例管理器

操作 `e2e/testcases.xlsx`，管理测试用例和基线数据。

## 导入方式

```python
import sys; sys.path.insert(0, "e2e")
from utils.excel_manager import add_testcase, add_baseline, update_result, read_testcases, read_baseline
```

也可用 venv: `e2e/.venv/bin/python`

## Sheet1: 测试用例

### 写入用例

```python
case_id = add_testcase({
    "category": "核心管道",   # 决定 ID 前缀
    "scenario": "场景描述",
    "precondition": "前置条件",
    "steps": "1. 步骤\n2. ...",
    "expected": "期望结果",
    "fixture": "scenarios/csv/员工信息表.csv",
    "priority": "P1",
    "test_file": "desensitize_csv.rs",
})
```

### 分类 → ID 前缀

核心管道→C, 策略切换→S, 类型过滤→T, 字典/白名单→D, 列级规则→L, 一致性替换→K, 还原→R, 批量处理→B, 工作区管理→W, UI交互→U

### 回写结果

```python
update_result("C01", {
    "exec_result": "通过",    # 或 "失败"
    "fail_reason": "",
    "coverage": "已覆盖",     # 或 "部分覆盖" / "未覆盖"
})
```

### 读取用例

```python
cases = read_testcases()  # → [{"id", "category", "scenario", "priority", "exec_result", ...}]
```

## Sheet2: Fixture 数据基线

### 写入基线

```python
add_baseline("scenarios/csv/员工信息表.csv", [
    {"value": "13800138001", "type": "Phone", "assert_mode": "hard"},
    {"value": "张三", "type": "PersonName", "assert_mode": "soft"},
])
```

### 读取基线

```python
items = read_baseline("scenarios/csv/员工信息表.csv")
# → [{"value": "13800138001", "type": "Phone", "assert_mode": "hard"}, ...]
```

### 断言模式

- **hard**: 正则类（Phone, IdCard, Email, CreditCode, BankCard）— 必须命中
- **soft**: NER 类（PersonName, OrgName, Address）— 未命中只 warning

## 当前数据量

- Sheet1: 69 条用例
- Sheet2: 498 条基线，覆盖 25 个 fixture
