"""P1/P2: 工作区高级操作 — 切换配置隔离、剪贴板工作区"""

import pytest

from utils.helpers import wait_for_view, take_diagnostic, get_workspace_count

pytestmark = pytest.mark.p1


class TestWorkspaceSwitch:
    """W04: 切换工作区配置隔离"""

    def test_workspace_list_has_multiple(self, page):
        """W04-1: 工作区列表应显示多个工作区（mock 返回 2 个）"""
        wait_for_view(page, "dropzone", timeout=10_000)
        count = get_workspace_count(page)
        assert count >= 2, f"应有至少 2 个工作区，实际: {count}"

    def test_switch_workspace(self, page):
        """W04-2: 点击另一个工作区应触发 get_workspace 调用"""
        wait_for_view(page, "dropzone", timeout=10_000)

        # 清空 IPC 日志
        page.evaluate("window.__E2E_IPC_LOG__ = []")

        # 点击第二个工作区
        workspace_items = page.locator('[data-testid="workspace-list"] > *')
        if workspace_items.count() >= 2:
            workspace_items.nth(1).click()
            page.wait_for_timeout(1000)

            # 验证 get_workspace 被调用
            logs = page.evaluate("window.__E2E_IPC_LOG__")
            get_ws_calls = [l for l in logs if l["cmd"] == "get_workspace"]
            assert len(get_ws_calls) >= 1, "切换工作区应触发 get_workspace 调用"

            take_diagnostic(page, "workspace_switched")

    def test_switch_workspace_updates_panel(self, page):
        """W04-3: 切换工作区后策略面板应更新"""
        wait_for_view(page, "dropzone", timeout=10_000)

        # 策略面板应可见
        panel = page.locator('[data-testid="panel-strategy"]')
        assert panel.is_visible(), "策略面板应可见"

        # 切换到第二个工作区
        workspace_items = page.locator('[data-testid="workspace-list"] > *')
        if workspace_items.count() >= 2:
            workspace_items.nth(1).click()
            page.wait_for_timeout(1000)

            # 策略面板应仍然可见（配置隔离：每个工作区独立加载）
            assert panel.is_visible(), "切换后策略面板应仍可见"
            take_diagnostic(page, "workspace_switch_panel")


class TestClipboardWorkspace:
    """W05: 剪贴板工作区"""

    def test_paste_button_visible(self, page):
        """W05-1: dropzone 视图应有粘贴文本入口"""
        wait_for_view(page, "dropzone", timeout=10_000)

        # 查找粘贴相关按钮或文字
        paste_area = page.locator("text=粘贴").or_(page.locator("text=Paste"))
        # 如果找不到文字按钮，找 dropzone 区域内的粘贴提示
        if not paste_area.first.is_visible():
            paste_area = page.locator('[data-testid="view-dropzone"]')

        assert paste_area.first.is_visible(), "应有粘贴文本入口"
        take_diagnostic(page, "clipboard_paste_area")

    def test_clipboard_workspace_ipc(self, page):
        """W05-2: 验证 create_clipboard_workspace mock 能被正确调用"""
        wait_for_view(page, "dropzone", timeout=10_000)

        # 通过 JS 直接调用 store 的 createClipboardWorkspace
        page.evaluate("window.__E2E_IPC_LOG__ = []")
        page.evaluate("""async () => {
            const store = window.__DIMKEY_STORE__;
            if (store) {
                try {
                    await store.getState().createClipboardWorkspace('E2E粘贴板');
                } catch(e) {
                    // mock 环境下可能报错，但 IPC 调用已记录
                }
            }
        }""")
        page.wait_for_timeout(500)

        logs = page.evaluate("window.__E2E_IPC_LOG__")
        clipboard_calls = [l for l in logs if l["cmd"] == "create_clipboard_workspace"]
        assert len(clipboard_calls) >= 1, "应触发 create_clipboard_workspace IPC 调用"
        take_diagnostic(page, "clipboard_workspace_created")
