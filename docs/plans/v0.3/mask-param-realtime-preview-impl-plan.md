# 遮罩参数实时预览 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复 Mask 遮罩参数调整后对比视图不更新的 bug，并增加策略变更的视觉反馈。

**Architecture:** 新增 Rust 后端命令 `clear_type_consistency_mappings` 实现按类型精准清除一致性映射缓存。前端在策略参数变更时调用该命令使缓存失效，确保 `reDesensitizeWithFilteredItems` 重新计算。同时在重新脱敏成功后添加 toast 反馈。

**Tech Stack:** Rust (Tauri v2 command), TypeScript/React (Zustand store, react-hot-toast)

---

### Task 1: Rust 后端 — 新增 `clear_type_consistency_mappings` 命令

**Files:**
- Modify: `src-tauri/src/commands/workspace.rs:555-570` (在 `clear_consistency_mappings` 之后添加)
- Modify: `src-tauri/src/lib.rs:14-20` (import 新命令)
- Modify: `src-tauri/src/lib.rs:124` (注册新命令)

**Step 1: 在 `workspace.rs` 中添加新命令**

在 `clear_consistency_mappings` 函数之后（第 570 行之后）添加：

```rust
/// 清除工作区中指定类型的一致性替换映射
#[tauri::command]
pub async fn clear_type_consistency_mappings(
    workspace_id: String,
    sensitive_type_key: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let path = get_workspace_path(&app_handle, &workspace_id)?;
    if !path.exists() {
        return Err("工作区不存在".to_string());
    }

    let mut data = read_workspace_data(&path)?;
    data.workspace.consistency_mappings
        .retain(|m| m.sensitive_type_key != sensitive_type_key);
    data.workspace.updated_at = chrono_now();
    write_workspace_data(&path, &data)
}
```

**Step 2: 在 `lib.rs` 中注册新命令**

修改 `src-tauri/src/lib.rs` 第 14-20 行的 import，添加 `clear_type_consistency_mappings`：

```rust
use commands::workspace::{
    create_workspace, create_clipboard_workspace, list_workspaces, get_workspace,
    update_workspace, delete_workspace, rename_workspace, add_processing_record,
    update_processing_record_mappings, delete_processing_record,
    restore_processing, restore_from_workspace, restore_ai_response,
    clear_consistency_mappings, clear_type_consistency_mappings,
};
```

在 `invoke_handler` 的命令列表中（第 124 行 `clear_consistency_mappings` 之后）添加：

```rust
            clear_consistency_mappings,
            clear_type_consistency_mappings,
```

**Step 3: 运行 Rust 类型检查**

Run: `cd src-tauri && cargo check`
Expected: 编译通过，无错误

**Step 4: Commit**

```bash
git add src-tauri/src/commands/workspace.rs src-tauri/src/lib.rs
git commit -m "feat: 新增按类型清除一致性映射的后端命令"
```

---

### Task 2: 前端 Store — 新增 `clearTypeConsistencyMappings` 方法

**Files:**
- Modify: `src/stores/workspaceStore.ts:98-99` (类型声明)
- Modify: `src/stores/workspaceStore.ts:281-291` (实现)

**Step 1: 在 `WorkspaceState` 接口中添加类型声明**

在 `src/stores/workspaceStore.ts` 第 99 行（`clearConsistencyMappings` 之后）添加：

```typescript
  // --- 一致性映射 ---
  clearConsistencyMappings: () => Promise<void>;
  clearTypeConsistencyMappings: (typeKey: string) => Promise<void>;
```

**Step 2: 在 store 实现中添加方法**

在 `src/stores/workspaceStore.ts` 第 291 行（`clearConsistencyMappings` 实现之后）添加：

```typescript
  clearTypeConsistencyMappings: async (typeKey: string) => {
    const id = get().activeWorkspaceId;
    if (!id) return;
    try {
      await invoke("clear_type_consistency_mappings", { workspaceId: id, sensitiveTypeKey: typeKey });
    } catch (e) {
      console.error("清除类型一致性映射失败:", e);
    }
  },
```

注意：此处不需要 `refreshActiveWorkspace()`，因为紧接着的 `reDesensitizeWithFilteredItems` 会重新写入新的映射。

**Step 3: 运行前端类型检查**

Run: `npx tsc --noEmit`
Expected: 无类型错误

**Step 4: Commit**

```bash
git add src/stores/workspaceStore.ts
git commit -m "feat: store 新增 clearTypeConsistencyMappings 方法"
```

---

### Task 3: RulesSection — 策略变更时清除对应类型映射

**Files:**
- Modify: `src/components/StrategyPanel/RulesSection.tsx:16-21` (读取新方法)
- Modify: `src/components/StrategyPanel/RulesSection.tsx:47-67` (handleStrategyChange)
- Modify: `src/components/StrategyPanel/RulesSection.tsx:91-112` (handleMaskParamChange)

**Step 1: 从 store 读取新方法**

