"""Dimkey E2E pytest fixtures"""

import os
from pathlib import Path

import pytest
from playwright.sync_api import sync_playwright

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent


@pytest.fixture(scope="session")
def browser():
    """会话级浏览器实例"""
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        yield browser
        browser.close()


@pytest.fixture
def page(browser):
    """每个测试独立的页面"""
    context = browser.new_context(
        viewport={"width": 1200, "height": 800},
    )
    # 在页面加载前：跳过弹窗 + mock Tauri IPC（让 React 在普通浏览器中不崩溃）
    context.add_init_script("""
        // 跳过匿名统计弹窗
        localStorage.setItem('analytics_consent_shown', '1');

        // Mock Tauri IPC — 仅当不在真实 Tauri WebView 中时
        if (!window.__TAURI_INTERNALS__) {
            const callbacks = new Map();
            window.__TAURI_INTERNALS__ = {
                metadata: {
                    currentWindow: { label: 'main' },
                    currentWebview: { windowLabel: 'main', label: 'main' },
                },
                invoke: async (cmd, args) => {
                    // 返回各命令的默认空值，让 React 不崩溃
                    const defaults = {
                        'list_workspaces': [{ id: 'e2e-ws', name: 'E2E 测试', created_at: Date.now(), updated_at: Date.now() }],
                        'get_workspace': {
                            workspace: {
                                id: 'e2e-ws', name: 'E2E 测试', source: null,
                                created_at: new Date().toISOString(),
                                updated_at: new Date().toISOString(),
                                strategies: {}, replace_style: 'fake',
                                dict_entries: [], column_rules: {},
                                output_dir: null, consistency_mappings: [],
                                enabled_types: ['PersonName','Phone','IdCard','Email','Address','OrgName','BankCard','CreditCode'],
                                mode: 'Desensitize', whitelist: [], alias_groups: [],
                            },
                            history: [],
                        },
                        'create_workspace': 'mock-ws-id',
                        'load_config': { strategies: {}, replace_style: 'fake' },
                        'load_dict': [],
                        'get_builtin_dict': [],
                        'list_tasks': [],
                        'list_alias_groups': [],
                        'get_language': 'zh',
                        'get_analytics_enabled': false,
                        'set_analytics_enabled': null,
                        'set_language': null,
                        'save_config': null,
                        'save_dict': null,
                        'check_file_exists': true,
                    };
                    if (cmd in defaults) return defaults[cmd];
                    // 未知命令返回 null
                    console.warn('[E2E mock] unhandled invoke:', cmd, args);
                    return null;
                },
                transformCallback: (callback, once) => {
                    const id = window.crypto.getRandomValues(new Uint32Array(1))[0];
                    callbacks.set(id, { callback, once });
                    return id;
                },
                unregisterCallback: (id) => { callbacks.delete(id); },
                runCallback: (id, data) => {
                    const entry = callbacks.get(id);
                    if (entry) {
                        if (entry.once) callbacks.delete(id);
                        entry.callback(data);
                    }
                },
                convertFileSrc: (path) => path,
            };
            // event plugin 需要 unregisterListener
            window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
                unregisterListener: () => {},
            };
        }
    """)
    pg = context.new_page()

    url = os.environ.get("DIMKEY_TEST_URL", "http://127.0.0.1:1420")
    pg.goto(url)
    pg.wait_for_load_state("networkidle")

    # 等待 React 渲染，自动选中第一个工作区进入 dropzone 视图
    pg.wait_for_selector('[data-testid="workspace-list"]', timeout=10_000)
    pg.evaluate("""async () => {
        const store = window.__DIMKEY_STORE__;
        if (store) {
            const state = store.getState();
            if (state.workspaces.length > 0 && !state.activeWorkspaceId) {
                await state.selectWorkspace(state.workspaces[0].id);
            }
        }
    }""")
    pg.wait_for_timeout(500)

    yield pg

    pg.close()
    context.close()


@pytest.fixture
def sample_files():
    """测试样本文件路径"""
    fixtures_dir = Path(__file__).parent.parent / "fixtures"
    return {
        "xlsx": str(fixtures_dir / "sample.xlsx"),
        "csv": str(fixtures_dir / "sample.csv"),
        "docx": str(fixtures_dir / "sample.docx"),
        "txt": str(fixtures_dir / "sample.txt"),
    }
