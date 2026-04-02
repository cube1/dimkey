# PDF 对比视图增强实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 PDF 对比视图添加同步缩放、滚动同步、手动涂黑（文本关联 + 框选）功能。

**Architecture:** 后端 `render_pdf_pages` 扩展返回 text objects 的归一化坐标；前端 PdfPreviewView 新增缩放、透明文本层、框选覆盖层；ComparisonView 统一管理 zoom state 和滚动同步；pdf_export 支持 pdf_bbox 直接涂黑。

**Tech Stack:** React + TypeScript + TailwindCSS（前端）、Rust + pdfium-render（后端）

**Spec:** `docs/superpowers/specs/2026-03-21-pdf-comparison-view-enhancements-design.md`

---

## File Structure

| 文件 | 职责 | 操作 |
|------|------|------|
| `src-tauri/src/commands/file.rs` | `PdfPageRender` 结构体扩展、`render_pdf_pages` 返回 text objects | 修改 |
| `src-tauri/src/models/sensitive.rs` | 新增 `PdfBbox` 结构体、`SensitiveItem` 新增 `pdf_bbox` 字段 | 修改 |
| `src-tauri/src/parser/pdf_export.rs` | 支持 `pdf_bbox` 类型涂黑 | 修改 |
| `src/types/index.ts` | `PdfBbox` 接口、`SensitiveItem` 新增 `pdf_bbox` 字段 | 修改 |
| `src/components/PdfPreviewView/index.tsx` | zoom prop、scrollLeft prop、透明文本层、框选层、涂黑遮罩，导出 `PdfPageRender`/`PdfTextObjectInfo` 类型 | 修改 |
| `src/components/CenterPanel/ComparisonView.tsx` | pdfZoom state、PDF 缩放工具栏、框选模式切换、滚动同步修复、共享 pages 数据 | 修改 |
| `src/components/TextSelectionToolbar/index.tsx` | 适配 PDF 文本层选区：支持 `data-char-offset` 属性计算偏移 | 修改 |
| `src/components/PdfPreviewView/RectSelectionLayer.tsx` | 框选涂黑交互组件 | 新建 |
| `src/components/PdfPreviewView/PdfTextLayer.tsx` | 透明文本层组件 | 新建 |
| `src/components/PdfPreviewView/TypeSelectPopover.tsx` | 框选后类型选择弹窗 | 新建 |

---

### Task 1: Rust 数据模型扩展 — PdfBbox + SensitiveItem

**Files:**
- Modify: `src-tauri/src/models/sensitive.rs:170-193`
- Modify: `src/types/index.ts:31-42`

- [ ] **Step 1: 在 Rust 端新增 PdfBbox 结构体**

在 `src-tauri/src/models/sensitive.rs` 的 `SensitiveItem` 定义之前添加：

```rust
/// 手动框选涂黑区域（归一化屏幕坐标 0~1）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfBbox {
    pub page_index: usize,
    /// 左边界（0=页面左侧，1=页面右侧）
    pub left: f32,
    /// 上边界（0=页面顶部，1=页面底部）
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}
```

- [ ] **Step 2: SensitiveItem 新增 pdf_bbox 字段**

在 `SensitiveItem` 结构体末尾（`sheet_index` 之后）添加：

```rust
    /// PDF 手动框选涂黑区域（归一化屏幕坐标）
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub pdf_bbox: Option<PdfBbox>,
```

- [ ] **Step 3: 前端 TypeScript 类型同步**

在 `src/types/index.ts` 的 `SensitiveItem` 接口中添加：

项目 Rust serde 序列化使用默认 `snake_case`（`sensitive_type`、`sheet_index` 等），前端类型与 Rust 一致使用 `snake_case`：

```typescript
/** PDF 手动框选涂黑区域（归一化屏幕坐标 0~1） */
export interface PdfBbox {
  page_index: number;
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** 单条敏感信息 */
export interface SensitiveItem {
  // ... 现有字段
  sheet_index: number;
  pdf_bbox?: PdfBbox;  // 新增，snake_case 匹配 Rust serde
}
```