在第 20 行添加 `clearTypeConsistencyMappings`：

```typescript
  const clearConsistencyMappings = useWorkspaceStore((s) => s.clearConsistencyMappings);
  const clearTypeConsistencyMappings = useWorkspaceStore((s) => s.clearTypeConsistencyMappings);
```

**Step 2: 修改 `handleStrategyChange` 为 async 并调用清除映射**

将第 47-67 行替换为：

```typescript
  const handleStrategyChange = async (typeKey: string, value: string) => {
    let strategy: Strategy;
    if (value === "Mask") {
      strategy = { Mask: { keep_prefix: 3, keep_suffix: 4 } };
    } else if (value === "Generalize") {
      strategy = "Generalize";
    } else {
      strategy = { Replace: { style: currentReplaceStyle } };
    }
    const newStrategies = { ...strategies, [typeKey]: strategy };
    // 乐观更新 store
    if (wsData) {
      useWorkspaceStore.setState({
        activeWorkspaceData: {
          ...wsData,
          workspace: { ...wsData.workspace, strategies: newStrategies },
        },
      });
    }
    // 清除该类型的一致性映射（确保重新脱敏用新策略）
    await clearTypeConsistencyMappings(typeKey);
    debouncedSave(newStrategies);
  };
```

**Step 3: 修改 `handleMaskParamChange` 为 async 并调用清除映射**

将第 91-112 行替换为：

```typescript
  const handleMaskParamChange = async (
    typeKey: string,
    field: "keep_prefix" | "keep_suffix",
    val: number
  ) => {
    const current = strategies[typeKey];
    if (typeof current === "object" && "Mask" in current) {
      const strategy: Strategy = {
        Mask: { ...current.Mask, [field]: Math.max(0, val) },
      };
      const newStrategies = { ...strategies, [typeKey]: strategy };
      if (wsData) {
        useWorkspaceStore.setState({
          activeWorkspaceData: {
            ...wsData,
            workspace: { ...wsData.workspace, strategies: newStrategies },
          },
        });
      }
      // 清除该类型的一致性映射（确保重新脱敏用新参数）
      await clearTypeConsistencyMappings(typeKey);
      debouncedSave(newStrategies);
    }
  };
```

**Step 4: 运行前端类型检查**

Run: `npx tsc --noEmit`
Expected: 无类型错误

**Step 5: Commit**

```bash
git add src/components/StrategyPanel/RulesSection.tsx
git commit -m "fix: 策略变更时清除对应类型的一致性映射"
```

---

### Task 4: 重新脱敏增加成功 toast 反馈

**Files:**
- Modify: `src/hooks/useAutoDesensitize.ts:861-873` (reDesensitizeWithFilteredItems 的 try 块)

**Step 1: 在 `reDesensitizeWithFilteredItems` 成功后添加 toast**

将第 861-873 行的 try/catch 块修改为：

```typescript
    try {
      const result = await invoke<DesensitizeResult>("apply_desensitize", {
        content,
        items: filtered,
        strategies,
        workspaceId: ws.id,
      });
      store.setCurrentSensitiveItems(filtered);
      store.setCurrentResult(result);
      toast.success("脱敏策略已更新");
    } catch (err) {
      const message = typeof err === "string" ? err : "重新脱敏失败";
      toast.error(message);
    }
```

**Step 2: 运行前端类型检查**

Run: `npx tsc --noEmit`
Expected: 无类型错误

**Step 3: Commit**

```bash
git add src/hooks/useAutoDesensitize.ts
git commit -m "fix: 重新脱敏成功后显示 toast 反馈"
```

---

### Task 5: 端到端验证

**Step 1: 启动开发环境**

Run: `cargo tauri dev`
Expected: 应用启动成功

**Step 2: 手动验证 Mask 参数调整**

1. 拖入一个含 IP 地址的文件
2. 等待脱敏完成进入对比视图
3. 在右侧面板找到 IP 地址的 Mask 策略
4. 修改 `keep_prefix` 值（如从 8 改为 4）
5. **验证**：左侧对比视图应在约 1 秒内更新脱敏效果
6. **验证**：应显示 "脱敏策略已更新" toast 提示
7. 修改 `keep_suffix` 值
8. **验证**：对比视图再次更新，toast 再次出现

**Step 3: 验证策略类型切换**

1. 将 IP 地址策略从 Mask 切换为 Replace
2. **验证**：对比视图更新为替换后的假数据
3. 切换回 Mask
4. **验证**：对比视图更新为遮罩效果

**Step 4: 验证其他类型不受影响**

1. 确认文件中同时有手机号和 IP 地址
2. 修改 IP 地址的 Mask 参数
3. **验证**：手机号的脱敏结果保持不变（映射未被清除）

**Step 5: 最终 Commit（如有修复）**

```bash
git add -A
git commit -m "fix: 遮罩参数调整实时预览 + 策略变更视觉反馈"
```
