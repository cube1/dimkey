"""批量全自动模式 E2E 测试"""
import pytest
from playwright.sync_api import Page, expect

E2E_URL = "http://127.0.0.1:1420"


def _wait_workspace_ready(page: Page):
    page.goto(E2E_URL)
    page.wait_for_selector("[data-testid='workspace-list']", timeout=10000)
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


# ============================================================
# B06-B13：批量全自动模式补充用例
# ============================================================


def _select_output_dir_and_start(page: Page):
    """选择输出目录 → 等待 /tmp/e2e-output 回填 → 点击开始批量"""
    page.wait_for_selector("[data-testid='batch-mode-selector']", timeout=5000)
    page.click("[data-testid='btn-choose-dir']")
    page.wait_for_function(
        "() => document.querySelector(\"[data-testid='output-dir-section'] input\").value === '/tmp/e2e-output'",
        timeout=3000,
    )
    page.click("[data-testid='btn-start-batch']")


def test_batch_auto_zero_sensitive_silent_confirm(page: Page):
    """B06：0 敏感项文件在全自动中 confirmed 且不调 apply_desensitize/export_file"""
    page.add_init_script("""
        window.__E2E_APPLY_CALLS__ = 0;
        window.__E2E_EXPORT_CALLS__ = 0;
        window.__E2E_IPC_OVERRIDES__ = {
            check_file_exists: () => false,
            import_file: async () => ({
                type: 'Document',
                file_name: 'note.txt',
                file_type: 'Txt',
                paragraphs: [{ text: '无敏感信息', style: null }],
            }),
            detect_by_regex: async () => [],
            detect_by_ner: async () => [],
            detect_by_dict: async () => [],
            apply_desensitize: async () => {
                window.__E2E_APPLY_CALLS__++;
                return { content: { type: 'Document', file_name: 'note.txt', file_type: 'Txt', paragraphs: [] }, mappings: [], summary: { total: 0, by_type: {} } };
            },
            export_file: async () => { window.__E2E_EXPORT_CALLS__++; return null; },
            add_processing_record: async () => null,
        };
    """)
    _wait_workspace_ready(page)

    # 2 个文件：绕过"单文件不进批量"的约束，同时验证"0 敏感项短路"在批量里起作用
    page.evaluate("""
        const store = window.__DIMKEY_STORE__.getState();
        store.initFileQueue([
            { id: 'f0', filePath: '/tmp/note_a.txt', fileName: 'note_a.txt', status: 'pending' },
            { id: 'f1', filePath: '/tmp/note_b.txt', fileName: 'note_b.txt', status: 'pending' },
        ]);
    """)

    _select_output_dir_and_start(page)
    page.wait_for_selector("[data-testid='batch-result-report']", timeout=20000)

    expect(page.locator("[data-testid='result-row-confirmed']")).to_have_count(2)

    # apply_desensitize / export_file 都不应被调用（0 敏感项短路）
    apply_calls = page.evaluate("window.__E2E_APPLY_CALLS__")
    export_calls = page.evaluate("window.__E2E_EXPORT_CALLS__")
    assert apply_calls == 0, f"0 敏感项时 apply_desensitize 不应被调用，实际 {apply_calls}"
    assert export_calls == 0, f"0 敏感项时 export_file 不应被调用，实际 {export_calls}"

    # 每个文件 sensitiveCount=0 且 outputPath 为空/未设置
    queue = page.evaluate("window.__DIMKEY_STORE__.getState().fileQueue")
    for f in queue:
        assert f.get("status") == "confirmed", f"{f.get('fileName')} 状态应为 confirmed"
        assert f.get("sensitiveCount", 0) == 0, f"{f.get('fileName')} sensitiveCount 应为 0"
        assert not f.get("outputPath"), f"{f.get('fileName')} 不应有 outputPath（0 敏感项不导出）"