- [ ] **Step 4: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS，无编译错误

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/models/sensitive.rs src/types/index.ts
git commit -m "feat: 新增 PdfBbox 数据模型，SensitiveItem 支持 PDF 框选涂黑"
```

---

### Task 2: 后端 render_pdf_pages 扩展返回 text objects

**Files:**
- Modify: `src-tauri/src/commands/file.rs:13-25` (PdfPageRender 结构体)
- Modify: `src-tauri/src/commands/file.rs:444-513` (render_pdf_pages 函数)

- [ ] **Step 1: 新增 PdfTextObjectInfo DTO 结构体**

在 `src-tauri/src/commands/file.rs` 的 `PdfPageRender` 之后添加：

```rust
/// 单个 PDF text object 信息（用于前端透明文本层，归一化屏幕坐标）
#[derive(Debug, Clone, Serialize)]
pub struct PdfTextObjectInfo {
    pub text: String,
    /// 归一化坐标 0~1，屏幕坐标系（Y 轴向下）
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    /// 页面内全文本的 Unicode 字符偏移
    pub char_offset: usize,
}
```

- [ ] **Step 2: PdfPageRender 新增 text_objects 字段**

```rust
pub struct PdfPageRender {
    // ... 现有字段
    /// 页面内所有 text object（归一化屏幕坐标，用于前端透明文本层）
    pub text_objects: Vec<PdfTextObjectInfo>,
}
```

- [ ] **Step 3: render_pdf_pages 中提取 text objects 并转换坐标**

在 `render_pdf_pages` 函数的 for 循环内，渲染完位图后、push results 前，添加 text object 提取逻辑：

```rust
// 提取 text objects 并转换为归一化屏幕坐标
let mut text_objects = Vec::new();
let mut page_char_offset: usize = 0;
let objects = page.objects();
for i in 0..objects.len() {
    if let Ok(obj) = objects.get(i) {
        if let Some(text_obj) = obj.as_text_object() {
            let text = text_obj.text();
            if text.is_empty() { continue; }
            if let Ok(bounds) = obj.bounds() {
                let norm_left = bounds.left().value / page_width;
                let norm_top = 1.0 - bounds.top().value / page_height;
                let norm_right = bounds.right().value / page_width;
                let norm_bottom = 1.0 - bounds.bottom().value / page_height;
                text_objects.push(PdfTextObjectInfo {
                    text: text.clone(),
                    left: norm_left,
                    top: norm_top,
                    right: norm_right,
                    bottom: norm_bottom,
                    char_offset: page_char_offset,
                });
                page_char_offset += text.chars().count();
            }
        }
    }
}
```

然后在 `results.push(PdfPageRender { ... })` 中添加 `text_objects` 字段。

- [ ] **Step 4: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/file.rs
git commit -m "feat: render_pdf_pages 返回 text objects 归一化坐标"
```

---

### Task 3: 前端 PdfPreviewView 接收 zoom + scrollLeft

**Files:**
- Modify: `src/components/PdfPreviewView/index.tsx`

- [ ] **Step 1: 更新并导出 PdfPageRender/PdfTextObjectInfo 接口**

这些类型需要 `export`，因为 ComparisonView 和 PdfTextLayer 都需要 import：

```typescript
export interface PdfTextObjectInfo {
  text: string;
  left: number;
  top: number;
  right: number;
  bottom: number;
  char_offset: number;
}

export interface PdfPageRender {
  page_index: number;
  image_base64: string;
  image_width: number;
  image_height: number;
  page_width: number;
  page_height: number;
  text_objects: PdfTextObjectInfo[];
}
```

- [ ] **Step 2: 扩展 props 接口**

```typescript
interface PdfPreviewViewProps {
  filePath?: string;
  /** 外部传入的已渲染页面数据（共享，避免重复调用后端） */
  pages?: PdfPageRender[];
  items: SensitiveItem[];
  showRedacted?: boolean;
  onScroll?: (scrollLeft: number, scrollTop: number) => void;
  scrollTop?: number;
  scrollLeft?: number;
  /** 缩放比例，默认 100 */
  zoom?: number;
  /** 框选模式激活 */
  rectSelectMode?: boolean;
  /** 手动添加敏感项回调 */
  onAddItem?: (item: SensitiveItem) => void;
}
```

- [ ] **Step 3: 实现缩放逻辑**

在 `PdfPageView` 中，根据 `zoom` prop 计算显示宽度和高度：

```typescript
const displayWidth = page.image_width * (zoom / 100);
const displayHeight = page.image_height * (zoom / 100);
```

`<img>` 使用 `style={{ width: displayWidth }}` 替代 `className="w-full"`。
`displayWidth` 和 `displayHeight` 传给 PdfTextLayer、RectSelectionLayer、RedactionOverlay。

- [ ] **Step 4: 修复滚动同步，支持 scrollLeft**

更新 `handleScroll` 回调，发送 `(scrollLeft, scrollTop)` 双参数。
更新 `useEffect` 同步逻辑，同时设置 `scrollLeft` 和 `scrollTop`。

- [ ] **Step 5: 支持外部 pages 数据共享**

如果传入 `pages` prop，直接使用，跳过后端调用。修改 `useEffect`：

