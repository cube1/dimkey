# Phase 5：Word 支持 + 交互打磨 — 设计文档

日期：2026-02-06

## 目标

补齐 Word 格式导出支持，完善交互体验（手动标记、键盘快捷键、加载态、拖拽反馈、类型筛选），达到 v0.1 发布标准。

---

## 1. Word 导出（Rust 后端）

**方案**：读取原 docx ZIP → 只替换 document.xml 中的文本节点 → 写入新 ZIP 文件

- 新建 `src-tauri/src/parser/word_export.rs`
- 实现 `export_docx(original_path: &str, paragraphs: &[Paragraph], output_path: &str) -> Result<(), String>`
- 逻辑：
  - `zip::ZipArchive` 打开原文件
  - 读取 `word/document.xml`，用 quick_xml 遍历 XML
  - 维护 `para_index` 计数器，对每个 `<w:p>` 内的文本节点做替换
  - 替换策略：将段落内所有 run 合并为一个 run（保留第一个 run 的样式），写入新文本
  - 其他 ZIP entry（图片、样式、元数据等）原封不动复制
- 修改 `commands/file.rs`：
  - `export_content()` Document 分支调用 `export_docx`
  - `export_file` 命令增加 `original_path: Option<String>` 参数

## 2. 手动标记功能

**场景**：用户在预览页选中文本 → 浮动工具条 → 选择敏感类型 → 创建 SensitiveItem

- 新建 `src/components/TextSelectionToolbar/index.tsx`
- 监听 `mouseup`，通过 `window.getSelection()` 获取选中文本
- DOM 坐标映射：给文本容器加 `data-row`、`data-col` 属性
  - Spreadsheet：`<td>` 上 data-row + data-col
  - Document：`<p>` 上 data-row，col 固定为 0
- `start` 计算：从 anchorNode 向上找 data-row 容器，累加前面兄弟节点文本长度 + anchorOffset
- 工具条 UI：绝对定位在选区上方，展示 13 种敏感类型
- 点击后调用 `detectStore.addItem()`，source 为 `"Manual"`
- Rust 侧 `DetectSource` 枚举新增 `Manual` 变体

## 3. 撤销栈 + 键盘快捷键

**现状**：detectStore 已有完整 undoStack + undo() 实现，只缺键盘绑定。

- 在 `App.tsx` 添加全局 `useEffect` 监听 `keydown`
- 快捷键：
  - **Cmd/Ctrl+Z**：undo()（仅 preview 页）
  - **Cmd/Ctrl+O**：触发文件选择对话框
  - **Cmd/Ctrl+S**：result 页触发导出
  - **Esc**：关闭浮层（优先级：TextSelectionToolbar > SensitivePopover > DictManager/StrategyConfig）
- 焦点在 input/textarea 时不拦截 Cmd+Z
- 不做 Cmd+Shift+Z 重做（YAGNI）

## 4. 交互打磨

### 4a. 拖拽视觉反馈

- FileDropZone 监听 dragenter/dragleave/dragover
- isDragOver 状态：边框蓝色虚线 + 背景浅蓝 + "松开导入文件"提示

### 4b. 加载态

- 文件解析中：PreviewPage 内容区显示骨架屏（灰色闪烁条），直到 setItems 调用
- 脱敏执行中：内容区半透明遮罩 + spinner

### 4c. SummaryBar 按类型筛选

- 类型标签改为可点击 toggle
- detectStore 新增 `hiddenTypes: Set<string>`
- useActiveItems 加 hiddenTypes 过滤
- 新增全选/取消全选按钮

### 4d. 错误处理完善

- 密码保护检测：docx 检查 ZIP 内 EncryptionInfo entry；Excel 捕获 calamine 密码错误
- 中文错误信息："该文件已加密，请先解除密码保护后再导入"

---

## 实现顺序

1. Rust: DetectSource 加 Manual 变体
2. Rust: Word 导出（word_export.rs + export_content 修改）
3. Rust: 错误处理完善（密码保护/损坏文件检测）
4. 前端: 手动标记（TextSelectionToolbar + data 属性）
5. 前端: 交互打磨（拖拽反馈、加载态、SummaryBar 筛选）
6. 前端: 键盘快捷键（App.tsx 全局 keydown）

## 涉及文件

| 文件 | 操作 |
|------|------|
| `src-tauri/src/models/sensitive.rs` | 改：DetectSource 加 Manual |
| `src-tauri/src/parser/word_export.rs` | 新建：docx 导出 |
| `src-tauri/src/parser/mod.rs` | 改：pub mod word_export |
| `src-tauri/src/commands/file.rs` | 改：export_content 支持 Document，export_file 加 original_path |
| `src/types/index.ts` | 改：DetectSource 加 "Manual" |
| `src/components/TextSelectionToolbar/index.tsx` | 新建：选中文本浮动工具条 |
| `src/components/HighlightedText/index.tsx` | 改：容器加 data-row/data-col |
| `src/components/SpreadsheetView/index.tsx` | 改：td 加 data-row/data-col |
| `src/components/ContentRenderer/index.tsx` | 改：DocumentView 段落加 data-row |
| `src/components/SummaryBar/index.tsx` | 改：类型标签可点击筛选 |
| `src/components/FileDropZone/index.tsx` | 改：拖拽视觉反馈 |
| `src/stores/detectStore.ts` | 改：加 hiddenTypes + toggleType |
| `src/pages/PreviewPage/index.tsx` | 改：加载态 + TextSelectionToolbar 集成 |
| `src/App.tsx` | 改：全局键盘快捷键 |
| `src/pages/ResultPage/index.tsx` | 改：export_file 传 original_path |
