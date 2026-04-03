"""P1: 工作区管理 — 创建/重命名/删除"""

import pytest

from utils.helpers import (
    wait_for_view,
    get_workspace_count,
    take_diagnostic,
)

pytestmark = pytest.mark.p1


class TestWorkspaceCRUD:
    """工作区 CRUD 测试"""

    def test_create_workspace_via_shortcut(self, page):
        """Cmd+N 创建新工作区"""
        count_before = get_workspace_count(page)
        page.keyboard.press("Meta+n")
        page.wait_for_timeout(1000)
        count_after = get_workspace_count(page)
        assert count_after == count_before + 1, \
            f"创建后工作区数应 +1，之前: {count_before}，之后: {count_after}"
        take_diagnostic(page, "workspace_created")

    def test_workspace_list_visible(self, page):
        """工作区列表应可见"""
        workspace_list = page.locator('[data-testid="workspace-list"]')
        assert workspace_list.is_visible(), "工作区列表应可见"

    def test_strategy_panel_visible(self, page):
        """策略面板应可见"""
        panel = page.locator('[data-testid="panel-strategy"]')
        assert panel.is_visible(), "策略面板应可见"
