# INK-018 批量文件导入队列设计

> 日期: 2026-03-10 | 版本: v0.4.x | 状态: 已确认

## 概述

在现有工作区的 DropzoneView 中扩展多文件拖入能力，用户一次性导入多个文件后，中栏顶部出现 Tab 栏展示文件队列，逐个文件进入对比视图微调确认并导出。

## 核心决策

| 决策项 | 选择 | 理由 |
|--------|------|------|
| 触发方式 | 扩展现有 DropzoneView | 不引入新工作区类型，改动最小 |
| 策略管理 | 共享工作区策略 + 一致性映射 | 符合现有架构，同一工作区内"张三"在所有文件中脱敏为同一假名 |
| 确认方式 | 逐个微调后确认 | 保留完整的手动标记/列级调整能力 |
| 导出方式 | 每个文件确认后立即导出 | 流程直观，无需额外的批量导出状态 |
| 队列 UI | 中栏顶部 Tab 栏 | 类似浏览器标签页，直观展示队列和状态 |
| 失败处理 | 跳过并标记失败 | Toast 提示错误原因，不阻塞后续文件 |
| 实现方案 | 前端队列管理 | Rust 后端不改动，复用全部现有命令 |

## 数据模型

### 新增类型

```typescript
interface QueueFile {
  id: string;              // nanoid 唯一标识
  filePath: string;        // 文件完整路径
  fileName: string;        // 文件名（显示用）
  status: "pending" | "processing" | "confirmed" | "failed";
  errorMessage?: string;   // 失败时的错误信息
}
```

### workspaceStore 新增字段

```typescript
// 在 WorkspaceState 中新增
fileQueue: QueueFile[];           // 文件队列
activeQueueIndex: number;         // 当前处理的文件索引
```

### 单文件兼容

`fileQueue.length <= 1` 时 Tab 栏不渲染，UI 和行为与现有逻辑完全一致。

## 文件导入与队列触发

### DropzoneView 改造

```
拖入/选择文件
  ├─ paths.length === 1 → 走现有单文件流程（不变）
  └─ paths.length > 1  → 进入批量模式：
      1. 对所有 paths 做 validateFile() 格式校验
      2. 有效文件构建 QueueFile[]，无效文件 Toast 提示跳过
      3. 写入 fileQueue，activeQueueIndex = 0
      4. 自动触发第一个文件的 processFile()
```

"选择文件"按钮同步支持多选（Tauri `open` dialog 设置 `multiple: true`）。

### 文件切换流程

```
当前文件确认导出完成
  → fileQueue[activeQueueIndex].status = "confirmed"
  → 找下一个 status === "pending" 的文件
    ├─ 找到 → activeQueueIndex 指向它，自动调用 processFile()
    └─ 找不到 → 全部完毕，Toast "所有文件已处理完成"
```

### 失败处理

- processFile() 任何阶段（导入/识别/脱敏）抛错时捕获
- 当前文件 `status = "failed"`，`errorMessage` 记录原因
- Toast 提示 "xxx.xlsx 处理失败：原因"
- 自动跳到下一个 pending 文件

## Tab 栏 UI

### 位置

中栏顶部，CenterPanel 内、对比视图/处理视图上方。

### 显示条件

仅 `fileQueue.length > 1` 时渲染。

### 视觉设计

```
┌──────────────┬──────────────┬──────────────┬──────────────┐
│ ✓ data.xlsx  │ ● users.csv  │ ○ report.docx│ ✗ bad.xlsx   │
└──────────────┴──────────────┴──────────────┴──────────────┘
```

| 状态 | 图标 | 颜色 |
|------|------|------|
| pending | ○ | 灰色 |
| processing | ● | 主色（primary），Tab 高亮 |
| confirmed | ✓ | 绿色 |
| failed | ✗ | 红色 |

### 交互

- 点击 confirmed Tab：Toast 提示"该文件已导出"
- 点击 failed Tab：Toast 显示失败原因
- 点击 pending Tab：不允许跳转（一致性映射依赖顺序累积）
- 点击 processing Tab：无操作
- 溢出时水平滚动，当前 processing Tab 自动滚动到可见区域

### 顺序限制理由

一致性映射是累积的，文件 A 脱敏后的映射会影响文件 B。跳跃处理可能导致不一致结果。

## 对比视图操作变化

### 批量模式按钮区域

当 `fileQueue.length > 1` 且当前文件 status 为 processing 时：

```
┌─────────────────────────────────────────┐
│          [导出并处理下一个]  [仅导出]     │
└─────────────────────────────────────────┘
```

- **导出并处理下一个**（主按钮）：保存对话框 → 导出 → confirmed → 自动加载下一个
- **仅导出**：保存对话框 → 导出 → confirmed → 停留在当前视图

最后一个文件时只显示"导出"按钮（与单文件一致）。

### 其他操作不变

右侧面板策略配置、类型开关、词典编辑、列级脱敏、手动标记/取消标记均照常使用。策略修改持久化到工作区，影响后续文件。

## 边界情况与约束

### 文件数量限制

单次拖入上限 **20 个文件**，超出时 Toast "最多同时处理 20 个文件"，取前 20 个。

### 队列中途操作

| 场景 | 行为 |
|------|------|
| 处理中又拖入文件 | Toast "请先完成当前队列中的文件处理"，拒绝新增 |
| 切换工作区 | 确认框"当前有 N 个文件未处理，切换将放弃队列，是否继续？" |
| 删除工作区 | 同上，需确认 |

### 处理历史

每个文件确认导出后照常调用 `add_processing_record`，每个文件在工作区历史中留下独立记录。

### 还原入口

批量模式进行中时，DropzoneView 的"导入文件还原"和"粘贴 AI 回复"入口隐藏，队列处理完毕后恢复。

## 技术要点

### 后端

无改动。完全复用现有 Rust 命令：`import_file`、`detect_by_regex`、`detect_by_ner`、`detect_by_dict`、`detect_columns`、`apply_desensitize`、`export_file`、`add_processing_record`。

### 前端改动范围

| 模块 | 改动 |
|------|------|
| `workspaceStore` | 新增 fileQueue、activeQueueIndex 及相关 actions |
| `DropzoneView` | 多文件拖入支持，文件选择对话框 multiple |
| `useAutoDesensitize` | processFile 失败捕获，队列切换触发 |
| `CenterPanel` | 新增 FileQueueTabs 组件，条件渲染 |
| `ComparisonView` | 批量模式下按钮区域替换 |

### 新增组件

- `FileQueueTabs`：Tab 栏组件，接收 fileQueue 和 activeQueueIndex