```typescript
useEffect(() => {
  // 如果外部已传入 pages，跳过加载
  if (externalPages) {
    setPages(externalPages);
    setLoading(false);
    return;
  }
  if (!filePath) return;
  let cancelled = false;
  setLoading(true);
  invoke<PdfPageRender[]>("render_pdf_pages", { filePath })
    .then((result) => { if (!cancelled) { setPages(result); setLoading(false); } })
    .catch((err) => { if (!cancelled) { setError(typeof err === "string" ? err : "渲染 PDF 失败"); setLoading(false); } });
  return () => { cancelled = true; };
}, [filePath, externalPages]);
```

同时将 `rectSelectMode` 和 `onAddItem` props 传递给 `PdfPageView` 子组件。

- [ ] **Step 6: 验证编译**

Run: `npm run dev`（确认 TypeScript 无报错）

- [ ] **Step 7: Commit**

```bash
git add src/components/PdfPreviewView/index.tsx
git commit -m "feat: PdfPreviewView 支持 zoom、scrollLeft、共享 pages 数据"
```

---

### Task 4: ComparisonView 缩放工具栏 + 滚动同步 + 共享 pages

**Files:**
- Modify: `src/components/CenterPanel/ComparisonView.tsx`

- [ ] **Step 1: 新增 PDF 相关 state**

```typescript
const PDF_ZOOM_LEVELS = [50, 75, 100, 125, 150];
const DEFAULT_PDF_ZOOM = 100;

// 在 ComparisonView 内：
const [pdfZoom, setPdfZoom] = useState(DEFAULT_PDF_ZOOM);
const [pdfPages, setPdfPages] = useState<PdfPageRender[] | null>(null);
const [pdfLoading, setPdfLoading] = useState(false);
const [rectSelectMode, setRectSelectMode] = useState(false);

// PDF 滚动同步
const [pdfLeftScroll, setPdfLeftScroll] = useState({ left: 0, top: 0 });
const [pdfRightScroll, setPdfRightScroll] = useState({ left: 0, top: 0 });
const pdfSyncing = useRef(false);
```

- [ ] **Step 2: 加载 PDF 页面数据（一次加载，共享给两侧）**

```typescript
useEffect(() => {
  if (!isPdf || !currentFilePath) return;
  setPdfLoading(true);
  invoke<PdfPageRender[]>("render_pdf_pages", { filePath: currentFilePath })
    .then(setPdfPages)
    .catch((err) => console.error("渲染 PDF 失败:", err))
    .finally(() => setPdfLoading(false));
}, [isPdf, currentFilePath]);
```

- [ ] **Step 3: PDF 滚动同步回调**

```typescript
const handlePdfLeftScroll = useCallback((scrollLeft: number, scrollTop: number) => {
  if (pdfSyncing.current) return;
  pdfSyncing.current = true;
  setPdfRightScroll({ left: scrollLeft, top: scrollTop });
  requestAnimationFrame(() => { pdfSyncing.current = false; });
}, []);

const handlePdfRightScroll = useCallback((scrollLeft: number, scrollTop: number) => {
  if (pdfSyncing.current) return;
  pdfSyncing.current = true;
  setPdfLeftScroll({ left: scrollLeft, top: scrollTop });
  requestAnimationFrame(() => { pdfSyncing.current = false; });
}, []);
```

- [ ] **Step 4: 渲染 PDF 缩放工具栏**

在 `{isSpreadsheet && (` 条件块之后添加 PDF 缩放工具栏：

```typescript
{isPdf && (
  <div className="bg-white border-b border-slate-200 px-4 py-1.5 shrink-0 flex items-center justify-end gap-2">
    {/* 框选涂黑按钮 */}
    <button
      onClick={() => setRectSelectMode((v) => !v)}
      className={`flex items-center gap-1 text-xs px-2 py-1 rounded-md transition-colors ${
        rectSelectMode
          ? "bg-blue-100 text-blue-700"
          : "text-slate-500 hover:bg-slate-100"
      }`}
      title="框选涂黑"
    >
      {/* 矩形图标 */}
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" className="w-4 h-4">
        <rect x="3" y="3" width="14" height="14" rx="1" strokeWidth="1.5" strokeDasharray="4 2" />
      </svg>
      框选涂黑
    </button>
    <div className="w-px h-4 bg-slate-200 mx-1" />
    <span className="text-xs text-slate-400 mr-1">缩放</span>
    <button
      className="w-6 h-6 flex items-center justify-center rounded text-slate-500 hover:bg-slate-100 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
      disabled={pdfZoom <= PDF_ZOOM_LEVELS[0]}
      onClick={() => setPdfZoom((z) => { const idx = PDF_ZOOM_LEVELS.indexOf(z); return idx > 0 ? PDF_ZOOM_LEVELS[idx - 1] : z; })}
      title="缩小"
    >
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" className="w-4 h-4"><path fillRule="evenodd" d="M4 10a.75.75 0 0 1 .75-.75h10.5a.75.75 0 0 1 0 1.5H4.75A.75.75 0 0 1 4 10Z" clipRule="evenodd" /></svg>
    </button>
    <span className="text-xs text-slate-600 tabular-nums w-8 text-center">{pdfZoom}%</span>
    <button
      className="w-6 h-6 flex items-center justify-center rounded text-slate-500 hover:bg-slate-100 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
      disabled={pdfZoom >= PDF_ZOOM_LEVELS[PDF_ZOOM_LEVELS.length - 1]}
      onClick={() => setPdfZoom((z) => { const idx = PDF_ZOOM_LEVELS.indexOf(z); return idx < PDF_ZOOM_LEVELS.length - 1 ? PDF_ZOOM_LEVELS[idx + 1] : z; })}
      title="放大"
    >
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" className="w-4 h-4"><path d="M10.75 4.75a.75.75 0 0 0-1.5 0v4.5h-4.5a.75.75 0 0 0 0 1.5h4.5v4.5a.75.75 0 0 0 1.5 0v-4.5h4.5a.75.75 0 0 0 0-1.5h-4.5v-4.5Z" /></svg>
    </button>
  </div>
)}
```

