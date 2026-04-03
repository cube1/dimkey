# Dimkey E2E 自动化测试设计

## 背景

Dimkey 已发展为功能丰富的本地脱敏工具（5 个视图、40+ Tauri 命令、6 条核心流程），但目前零前端测试基础设施。一人快速迭代场景下，改了 A 容易坏 B 且无法及时发现。需要建立系统性的自动化测试能力。

## 目标

1. 建立 E2E 测试框架，覆盖核心用户流程的功能组合测试
2. 创建配套 skill (`dimkey-e2e`)，支持自然语言描述场景 → 生成/执行测试
3. 先本地命令行运行，后续可接入 CI

## 技术选型

| 组件 | 选择 | 理由 |
|------|------|------|
| 浏览器自动化 | Playwright (Python, sync mode) | 成熟稳定，支持 WebView 连接 |
| 测试运行器 | pytest | fixture 复用、参数化、标记分组 |
| 进程管理 | 自研 `with_tauri.py` | 管理 Tauri 应用生命周期（启动→就绪→测试→关闭）|
| 截图/诊断 | Playwright screenshot + trace | 失败时自动截图，支持 trace 回放 |

## 目录结构

```
e2e/
├── scripts/
│   └── with_tauri.py            # Tauri 进程管理
├── fixtures/                     # 测试样本文件
│   ├── sample.xlsx              # 含手机号、身份证、姓名等敏感信息
│   ├── sample.csv               # CSV 格式样本
│   ├── sample.docx              # Word 文档样本
│   ├── sample.txt               # 纯文本样本
│   ├── sample.pdf               # PDF 样本
│   ├── sample_encrypted.xlsx    # 密码保护文件（密码: test123）
│   ├── sample_multi_sheet.xlsx  # 多 sheet 文件
│   └── sample_batch/            # 批量处理用（3-5 个文件）
│       ├── batch_1.xlsx
│       ├── batch_2.csv
│       └── batch_3.docx
├── tests/
│   ├── conftest.py              # pytest fixtures（浏览器、应用实例、工作区）
│   ├── test_basic_desensitize.py    # P0: 基础脱敏流程
│   ├── test_restore.py              # P0: 还原流程
│   ├── test_workspace_crud.py       # P1: 工作区管理
│   ├── test_column_rules.py         # P1: 列级规则
│   ├── test_batch_processing.py     # P2: 批量处理
│   └── test_dict_whitelist.py       # P2: 字典与白名单
└── utils/
    └── helpers.py               # 通用操作封装
```

## with_tauri.py 设计

管理 Tauri 应用的完整生命周期：

```
启动流程:
1. 检查是否已有 dev build（cargo tauri dev 产物）
2. 如果没有，先执行 cargo tauri build --debug
3. 启动编译产物（不是 cargo tauri dev，避免热重载干扰测试）
4. 轮询 WebView 端口直到就绪（超时 60 秒）
5. 设置环境变量 DIMKEY_TEST_URL 供测试脚本使用
6. 执行传入的测试命令
7. 测试完成后关闭 Tauri 进程

用法:
  python e2e/scripts/with_tauri.py -- pytest e2e/tests/
  python e2e/scripts/with_tauri.py -- pytest e2e/tests/test_basic_desensitize.py -v
  python e2e/scripts/with_tauri.py --keep-alive -- pytest e2e/tests/  # 测试后不关闭，方便调试
```

## conftest.py 核心 Fixtures

```python
@pytest.fixture(scope="session")
def browser():
    """会话级浏览器实例，所有测试共享"""
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        yield browser
        browser.close()

@pytest.fixture
def page(browser):
    """每个测试独立的页面，测试前后自动清理"""
    page = browser.new_page()
    page.goto(os.environ["DIMKEY_TEST_URL"])
    page.wait_for_load_state("networkidle")
    yield page
    page.close()

@pytest.fixture
def workspace(page):
    """创建一个干净的测试工作区，测试后删除"""
    # 创建 → yield → 清理
    ...

@pytest.fixture
def sample_files():
    """返回测试样本文件的路径字典"""
    base = Path(__file__).parent.parent / "fixtures"
    return {
        "xlsx": base / "sample.xlsx",
        "csv": base / "sample.csv",
        "docx": base / "sample.docx",
        ...
    }
```

## helpers.py 通用操作

```python
def wait_for_view(page, view_name: str, timeout: int = 30000):
    """等待视图切换（empty/dropzone/processing/comparison/restore）"""

def import_file(page, file_path: str):
    """通过文件选择器导入文件（模拟 file picker）"""

def wait_for_processing_done(page, timeout: int = 60000):
    """等待处理完成（parsing→detecting→desensitizing→saving→done）"""

def count_highlights(page) -> int:
    """统计当前视图中的敏感高亮项数量"""

def click_export(page) -> str:
    """点击导出按钮，返回导出文件路径"""

def take_diagnostic(page, name: str):
    """截图 + DOM snapshot 保存到 e2e/output/"""

def select_strategy(page, strategy: str):
    """在策略面板切换策略（Mask/Replace/Generalize）"""

def add_dict_entry(page, text: str, type_name: str):
    """在字典面板添加条目"""

def add_whitelist_entry(page, text: str):
    """在白名单面板添加条目"""
```

