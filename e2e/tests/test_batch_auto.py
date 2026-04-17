"""批量全自动模式 E2E 测试"""
import pytest
from playwright.sync_api import Page, expect

E2E_URL = "http://127.0.0.1:1420"


def _wait_workspace_ready(page: Page):
    page.goto(E2E_URL)
    page.wait_for_selector("[data-testid='view-dropzone']", timeout=10000)
    page.click("text=E2E 测试")
    page.wait_for_selector("[data-testid='view-dropzone']", timeout=5000)


def test_batch_auto_happy_path(page: Page):
    """拖入 5 个文件 → 全自动 → 结果报告显示 5 成功"""
    page.add_init_script("""
        window.__E2E_IPC_OVERRIDES__ = {
            check_file_exists: () => false,
            import_file: async () => {
                await new Promise(r => setTimeout(r, 50));
                return {
                    type: 'Spreadsheet',
                    file_type: 'xlsx',
                    sheets: [{ name: 'Sheet1', headers: ['姓名'], rows: [[{ text: '张三' }]] }],
                };
            },
            detect_by_regex: async () => [],
            detect_by_ner: async () => {
                await new Promise(r => setTimeout(r, 80));
                return [{
                    id: 't1', text: '张三', sensitive_type: 'PersonName',
                    sheet_index: 0, row: 0, col: 0, confidence: 0.95, source: 'ner',
                }];
            },
            detect_by_dict: async () => [],
            apply_desensitize: async () => ({
                content: { type: 'Spreadsheet', file_type: 'xlsx', sheets: [{ name: 'Sheet1', headers: ['姓名'], rows: [[{ text: 'XX' }]] }] },
                mappings: [{ original_text: '张三', replaced_text: 'XX', strategy: 'Replace', sensitive_type: 'PersonName' }],
                summary: { total: 1, by_type: {} },
            }),
            export_file: async () => null,
            add_processing_record: async () => null,
        };
    """)
    _wait_workspace_ready(page)

    # 直接通过 store 注入 fileQueue（绕过真实 Tauri dragDrop）
    page.evaluate("""
        const store = window.__DIMKEY_STORE__.getState();
        const files = Array.from({length: 5}, (_, i) => ({
            id: `f${i}`, filePath: `/tmp/file${i}.xlsx`,
            fileName: `file${i}.xlsx`, status: 'pending',
        }));
        store.initFileQueue(files);
    """)

    page.wait_for_selector("[data-testid='batch-mode-selector']", timeout=5000)
    # auto 模式是默认；点击选择目录 → dialog mock 返回 /tmp/e2e-output
    page.click("[data-testid='btn-choose-dir']")
    page.wait_for_function(
        "() => document.querySelector(\"[data-testid='output-dir-section'] input\").value === '/tmp/e2e-output'",
        timeout=3000,
    )
    page.click("[data-testid='btn-start-batch']")

    # 进度条出现
    page.wait_for_selector("[data-testid='batch-progress']", timeout=3000)
    # 结果报告出现
    page.wait_for_selector("[data-testid='batch-result-report']", timeout=20000)

    confirmed_rows = page.locator("[data-testid='result-row-confirmed']")
    expect(confirmed_rows).to_have_count(5)


def test_batch_auto_abort(page: Page):
    """批量处理中点击中止 → 未开始的标记为 aborted"""
    page.add_init_script("""
        window.__E2E_IPC_OVERRIDES__ = {
            check_file_exists: () => false,
            import_file: async () => {
                await new Promise(r => setTimeout(r, 500));
                return { type: 'Spreadsheet', file_type: 'xlsx', sheets: [] };
            },
            detect_by_regex: async () => [],
            detect_by_ner: async () => [],
            detect_by_dict: async () => [],
            apply_desensitize: async () => ({ content: { type: 'Spreadsheet', file_type: 'xlsx', sheets: [] }, mappings: [], summary: { total: 0, by_type: {} } }),
            export_file: async () => null,
            add_processing_record: async () => null,
        };
        window.confirm = () => true;
    """)
    _wait_workspace_ready(page)

    page.evaluate("""
        const store = window.__DIMKEY_STORE__.getState();
        const files = Array.from({length: 10}, (_, i) => ({
            id: `f${i}`, filePath: `/tmp/f${i}.xlsx`, fileName: `f${i}.xlsx`, status: 'pending',
        }));
        store.initFileQueue(files);
    """)

    page.wait_for_selector("[data-testid='batch-mode-selector']")
    page.click("[data-testid='btn-choose-dir']")
    page.wait_for_function(
        "() => document.querySelector(\"[data-testid='output-dir-section'] input\").value === '/tmp/e2e-output'",
    )
    page.click("[data-testid='btn-start-batch']")
    page.wait_for_selector("[data-testid='batch-progress']")

    page.wait_for_timeout(400)
    page.click("[data-testid='btn-abort-batch']")

    page.wait_for_selector("[data-testid='batch-result-report']", timeout=10000)
    aborted_rows = page.locator("[data-testid='result-row-aborted']")
    expect(aborted_rows.first).to_be_visible()


def test_batch_auto_retry_failed(page: Page):
    """前两次 apply_desensitize 失败 → 第三次成功（重试后转为 confirmed）"""
    page.add_init_script("""
        window.__E2E_RETRY_COUNT__ = 0;
        window.__E2E_IPC_OVERRIDES__ = {
            check_file_exists: () => false,
            import_file: async () => ({ type: 'Spreadsheet', file_type: 'xlsx', sheets: [] }),
            detect_by_regex: async () => [],
            detect_by_ner: async () => [],
            detect_by_dict: async () => [],
            apply_desensitize: async () => {
                window.__E2E_RETRY_COUNT__++;
                if (window.__E2E_RETRY_COUNT__ <= 2) {
                    throw '模拟失败';
                }
                return { content: { type: 'Spreadsheet', file_type: 'xlsx', sheets: [] }, mappings: [], summary: { total: 0, by_type: {} } };
            },
            export_file: async () => null,
            add_processing_record: async () => null,
        };
    """)
    _wait_workspace_ready(page)

    page.evaluate("""
        const store = window.__DIMKEY_STORE__.getState();
        store.initFileQueue([
            { id: 'f0', filePath: '/tmp/f0.xlsx', fileName: 'f0.xlsx', status: 'pending' },
            { id: 'f1', filePath: '/tmp/f1.xlsx', fileName: 'f1.xlsx', status: 'pending' },
        ]);
    """)
    page.wait_for_selector("[data-testid='batch-mode-selector']")
    page.click("[data-testid='btn-choose-dir']")
    page.wait_for_function(
        "() => document.querySelector(\"[data-testid='output-dir-section'] input\").value === '/tmp/e2e-output'",
    )
    page.click("[data-testid='btn-start-batch']")

    page.wait_for_selector("[data-testid='batch-result-report']", timeout=10000)
    failed_rows = page.locator("[data-testid='result-row-failed']")
    expect(failed_rows).to_have_count(2)

    page.locator("[data-testid='btn-retry-file']").first.click()
    page.wait_for_selector("[data-testid='result-row-confirmed']", timeout=5000)
    expect(page.locator("[data-testid='result-row-confirmed']").first).to_be_visible()
