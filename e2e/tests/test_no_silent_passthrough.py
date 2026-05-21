"""P0 回归: 防"测试过 UI 没替换"静默 passthrough bug

## 用户痛点

"测试都通过了，但打开 UI 发现全部都没有替换。"

## 旧测试的盲区

`test_basic_desensitize.py` 等 P0 测试只断言 `data-testid="view-comparison"` 可见，
**从不读 store 验证 currentResult.content 是否真的与原文不同**。组件渲染了 ≠ 业务正确。

## 本文件的策略

**用 IPC override 完整 mock 一条脱敏管线**（解析 → 识别 → 脱敏），喂语义正确的返回值，
然后**通过 store 行为断言**验证前端管线把 IPC 结果**正确装配进 currentResult**。

能抓的 bug:
- 前端 hook 漏调 setCurrentResult / setCenterView
- enabledTypes 配置导致全部识别项被过滤（mergedItems 空 → emptyResult passthrough）
- summary.total 与 mappings 不一致
- IPC 调用顺序错（apply_desensitize 在 detect 之前）

不能抓的 bug（留给方向 ① Rust regression 测试 + 方向 ④ build smoke）:
- Rust 真后端返回的 content 结构错
- ONNX 模型在打包后丢失导致 NER 识别失败
"""

import pytest

from utils.helpers import (
    assert_desensitization_applied,
    assert_ipc_called,
    get_fixture_path,
    get_ipc_calls,
    get_store_state,
    import_file_via_ipc,
    reset_ipc_log,
    wait_for_view,
)

pytestmark = [pytest.mark.p0]


# 一个语义正确的"识别 + 脱敏"mock 管线 — 模拟真后端正常返回时的行为
def _install_full_pipeline_mock(page, sensitive_count: int = 5):
    """注入一条完整的、语义正确的 IPC override 链

    对应 processFileStandalone 走过的 invoke 序列:
      import_file → detect_by_regex/dict/ner → apply_desensitize → add_processing_record
    """
    page.evaluate("""(args) => {
        const { sensitiveCount } = args;
        const originalContent = {
            type: 'Document',
            file_name: 'mock.txt',
            file_type: 'Txt',
            paragraphs: Array.from({ length: sensitiveCount }, (_, i) => ({
                index: i,
                text: `张三的电话是 138${String(i).padStart(8, '0')}`,
                style: 'Normal',
            })),
        };
        const desensitizedContent = {
            type: 'Document',
            file_name: 'mock.txt',
            file_type: 'Txt',
            paragraphs: Array.from({ length: sensitiveCount }, (_, i) => ({
                index: i,
                text: `[姓名]的电话是 [手机号]`,
                style: 'Normal',
            })),
        };
        const items = Array.from({ length: sensitiveCount }, (_, i) => ({
            id: `item-${i}`,
            text: `138${String(i).padStart(8, '0')}`,
            sensitive_type: 'Phone',
            source: 'Regex',
            confidence: 0.99,
            start: 7, end: 18, row: i, col: 0, sheet_index: 0,
        }));
        const mappings = items.map((it, i) => ({
            original_text: it.text,
            replaced_text: '[手机号]',
            sensitive_type: 'Phone',
            strategy: 'Replace',
            occurrences: 1,
        }));

        window.__E2E_IPC_OVERRIDES__ = window.__E2E_IPC_OVERRIDES__ || {};
        Object.assign(window.__E2E_IPC_OVERRIDES__, {
            import_file: () => originalContent,
            detect_by_regex: () => items,
            detect_by_dict: () => [],
            detect_by_ner: () => [],
            detect_columns: () => [],
            apply_desensitize: () => ({
                content: desensitizedContent,
                mappings,
                summary: { total: sensitiveCount, by_type: { Phone: sensitiveCount } },
            }),
            add_processing_record: () => null,
        });
    }""", {"sensitiveCount": sensitive_count})


def _install_silent_passthrough_mock(page):
    """注入"静默 passthrough"bug 的 mock — apply_desensitize 返回原文不变

    用于反向验证 assert_desensitization_applied 真的能抓到 bug。
    这模拟的是: Rust 后端跑了但策略没生效 / 返回错结构。
    """
    page.evaluate("""() => {
        const originalContent = {
            type: 'Document',
            file_name: 'mock.txt',
            file_type: 'Txt',
            paragraphs: [{ index: 0, text: '张三的电话 13800000000', style: 'Normal' }],
        };
        const items = [{
            id: 'i1', text: '13800000000', sensitive_type: 'Phone',
            source: 'Regex', confidence: 0.99,
            start: 5, end: 16, row: 0, col: 0, sheet_index: 0,
        }];
        window.__E2E_IPC_OVERRIDES__ = window.__E2E_IPC_OVERRIDES__ || {};
        Object.assign(window.__E2E_IPC_OVERRIDES__, {
            import_file: () => originalContent,
            detect_by_regex: () => items,
            detect_by_dict: () => [],
            detect_by_ner: () => [],
            detect_columns: () => [],
            // bug: 报了 1 次替换但 content 与原文一模一样
            apply_desensitize: () => ({
                content: originalContent,
                mappings: [{
                    original_text: '13800000000', replaced_text: '13800000000',
                    sensitive_type: 'Phone', strategy: 'Replace', occurrences: 1,
                }],
                summary: { total: 1, by_type: { Phone: 1 } },
            }),
            add_processing_record: () => null,
        });
    }""")