def test_batch_auto_mixed_formats(page: Page):
    """B07：混合格式批量（xlsx+csv+docx+txt）4 个全部成功，前 3 个导出第 4 个 0 敏感不导出"""
    page.add_init_script("""
        window.__E2E_EXPORT_CALLS__ = 0;
        const importByExt = (filePath) => {
            if (filePath.endsWith('.xlsx')) return { type: 'Spreadsheet', file_name: 'a.xlsx', file_type: 'xlsx',
                sheets: [{ name: 'Sheet1', headers: ['姓名'], rows: [[{ text: '张三' }]], row_count: 1 }] };
            if (filePath.endsWith('.csv')) return { type: 'Spreadsheet', file_name: 'b.csv', file_type: 'csv',
                sheets: [{ name: 'Sheet1', headers: ['姓名'], rows: [[{ text: '李四' }]], row_count: 1 }] };
            if (filePath.endsWith('.docx')) return { type: 'Document', file_name: 'c.docx', file_type: 'docx',
                paragraphs: [{ text: '王五是经理', style: null }] };
            return { type: 'Document', file_name: 'd.txt', file_type: 'Txt',
                paragraphs: [{ text: '无敏感', style: null }] };
        };
        window.__E2E_IPC_OVERRIDES__ = {
            check_file_exists: () => false,
            import_file: async ({filePath}) => importByExt(filePath),
            detect_by_regex: async () => [],
            detect_by_dict: async () => [],
            // 前 3 个格式返 1 项，txt 返 0 项
            detect_by_ner: async ({content}) => {
                if (content.file_type === 'Txt') return [];
                return [{ id: 'n1', text: '名', sensitive_type: 'PersonName',
                    sheet_index: 0, row: 0, col: 0, start: 0, end: 1, confidence: 0.9, source: 'ner' }];
            },
            apply_desensitize: async ({content}) => ({
                content, mappings: [{ original_text: '名', replaced_text: '*', strategy: 'Mask', sensitive_type: 'PersonName' }],
                summary: { total: 1, by_type: { PersonName: 1 } },
            }),
            export_file: async () => { window.__E2E_EXPORT_CALLS__++; return null; },
            add_processing_record: async () => null,
        };
    """)
    _wait_workspace_ready(page)

    page.evaluate("""
        const store = window.__DIMKEY_STORE__.getState();
        store.initFileQueue([
            { id: 'f0', filePath: '/tmp/a.xlsx', fileName: 'a.xlsx', status: 'pending' },
            { id: 'f1', filePath: '/tmp/b.csv',  fileName: 'b.csv',  status: 'pending' },
            { id: 'f2', filePath: '/tmp/c.docx', fileName: 'c.docx', status: 'pending' },
            { id: 'f3', filePath: '/tmp/d.txt',  fileName: 'd.txt',  status: 'pending' },
        ]);
    """)

    _select_output_dir_and_start(page)
    page.wait_for_selector("[data-testid='batch-result-report']", timeout=20000)

    expect(page.locator("[data-testid='result-row-confirmed']")).to_have_count(4)

    export_calls = page.evaluate("window.__E2E_EXPORT_CALLS__")
    assert export_calls == 3, f"前 3 个文件应调 export_file，第 4 个 0 敏感不导出，实际调用 {export_calls}"

    queue = page.evaluate("window.__DIMKEY_STORE__.getState().fileQueue")
    by_name = {f["fileName"]: f for f in queue}
    for name in ("a.xlsx", "b.csv", "c.docx"):
        assert by_name[name].get("outputPath"), f"{name} 应有 outputPath"
        assert by_name[name].get("sensitiveCount") == 1, f"{name} sensitiveCount 应为 1"
    assert by_name["d.txt"].get("sensitiveCount", 0) == 0, "d.txt sensitiveCount 应为 0"
    assert not by_name["d.txt"].get("outputPath"), "d.txt 不应有 outputPath"


