# Dimkey E2E Skill 升级设计 — 自然语言驱动测试

## 背景

E2E 测试框架已搭建（Playwright + pytest），但存在两个问题：
1. 测试断言太弱（`highlights > 0`），不知道具体检测是否正确
2. 添加用例需要手动写 Python 代码，用户无法直接参与

## 目标

用户用自然语言描述测试场景 → skill 自动完成：写入 Excel、生成代码、执行测试、回报结果。Excel 是用例的唯一真相来源，Python 代码是编译产物。

## 工作流

```
用户自然语言描述场景
    ↓
① 写入 Excel（testcases.xlsx）
   - Sheet1 新增用例行（ID、分类、场景、步骤、期望结果、fixture）
   - Sheet2 补基线数据（新的敏感值期望）
    ↓
② 生成测试代码
   - e2e/tests/ 下创建或追加测试函数
   - 引用 helpers + baseline 对照
    ↓
③ 执行测试
   - 运行 pytest 用例（需要 dev server 已启动）
   - 截图保存到 e2e/output/
    ↓
④ 回报结果
   - 对话中：通过/失败 + 关键数据对比 + 截图路径
   - Excel 中：更新覆盖状态、执行结果、时间戳
```

## 新增模块

### 1. excel_manager.py（Excel 读写）

路径：`e2e/utils/excel_manager.py`

功能：
- `read_testcases()` — 读取 Sheet1 所有用例，返回 list of dict
- `add_testcase(case: dict)` — 写入新用例行，自动分配 ID
- `read_baseline(fixture_file: str)` — 读取 Sheet2 中某 fixture 的所有期望敏感值
- `add_baseline(fixture_file: str, items: list)` — 写入新基线条目
- `update_result(case_id: str, result: dict)` — 回写执行结果到 Sheet1（覆盖状态、通过/失败、时间戳）

用例 ID 规则：读取当前 Sheet1 最大 ID，按分类前缀自增（C10, S07, D06...）。

### 2. baseline.py（基线对照）

路径：`e2e/utils/baseline.py`

功能：
- `load_baseline(fixture_file: str) -> list[tuple[str, str]]` — 从 Excel Sheet2 读取 (敏感值, 类型) 列表
- `get_detected_items(page) -> list[str]` — 从页面提取所有 `data-testid="sensitive-highlight"` 元素的文本内容
- `assert_baseline(page, fixture_file: str)` — 对照断言：期望的敏感值是否都在检测结果中

对照逻辑：
```python
def assert_baseline(page, fixture_file):
    expected = load_baseline(fixture_file)
    actual = get_detected_items(page)
    missing = []
    for value, type_name in expected:
        if value not in actual:
            missing.append(f"{type_name}: {value}")
    assert not missing, f"未检测到: {missing}"
```

注意事项：
- NER 识别（PersonName, OrgName, Address）有不确定性，baseline 中标记 `NER 识别` 的条目用软断言（miss 了记录 warning 但不 fail）
- 正则识别（Phone, IdCard, Email, CreditCode）是确定性的，必须全部命中

### 3. dimkey-e2e Skill 升级

路径：`skills/dimkey-e2e/SKILL.md`

新增的流程指引：

**当用户描述测试场景时：**

1. 解析意图 — 从自然语言提取：输入文件、操作步骤、期望结果
2. 写入 Excel — 调用 excel_manager 添加用例和基线
3. 生成代码 — 在对应的 test_*.py 中追加测试函数，使用 baseline 对照
4. 执行测试 — `cd e2e && python -m pytest tests/test_xxx.py::test_name -v`
5. 回报结果 — 对话摘要 + Excel 回写

**当用户说"跑测试"时：**
- 执行指定范围的测试
- 收集结果批量回写 Excel

**当用户说"看覆盖率"时：**
- 读取 Excel Sheet3 覆盖率统计
- 在对话中展示

### 4. get_detected_items 实现

需要在 helpers.py 中新增函数，从页面 DOM 提取实际检测到的敏感值文本：

```python
def get_detected_items(page) -> list[str]:
    """提取页面上所有敏感高亮项的原始文本"""
    elements = page.locator('[data-testid="sensitive-highlight"]').all()
    return [el.text_content().strip() for el in elements]
```

### 5. Excel 结构调整

Sheet1 新增列：
- **执行结果** — 通过/失败/未执行
- **失败原因** — 具体的断言错误信息
- **执行时间** — 时间戳
- **截图路径** — e2e/output/ 下的截图文件名

Sheet2 新增列：
- **断言模式** — hard（正则类，必须命中）/ soft（NER 类，miss 只 warning）

Sheet3 覆盖率统计 — 由脚本自动根据 Sheet1 数据重新生成

## 不变的部分

- 测试框架（Playwright + pytest）不变
- helpers.py 现有函数不变，只新增
- conftest.py 不变
- with_tauri.py 不变
- fixture 文件不变
- data-testid 属性不变