- [ ] **Step 5: 更新 PdfPreviewView 调用**

左侧：
```typescript
<PdfPreviewView
  pages={pdfPages ?? undefined}
  items={sheetItems}
  zoom={pdfZoom}
  onScroll={handlePdfLeftScroll}
  scrollTop={pdfLeftScroll.top}
  scrollLeft={pdfLeftScroll.left}
  rectSelectMode={rectSelectMode}
  onAddItem={handleManualAddItem}
/>
```

右侧：
```typescript
<PdfPreviewView
  pages={pdfPages ?? undefined}
  items={sheetItems}
  showRedacted
  zoom={pdfZoom}
  onScroll={handlePdfRightScroll}
  scrollTop={pdfRightScroll.top}
  scrollLeft={pdfRightScroll.left}
/>
```

- [ ] **Step 6: 验证编译**

Run: `npm run dev`

- [ ] **Step 7: Commit**

```bash
git add src/components/CenterPanel/ComparisonView.tsx
git commit -m "feat: PDF 对比视图同步缩放 + 滚动同步 + 框选模式切换"
```

---

### Task 5: 透明文本层组件 PdfTextLayer

**Files:**
- Create: `src/components/PdfPreviewView/PdfTextLayer.tsx`
- Modify: `src/components/PdfPreviewView/index.tsx`

- [ ] **Step 1: 创建 PdfTextLayer 组件**

```typescript
// src/components/PdfPreviewView/PdfTextLayer.tsx
import type { PdfTextObjectInfo } from "./index";

interface PdfTextLayerProps {
  textObjects: PdfTextObjectInfo[];
  displayWidth: number;
  displayHeight: number;
  pageIndex: number;
}

/**
 * 透明文本层：在 PDF 页面图片上叠加不可见的真实 DOM 文本，
 * 使用户可以用鼠标框选文本触发 TextSelectionToolbar。
 *
 * 每个 text object 对应一个 <span>，通过归一化坐标绝对定位。
 * data-row = pageIndex，data-char-offset = charOffset，用于选区映射。
 */
export function PdfTextLayer({ textObjects, displayWidth, displayHeight, pageIndex }: PdfTextLayerProps) {
  return (
    <div
      className="absolute inset-0 select-text"
      style={{ width: displayWidth, height: displayHeight }}
      data-row={pageIndex}
      data-col={0}
    >
      {textObjects.map((obj, idx) => {
        const left = obj.left * displayWidth;
        const top = obj.top * displayHeight;
        const width = (obj.right - obj.left) * displayWidth;
        const height = (obj.bottom - obj.top) * displayHeight;
        return (
          <span
            key={idx}
            data-char-offset={obj.char_offset}
            className="absolute whitespace-pre"
            style={{
              left, top, width, height,
              color: "transparent",
              fontSize: `${height * 0.8}px`,
              lineHeight: `${height}px`,
              overflow: "hidden",
            }}
          >
            {obj.text}
          </span>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: 在 PdfPageView 中集成文本层**

在 `PdfPageView` 的 `<img>` 之后、`RedactionOverlay` 之前添加：

```typescript
{!rectSelectMode && page.text_objects.length > 0 && (
  <PdfTextLayer
    textObjects={page.text_objects}
    displayWidth={displayWidth}
    displayHeight={displayHeight}
    pageIndex={page.page_index}
  />
)}
```

- [ ] **Step 3: Commit**

```bash
git add src/components/PdfPreviewView/PdfTextLayer.tsx src/components/PdfPreviewView/index.tsx
git commit -m "feat: PDF 透明文本层，恢复手动文本选择标记"
```

---

### Task 5b: 适配 TextSelectionToolbar 支持 PDF 文本层选区

**Files:**
- Modify: `src/components/TextSelectionToolbar/index.tsx`

- [ ] **Step 1: 修改 getSelectionInfo() 支持 PDF 文本层的 char_offset**

当前 `getSelectionInfo()` 通过遍历 DOM 文本节点计算 `start`/`end`。在 PDF 文本层中，每个 `<span>` 都有 `data-char-offset` 属性。需要适配：

```typescript
function getSelectionInfo(): SelectionInfo | null {
  const sel = window.getSelection();
  if (!sel || sel.isCollapsed || !sel.rangeCount) return null;
  const text = sel.toString().trim();
  if (!text) return null;
  const range = sel.getRangeAt(0);

  // 找到包含 data-row 的祖先容器
  const container = findDataContainer(range.startContainer);
  if (!container) return null;

  const row = parseInt(container.getAttribute("data-row") || "-1", 10);
  const col = parseInt(container.getAttribute("data-col") || "0", 10);
  if (row < 0) return null;

  // PDF 文本层模式：使用 data-char-offset 计算偏移
  const startSpan = findCharOffsetSpan(range.startContainer);
  if (startSpan) {
    const baseOffset = parseInt(startSpan.getAttribute("data-char-offset") || "0", 10);
    const start = baseOffset + range.startOffset;
    const end = start + text.length;
    const rect = range.getBoundingClientRect();
    return { text, row, col, start, end, rect };
  }

  // 原有逻辑：遍历 DOM 文本节点
  const start = getTextOffset(container, range.startContainer, range.startOffset);
  const end = getTextOffset(container, range.endContainer, range.endOffset);
  if (start < 0 || end <= start) return null;
  const rect = range.getBoundingClientRect();
  return { text, row, col, start, end, rect };
}

