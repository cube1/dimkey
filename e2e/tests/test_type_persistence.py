"""P1: 类型过滤 UI 持久化 — enabled_types 通过 store 操作验证"""

import pytest

from utils.helpers import wait_for_view, take_diagnostic

pytestmark = pytest.mark.p1


class TestEnabledTypesPersistence:
    """T13: workspace 持久化 enabled_types — store 层面验证"""

    def test_initial_enabled_types_loaded(self, page):
        """T13-1: 初始加载后 store 中应有 enabled_types"""
        wait_for_view(page, "dropzone", timeout=10_000)

        enabled = page.evaluate("""() => {
            const store = window.__DIMKEY_STORE__;
            if (!store) return null;
            const state = store.getState();
            return state.activeWorkspaceData?.workspace?.enabled_types || null;
        }""")

        assert enabled is not None, "store 中应有 enabled_types"
        assert len(enabled) >= 4, f"应至少启用 4 种类型，实际: {len(enabled)}"
        assert "Phone" in enabled, "Phone 应在启用类型中"
        assert "Email" in enabled, "Email 应在启用类型中"

    def test_update_enabled_types_triggers_ipc(self, page):
        """T13-2: 更新 enabled_types 应触发 update_workspace IPC 调用"""
        wait_for_view(page, "dropzone", timeout=10_000)

        # 清空 IPC 日志
        page.evaluate("window.__E2E_IPC_LOG__ = []")

        # 通过 store 更新 enabled_types（移除 Phone）
        page.evaluate("""async () => {
            const store = window.__DIMKEY_STORE__;
            if (store) {
                const state = store.getState();
                const current = state.activeWorkspaceData?.workspace?.enabled_types || [];
                const newTypes = current.filter(t => t !== 'Phone');
                try {
                    await state.updateEnabledTypes(newTypes);
                } catch(e) {
                    // mock 环境下 update_workspace 返回 null，后续 get_workspace 会被调用
                }
            }
        }""")
        page.wait_for_timeout(500)

        # 验证 update_workspace 被调用
        logs = page.evaluate("window.__E2E_IPC_LOG__")
        update_calls = [l for l in logs if l["cmd"] == "update_workspace"]
        assert len(update_calls) >= 1, "更新 enabled_types 应触发 update_workspace"

        take_diagnostic(page, "enabled_types_updated")

    def test_enabled_types_roundtrip(self, page):
        """T13-3: 更新 → 重新加载 → 验证 store 状态"""
        wait_for_view(page, "dropzone", timeout=10_000)

        # 自定义 get_workspace 返回修改后的 enabled_types
        page.evaluate("""() => {
            window.__E2E_IPC_OVERRIDES__ = window.__E2E_IPC_OVERRIDES__ || {};
            window.__E2E_IPC_OVERRIDES__['get_workspace'] = (args) => ({
                workspace: {
                    id: args?.id || 'e2e-ws', name: 'E2E 测试', source: null,
                    created_at: new Date().toISOString(),
                    updated_at: new Date().toISOString(),
                    strategies: {}, replace_style: 'Fake',
                    dict_entries: [], column_rules: {},
                    output_dir: null, consistency_mappings: [],
                    enabled_types: ['Phone', 'Email'],  // 仅保留 2 种
                    mode: 'Desensitize', whitelist: [], alias_groups: [],
                    replace_seed: 42, replace_counters: {},
                },
                history: [],
            });
        }""")

        # 重新选择工作区触发加载
        page.evaluate("""async () => {
            const store = window.__DIMKEY_STORE__;
            if (store) {
                try {
                    await store.getState().selectWorkspace('e2e-ws');
                } catch(e) {}
            }
        }""")
        page.wait_for_timeout(1000)

        # 验证 store 中 enabled_types 已更新
        enabled = page.evaluate("""() => {
            const store = window.__DIMKEY_STORE__;
            if (!store) return null;
            return store.getState().wsData?.workspace?.enabled_types || null;
        }""")

        assert enabled is not None, "重新加载后应有 enabled_types"
        assert len(enabled) == 2, f"应仅保留 2 种类型，实际: {len(enabled)}"
        assert "Phone" in enabled
        assert "Email" in enabled

        # 清理 override
        page.evaluate("delete window.__E2E_IPC_OVERRIDES__['get_workspace']")

        take_diagnostic(page, "enabled_types_roundtrip")
