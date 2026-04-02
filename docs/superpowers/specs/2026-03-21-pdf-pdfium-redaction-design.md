# PDF 脱敏方案升级设计 — 基于 PDFium 的原地涂黑

> 日期：2026-03-21
> 状态：待实施

---

## 一、背景与问题

当前 PDF 脱敏实现采用 `pdf_extract`（提取纯文本）+ `printpdf`（从零生成新 PDF）的方式。导出后原始 PDF 的所有样式（字体、布局、图片、颜色等）全部丢失，输出的 PDF 仅包含纯文本内容。

根本原因：解析阶段只提取了文本，丢弃了所有样式和坐标信息；导出阶段用固定的 A4 模板重新排版，无法还原原始布局。

## 二、设计目标

1. 脱敏后的 PDF 保留原始样式（布局、字体、图片等），仅敏感文本被涂黑
2. 涂黑为"真涂黑"——底层文本被删除，不可通过复制/搜索/提取恢复
3. 操作失败时自动降级为图片化方案，保证功能可用
4. PDFium 动态库打包进应用，零网络通信

## 三、技术选型

**确定方案**：PDFium 手动操作 text object

- 使用 `pdfium-render` crate（BSD 3-Clause 许可）
- 通过 PDFium API 提取文本+坐标、删除 text object、画黑色矩形
- 不依赖 Redact 注释 API（可用性未经验证），直接操作 text object 层

**降级方案**：渲染为图片

- PDFium 逐页渲染为 300dpi 位图 → 涂黑 → `printpdf` 重组为 PDF
- 输出为图片型 PDF（不可搜索/选中文本）

**脱敏策略**：仅支持涂黑（黑色矩形覆盖 + 底层文本删除），不支持替换和泛化。

## 四、依赖变更

### 移除

- `pdf_extract` — 文本提取改用 PDFium

### 保留（仅用于降级路径）

- `printpdf` — 主路径不再使用，仅在图片降级方案中用于将位图重组为 PDF

### 新增

- `pdfium-render` — PDFium 的 Rust 绑定

### PDFium 动态库分发

打包进 `src-tauri/resources/`，按平台分发：

| 平台 | 文件 | 预计大小 |
|------|------|---------|
| macOS arm64 | `libpdfium.dylib` | ~20-30MB |
| macOS x86_64 | `libpdfium.dylib` | ~20-30MB |
| Windows x86_64 | `pdfium.dll` | ~20-30MB |

预编译库来源：[bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries/releases)

## 五、数据模型扩展

### 新增类型

```rust
/// 单个 text object 的信息（用于导出时重新匹配）
pub struct PdfTextObject {
    pub text: String,    // 文本内容（用于导出时按内容+坐标重新匹配，不依赖索引）
    pub bbox: BBox,      // 包围盒
    pub char_offset: usize,  // 该 object 的文本在所属段落中的字符偏移量（Unicode 字符计数，非字节数，用 .chars().count()）
}

/// PDF 文本块的页面坐标信息
pub struct PdfTextPosition {
    pub page_index: usize,              // 所在页码（0-based）
    pub text_objects: Vec<PdfTextObject>, // 组成该段落的所有 text object
    pub bbox: BBox,                      // 段落整体包围盒
}

pub struct BBox {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}
```

> **设计决策**：不存储 PDFium 的 object index，因为删除操作会导致后续 object 的索引偏移。
> 导出时通过 text + bbox 坐标重新匹配 text object，更加健壮。

### Paragraph 扩展

在 `Paragraph` 结构体中新增可选字段：

```rust
pub struct Paragraph {
    pub index: usize,
    pub text: String,
    pub style: String,
    pub table_position: Option<TablePosition>,
    #[serde(skip)]  // 不序列化到前端，仅 Rust 侧使用
    pub pdf_position: Option<PdfTextPosition>,  // 新增：PDF 坐标信息
}
```

> **注意**：`pdf_position` 标记为 `#[serde(skip)]`，不随 IPC 发送到前端。前端不需要坐标信息，预览仍基于 `text` 字段。
> `pdf_position` 需要加 `#[serde(default)]` 以兼容反序列化。

## 六、模块结构

```
parser/
  ├── pdf.rs          ← 重写：用 PDFium 解析，提取文本+坐标
  └── pdf_export.rs   ← 新增：用 PDFium 执行涂黑并导出
```

## 七、核心流程

### 7.1 解析阶段（`parser/pdf.rs`）

1. PDFium 打开 PDF 文件
2. 遍历每一页（`PdfPage`）
3. 获取页面所有 text object（`PdfPageObjects` → 过滤 text 类型）
4. 对每个 text object：
   - 提取文本内容（`.text()`）
   - 获取 bounding box（`.bounds()`）
   - 记录 `page_index`、文本内容、bounding box（构建 `PdfTextObject`）
5. 将同一页的 text object 按阅读顺序排序（从上到下、从左到右）
6. 合并为段落：将 y 坐标差值小于行高（取当前 object 高度的 1.2 倍）的相邻 text object 合并为同一段落。同时记录每个 object 在段落文本中的字符偏移量（`char_offset`）
7. 输出 `FileContent::Document`，每个 `Paragraph` 携带 `PdfTextPosition`

> **v1 限制**：段落合并假设单栏、从左到右的文本布局。多栏 PDF 可能导致跨栏合并。此为已知限制，后续版本可通过 x 坐标聚类检测多栏。

### 7.2 脱敏导出阶段（`parser/pdf_export.rs`）

