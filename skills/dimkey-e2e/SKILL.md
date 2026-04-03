---
name: dimkey-e2e
description: Dimkey 应用 E2E 自动化测试工具。使用 Playwright + pytest 测试 Tauri WebView 界面。当用户提到"测试"、"跑测试"、"E2E"、"验证功能"、"测一下"、"帮我测"、"写测试用例"、"界面测试"、"自动化测试"、"回归测试"时触发。即使用户只说"测一下导入功能"这样简短的话也应触发。
---

# Dimkey E2E 测试

## 测试框架结构

```
e2e/
├── scripts/with_tauri.py    # Tauri 进程管理
├── fixtures/                 # 测试样本文件（xlsx/csv/docx/txt）
├── tests/
│   ├── conftest.py          # pytest fixtures（browser、page、workspace）
│   ├── test_basic_desensitize.py  # P0: 导入→识别→导出
│   ├── test_restore.py            # P0: 还原流程
│   ├── test_workspace_crud.py     # P1: 工作区管理
│   ├── test_column_rules.py       # P1: 列级规则
│   ├── test_batch_processing.py   # P2: 批量处理
│   └── test_dict_whitelist.py     # P2: 字典与白名单
├── utils/helpers.py         # 通用操作封装
└── output/                  # 截图和诊断
```

## 运行测试

```bash
# 需要先有 debug build 和前端 dev server
# cargo build (在 src-tauri/)
# npm run dev

# 跑所有测试
cd e2e && python -m pytest -v

# 只跑 P0 核心测试
cd e2e && python -m pytest -v -m p0

# 跑单个文件
cd e2e && python -m pytest tests/test_basic_desensitize.py -v

# 使用 with_tauri.py 管理完整生命周期
python e2e/scripts/with_tauri.py -- pytest e2e/tests/ -v
```

## 可用的 helpers

这些函数在 `e2e/utils/helpers.py` 中：

| 函数 | 用途 |
|------|------|
| `wait_for_view(page, name)` | 等待视图切换（empty/dropzone/processing/comparison/restore）|
| `wait_for_processing_done(page)` | 等待文件处理完成 |
| `count_highlights(page)` | 统计敏感高亮数量 |
| `click_export(page)` | 点击导出 |
| `click_export_and_next(page)` | 点击"导出并下一个" |
| `click_back(page)` | 点击返回 |
| `click_restore_ai(page)` | 点击 AI 还原 |
| `click_restore_workspace(page)` | 点击工作区还原 |
| `get_workspace_count(page)` | 获取工作区数量 |
| `take_diagnostic(page, name)` | 截图保存 |
| `get_fixture_path(filename)` | 获取样本文件路径 |
| `import_file_via_store(page, path)` | 通过 store 导入文件 |

## data-testid 选择器约定

```
视图:     view-empty, view-dropzone, view-processing, view-comparison, view-restore
按钮:     btn-export, btn-export-next, btn-back, btn-restore-ai, btn-restore-workspace, btn-export-restore
面板:     panel-strategy, panel-dict, panel-whitelist
列表:     workspace-list, file-queue
高亮项:   sensitive-highlight
```

## Dimkey 视图状态机

```
empty ──(选择工作区)──→ dropzone
dropzone ──(导入文件)──→ processing
processing ──(完成)──→ comparison
comparison ──(返回)──→ dropzone
dropzone ──(AI还原/工作区还原)──→ restore
restore ──(返回)──→ dropzone
```

## 编写新测试用例

1. 确定起始视图（大多数从 dropzone 开始）
2. 用 `import_file_via_store(page, path)` 导入文件
3. 用 `wait_for_view` 或 `wait_for_processing_done` 等待视图切换
4. 用 helpers 或 `page.click/fill/keyboard` 执行操作
5. 断言：高亮数量、视图状态、元素可见性
6. 用 `take_diagnostic` 截图

测试文件放 `e2e/tests/`，命名 `test_<场景>.py`，用 `pytestmark` 标记优先级。

## 文件导入方式

Tauri 使用原生文件对话框，Playwright 无法拦截。测试中通过暴露的 store 直接触发：

```python
import_file_via_store(page, file_path)
```

前提：DEV 模式下 store 已暴露到 `window.__DIMKEY_STORE__`。

## 常见问题排查

- **视图切换超时** — 检查应用是否启动、文件路径是否正确
- **高亮数为 0** — 检查 fixture 文件是否包含敏感信息
- **store 未暴露** — 确认 DEV 模式或 DIMKEY_E2E 环境变量已设置
- **导出测试** — 原生保存对话框需要通过 IPC 调用 export_file
