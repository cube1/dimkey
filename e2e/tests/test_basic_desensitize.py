"""P0: 基础脱敏流程 — 导入→识别→验证高亮→导出"""

import pytest

from utils.helpers import (
    wait_for_view,
    wait_for_processing_done,
    count_highlights,
    take_diagnostic,
    get_fixture_path,
    import_file_via_ipc,
)

pytestmark = pytest.mark.p0


class TestBasicDesensitize:
    """基础脱敏流程测试"""

    @pytest.mark.parametrize("file_type,filename", [
        ("xlsx", "sample.xlsx"),
        ("csv", "sample.csv"),
        ("docx", "sample.docx"),
        ("txt", "sample.txt"),
    ])
    def test_import_detect_and_verify_highlights(self, page, file_type, filename):
        """导入文件 → 等待识别完成 → 验证敏感高亮出现"""
        fixture_path = get_fixture_path(filename)

        wait_for_view(page, "dropzone", timeout=10_000)
        import_file_via_ipc(page, fixture_path)
        wait_for_processing_done(page, timeout=60_000)

        highlights = count_highlights(page)
        assert highlights > 0, f"{file_type} 文件应检测到敏感信息，实际高亮数: {highlights}"
        take_diagnostic(page, f"desensitize_{file_type}")

    def test_comparison_view_has_content(self, page):
        """comparison 视图应存在且可见"""
        fixture_path = get_fixture_path("sample.txt")
        wait_for_view(page, "dropzone", timeout=10_000)
        import_file_via_ipc(page, fixture_path)
        wait_for_processing_done(page)

        comparison = page.locator('[data-testid="view-comparison"]')
        assert comparison.is_visible()

    def test_export_button_available(self, page):
        """脱敏完成后导出按钮应可用"""
        fixture_path = get_fixture_path("sample.csv")
        wait_for_view(page, "dropzone", timeout=10_000)
        import_file_via_ipc(page, fixture_path)
        wait_for_processing_done(page)

        export_btn = page.locator('[data-testid="btn-export"]')
        assert export_btn.is_visible()
        assert export_btn.is_enabled()
