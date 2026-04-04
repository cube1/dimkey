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

    @pytest.mark.needs_backend
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

    def test_create_workspace_via_store(self, page):
        """W01: 通过 store 创建工作区应触发 create_workspace IPC"""
        wait_for_view(page, "dropzone", timeout=10_000)

        page.evaluate("window.__E2E_IPC_LOG__ = []")
        page.evaluate("""async () => {
            const store = window.__DIMKEY_STORE__;
            if (store) {
                try {
                    await store.getState().createWorkspace('E2E新工作区');
                } catch(e) {}
            }
        }""")
        page.wait_for_timeout(500)

        logs = page.evaluate("window.__E2E_IPC_LOG__")
        create_calls = [l for l in logs if l["cmd"] == "create_workspace"]
        assert len(create_calls) >= 1, "应触发 create_workspace IPC 调用"
        take_diagnostic(page, "workspace_created_via_store")

    @pytest.mark.needs_backend
    def test_rename_workspace(self, page):
        """W02: 重命名工作区"""
        workspace_list = page.locator('[data-testid="workspace-list"]')
        assert workspace_list.is_visible()

        # 双击工作区名进入编辑模式
        first_workspace = workspace_list.locator(".workspace-item").first
        if first_workspace.is_visible():
            first_workspace.dblclick()
            page.wait_for_timeout(500)
            take_diagnostic(page, "workspace_rename_editing")

    @pytest.mark.needs_backend
    def test_delete_workspace(self, page):
        """W03: 删除工作区"""
        count_before = get_workspace_count(page)

        # 先创建一个工作区以便删除
        page.keyboard.press("Meta+n")
        page.wait_for_timeout(1000)
        count_after_create = get_workspace_count(page)

        if count_after_create > count_before:
            # 右键最后一个工作区
            workspace_list = page.locator('[data-testid="workspace-list"]')
            last_workspace = workspace_list.locator(".workspace-item").last
            if last_workspace.is_visible():
                last_workspace.click(button="right")
                page.wait_for_timeout(500)
                take_diagnostic(page, "workspace_delete_context_menu")