## 测试用例详细设计

### P0: 基础脱敏流程 (`test_basic_desensitize.py`)

```python
@pytest.mark.parametrize("file_type", ["xlsx", "csv", "docx", "txt"])
def test_import_detect_export(page, workspace, sample_files, file_type):
    """导入文件 → 自动识别 → 验证高亮 → 导出 → 验证输出"""
    # 1. 导入文件
    import_file(page, sample_files[file_type])
    # 2. 等待处理完成
    wait_for_view(page, "comparison")
    # 3. 验证高亮出现
    assert count_highlights(page) > 0
    # 4. 导出
    output_path = click_export(page)
    # 5. 验证导出文件存在且非空
    assert Path(output_path).exists()
    assert Path(output_path).stat().st_size > 0

def test_strategy_switch(page, workspace, sample_files):
    """切换策略后预览应更新"""
    import_file(page, sample_files["xlsx"])
    wait_for_view(page, "comparison")
    # 记录 Mask 下的内容
    mask_content = get_desensitized_text(page)
    # 切换到 Replace
    select_strategy(page, "Replace")
    wait_for_content_change(page, mask_content)
    replace_content = get_desensitized_text(page)
    # 两种策略结果应不同
    assert mask_content != replace_content

def test_pdf_import_and_redact(page, workspace, sample_files):
    """PDF 导入 → 渲染 → 验证遮挡区域"""
    import_file(page, sample_files["pdf"])
    wait_for_view(page, "comparison")
    assert count_highlights(page) > 0
```

### P0: 还原流程 (`test_restore.py`)

```python
def test_restore_from_workspace(page, workspace, sample_files):
    """脱敏 → 导出 → 还原 → 验证内容恢复"""
    # 1. 完成脱敏导出
    import_file(page, sample_files["xlsx"])
    wait_for_view(page, "comparison")
    output_path = click_export(page)
    # 2. 回到 dropzone
    click_back(page)
    wait_for_view(page, "dropzone")
    # 3. 点击"从工作区还原"
    click_restore_from_workspace(page, output_path)
    wait_for_view(page, "restore")
    # 4. 验证还原结果
    assert get_restore_match_count(page) > 0

def test_restore_ai_response(page, workspace, sample_files):
    """脱敏 → 模拟 AI 回复 → 还原"""
    import_file(page, sample_files["xlsx"])
    wait_for_view(page, "comparison")
    # 记录脱敏后的文本（含假数据）
    desensitized_text = get_desensitized_text(page)
    click_export(page)
    click_back(page)
    # 用脱敏后文本模拟 AI 回复
    click_restore_from_ai(page, desensitized_text)
    wait_for_view(page, "restore")
    assert get_restore_match_count(page) > 0
```

### P1: 工作区管理 (`test_workspace_crud.py`)

```python
def test_create_workspace(page):
    """Cmd+N 创建工作区"""
    count_before = get_workspace_count(page)
    page.keyboard.press("Meta+n")
    wait_for_workspace_count(page, count_before + 1)

def test_rename_workspace(page, workspace):
    """重命名工作区"""
    trigger_rename(page, workspace["id"])
    page.keyboard.type("新名称")
    page.keyboard.press("Enter")
    assert get_workspace_name(page, workspace["id"]) == "新名称"

def test_delete_workspace(page, workspace):
    """删除工作区"""
    count_before = get_workspace_count(page)
    trigger_delete(page, workspace["id"])
    confirm_dialog(page)
    wait_for_workspace_count(page, count_before - 1)

def test_clipboard_workspace(page):
    """粘贴文本创建剪贴板工作区"""
    click_create_clipboard_workspace(page)
    paste_text(page, "张三的手机号是13800138000")
    wait_for_view(page, "comparison")
    assert count_highlights(page) > 0
```

### P1: 列级规则 (`test_column_rules.py`)

```python
def test_column_inference(page, workspace, sample_files):
    """导入 xlsx → 验证列推断结果"""
    import_file(page, sample_files["xlsx"])
    wait_for_view(page, "comparison")
    inferences = get_column_inferences(page)
    assert len(inferences) > 0

def test_change_column_strategy(page, workspace, sample_files):
    """修改某列策略 → 验证该列重新脱敏"""
    import_file(page, sample_files["xlsx"])
    wait_for_view(page, "comparison")
    content_before = get_column_content(page, col=1)
    change_column_rule(page, col=1, strategy="Replace")
    wait_for_content_change(page, content_before)
    content_after = get_column_content(page, col=1)
    assert content_before != content_after
```