def test_batch_auto_encrypted_skip(page: Page):
    """B08：加密文件标记 failed 且 errorMessage=hook.encryptedSkipped 文案；row-failed 有重试按钮"""
    page.add_init_script("""
        window.__E2E_IPC_OVERRIDES__ = {
            check_file_exists: () => false,
            // parseEncryptedError 识别 "ENCRYPTED:<ext>" 前缀
            import_file: async () => { throw 'ENCRYPTED:xlsx'; },
            detect_by_regex: async () => [],
            detect_by_ner: async () => [],
            detect_by_dict: async () => [],
            apply_desensitize: async () => ({ content: {}, mappings: [], summary: { total: 0, by_type: {} } }),
            export_file: async () => null,
            add_processing_record: async () => null,
        };
    """)
    _wait_workspace_ready(page)

    page.evaluate("""
        const store = window.__DIMKEY_STORE__.getState();
        store.initFileQueue([
            { id: 'e0', filePath: '/tmp/enc1.xlsx', fileName: 'enc1.xlsx', status: 'pending' },
            { id: 'e1', filePath: '/tmp/enc2.xlsx', fileName: 'enc2.xlsx', status: 'pending' },
        ]);
    """)

    _select_output_dir_and_start(page)
    page.wait_for_selector("[data-testid='batch-result-report']", timeout=20000)

    expect(page.locator("[data-testid='result-row-failed']")).to_have_count(2)
    expect(page.locator("[data-testid='btn-retry-file']")).to_have_count(2)

    queue = page.evaluate("window.__DIMKEY_STORE__.getState().fileQueue")
    expected_msg = "文件已加密，批量模式下已跳过"  # hook.encryptedSkipped (zh)
    for f in queue:
        assert f.get("status") == "failed", f"{f.get('fileName')} 状态应为 failed"
        assert f.get("errorMessage") == expected_msg, (
            f"{f.get('fileName')} errorMessage 应为加密跳过文案，实际 {f.get('errorMessage')!r}"
        )


def test_batch_auto_concurrency_cap(page: Page):
    """B09：5 个文件批量处理时，同时 processing 的数量不超过 MAX_CONCURRENCY=3"""
    page.add_init_script("""
        window.__E2E_CONCURRENT__ = 0;
        window.__E2E_PEAK__ = 0;
        window.__E2E_IPC_OVERRIDES__ = {
            check_file_exists: () => false,
            import_file: async () => {
                window.__E2E_CONCURRENT__++;
                window.__E2E_PEAK__ = Math.max(window.__E2E_PEAK__, window.__E2E_CONCURRENT__);
                await new Promise(r => setTimeout(r, 200));
                window.__E2E_CONCURRENT__--;
                return { type: 'Spreadsheet', file_name: 'x.xlsx', file_type: 'xlsx',
                    sheets: [{ name: 'S', headers: [], rows: [], row_count: 0 }] };
            },
            detect_by_regex: async () => [],
            detect_by_ner: async () => [],
            detect_by_dict: async () => [],
            apply_desensitize: async () => ({ content: {}, mappings: [], summary: { total: 0, by_type: {} } }),
            export_file: async () => null,
            add_processing_record: async () => null,
        };
    """)
    _wait_workspace_ready(page)

    page.evaluate("""
        const store = window.__DIMKEY_STORE__.getState();
        const files = Array.from({length: 5}, (_, i) => ({
            id: `f${i}`, filePath: `/tmp/c${i}.xlsx`, fileName: `c${i}.xlsx`, status: 'pending',
        }));
        store.initFileQueue(files);
    """)

    _select_output_dir_and_start(page)
    page.wait_for_selector("[data-testid='batch-result-report']", timeout=20000)

    expect(page.locator("[data-testid='result-row-confirmed']")).to_have_count(5)

    peak = page.evaluate("window.__E2E_PEAK__")
    assert peak <= 3, f"并发峰值应 ≤ 3（MAX_CONCURRENCY），实际观测到 {peak}"
    assert peak >= 2, f"并发峰值应 ≥ 2（证明确实并发，而非串行），实际观测到 {peak}"


