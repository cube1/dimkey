# 粘贴板脱敏 + AI 回复还原 设计文档

> 日期: 2026-02-11

## 核心工作流

```
复制原文 → 粘贴进Dimkey → 自动识别+脱敏 → 复制脱敏文本
→ 发给 ChatGPT → 拿到回复 → 粘贴 AI 回复进Dimkey → 自动还原 → 复制还原结果
```

## 脱敏阶段

1. 用户在 DropzoneView 通过 `Ctrl/Cmd+V` 或"粘贴文本"按钮输入内容
2. 系统创建一个**粘贴板工作区**（source 标记为 `Clipboard`，区别于 `File` 类型），将文本当作 TXT 处理
3. 复用现有三层识别引擎扫描 → 高亮预览 → 用户调整策略 → 执行脱敏
4. 脱敏结果旁显示"复制脱敏文本"按钮，一键复制

## 还原阶段

1. 工作区内新增"粘贴 AI 回复"按钮
2. 用户粘贴 AI 回复内容，系统用当前工作区的一致性映射表做**反向替换**
3. 在对比视图中展示：左侧 AI 原始回复，右侧还原后的结果
4. "复制还原结果"按钮一键复制

## Rust 后端改动

### 新增 Tauri Command

- `import_clipboard_text(text: String)` — 接收粘贴文本，解析为 FileContent（复用 TXT 段落解析逻辑）
- `restore_ai_response(workspace_id: String, ai_text: String)` — 接收 AI 回复文本，从工作区加载一致性映射表，执行反向替换，返回还原结果

### 工作区模型扩展

- `Workspace` 新增 `source: WorkspaceSource` 字段（枚举 `File` | `Clipboard`），默认 `File`，`#[serde(default)]` 兼容旧数据

## 前端改动

### DropzoneView 改造

- 增加"或 粘贴文本"按钮
- 监听全局 `paste` 事件
- 粘贴板工作区显示"粘贴 AI 回复"按钮

### WorkspaceList

- 粘贴板工作区用不同图标区分（ClipboardList vs FolderOpen）
- 标题自动生成："粘贴板 MM-DD HH:mm"

### 复用

- ContentRenderer / ComparisonView / StrategyPanel 全部复用，零改动

## 边界

- 粘贴板模式默认推荐使用替换策略，UI 提示
- 第一阶段只做纯文本
- 不做全局快捷键、剪贴板自动监听、表格结构识别
