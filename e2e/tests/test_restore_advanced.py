"""P0/P2: 还原高级流程 — AI 回复还原、历史记录还原"""

import pytest

from utils.helpers import wait_for_view, take_diagnostic

pytestmark = pytest.mark.p0


class TestAiRestore:
    """R02: AI 回复还原 — 将 AI 回复中的脱敏占位符还原为真实数据"""

    def test_ai_restore_button_exists(self, page):
        """R02-1: AI 还原按钮在 dropzone 视图应可见"""
        wait_for_view(page, "dropzone", timeout=10_000)
        btn = page.locator('[data-testid="btn-restore-ai"]')
        assert btn.is_visible(), "AI 还原按钮应可见"

    def test_ai_restore_triggers_ipc(self, page):
        """R02-2: 点击 AI 还原应读取剪贴板并调用 restore_ai_response"""
        wait_for_view(page, "dropzone", timeout=10_000)

        # 先向剪贴板写入模拟的 AI 回复文本
        page.evaluate("""() => {
            // 覆盖 navigator.clipboard.readText 返回模拟文本
            navigator.clipboard.readText = async () =>
                '您好，人员1的手机号是15000000001，身份证号为110101********1234。';
        }""")

        # 清空 IPC 日志
        page.evaluate("window.__E2E_IPC_LOG__ = []")

        # 点击 AI 还原按钮
        btn = page.locator('[data-testid="btn-restore-ai"]')
        btn.click()
        page.wait_for_timeout(1500)

        # 验证 restore_ai_response 被调用
        logs = page.evaluate("window.__E2E_IPC_LOG__")
        restore_calls = [l for l in logs if l["cmd"] == "restore_ai_response"]
        assert len(restore_calls) >= 1, "应触发 restore_ai_response IPC 调用"

        # 验证传入的 aiText 参数包含模拟文本
        call_args = restore_calls[0].get("args", {})
        assert "aiText" in call_args, "应传入 aiText 参数"
        assert "人员1" in call_args["aiText"], f"aiText 应包含模拟文本: {call_args['aiText']}"

        take_diagnostic(page, "ai_restore_triggered")

    def test_ai_restore_empty_clipboard_shows_error(self, page):
        """R02-3: 剪贴板为空时应提示错误"""
        wait_for_view(page, "dropzone", timeout=10_000)

        # 覆盖 clipboard 返回空字符串
        page.evaluate("""() => {
            navigator.clipboard.readText = async () => '';
        }""")

        page.evaluate("window.__E2E_IPC_LOG__ = []")

        btn = page.locator('[data-testid="btn-restore-ai"]')
        btn.click()
        page.wait_for_timeout(1000)

        # 不应调用 restore_ai_response（空文本应被前端拦截）
        logs = page.evaluate("window.__E2E_IPC_LOG__")
        restore_calls = [l for l in logs if l["cmd"] == "restore_ai_response"]
        assert len(restore_calls) == 0, "空剪贴板不应触发 restore_ai_response"

        take_diagnostic(page, "ai_restore_empty_clipboard")


class TestHistoryRestore:
    """R04: 历史记录还原"""

    def test_workspace_restore_button_exists(self, page):
        """R04-1: 工作区还原按钮在 dropzone 视图应可见"""
        wait_for_view(page, "dropzone", timeout=10_000)
        btn = page.locator('[data-testid="btn-restore-workspace"]')
        assert btn.is_visible(), "工作区还原按钮应可见"

    def test_workspace_restore_button_clickable(self, page):
        """R04-2: 工作区还原按钮应可点击"""
        wait_for_view(page, "dropzone", timeout=10_000)
        btn = page.locator('[data-testid="btn-restore-workspace"]')
        assert btn.is_enabled(), "工作区还原按钮应可点击"
        take_diagnostic(page, "workspace_restore_ready")
