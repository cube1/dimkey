"""P0: 还原流程 — 脱敏后还原验证"""

import pytest
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from utils.helpers import (
    wait_for_view,
    wait_for_processing_done,
    click_back,
    click_restore_ai,
    click_restore_workspace,
    take_diagnostic,
    get_fixture_path,
    import_file_via_store,
)

pytestmark = pytest.mark.p0


class TestRestore:
    """还原流程测试"""

    def test_restore_buttons_visible_on_dropzone(self, page):
        """dropzone 视图应显示还原按钮"""
        wait_for_view(page, "dropzone", timeout=10_000)

        ai_btn = page.locator('[data-testid="btn-restore-ai"]')
        ws_btn = page.locator('[data-testid="btn-restore-workspace"]')

        assert ai_btn.is_visible(), "AI 还原按钮应可见"
        assert ws_btn.is_visible(), "工作区还原按钮应可见"

    def test_desensitize_then_back_to_dropzone(self, page):
        """脱敏完成后点返回 → 回到 dropzone"""
        fixture_path = get_fixture_path("sample.txt")
        wait_for_view(page, "dropzone", timeout=10_000)
        import_file_via_store(page, fixture_path)
        wait_for_processing_done(page)

        click_back(page)
        wait_for_view(page, "dropzone", timeout=10_000)
        take_diagnostic(page, "restore_back_to_dropzone")

    def test_ai_restore_button_clickable(self, page):
        """AI 还原按钮应可点击"""
        wait_for_view(page, "dropzone", timeout=10_000)
        ai_btn = page.locator('[data-testid="btn-restore-ai"]')
        assert ai_btn.is_enabled()
        take_diagnostic(page, "restore_ai_ready")