### P2: 批量处理 (`test_batch_processing.py`)

```python
def test_batch_import_and_export(page, workspace, sample_files):
    """批量导入 → 逐个处理 → 导出并下一个"""
    batch_files = list((sample_files["batch_dir"]).glob("*"))
    import_files(page, batch_files)
    # 验证队列显示
    assert get_queue_count(page) == len(batch_files)
    # 逐个处理
    for i in range(len(batch_files)):
        wait_for_view(page, "comparison")
        assert count_highlights(page) > 0
        if i < len(batch_files) - 1:
            click_export_and_next(page)
        else:
            click_export(page)
```

### P2: 字典与白名单 (`test_dict_whitelist.py`)

```python
def test_add_dict_entry_triggers_detection(page, workspace, sample_files):
    """添加字典条目 → 新增命中"""
    import_file(page, sample_files["xlsx"])
    wait_for_view(page, "comparison")
    count_before = count_highlights(page)
    add_dict_entry(page, text="某公司", type_name="OrgName")
    wait_for_highlight_count_change(page, count_before)
    assert count_highlights(page) > count_before

def test_whitelist_excludes_item(page, workspace, sample_files):
    """添加白名单 → 对应项不再高亮"""
    import_file(page, sample_files["xlsx"])
    wait_for_view(page, "comparison")
    first_item_text = get_first_highlight_text(page)
    count_before = count_highlights(page)
    add_whitelist_entry(page, first_item_text)
    wait_for_highlight_count_change(page, count_before)
    assert count_highlights(page) < count_before
```

## 测试运行方式

```bash
# 安装依赖
pip install playwright pytest
playwright install chromium

# 运行所有测试
python e2e/scripts/with_tauri.py -- pytest e2e/tests/ -v

# 只跑 P0 测试
python e2e/scripts/with_tauri.py -- pytest e2e/tests/ -v -m p0

# 只跑某个文件
python e2e/scripts/with_tauri.py -- pytest e2e/tests/test_basic_desensitize.py -v

# 失败时截图
python e2e/scripts/with_tauri.py -- pytest e2e/tests/ -v --screenshot on-failure

# 调试模式（测试后不关闭应用）
python e2e/scripts/with_tauri.py --keep-alive -- pytest e2e/tests/test_basic_desensitize.py -v -s
```

## UI 选择器策略

为了让测试稳定，需要在前端组件上添加 `data-testid` 属性：

```
关键元素的 data-testid 约定：

视图容器:
  data-testid="view-dropzone"
  data-testid="view-processing"
  data-testid="view-comparison"
  data-testid="view-restore"

操作按钮:
  data-testid="btn-export"
  data-testid="btn-export-next"
  data-testid="btn-back"
  data-testid="btn-restore-workspace"
  data-testid="btn-restore-ai"

面板:
  data-testid="panel-strategy"
  data-testid="panel-dict"
  data-testid="panel-whitelist"

工作区:
  data-testid="workspace-list"
  data-testid="workspace-item-{id}"

敏感高亮:
  data-testid="sensitive-highlight"

文件队列:
  data-testid="file-queue"
  data-testid="queue-item-{index}"
```

## dimkey-e2e Skill 设计

### 触发词
"测试"、"跑测试"、"E2E"、"验证功能"、"测一下"、"帮我测"、"写测试用例"

### 核心能力

1. **执行已有测试**
   - 用户: "跑一下基础脱敏测试"
   - Skill: 执行 `with_tauri.py -- pytest e2e/tests/test_basic_desensitize.py`

2. **从描述生成测试**
   - 用户: "写个测试：导入加密 Excel，输入密码，验证脱敏正常"
   - Skill: 生成 Playwright 测试脚本，保存到 `e2e/tests/`，然后执行

3. **组合场景编排**
   - 用户: "测试完整的脱敏→还原链路"
   - Skill: 串联 test_basic_desensitize + test_restore 相关用例

4. **失败诊断**
   - 测试失败时自动读取截图 + 日志，分析原因并建议修复

### Skill 内置知识
- Dimkey 视图状态机和切换条件
- data-testid 选择器约定
- helpers.py 中可用的封装函数
- 测试 fixtures 文件列表及其内容特征
- 常见失败模式（视图切换超时、元素未加载、异步检测未完成等）

## 前端改动要求

需要在现有 React 组件上补充 `data-testid` 属性。这是唯一需要修改现有代码的部分，改动量小且不影响功能。

## 后续扩展

1. **CI 集成** — GitHub Actions 中运行 E2E（需要 Tauri build + Playwright）
2. **视觉回归** — Playwright screenshot 对比，防止 UI 样式意外变化
3. **性能基线** — 记录处理时间，检测性能退化
4. **跨平台** — macOS + Windows 矩阵测试
