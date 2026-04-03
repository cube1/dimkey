"""Dimkey E2E 测试通用操作封装"""

from pathlib import Path
from playwright.sync_api import Page

FIXTURES_DIR = Path(__file__).resolve().parent.parent / "fixtures"
OUTPUT_DIR = Path(__file__).resolve().parent.parent / "output"


def wait_for_view(page: Page, view_name: str, timeout: int = 30_000):
    """等待视图切换到指定状态

    view_name: empty | dropzone | processing | comparison | restore
    """
    page.wait_for_selector(f'[data-testid="view-{view_name}"]', timeout=timeout)


def wait_for_processing_done(page: Page, timeout: int = 60_000):
    """等待处理完成，从 processing 视图切换到 comparison 视图"""
    wait_for_view(page, "comparison", timeout=timeout)


def count_highlights(page: Page) -> int:
    """统计当前页面中的敏感高亮项数量"""
    return page.locator('[data-testid="sensitive-highlight"]').count()


def click_export(page: Page):
    """点击导出按钮"""
    page.click('[data-testid="btn-export"]')


def click_export_and_next(page: Page):
    """点击"导出并下一个"按钮"""
    page.click('[data-testid="btn-export-next"]')


def click_back(page: Page):
    """点击返回按钮"""
    page.click('[data-testid="btn-back"]')


def click_restore_ai(page: Page):
    """点击"从 AI 回复还原"按钮"""
    page.click('[data-testid="btn-restore-ai"]')


def click_restore_workspace(page: Page):
    """点击"从工作区还原"按钮"""
    page.click('[data-testid="btn-restore-workspace"]')


def get_workspace_count(page: Page) -> int:
    """获取工作区列表中的工作区数量"""
    return page.locator('[data-testid="workspace-list"] > *').count()


def take_diagnostic(page: Page, name: str):
    """截图保存到 output 目录"""
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    page.screenshot(path=str(OUTPUT_DIR / f"{name}.png"), full_page=True)


def get_fixture_path(filename: str) -> str:
    """获取 fixture 文件的绝对路径"""
    path = FIXTURES_DIR / filename
    if not path.exists():
        raise FileNotFoundError(f"Fixture 不存在: {path}")
    return str(path)


def import_file_via_store(page: Page, file_path: str):
    """通过暴露的 store 导入文件（绕过原生文件对话框）"""
    abs_path = str(Path(file_path).resolve())
    page.evaluate(f"""
        async () => {{
            const store = window.__DIMKEY_STORE__;
            if (store) {{
                await store.getState().handleFileDrop(['{abs_path}']);
            }}
        }}
    """)