/** 向上查找带 data-char-offset 属性的 span */
function findCharOffsetSpan(node: Node): HTMLElement | null {
  let current: Node | null = node;
  while (current) {
    if (current instanceof HTMLElement && current.hasAttribute("data-char-offset")) {
      return current;
    }
    current = current.parentNode;
  }
  return null;
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/TextSelectionToolbar/index.tsx
git commit -m "feat: TextSelectionToolbar 适配 PDF 文本层 data-char-offset 选区"
```

---

### Task 6: 框选涂黑组件 RectSelectionLayer + TypeSelectPopover

**Files:**
- Create: `src/components/PdfPreviewView/RectSelectionLayer.tsx`
- Create: `src/components/PdfPreviewView/TypeSelectPopover.tsx`
- Modify: `src/components/PdfPreviewView/index.tsx`

- [ ] **Step 1: 创建 TypeSelectPopover 组件**

```typescript
// src/components/PdfPreviewView/TypeSelectPopover.tsx
import { SENSITIVE_TYPE_CONFIG } from "../../types";
import type { SensitiveType } from "../../types";

const COMMON_TYPES = ["Phone", "IdCard", "PersonName", "Email", "Address", "BankCard"];

interface TypeSelectPopoverProps {
  position: { x: number; y: number };
  onSelect: (type: SensitiveType) => void;
  onCancel: () => void;
}

export function TypeSelectPopover({ position, onSelect, onCancel }: TypeSelectPopoverProps) {
  return (
    <div
      className="fixed z-50 bg-white rounded-xl shadow-float border border-slate-200/80 overflow-hidden animate-slide-up"
      style={{ top: position.y, left: position.x }}
    >
      <div className="px-3 pt-2.5 pb-2">
        <div className="text-[11px] text-slate-400 mb-1.5 tracking-wider">选择涂黑类型</div>
        <div className="flex flex-wrap gap-1">
          {COMMON_TYPES.map((type) => {
            const config = SENSITIVE_TYPE_CONFIG[type];
            return (
              <button
                key={type}
                onClick={() => onSelect(type as SensitiveType)}
                className={`text-xs px-2 py-1 rounded-md ${config?.bgClass} ${config?.textClass} hover:opacity-75 transition-opacity`}
              >
                {config?.label || type}
              </button>
            );
          })}
          <button
            onClick={onCancel}
            className="text-xs px-2 py-1 rounded-md text-slate-400 hover:text-slate-600 hover:bg-slate-100 transition-colors"
          >
            取消
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: 创建 RectSelectionLayer 组件**

```typescript
// src/components/PdfPreviewView/RectSelectionLayer.tsx
import { useState, useCallback, useRef } from "react";
import { TypeSelectPopover } from "./TypeSelectPopover";
import type { SensitiveType, SensitiveItem, PdfBbox } from "../../types";

const MIN_DRAG_DISTANCE = 5;

interface RectSelectionLayerProps {
  displayWidth: number;
  displayHeight: number;
  pageIndex: number;
  onAddItem: (item: SensitiveItem) => void;
}

export function RectSelectionLayer({
  displayWidth,
  displayHeight,
  pageIndex,
  onAddItem,
}: RectSelectionLayerProps) {
  const [dragging, setDragging] = useState(false);
  const [rect, setRect] = useState<{ x1: number; y1: number; x2: number; y2: number } | null>(null);
  const [pendingRect, setPendingRect] = useState<{ x1: number; y1: number; x2: number; y2: number } | null>(null);
  const [popoverPos, setPopoverPos] = useState<{ x: number; y: number } | null>(null);
  const layerRef = useRef<HTMLDivElement>(null);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (pendingRect) return; // 正在选择类型时忽略新的拖拽
    const layerRect = layerRef.current?.getBoundingClientRect();
    if (!layerRect) return;
    const x = e.clientX - layerRect.left;
    const y = e.clientY - layerRect.top;
    setDragging(true);
    setRect({ x1: x, y1: y, x2: x, y2: y });
  }, [pendingRect]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!dragging || !rect) return;
    const layerRect = layerRef.current?.getBoundingClientRect();
    if (!layerRect) return;
    const x = Math.max(0, Math.min(e.clientX - layerRect.left, displayWidth));
    const y = Math.max(0, Math.min(e.clientY - layerRect.top, displayHeight));
    setRect((prev) => prev ? { ...prev, x2: x, y2: y } : null);
  }, [dragging, rect, displayWidth, displayHeight]);

  const handleMouseUp = useCallback((e: React.MouseEvent) => {
    if (!dragging || !rect) return;
    setDragging(false);
    const dx = Math.abs(rect.x2 - rect.x1);
    const dy = Math.abs(rect.y2 - rect.y1);
    if (dx < MIN_DRAG_DISTANCE && dy < MIN_DRAG_DISTANCE) {
      setRect(null);
      return;
    }
    setPendingRect(rect);
    setPopoverPos({ x: e.clientX, y: e.clientY });
    setRect(null);
  }, [dragging, rect]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      setDragging(false);
      setRect(null);
      setPendingRect(null);
      setPopoverPos(null);
    }
  }, []);

  const handleSelectType = useCallback((type: SensitiveType) => {
    if (!pendingRect) return;
    const x1 = Math.min(pendingRect.x1, pendingRect.x2);
    const y1 = Math.min(pendingRect.y1, pendingRect.y2);
    const x2 = Math.max(pendingRect.x1, pendingRect.x2);
    const y2 = Math.max(pendingRect.y1, pendingRect.y2);

    const pdf_bbox: PdfBbox = {
      page_index: pageIndex,
      left: x1 / displayWidth,
      top: y1 / displayHeight,
      right: x2 / displayWidth,
      bottom: y2 / displayHeight,
    };

    const item: SensitiveItem = {
      id: `rect_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
      text: `[框选区域 p${pageIndex + 1}]`,
      sensitive_type: type,
      source: "Manual",
      confidence: 1.0,
      start: 0,
      end: 0,
      row: pageIndex,
      col: 0,
      sheet_index: 0,
      pdf_bbox: pdf_bbox,
    };
    onAddItem(item);
    setPendingRect(null);
    setPopoverPos(null);
  }, [pendingRect, pageIndex, displayWidth, displayHeight, onAddItem]);

  const handleCancel = useCallback(() => {
    setPendingRect(null);
    setPopoverPos(null);
  }, []);

  // 计算当前绘制中矩形的样式
  const drawingStyle = rect ? {
    left: Math.min(rect.x1, rect.x2),
    top: Math.min(rect.y1, rect.y2),
    width: Math.abs(rect.x2 - rect.x1),
    height: Math.abs(rect.y2 - rect.y1),
  } : null;

  // 待确认矩形
  const pendingStyle = pendingRect ? {
    left: Math.min(pendingRect.x1, pendingRect.x2),
    top: Math.min(pendingRect.y1, pendingRect.y2),
    width: Math.abs(pendingRect.x2 - pendingRect.x1),
    height: Math.abs(pendingRect.y2 - pendingRect.y1),
  } : null;

  return (
    <>
      <div
        ref={layerRef}
        className="absolute inset-0 cursor-crosshair"
        style={{ width: displayWidth, height: displayHeight }}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onKeyDown={handleKeyDown}
        tabIndex={0}
      >
        {drawingStyle && (
          <div
            className="absolute border-2 border-blue-500 bg-blue-500/10 pointer-events-none"
            style={drawingStyle}
          />
        )}
        {pendingStyle && (
          <div
            className="absolute border-2 border-blue-500 bg-blue-500/20 pointer-events-none animate-pulse"
            style={pendingStyle}
          />
        )}
      </div>
      {popoverPos && pendingRect && (
        <TypeSelectPopover
          position={popoverPos}
          onSelect={handleSelectType}
          onCancel={handleCancel}
        />
      )}
    </>
  );
}
```

- [ ] **Step 3: 在 PdfPageView 中集成框选层**

在 `PdfPageView` 中，当 `rectSelectMode` 激活时显示：

```typescript
{rectSelectMode && (
  <RectSelectionLayer
    displayWidth={displayWidth}
    displayHeight={displayHeight}
    pageIndex={page.page_index}
    onAddItem={onAddItem}
  />
)}
```

- [ ] **Step 4: Commit**

```bash
git add src/components/PdfPreviewView/RectSelectionLayer.tsx src/components/PdfPreviewView/TypeSelectPopover.tsx src/components/PdfPreviewView/index.tsx
git commit -m "feat: PDF 框选涂黑 — 拖拽矩形 + 类型选择 + 创建 SensitiveItem"
```

---

### Task 7: 右侧 RedactionOverlay 显示涂黑遮罩

**Files:**
- Modify: `src/components/PdfPreviewView/index.tsx`

- [ ] **Step 1: 重写 RedactionOverlay 组件**

现有的 `RedactionOverlay` 只显示一个提示标签。需要改为根据 `items` 中有 `pdf_bbox` 的项绘制实际遮罩：

```typescript
function RedactionOverlay({
  page,
  items,
  displayWidth,
  displayHeight,
}: {
  page: PdfPageRender;
  items: SensitiveItem[];
  displayWidth: number;
  displayHeight: number;
}) {
  // 筛选当前页有 pdf_bbox 的项
  const bboxItems = items.filter(
    (i) => i.pdf_bbox && i.pdf_bbox.page_index === page.page_index
  );

  // 所有敏感项计数（包括文本类型）
  const totalCount = items.length;

  return (
    <div className="absolute inset-0 pointer-events-none" style={{ width: displayWidth, height: displayHeight }}>
      {/* 框选涂黑遮罩 */}
      {bboxItems.map((item) => {
        const bbox = item.pdf_bbox!;
        return (
          <div
            key={item.id}
            className="absolute bg-black/80"
            style={{
              left: bbox.left * displayWidth,
              top: bbox.top * displayHeight,
              width: (bbox.right - bbox.left) * displayWidth,
              height: (bbox.bottom - bbox.top) * displayHeight,
            }}
          />
        );
      })}
      {/* 计数提示 */}
      {totalCount > 0 && (
        <div className="absolute top-2 right-2 bg-black/70 text-white text-xs px-2 py-1 rounded">
          {totalCount} 处敏感信息将被涂黑
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: 更新 PdfPageView 中 RedactionOverlay 调用**

传入 `displayWidth` 和 `displayHeight`。

- [ ] **Step 3: Commit**

```bash
git add src/components/PdfPreviewView/index.tsx
git commit -m "feat: PDF 右侧预览显示框选涂黑遮罩"
```

---

### Task 8: 后端 pdf_export 支持 pdf_bbox 涂黑

**Files:**
- Modify: `src-tauri/src/parser/pdf_export.rs`

- [ ] **Step 1: 在 `doc` 打开之后收集 pdf_bbox 涂黑区域**

**重要**：此代码必须放在 `let doc = pdfium.load_pdf_from_file(...)` 之后（即 pdf_export.rs 第 114 行之后），因为需要通过 `doc` 读取页面宽高。不能放在 `compute_redact_targets` 之后（第 103 行），那时 `doc` 尚未打开。

在 `export_pdf_redacted` 函数中，`let doc = ...` 之后、`for (&page_index, targets) in &redact_by_page` 循环之前添加：

```rust
// 额外收集 pdf_bbox 类型的涂黑区域（用户手动框选）
for item in sensitive_items {
    if let Some(ref bbox) = item.pdf_bbox {
        // 归一化屏幕坐标 → PDF 点坐标
        let page = doc.pages().get(bbox.page_index as u16)
            .map_err(|e| format!("读取第 {} 页失败：{}", bbox.page_index + 1, e))?;
        let pw = page.width().value;
        let ph = page.height().value;

        let targets = redact_by_page.entry(bbox.page_index).or_default();
        targets.push(RedactTarget {
            text: String::new(),  // 空 text 标记为框选类型
            left: bbox.left * pw,
            top: (1.0 - bbox.top) * ph,      // 屏幕坐标 → PDF 坐标
            right: bbox.right * pw,
            bottom: (1.0 - bbox.bottom) * ph,
        });
    }
}
```

- [ ] **Step 2: 调整涂黑逻辑——分离 text object 匹配和框选矩形**

在每页的涂黑循环中，`targets` 中 `text` 为空的是框选类型，跳过 text object 匹配。画矩形时，合并两类来源：

```rust
// 收集需要画矩形的 bbox：
// 1) 从 to_remove 中来的（成功匹配并删除的 text objects）
let mut rects_to_draw: Vec<RedactTarget> = to_remove.iter().map(|(_, t)| t.clone()).collect();
// 2) 从 pdf_bbox 来的（text 为空的框选区域，无需匹配 text object）
rects_to_draw.extend(
    targets.iter().filter(|t| t.text.is_empty()).cloned()
);

// 画黑色矩形
for rect_target in &rects_to_draw {
    let rect = PdfRect::new_from_values(
        rect_target.bottom, rect_target.left, rect_target.top, rect_target.right,
    );
    page.objects_mut().create_path_object_rect(
        rect, None, None, Some(PdfColor::BLACK),
    ).ok();
}
```

同时在 text object 匹配循环中跳过空 text 的 target：

```rust
for target in targets {
    if target.text.is_empty() { continue; } // 框选区域不匹配 text object
    // ... 现有匹配逻辑
}
```

- [ ] **Step 3: 同样更新 export_pdf_as_images 降级方案**

在 `export_pdf_as_images` 中，`compute_redact_targets` 之后、渲染循环之前，添加同样的 `pdf_bbox` 收集逻辑（此函数中 `doc` 在第 257 行已打开）。`draw_black_rect_on_image` 不依赖 `text` 字段，可以直接处理框选 targets。

- [ ] **Step 4: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/parser/pdf_export.rs
git commit -m "feat: PDF 导出支持 pdf_bbox 手动框选涂黑"
```

---

### Task 9: 集成测试 + 缩放时滚动位置保持

**Files:**
- Modify: `src/components/PdfPreviewView/index.tsx`
- Modify: `src/components/CenterPanel/ComparisonView.tsx`

- [ ] **Step 1: 缩放时保持滚动位置比例**

在 ComparisonView 中，`pdfZoom` 变化时记录并恢复滚动比例：

```typescript
const pdfLeftContainerRef = useRef<HTMLDivElement>(null);

// zoom 变化时保持滚动位置比例
useEffect(() => {
  const el = pdfLeftContainerRef.current;
  if (!el) return;
  // 浏览器会自动处理 scrollTop，因为我们使用 width 缩放
  // 但如果需要精确控制：
  // 缩放前保存比例，缩放后恢复（由于 React 渲染是同步的，使用 useLayoutEffect）
}, [pdfZoom]);
```

实际上，由于缩放通过改变 `<img>` 的 `width` 实现，浏览器在重排后 `scrollTop` 会保持绝对值不变，但内容高度变了，所以视口位置会偏移。需要在 zoom 变化前记录 `scrollTop / scrollHeight`，变化后乘以新的 `scrollHeight`。

在 PdfPreviewView 中通过 `ref` 暴露滚动容器：

```typescript
// PdfPreviewView 暴露 ref
const PdfPreviewView = forwardRef<HTMLDivElement, PdfPreviewViewProps>(...)
```

在 ComparisonView 中：

```typescript
const pdfLeftRef = useRef<HTMLDivElement>(null);
const prevZoom = useRef(pdfZoom);

useEffect(() => {
  const el = pdfLeftRef.current;
  if (!el || prevZoom.current === pdfZoom) return;
  const ratio = el.scrollTop / (el.scrollHeight - el.clientHeight || 1);
  requestAnimationFrame(() => {
    el.scrollTop = ratio * (el.scrollHeight - el.clientHeight);
  });
  prevZoom.current = pdfZoom;
}, [pdfZoom]);
```

- [ ] **Step 2: 手动功能测试**

Run: `cargo tauri dev`

测试清单：
1. 导入 PDF 文件，确认左右对比视图正常渲染
2. 点击 `+`/`-` 缩放按钮，确认两侧同步缩放
3. 滚动左侧，确认右侧同步滚动（反之亦然）
4. 缩放后滚动到某位置，再缩放，确认视口不跳变
5. 默认模式下在左侧 PDF 上选择文本，确认 TextSelectionToolbar 弹出
6. 选择类型后确认右侧显示涂黑遮罩
7. 点击「框选涂黑」按钮，确认鼠标变十字光标
8. 拖拽矩形，确认类型选择弹窗弹出
9. 选择类型后确认右侧显示涂黑遮罩
10. Escape 取消框选
11. 导出 PDF，确认涂黑区域正确

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: PDF 对比视图缩放滚动位置保持 + 集成验证"
```