def test_batch_auto_spot_check_enters_comparison(page: Page):
    """B10：批量完成后点击 btn-view-result → centerView='comparison' + toast 含只读抽查文案"""
    page.add_init_script("""
        window.__E2E_IPC_OVERRIDES__ = {
            check_file_exists: () => false,
            import_file: async () => ({ type: 'Spreadsheet', file_name: 'x.xlsx', file_type: 'xlsx',
                sheets: [{ name: 'S', headers: ['name'], rows: [[{ text: '张三' }]], row_count: 1 }] }),
            detect_by_regex: async () => [],
            detect_by_dict: async () => [],
            detect_by_ner: async () => [{ id: 't1', text: '张三', sensitive_type: 'PersonName',
                sheet_index: 0, row: 0, col: 0, start: 0, end: 2, confidence: 0.95, source: 'ner' }],
            apply_desensitize: async ({content}) => ({
                content,
                mappings: [{ original_text: '张三', replaced_text: 'XX', strategy: 'Replace', sensitive_type: 'PersonName' }],
                summary: { total: 1, by_type: { PersonName: 1 } },
            }),
            export_file: async () => null,
            add_processing_record: async () => null,
        };
    """)
    _wait_workspace_ready(page)

    page.evaluate("""
        const store = window.__DIMKEY_STORE__.getState();
        store.initFileQueue([
            { id: 'f0', filePath: '/tmp/x1.xlsx', fileName: 'x1.xlsx', status: 'pending' },
            { id: 'f1', filePath: '/tmp/x2.xlsx', fileName: 'x2.xlsx', status: 'pending' },
        ]);
    """)

    _select_output_dir_and_start(page)
    page.wait_for_selector("[data-testid='batch-result-report']", timeout=20000)
    expect(page.locator("[data-testid='result-row-confirmed']")).to_have_count(2)

    # 点击结果报告中的"查看"按钮（只对 confirmed 且 result+recordId 齐全的行渲染）
    page.locator("[data-testid='btn-view-result']").first.click()

    # centerView 切换到 comparison
    page.wait_for_function(
        "() => window.__DIMKEY_STORE__.getState().centerView === 'comparison'",
        timeout=3000,
    )

    # comparison 视图的 viewonly-badge 可见（只读抽查模式的持久视觉标记；避开 toast 的 fade-out）
    expect(page.locator("[data-testid='viewonly-badge']")).to_be_visible(timeout=3000)


def test_batch_auto_single_file_skips_batch_selector(page: Page):
    """B11：拖入 1 个文件时不触发 BatchModeSelector（设计意图：单文件走单文件流程）"""
    page.add_init_script("""
        window.__E2E_IPC_OVERRIDES__ = {
            check_file_exists: () => false,
            import_file: async () => ({ type: 'Spreadsheet', file_name: 's.xlsx', file_type: 'xlsx',
                sheets: [{ name: 'S', headers: [], rows: [], row_count: 0 }] }),
        };
    """)
    _wait_workspace_ready(page)

    page.evaluate("""
        const store = window.__DIMKEY_STORE__.getState();
        store.initFileQueue([
            { id: 'f0', filePath: '/tmp/s.xlsx', fileName: 's.xlsx', status: 'pending' },
        ]);
    """)

    # 单文件不应出现 batch-mode-selector（断言不可见）
    page.wait_for_timeout(500)  # 给 React 渲染的余地
    assert page.locator("[data-testid='batch-mode-selector']").count() == 0, (
        "fileQueue.length=1 时不应渲染 BatchModeSelector"
    )

    # batchSession 应为 null（未进入批量会话）
    phase = page.evaluate("window.__DIMKEY_STORE__.getState().batchSession")
    assert phase is None, f"单文件 initFileQueue 不应开启 batchSession，实际 {phase!r}"

    # dropzone 视图应保留（走单文件流程）
    expect(page.locator("[data-testid='view-dropzone']")).to_be_visible()


