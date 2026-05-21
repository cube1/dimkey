"""Dimkey E2E 测试通用操作封装"""

from pathlib import Path
from playwright.sync_api import Page

FIXTURES_DIR = Path(__file__).resolve().parent.parent / "fixtures"
OUTPUT_DIR = Path(__file__).resolve().parent.parent / "output"


def wait_for_view(page: Page, view_name: str, timeout: int = 30_000):
    """等待视图切换到指定状态

    view_name: empty | dropzone | processing | comparison | restore
    """
    page.wait_for_selector(f'[data-testid="view-{view_name}"]', timeout=timeout)


def wait_for_processing_done(page: Page, timeout: int = 60_000):
    """等待处理完成，从 processing 视图切换到 comparison 视图"""
    wait_for_view(page, "comparison", timeout=timeout)


def count_highlights(page: Page) -> int:
    """统计当前页面中的敏感高亮项数量"""
    return page.locator('[data-testid="sensitive-highlight"]').count()


def click_export(page: Page):
    """点击导出按钮"""
    page.click('[data-testid="btn-export"]')


def click_export_and_next(page: Page):
    """点击"导出并下一个"按钮"""
    page.click('[data-testid="btn-export-next"]')


def click_back(page: Page):
    """点击返回按钮"""
    page.click('[data-testid="btn-back"]')


def click_restore_ai(page: Page):
    """点击"从 AI 回复还原"按钮"""
    page.click('[data-testid="btn-restore-ai"]')


def click_restore_workspace(page: Page):
    """点击"从工作区还原"按钮"""
    page.click('[data-testid="btn-restore-workspace"]')


def get_workspace_count(page: Page) -> int:
    """获取工作区列表中的工作区数量"""
    return page.locator('[data-testid="workspace-list"] > *').count()


def take_diagnostic(page: Page, name: str):
    """截图保存到 output 目录"""
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    page.screenshot(path=str(OUTPUT_DIR / f"{name}.png"), full_page=True)


def get_fixture_path(filename: str) -> str:
    """获取 fixture 文件的绝对路径"""
    path = FIXTURES_DIR / filename
    if not path.exists():
        raise FileNotFoundError(f"Fixture 不存在: {path}")
    return str(path)


def import_file_via_ipc(page: Page, file_path: str):
    """通过暴露的 processFileStandalone 导入文件并触发完整处理流程

    调用 DEV 模式下暴露的 window.__DIMKEY_PROCESS_FILE__，
    触发：解析 → 识别（regex/dict/NER）→ 脱敏 → UI 更新
    """
    abs_path = str(Path(file_path).resolve())
    page.evaluate("""
        async (filePath) => {
            const processFile = window.__DIMKEY_PROCESS_FILE__;
            if (!processFile) {
                throw new Error('processFile 未暴露到 window，请确认 DEV 模式');
            }
            await processFile(filePath);
        }
    """, abs_path)


def import_text_via_clipboard(page: Page, text: str):
    """通过剪贴板粘贴文本导入（不需要文件对话框）

    调用 DEV 模式下暴露的 window.__DIMKEY_PROCESS_TEXT__。
    """
    page.evaluate("""
        async (content) => {
            const processText = window.__DIMKEY_PROCESS_TEXT__;
            if (!processText) {
                throw new Error('processClipboardText 未暴露到 window，请确认 DEV 模式');
            }
            await processText(content);
        }
    """, text)


def get_detected_items(page: Page) -> list[str]:
    """提取页面上所有敏感高亮项的原始文本"""
    elements = page.locator('[data-testid="sensitive-highlight"]').all()
    return [el.text_content().strip().replace('\xa0', ' ') for el in elements if el.text_content()]


# ============================================================
# 行为断言 helpers — 从 store 读真实 state 验证业务真的发生了
# 解决"组件渲染了但功能没真做"的盲区
# ============================================================


