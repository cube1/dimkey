# UI E2E 测试模式（Playwright + IPC Mock）

## 环境

- Python venv: `e2e/.venv/`
- 依赖: playwright, pytest, openpyxl
- 需先启动 Vite: `TAURI_DEV_HOST=127.0.0.1 npm run dev`
- 运行在 mock 模式（无 Tauri 后端），conftest.py 自动注入 `__TAURI_INTERNALS__` mock

## 测试模板

```python
import pytest
from utils.helpers import wait_for_view, take_diagnostic

pytestmark = pytest.mark.p1  # p0/p1/p2

class TestScenarioName:
    def test_panel_visible(self, page):
        """面板应可见"""
        wait_for_view(page, "dropzone", timeout=10_000)
        panel = page.locator('[data-testid="panel-xxx"]')
        assert panel.is_visible()
```

需要后端的测试加 `@pytest.mark.needs_backend`，mock 模式下会被排除。

## 可用 helpers（`e2e/utils/helpers.py`）

| 函数 | 用途 |
|------|------|
| `wait_for_view(page, name)` | 等待视图切换 |
| `wait_for_processing_done(page)` | 等待处理完成（needs_backend） |
| `count_highlights(page)` | 统计高亮数（needs_backend） |
| `get_detected_items(page)` | 提取高亮文本（needs_backend） |
| `import_file_via_ipc(page, path)` | 导入文件（needs_backend） |
| `click_export/back/restore_*` | 按钮操作 |
| `take_diagnostic(page, name)` | 截图到 output/ |
| `get_fixture_path(filename)` | 获取 fixture 路径 |

## data-testid 选择器

```
视图:  view-empty, view-dropzone, view-processing, view-comparison, view-restore
按钮:  btn-export, btn-export-next, btn-back, btn-restore-ai, btn-restore-workspace
面板:  panel-strategy, panel-dict, panel-whitelist
列表:  workspace-list, file-queue
高亮:  sensitive-highlight
```

## 视图状态机

```
empty ──(选择工作区)──→ dropzone
dropzone ──(导入文件)──→ processing ──(完成)──→ comparison
comparison ──(返回)──→ dropzone
dropzone ──(还原)──→ restore ──(返回)──→ dropzone
```

## 执行命令

```bash
DIMKEY_E2E=1 DIMKEY_TEST_URL=http://127.0.0.1:1420 \
  e2e/.venv/bin/pytest e2e/tests/ -v -m "not needs_backend"
```
