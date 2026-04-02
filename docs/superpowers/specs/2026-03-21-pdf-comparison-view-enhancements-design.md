# PDF 对比视图增强设计

## 概述

优化 PDF 脱敏对比视图的三项功能：同步缩放、滚动同步、手动涂黑（文本关联 + 框选两种模式）。

## 现状

- PDF 对比视图使用 `PdfPreviewView` 组件，后端 `render_pdf_pages` 渲染 PDF 页面为 PNG 图片（base64），前端以 `<img>` 标签展示
- 手动标记工具 `TextSelectionToolbar` 基于 DOM 文本选区 (`window.getSelection()`)，PDF 图片模式下无法工作
- 滚动同步逻辑已存在于 `ComparisonView`，但 PDF 模式下对接不完整
- 缩放仅支持 Spreadsheet 模式（字号调整），PDF 无缩放功能

## 模块 1：同步缩放

### State

- ComparisonView 新增 `pdfZoom` state，类型 `number`，默认 `100`
- 预设比例：`[50, 75, 100, 125, 150]`

### 工具栏

- 复用现有缩放工具栏 UI 样式（`-` / 比例显示 / `+`）
- 当文件类型为 PDF 时替换 fontSize 缩放栏，显示百分比缩放栏

### 渲染

- PdfPreviewView 接收 `zoom` prop
- 每个 `<img>` 通过 `width: ${imageWidth * zoom / 100}px` 实现缩放
- 不使用 `transform: scale()`，直接改宽度让浏览器重排，滚动容器 `scrollHeight`/`scrollWidth` 自然更新

### 同步

- 左右两侧共享同一个 `pdfZoom` state，工具栏操作一次两边同时变化

## 模块 2：滚动同步

### 设计

- PdfPreviewView 滚动容器绑定 `onScroll` 事件，向外发送 `scrollLeft` 和 `scrollTop`
- 组件 props 接口扩展：新增 `scrollLeft` prop（当前仅有 `scrollTop`），同时 `onScroll` 回调签名统一为 `(scrollLeft: number, scrollTop: number) => void`
- 接收外部传入的 `scrollTop`/`scrollLeft` prop，prop 变化时设置自身滚动位置
- 复用 ComparisonView 已有的 `isSyncing` ref + `requestAnimationFrame` 防循环机制
- 缩放变化时，记录当前 `scrollTop / scrollHeight` 比例，缩放完成后按比例恢复，避免视口跳变

### 关键点

左右两侧 PDF 页面数和尺寸完全一致（同一文件渲染两次），直接同步绝对像素值，不需要比例换算。

## 模块 3：手动涂黑

### 3a. 文本关联模式 — 透明文本层

**数据流变更**：

- 后端 `render_pdf_pages` 扩展返回值，每页额外携带 text objects 列表：

