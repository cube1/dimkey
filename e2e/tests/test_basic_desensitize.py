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

pytestmark = [pytest.mark.p0, pytest.mark.needs_backend]


class TestBasicDesensitize:
    """基础脱敏流程测试"""

    @pytest.mark.parametrize("file_type,filename,min_highlights", [
        ("xlsx", "sample.xlsx", 25),   # C01: 5人×5列
        ("csv", "sample.csv", 25),     # C02: 同 C01
        ("docx", "sample.docx", 8),    # C03: 姓名+手机+身份证等
        ("txt", "sample.txt", 8),      # C04: 手机+身份证+邮箱等
    ])
    def test_import_detect_and_verify_highlights(self, page, file_type, filename, min_highlights):
        """导入文件 → 等待识别完成 → 验证敏感高亮数量达到预期阈值"""
        fixture_path = get_fixture_path(filename)

        wait_for_view(page, "dropzone", timeout=10_000)
        import_file_via_ipc(page, fixture_path)
        wait_for_processing_done(page, timeout=60_000)

        highlights = count_highlights(page)
        assert highlights >= min_highlights, \
            f"{file_type} 文件高亮数应 ≥ {min_highlights}，实际: {highlights}"
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