def test_batch_auto_mid_batch_strategy_snapshot(page: Page):
    """B12：批量中途改 workspace 快照，已启动的 worker 保留旧快照，后启动的 worker 用新快照"""
    page.add_init_script("""
        window.__E2E_ENABLED_LENGTHS__ = [];
        window.__E2E_IPC_OVERRIDES__ = {
            check_file_exists: () => false,
            import_file: async () => ({ type: 'Spreadsheet', file_name: 'x.xlsx', file_type: 'xlsx',
                sheets: [{ name: 'S', headers: [], rows: [], row_count: 0 }] }),
            // 记录每次调用时看到的 enabledTypes 长度；200ms 延迟制造并发窗口
            detect_by_regex: async ({enabledTypes}) => {
                window.__E2E_ENABLED_LENGTHS__.push(enabledTypes.length);
                await new Promise(r => setTimeout(r, 200));
                return [];
            },
            detect_by_ner: async () => [],
            detect_by_dict: async () => [],
            apply_desensitize: async () => ({ content: {}, mappings: [], summary: { total: 0, by_type: {} } }),
            export_file: async () => null,
            add_processing_record: async () => null,
        };
    """)
    _wait_workspace_ready(page)

    # 初始 enabled_types 长度=2；注入 5 个文件
    page.evaluate("""
        const store = window.__DIMKEY_STORE__;
        const cur = store.getState().activeWorkspaceData;
        store.setState({
            activeWorkspaceData: { ...cur, workspace: { ...cur.workspace, enabled_types: ['Phone', 'IdCard'] } },
        });
        const files = Array.from({length: 5}, (_, i) => ({
            id: `f${i}`, filePath: `/tmp/m${i}.xlsx`, fileName: `m${i}.xlsx`, status: 'pending',
        }));
        store.getState().initFileQueue(files);
    """)

    _select_output_dir_and_start(page)

    # 等到 regex 已记录 3 个调用（首批 3 个 worker 都已进入 worker 拿完旧快照）
    page.wait_for_function("() => window.__E2E_ENABLED_LENGTHS__.length >= 3", timeout=5000)

    # 此刻再改 enabled_types；前 3 个 worker 已持有旧快照（options.enabledTypes）继续跑，
    # 后续 worker（idx=3,4）在进入 worker 时 getState() 读新快照
    page.evaluate("""
        const store = window.__DIMKEY_STORE__;
        const cur = store.getState().activeWorkspaceData;
        store.setState({
            activeWorkspaceData: { ...cur, workspace: { ...cur.workspace, enabled_types: ['Phone', 'IdCard', 'Email', 'PersonName'] } },
        });
    """)

    page.wait_for_selector("[data-testid='batch-result-report']", timeout=20000)

    lengths = page.evaluate("window.__E2E_ENABLED_LENGTHS__")
    assert len(lengths) == 5, f"应有 5 次 detect_by_regex 调用，实际 {lengths}"
    # 前 3 次用旧快照（长度 2）
    assert lengths[:3] == [2, 2, 2], f"首批 3 个 worker 应持有旧快照 len=2，实际 {lengths[:3]}"
    # 后 2 次用新快照（长度 4）
    assert lengths[3:] == [4, 4], f"后续 worker 应读到新快照 len=4，实际 {lengths[3:]}"


def test_batch_auto_all_failed(page: Page):
    """B13：全部失败时结果报告渲染 5 行 failed + 每行有重试按钮；汇总 toast 显示 0/5/0"""
    page.add_init_script("""
        window.__E2E_IPC_OVERRIDES__ = {
            check_file_exists: () => false,
            import_file: async () => { throw '文件损坏'; },
            detect_by_regex: async () => [],
            detect_by_ner: async () => [],
            detect_by_dict: async () => [],
            apply_desensitize: async () => ({ content: {}, mappings: [], summary: { total: 0, by_type: {} } }),
            export_file: async () => null,
            add_processing_record: async () => null,
        };
    """)
    _wait_workspace_ready(page)

    page.evaluate("""
        const store = window.__DIMKEY_STORE__.getState();
        const files = Array.from({length: 5}, (_, i) => ({
            id: `f${i}`, filePath: `/tmp/bad${i}.xlsx`, fileName: `bad${i}.xlsx`, status: 'pending',
        }));
        store.initFileQueue(files);
    """)

    _select_output_dir_and_start(page)
    page.wait_for_selector("[data-testid='batch-result-report']", timeout=20000)

    expect(page.locator("[data-testid='result-row-failed']")).to_have_count(5)
    expect(page.locator("[data-testid='btn-retry-file']")).to_have_count(5)

    # 结果报告中的汇总段落（持久，比 toast 更稳；toast 文本也是同一份，这里选持久版本）
    expect(page.locator("[data-testid='batch-result-report']")).to_contain_text("成功 0 · 失败 5 · 取消 0")
