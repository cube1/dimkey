# 脱敏预览编辑纠正功能设计

## 背景

用户在脱敏预览视图中需要纠正引擎的误识别和漏识别。当前已有"取消标记"和"手动标记"功能，但存在两个核心问题：

1. **可发现性差**：用户不知道点击高亮项可以弹出编辑浮层（SensitivePopover），也不知道选中文本可以手动标记
2. **词典 bug**：自定义词典保存后，不会触发重新检测，导致新添加的词条匹配不到

## 目标

- 让用户直觉地发现"点击高亮项可编辑"
- 让"取消标记"操作第一眼可见
- 修复词典检测不生效的 bug

## 设计方案

### 1. 高亮项交互增强

**改动文件**：`src/components/HighlightedText/index.tsx`

**当前状态**：高亮 span 已有 `cursor-pointer` 和 `hover:brightness-95`，但缺少明显的 hover 边框反馈。

**改动**：
- 为非 diffMode 的高亮项添加 hover 时的 `ring` 效果（如 `hover:ring-2 hover:ring-current/20`），让用户感知到可交互
- 确保所有模式（普通、模版替换、diff）的高亮项都有一致的 hover 反馈

### 2. SensitivePopover "取消标记"布局优化

**改动文件**：`src/components/SensitivePopover/index.tsx`

**当前状态**：脱敏模式下"取消标记"按钮在 Popover 最底部，用户不容易发现。模版替换模式已有单独的布局分支。

**改动**：
- 将 Popover 顶部的敏感文本和"取消标记"放在同一行（flex 布局）
- "取消标记"改为 `× 取消标记` 样式的文字链接或小按钮，靠右对齐
- 移除底部的独立"取消标记"按钮

**布局示意**：
```
┌─────────────────────────┐
│ 张三         [× 取消标记] │
│ [   人名   ]              │
│                           │
│ 脱敏策略  [掩码 ▼]        │
│ 保留前 [1]  保留后 [0]    │
│ 预览: 张**                │
└─────────────────────────┘
```

### 3. 修复词典重检测

**改动文件**：`src/pages/PreviewPage/index.tsx`、`src/components/DictManager/index.tsx`

**问题根因**：`detect_by_dict` 只在 PreviewPage 初始化的 `useEffect` 中调用一次。用户在 DictManager 中添加词条后，不会触发重新检测。`detectStore.replaceDictItems()` 方法已存在但从未被调用。

**修复方案**：
- 方案：在 DictManager 关闭时（`onClose` 回调），如果词典发生了变更，触发一次 `detect_by_dict` 重新检测
- 用检测结果调用 `replaceDictItems()` 更新 store 中的词典识别项
- 具体实现：PreviewPage 包裹 DictManager 的 `onClose`，在关闭时执行重检测

## 不做的事

- 不做首次使用引导气泡
- 不做右侧敏感项列表面板
- 不做批量操作
- 不做跨文件白名单/忽略列表
- 不做模版替换模式的 Popover 布局调整（已有独立分支，体验已满足）

## 影响范围

| 文件 | 改动类型 |
|------|----------|
| `src/components/HighlightedText/index.tsx` | 样式微调 |
| `src/components/SensitivePopover/index.tsx` | 布局重构（脱敏模式分支） |
| `src/pages/PreviewPage/index.tsx` | 添加词典重检测逻辑 |