注意：现有 `BBox`/`PdfTextObject` 类型未实现 `Serialize`/`Deserialize`，且 `Paragraph.pdf_position` 标记为 `#[serde(skip)]`。因此需要定义独立的可序列化 DTO 用于 IPC 传输，不复用现有内部类型。

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PdfPageRender {
    // ... 现有字段
    pub text_objects: Vec<PdfTextObjectInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PdfTextObjectInfo {
    pub text: String,
    pub left: f32,    // 归一化坐标 0~1（相对页面宽高）
    pub top: f32,     // 归一化坐标，屏幕坐标系（0=顶部，1=底部）
    pub right: f32,
    pub bottom: f32,
    pub char_offset: usize,  // 页面内全文本的 Unicode 字符偏移
}
```

**坐标约定**：所有坐标使用归一化值（0~1），且采用**屏幕坐标系**（Y 轴向下，0=页面顶部，1=页面底部）。后端在 `render_pdf_pages` 中将 PDFium 的 PDF 坐标（Y 轴向上）转换为屏幕坐标：

```rust
// PDF 坐标 → 归一化屏幕坐标
let norm_left = pdf_bbox.left / page_width_pt;
let norm_top = 1.0 - pdf_bbox.top / page_height_pt;    // Y 翻转
let norm_right = pdf_bbox.right / page_width_pt;
let norm_bottom = 1.0 - pdf_bbox.bottom / page_height_pt;
```

这样前端和 `pdfBbox`（框选涂黑）统一使用同一套归一化屏幕坐标，无需关心 PDF 坐标系。

**`char_offset` 用途**：记录每个 text object 在页面全文本中的字符偏移。当用户在透明文本层选择文本后，前端通过 DOM 选区的 `startOffset`/`endOffset` + 所在 `<span>` 的 `data-char-offset` 属性，计算出选中文本在页面内的绝对字符范围，从而映射到 `SensitiveItem` 的 `start`/`end` 字段。`SensitiveItem.row` 对应 `pageIndex`。

**前端渲染**：

- 每个 PDF 页面的 `<img>` 上叠加绝对定位的透明文本层
- 为每个 text object 创建 `<span data-char-offset={charOffset}>`，通过归一化坐标定位
- 文本颜色 `transparent`，DOM 中有真实文本，用户可正常框选
- 选中后 `TextSelectionToolbar` 的 `getSelectionInfo()` 正常工作

**坐标换算（前端）**：

```
px_left = norm_left * img_display_width
px_top = norm_top * img_display_height
```

缩放时文本层跟随 `zoom` 等比缩放（使用同样的宽度计算）。

### 3b. 框选模式 — RectSelectionLayer

**交互流程**：

1. 缩放工具栏旁添加「框选涂黑」切换按钮（矩形+十字光标图标）
2. 激活后鼠标变为十字光标，在 PDF 图片上拖拽绘制矩形（蓝色半透明边框）
3. 松手后弹出类型选择面板（复用 TextSelectionToolbar 的类型列表 UI）
4. 选择类型后创建 SensitiveItem，存储 `pdf_bbox`
5. 右侧预览立即显示对应颜色遮罩

**数据模型扩展**：

前端 `SensitiveItem` 新增可选字段：

```typescript
interface SensitiveItem {
  // ... 现有字段
  pdfBbox?: {
    pageIndex: number;
    left: number;   // 归一化坐标 0~1
    top: number;
    right: number;
    bottom: number;
  };
}
```

使用归一化坐标（0~1 比例），与缩放无关，导出时按实际 PDF 尺寸换算。

Rust 侧 `SensitiveItem` 同步新增：

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PdfBbox {
    pub page_index: usize,
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

// SensitiveItem 中新增
pub pdf_bbox: Option<PdfBbox>,
```

**导出适配**：

`pdf_export.rs` 涂黑逻辑支持 `pdf_bbox` 类型的 item —— 直接用 bbox 坐标在对应页面绘制黑色矩形，不需要匹配 text object。

### 两种模式共存

- 默认文本关联模式（可直接选择文本）
- 点击「框选涂黑」按钮切换为框选模式（文本选择禁用，避免冲突）
- 再次点击按钮退出框选模式，恢复文本选择

## 交互细节

- 框选模式下按 `Escape` 取消当前正在绘制的矩形
- 框选最小拖拽距离 5px，低于此阈值视为点击而非框选，忽略
- 框选模式激活时按钮高亮（蓝色背景），鼠标在 PDF 区域显示十字光标
- 渲染页面仅在左侧调用一次 `render_pdf_pages`，左右共享同一份图片数据，减少内存占用和后端调用

## 已知限制

- 大型 PDF（50+ 页）时两份 base64 图片数据会占用较多内存，后续可考虑虚拟化仅渲染可视区域页面
- 文本层定位精度依赖 PDFium 返回的 text object 坐标质量，对于扫描型 PDF（纯图片）无法提供文本选择

## 涉及文件

| 文件 | 变更 |
|------|------|
| `src/components/PdfPreviewView/index.tsx` | 新增 zoom prop、透明文本层、RectSelectionLayer |
| `src/components/CenterPanel/ComparisonView.tsx` | 新增 pdfZoom state、PDF 缩放工具栏、框选模式切换 |
| `src/components/TextSelectionToolbar/index.tsx` | 适配 PDF 文本层的选区计算 |
| `src-tauri/src/commands/file.rs` | `render_pdf_pages` 扩展返回 text objects |
| `src-tauri/src/parser/pdf_export.rs` | 支持 pdf_bbox 类型涂黑 |
| `src-tauri/src/models/sensitive.rs` | 新增 PdfBbox 结构体 |
| `src/types/index.ts` | SensitiveItem 新增 pdfBbox 字段 |
