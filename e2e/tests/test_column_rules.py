"""P1: 列级规则 — 导入 xlsx 后验证列推断"""

import pytest

from utils.helpers import (
    wait_for_view,
    wait_for_processing_done,
    count_highlights,
    take_diagnostic,
    get_fixture_path,
    import_file_via_ipc,
)

pytestmark = pytest.mark.p1


class TestColumnRules:
    """列级规则测试"""

    def test_xlsx_shows_highlights(self, page):
        """导入 xlsx → 应出现敏感高亮"""
        fixture_path = get_fixture_path("sample.xlsx")
        wait_for_view(page, "dropzone", timeout=10_000)
        import_file_via_ipc(page, fixture_path)
        wait_for_processing_done(page)

        highlights = count_highlights(page)
        assert highlights > 0, "xlsx 文件应检测到列级敏感信息"
        take_diagnostic(page, "column_rules_xlsx")