class TestNoSilentPassthrough:
    """脱敏管线必须真把内容改了 — 不只是切到 comparison 视图"""

    def test_normal_pipeline_passes_assertions(self, page):
        """正向: 完整管线跑完后，行为断言全过"""
        wait_for_view(page, "dropzone", timeout=10_000)
        _install_full_pipeline_mock(page, sensitive_count=5)
        reset_ipc_log(page)

        import_file_via_ipc(page, get_fixture_path("sample.txt"))
        wait_for_view(page, "comparison", timeout=15_000)

        assert_desensitization_applied(page, min_replacements=5)
        assert_ipc_called(page, "import_file")
        assert_ipc_called(page, "apply_desensitize")
        assert_ipc_called(page, "add_processing_record")

    def test_silent_passthrough_is_caught(self, page):
        """反向: 后端"假装替换了"但 content 没变，必须被 helper 抓住

        这是用户痛点 #1 的精确复现 — 验证我们的断言体系真的有效。
        """
        wait_for_view(page, "dropzone", timeout=10_000)
        _install_silent_passthrough_mock(page)
        reset_ipc_log(page)

        import_file_via_ipc(page, get_fixture_path("sample.txt"))
        wait_for_view(page, "comparison", timeout=15_000)

        with pytest.raises(AssertionError, match="脱敏后内容与原文完全相同"):
            assert_desensitization_applied(page, min_replacements=1)

    def test_ipc_call_order_is_correct(self, page):
        """IPC 调用顺序必须是 import → detect → apply → add_record"""
        wait_for_view(page, "dropzone", timeout=10_000)
        _install_full_pipeline_mock(page, sensitive_count=3)
        reset_ipc_log(page)

        import_file_via_ipc(page, get_fixture_path("sample.txt"))
        wait_for_view(page, "comparison", timeout=15_000)

        cmds = [e["cmd"] for e in get_ipc_calls(page)]

        idx_import = cmds.index("import_file")
        idx_detect = cmds.index("detect_by_regex")
        idx_apply = cmds.index("apply_desensitize")
        idx_record = cmds.index("add_processing_record")

        assert idx_import < idx_detect < idx_apply < idx_record, (
            f"IPC 调用顺序错乱: {cmds}"
        )

    def test_summary_and_mappings_are_consistent(self, page):
        """summary.total 与 mappings 数量必须一致 — 否则还原会错位"""
        wait_for_view(page, "dropzone", timeout=10_000)
        _install_full_pipeline_mock(page, sensitive_count=7)
        reset_ipc_log(page)

        import_file_via_ipc(page, get_fixture_path("sample.txt"))
        wait_for_view(page, "comparison", timeout=15_000)

        state = get_store_state(page)
        assert state["summaryTotal"] == 7, f"summary.total 期望 7，实际 {state['summaryTotal']}"
        assert state["mappingCount"] == 7, f"mappings 期望 7，实际 {state['mappingCount']}"

    def test_processing_step_reaches_done(self, page):
        """processingStep 必须从 parsing→detecting→desensitizing→saving→done 全程走完"""
        wait_for_view(page, "dropzone", timeout=10_000)
        _install_full_pipeline_mock(page, sensitive_count=2)
        reset_ipc_log(page)

        import_file_via_ipc(page, get_fixture_path("sample.txt"))
        wait_for_view(page, "comparison", timeout=15_000)

        state = get_store_state(page)
        assert state["processingStep"] == "done", (
            f"processingStep 应为 done，实际 {state['processingStep']}"
        )

    def test_zero_sensitive_handled_correctly(self, page):
        """识别为 0 时仍应有 result（empty result），但不应误报为静默 passthrough"""
        wait_for_view(page, "dropzone", timeout=10_000)
        # mock: detect 返回空，不调 apply_desensitize（hook 内部短路）
        page.evaluate("""() => {
            const content = {
                type: 'Document', file_name: 'clean.txt', file_type: 'Txt',
                paragraphs: [{ index: 0, text: '这是一段没有敏感信息的文本', style: 'Normal' }],
            };
            window.__E2E_IPC_OVERRIDES__ = {
                import_file: () => content,
                detect_by_regex: () => [],
                detect_by_dict: () => [],
                detect_by_ner: () => [],
                detect_columns: () => [],
            };
        }""")
        reset_ipc_log(page)

        import_file_via_ipc(page, get_fixture_path("sample.txt"))
        wait_for_view(page, "comparison", timeout=15_000)

        state = get_store_state(page)
        assert state["hasResult"], "0 敏感项也应有 emptyResult，否则 UI 渲染会空"
        assert state["summaryTotal"] == 0
        # 0 项情况不调 apply_desensitize（这是优化，不是 bug）
        assert len(get_ipc_calls(page, "apply_desensitize")) == 0
