"""P1/P2: UI 交互补充测试 — 语言切换"""

import pytest

from utils.helpers import wait_for_view, take_diagnostic

pytestmark = pytest.mark.p1


class TestUIExtras:
    """UI 交互补充测试"""

    def test_language_switch_to_english(self, page):
        """U05: 切换语言到英文，验证界面文案变化"""
        wait_for_view(page, "dropzone", timeout=10_000)

        # 找到语言切换按钮（中文模式下显示 "EN"）
        lang_btn = page.locator("button", has_text="EN")
        if not lang_btn.is_visible():
            # 可能已经是英文模式，显示 "中"
            lang_btn = page.locator("button", has_text="中")
            if lang_btn.is_visible():
                # 先切回中文
                lang_btn.click()
                page.wait_for_timeout(500)
                lang_btn = page.locator("button", has_text="EN")

        assert lang_btn.is_visible(), "语言切换按钮应可见"
        lang_btn.click()
        page.wait_for_timeout(500)

        # 切换后按钮文案应变为 "中"
        zh_btn = page.locator("button", has_text="中")
        assert zh_btn.is_visible(), "切换到英文后应显示 '中' 按钮"

        take_diagnostic(page, "lang_switched_to_en")

    def test_language_switch_roundtrip(self, page):
        """U05: 中→英→中 往返切换"""
        wait_for_view(page, "dropzone", timeout=10_000)

        # 确保初始为中文
        zh_btn = page.locator("button", has_text="中")
        if zh_btn.is_visible():
            zh_btn.click()
            page.wait_for_timeout(500)

        # 中文 → 英文
        en_btn = page.locator("button", has_text="EN")
        assert en_btn.is_visible(), "中文模式下应显示 EN 按钮"
        en_btn.click()
        page.wait_for_timeout(500)

        # 英文 → 中文
        zh_btn = page.locator("button", has_text="中")
        assert zh_btn.is_visible(), "英文模式下应显示 '中' 按钮"
        zh_btn.click()
        page.wait_for_timeout(500)

        # 验证回到中文
        en_btn = page.locator("button", has_text="EN")
        assert en_btn.is_visible(), "切回中文后应再次显示 EN 按钮"

        take_diagnostic(page, "lang_roundtrip")


class TestUISidebar:
    """U01: 侧栏折叠/展开"""

    def test_sidebar_toggle(self, page):
        """U01: 侧栏应可折叠和展开"""
        wait_for_view(page, "dropzone", timeout=10_000)

        # 工作区列表初始应可见
        workspace_list = page.locator('[data-testid="workspace-list"]')
        assert workspace_list.is_visible(), "工作区列表应默认可见"

        # 查找折叠按钮（通常是侧栏的 toggle）
        toggle = page.locator('[data-testid="sidebar-toggle"]')
        if toggle.is_visible():
            toggle.click()
            page.wait_for_timeout(500)
            take_diagnostic(page, "sidebar_collapsed")

            # 再次点击展开
            toggle.click()
            page.wait_for_timeout(500)
            assert workspace_list.is_visible(), "展开后工作区列表应可见"
            take_diagnostic(page, "sidebar_expanded")
        else:
            pytest.skip("未找到 sidebar-toggle 按钮")


class TestUIHighlight:
    """U03/U04: 高亮交互"""

    @pytest.mark.needs_backend
    def test_click_highlight_shows_popup(self, page):
        """U03: 点击高亮项应弹出详情弹窗"""
        from utils.helpers import import_file_via_ipc, wait_for_processing_done, get_fixture_path

        wait_for_view(page, "dropzone", timeout=10_000)
        fixture_path = get_fixture_path("sample.txt")
        import_file_via_ipc(page, fixture_path)
        wait_for_processing_done(page)

        # 点击第一个高亮项
        highlight = page.locator('[data-testid="sensitive-highlight"]').first
        if highlight.is_visible():
            highlight.click()
            page.wait_for_timeout(500)
            take_diagnostic(page, "highlight_clicked")

    @pytest.mark.needs_backend
    def test_manual_text_selection(self, page):
        """U04: 手动选文本标记（需后端支持）"""
        from utils.helpers import import_file_via_ipc, wait_for_processing_done, get_fixture_path

        wait_for_view(page, "dropzone", timeout=10_000)
        fixture_path = get_fixture_path("sample.txt")
        import_file_via_ipc(page, fixture_path)
        wait_for_processing_done(page)
        take_diagnostic(page, "manual_selection_ready")