1. 接收原始 PDF 路径 + 脱敏项列表（含 `PdfTextPosition` 坐标信息）
2. PDFium 重新打开原始 PDF
3. 按页分组脱敏项

#### SensitiveItem → text object 的映射

`SensitiveItem` 的 `start`/`end` 是段落内的字符偏移量。通过 `PdfTextPosition.text_objects` 中每个 object 的 `char_offset` 和 `text.len()`，可以定位哪些 text object 包含敏感字符：

```
段落文本: "张三的手机号是13812345678"
object A: char_offset=0,  text="张三的手机号是"  (offset 0..7)
object B: char_offset=7,  text="13812345678"    (offset 7..18)
SensitiveItem: start=7, end=18 → 命中 object B
```

4. 对每一页：
   - 通过文本内容 + bbox 坐标重新匹配当前页的 text object（不依赖索引，bbox 匹配使用 0.1 PDF points 的容差）
   - 确定需要涂黑的 text object 后，**按 object 索引从大到小排序**再执行删除，避免索引偏移问题
   - 如果整个 object 都是敏感文本 → 直接 `remove_object()`
   - 如果 object 部分敏感 → 整个 object 移除，非敏感部分尝试用相同字体字号重新创建 text object；重绘失败则整个涂黑
   - 在被删除 object 的 bbox 位置画黑色填充矩形（`PdfPathSegments` 矩形 + 黑色填充）
   - `GenerateContent()` 重写页面 Content Stream
5. 清除文档级元数据（标题、作者、关键词等）
6. 保存为新文件（完整写出，非增量保存）

### 7.3 降级为图片方案

触发条件：PDFium 操作过程中返回错误（加密 PDF、损坏结构等）

1. PDFium 逐页渲染为 300dpi 位图（`PdfPage::render()`）
2. 在位图上根据 bbox 坐标绘制黑色填充矩形
3. 用 `printpdf` 将位图逐页嵌入新 PDF
4. 返回结果，附带降级提示

### 7.4 部分敏感的 text object 处理

一个 text object 可能同时包含敏感和非敏感文本。处理策略：

1. 记录原 object 的完整文本、字体、字号、坐标
2. 删除整个 object
3. 在敏感文本对应的 bbox 区域画黑色矩形
4. 在非敏感部分的坐标位置，用相同字体字号重新创建 text object
5. 如果重绘失败（字体不可用等），退回到整个 object 一起涂黑——宁可多涂不漏涂

## 八、PDFium 生命周期管理

- 在 Tauri `setup` 钩子中初始化 `Pdfium` 实例
- 存入 `AppState`（Tauri managed state）
- 各 command 通过 `State<AppState>` 获取 PDFium 实例复用
- 启动时检测 PDFium 动态库是否可加载，不可用则禁用 PDF 功能并提示用户

### 线程安全

`Pdfium` 结构体本身只是动态库的句柄，可以安全地跨线程共享。但 `PdfDocument` 实例**不是线程安全的**，必须在单个 command 调用内创建和消费，不存入 `AppState`。即：每次解析或导出操作都重新 `pdfium.load_pdf_from_file()`，操作完成后 `PdfDocument` 随函数返回自动释放。

### 库路径解析

PDFium 动态库路径解析逻辑参照现有 NER 模型加载模式（`lib.rs` 中的 `resource_dir` 解析）：

1. 生产环境：`app.path().resource_dir()` → `resources/libpdfium.dylib`（或 `.dll`）
2. 开发环境：`src-tauri/resources/libpdfium.dylib` 作为 fallback

## 九、错误处理

| 场景 | 处理方式 |
|------|---------|
| PDF 加密/有密码 | 返回错误提示"该 PDF 已加密，暂不支持脱敏" |
| 扫描件（无文本层） | 解析阶段检测到 text object 数量为 0，返回"该 PDF 为扫描件（图片型），暂不支持" |
| PDFium 操作失败 | 自动降级为图片方案，附带提示 |
| PDFium 动态库加载失败 | 启动时检测，禁用 PDF 功能并提示用户 |
| 部分页面处理失败 | 已成功的页面保留，失败页面降级为图片渲染 |

## 十、前端交互变化

- **策略选择**：PDF 文件时，前端只展示"涂黑"策略，隐藏"替换"和"泛化"选项
- **降级提示**：如果后端返回降级标识，前端展示提示信息
- **预览**：脱敏前后对比预览逻辑不变，仍基于 `Paragraph.text` 渲染

## 十一、测试策略

- **单元测试**：准备典型 PDF 测试文件（纯文本、含图片混排、多页），验证解析出的文本和坐标正确性
- **集成测试**：端到端测试涂黑后的 PDF 是否确实删除了底层文本（用文本提取再验证）
- **边界测试**：加密 PDF、空 PDF、扫描件、超大文件

## 十二、已知限制

以下场景在 v1 中**不覆盖**，作为已知限制记录：

- **PDF 表单字段**：表单中的敏感数据不会被识别和脱敏
- **注释/批注**：PDF 注释中的文本不在扫描范围内
- **书签/目录**：书签标题中的敏感信息不处理
- **嵌入文件/附件**：PDF 中嵌入的其他文件不处理
- **JavaScript**：嵌入的 JS 脚本不处理
- **多栏布局**：段落合并假设单栏布局，多栏 PDF 可能导致文本合并错误
- **大文件内存**：图片降级方案中，100 页 A4 @ 300dpi 约需 2.5GB 内存。建议对超过 50 页的 PDF 在降级时逐页处理，避免一次性加载所有位图
