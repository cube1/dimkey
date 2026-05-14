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
        // 强制 i18n 使用中文（默认 detection 读取 navigator.language，headless chromium 为 en）
        localStorage.setItem('dimkey-lang', 'zh');

        // Mock Tauri IPC — 仅当不在真实 Tauri WebView 中时
        if (!window.__TAURI_INTERNALS__) {
            const callbacks = new Map();
            window.__TAURI_INTERNALS__ = {
                metadata: {
                    currentWindow: { label: 'main' },
                    currentWebview: { windowLabel: 'main', label: 'main' },
                },
                invoke: async (cmd, args) => {
                    // 记录 IPC 调用，供测试断言
                    if (!window.__E2E_IPC_LOG__) window.__E2E_IPC_LOG__ = [];
                    window.__E2E_IPC_LOG__.push({ cmd, args, ts: Date.now() });

                    // 动态覆盖：测试可通过 window.__E2E_IPC_OVERRIDES__ 自定义返回值
                    if (window.__E2E_IPC_OVERRIDES__ && cmd in window.__E2E_IPC_OVERRIDES__) {
                        const override = window.__E2E_IPC_OVERRIDES__[cmd];
                        return typeof override === 'function' ? override(args) : override;
                    }

                    const now = new Date().toISOString();
                    const defaults = {
                        'list_workspaces': [
                            { id: 'e2e-ws', name: 'E2E 测试', created_at: now, updated_at: now },
                            { id: 'e2e-ws-2', name: 'E2E 测试 2', created_at: now, updated_at: now },
                        ],
                        'get_workspace': {
                            workspace: {
                                id: args?.id || 'e2e-ws', name: 'E2E 测试', source: null,
                                created_at: now, updated_at: now,
                                strategies: {}, replace_style: 'Fake',
                                dict_entries: [], column_rules: {},
                                output_dir: null, consistency_mappings: [],
                                enabled_types: ['PersonName','Phone','IdCard','Email','Address','OrgName','BankCard','CreditCode'],
                                mode: 'Desensitize', whitelist: [], alias_groups: [],
                                replace_seed: 42, replace_counters: {},
                            },
                            history: [],
                        },
                        'create_workspace': { id: 'mock-ws-new', name: args?.name || '新工作区', created_at: now, updated_at: now, strategies: {}, dict_entries: [], column_rules: {}, output_dir: null, consistency_mappings: [], enabled_types: ['Phone','IdCard','Email'], mode: 'Desensitize', whitelist: [], alias_groups: [], replace_style: 'Fake', replace_seed: 42, replace_counters: {}, source: null },
                        'create_clipboard_workspace': { id: 'mock-clipboard-ws', name: args?.name || '粘贴板', created_at: now, updated_at: now, strategies: {}, dict_entries: [], column_rules: {}, output_dir: null, consistency_mappings: [], enabled_types: ['Phone','IdCard','Email'], mode: 'Desensitize', whitelist: [], alias_groups: [], replace_style: 'Fake', replace_seed: 42, replace_counters: {}, source: 'Clipboard' },
                        'rename_workspace': null,
                        'update_workspace': null,
                        'delete_workspace': null,
                        'restore_ai_response': { original_content: null, restored_content: null, matched_count: 3, restore_items: [], original_items: [], file_path: '' },
                        'restore_from_workspace': { original_content: null, restored_content: null, matched_count: 2, restore_items: [], original_items: [], file_path: '' },
                        'restore_processing': { original_content: null, restored_content: null, matched_count: 1, restore_items: [], original_items: [], file_path: '' },
                        'load_config': { strategies: {}, replace_style: 'Fake' },
                        'load_dict': [],
                        'get_builtin_dict': [],
                        'list_tasks': [],
                        'list_alias_groups': [],
                        'get_language': 'zh',
                        'get_analytics_enabled': false,
                        'set_analytics_enabled': null,
                        'set_language': null,
                        // license — Phase 12 默认值（Trial 30 天，无指纹冲突）
                        'license_get_state': { kind: 'Trial', days_remaining: 30 },
                        'license_get_fingerprint': 'a3f9c2110ab8d4e7f1c0a2b3d5e6f7a8',
                        'license_get_fingerprint_mismatch_hint': null,
                        'license_get_trial_info': { days_remaining: 30, expired: false },
                        'license_activate': { email: 'test@example.com', max_devices: 3, active_devices: 1, device_id: 'dev-mock' },
                        'license_deactivate_current': null,
                        'license_list_devices': [],
                        'license_deactivate_device': null,
                        'license_recover_email': null,
                        'license_open_purchase_page': null,
                        'save_config': null,
                        'save_dict': null,
                        'check_file_exists': true,
                        'plugin:dialog|open': (args) => {
                            if (args?.options?.directory) return '/tmp/e2e-output';
                            return null;
                        },
                    };
                    if (cmd in defaults) {
                        const val = defaults[cmd];
                        return typeof val === 'function' ? val(args) : val;
                    }
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

    # 等待 workspaces 加载完成（mock IPC 返回 list_workspaces 后 store 才更新）
    pg.wait_for_function("""() => {
        const store = window.__DIMKEY_STORE__;
        return store && store.getState().workspaces.length > 0;
    }""", timeout=10_000)

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