def get_store_state(page: Page) -> dict:
    """读取 zustand store 的当前 state 快照"""
    return page.evaluate("""() => {
        const store = window.__DIMKEY_STORE__;
        if (!store) throw new Error('store 未暴露');
        const s = store.getState();
        return {
            hasFileContent: s.currentFileContent !== null,
            hasResult: s.currentResult !== null,
            sensitiveCount: (s.currentSensitiveItems || []).length,
            rawCount: (s.rawSensitiveItems || []).length,
            summaryTotal: s.currentResult ? s.currentResult.summary.total : 0,
            summaryByType: s.currentResult ? s.currentResult.summary.by_type : {},
            mappingCount: s.currentResult ? s.currentResult.mappings.length : 0,
            processingStep: s.processingStep,
            centerView: s.centerView,
            originalContent: s.currentFileContent,
            desensitizedContent: s.currentResult ? s.currentResult.content : null,
        };
    }""")


def _flatten_text(content: dict | None) -> str:
    """把 FileContent 拍平成纯文本字符串，方便整体比对"""
    if not content:
        return ""
    if content.get("type") == "Spreadsheet":
        parts: list[str] = []
        for sheet in content.get("sheets", []):
            for row in sheet.get("rows", []):
                for cell in row:
                    if isinstance(cell, dict):
                        parts.append(str(cell.get("text", "")))
                    else:
                        parts.append(str(cell or ""))
        return "\n".join(parts)
    if content.get("type") == "Document":
        return "\n".join(p.get("text", "") for p in content.get("paragraphs", []))
    return ""


def assert_desensitization_applied(page: Page, min_replacements: int = 1):
    """断言脱敏真的发生了 — 不只是组件渲染

    验证三件事（任何一个不满足 = 用户打开 UI 看到的就是没替换）:
    1. store.currentResult 不为 null（脱敏跑完了）
    2. summary.total >= min_replacements（确实替换了内容）
    3. 脱敏后文本 ≠ 原文本（替换不是空操作）
    """
    state = get_store_state(page)
    assert state["hasResult"], (
        f"脱敏结果为 null — UI 渲染了但脱敏没跑或失败。"
        f"step={state['processingStep']}, view={state['centerView']}"
    )
    assert state["summaryTotal"] >= min_replacements, (
        f"脱敏 summary.total={state['summaryTotal']}，期望 >= {min_replacements}。"
        f"识别项数={state['sensitiveCount']}（识别有但替换没发生 = 策略错误或脱敏管线断了）"
    )
    original = _flatten_text(state["originalContent"])
    desensitized = _flatten_text(state["desensitizedContent"])
    assert original and desensitized, "原文或脱敏后内容为空"
    assert original != desensitized, (
        "脱敏后内容与原文完全相同 — 静默 passthrough bug。"
        "summary 报了替换数但实际 content 没变，前端渲染原文给用户看 = 用户痛点 #1"
    )


def assert_ipc_called(page: Page, cmd: str, min_times: int = 1):
    """断言某个 Tauri IPC 命令被调用过 — 验证用户操作真的触发了后端"""
    log = page.evaluate("() => window.__E2E_IPC_LOG__ || []")
    matches = [entry for entry in log if entry.get("cmd") == cmd]
    assert len(matches) >= min_times, (
        f"IPC '{cmd}' 调用次数 {len(matches)} < 期望 {min_times}。"
        f"已记录调用: {[e.get('cmd') for e in log[-10:]]}"
    )


def get_ipc_calls(page: Page, cmd: str | None = None) -> list[dict]:
    """获取所有（或指定 cmd 的）IPC 调用记录"""
    log = page.evaluate("() => window.__E2E_IPC_LOG__ || []")
    if cmd:
        return [entry for entry in log if entry.get("cmd") == cmd]
    return log


def reset_ipc_log(page: Page):
    """清空 IPC 调用日志（在用户操作前调用，避免被 setup 阶段的调用污染）"""
    page.evaluate("() => { window.__E2E_IPC_LOG__ = []; }")
