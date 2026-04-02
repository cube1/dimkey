# 遮罩参数调整实时预览 & 视觉反馈

**日期**: 2026-03-04
**状态**: 已批准

## 问题描述

在脱敏文档对比视图中，用户调整 Mask 策略的 `keep_prefix` / `keep_suffix` 参数后：
1. 左侧对比视图不更新（脱敏结果未变化）
2. 无任何视觉反馈（无 loading 状态、无成功/失败提示）

## 根因分析

### Bug 1: 一致性映射缓存未失效

- `handleMaskParamChange` 修改参数后触发 `reDesensitizeWithFilteredItems`
- Rust `apply_desensitize` 检查 `consistency_map.contains_key(&key)` 发现已有缓存直接跳过
- 对比：`handleReplaceStyleChange` 正确调用了 `clearConsistencyMappings()`，但 `handleMaskParamChange` 未做类似操作

### Bug 2: 无视觉反馈

- `reDesensitizeWithFilteredItems` 没有 loading 状态或成功 toast
- 与 `processFile` 形成对比（后者有完整的 `setProcessingStep` 状态管理）

## 修复方案（方案 A：按类型精准清除映射）

### 修改 1: Rust 后端 — 新增 `clear_type_consistency_mappings` 命令

**文件**: `src-tauri/src/commands/workspace.rs`

新增命令，接收 `sensitive_type_key` 参数，只清除该类型的映射条目（`retain` 过滤）。

### 修改 2: 前端 Store — 新增 `clearTypeConsistencyMappings` 方法

**文件**: `src/stores/workspaceStore.ts`

调用新的 Rust 命令，按类型清除一致性映射。

### 修改 3: RulesSection — `handleMaskParamChange` 调用清除映射

**文件**: `src/components/StrategyPanel/RulesSection.tsx`

在乐观更新 store 后、debouncedSave 前，await 调用 `clearTypeConsistencyMappings(typeKey)`。

### 修改 4: RulesSection — `handleStrategyChange` 也调用清除映射

**文件**: `src/components/StrategyPanel/RulesSection.tsx`

切换策略类型（Mask → Replace 等）时，同样清除对应类型的映射。

### 修改 5: `reDesensitizeWithFilteredItems` 增加成功 toast

**文件**: `src/hooks/useAutoDesensitize.ts`

脱敏成功后显示 `toast.success("脱敏策略已更新")`。

## 影响范围

- 不影响其他类型的一致性映射（精准清除）
- 不影响 Replace / Generalize 策略的现有行为
- 不改变 Rust 脱敏算法本身
