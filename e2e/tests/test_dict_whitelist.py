"""P2: 字典与白名单 — 添加条目验证检测变化"""

import pytest

from utils.helpers import (
    wait_for_view,
    wait_for_processing_done,
    count_highlights,
    take_diagnostic,
    get_fixture_path,
    import_file_via_ipc,
)

pytestmark = pytest.mark.p2


class TestDictWhitelist:
    """字典与白名单测试"""

    def test_dict_panel_visible(self, page):
        """字典面板应可见"""
        wait_for_view(page, "dropzone", timeout=10_000)
        panel = page.locator('[data-testid="panel-dict"]')
        assert panel.is_visible(), "字典面板应可见"

    @pytest.mark.skip(reason="白名单面板需要真实后端数据，mock 模式下不渲染")
    def test_whitelist_panel_exists(self, page):
        """白名单面板应存在（可能需滚动可见）"""
        wait_for_view(page, "dropzone", timeout=10_000)
        panel = page.locator('[data-testid="panel-whitelist"]')
        assert panel.count() > 0, "白名单面板应存在于 DOM 中"

    @pytest.mark.needs_backend
    def test_import_then_check_highlights(self, page):
        """导入文件后应有敏感高亮"""
        fixture_path = get_fixture_path("sample.txt")
        wait_for_view(page, "dropzone", timeout=10_000)
        import_file_via_ipc(page, fixture_path)
        wait_for_processing_done(page)

        highlights = count_highlights(page)
        assert highlights > 0
        take_diagnostic(page, "dict_whitelist_baseline")
