# PDF 脱敏技术调研报告

> 调研日期：2026-03-11
> 需求：PDF 文件中敏感信息的**真涂黑**（不可恢复），底层文本必须被永久删除

---

## 一、核心需求分析

PDF 脱敏（Redaction）与 Word/Excel 不同，有以下关键难点：

1. **PDF 不是结构化文档**：PDF 本质上是一种"绘图指令语言"，文本以 `BT`/`ET`（Begin/End Text）操作符块的形式存储在 Content Stream 中，没有"单元格"或"段落"的概念
2. **文本定位需要计算**：要知道某段文字在页面上的位置（bounding box），需要解析字体矩阵（`Tf`）、文本矩阵（`Tm`/`Td`）等操作符并进行坐标计算
3. **真脱敏 ≠ 画黑框**：仅在视觉层叠加黑色矩形是**不安全的**，底层文本仍然可以被复制、搜索、提取。必须同时删除 Content Stream 中的文本数据
4. **隐藏信息多样**：敏感信息可能存在于 Content Stream、元数据（Metadata）、书签（Bookmarks）、注释（Annotations）、表单字段（Form Fields）等多处

### 真涂黑的标准流程

1. 识别敏感文本及其在页面上的精确位置（bounding box）
2. **从 Content Stream 中删除对应的文本绘制指令**
3. 在对应位置绘制黑色填充矩形
4. 清除元数据、书签等中可能残留的敏感信息
5. 保存为新文件（增量保存可能保留历史，应完整重写）

---

## 二、Rust 生态 PDF 库评估

### 方案 A：`mupdf-rs`（MuPDF 的 Rust 绑定）

| 维度 | 评估 |
|------|------|
| **能力** | MuPDF 底层 C 库**原生支持 Redaction API**（添加 Redact 注释 → apply_redactions 永久删除文本） |
| **文本提取+坐标** | 支持，MuPDF 能返回带 bounding box 的文本块 |
| **Rust 绑定完整度** | `PdfAnnotationType::Redact` 已暴露；`PdfPage::redact()` 方法可能已在最新版本中可用（对应底层 `mupdf_pdf_redact_page`），需验证。MuPDF 的 redaction 是逐字符操作的——字符 bounding box 与 redaction 矩形有交叠即被删除，Content Stream 会被重写 |
| **许可证** | **AGPL-3.0**（强传染性）。如果链接 MuPDF，整个应用必须以 AGPL 开源并公开全部源码。商业使用需向 Artifex 购买商业许可 |
| **跨平台** | macOS / Windows 均支持，需编译 MuPDF C 库 |
| **包体积** | MuPDF 静态链接约增加 15-25MB |
| **编译** | 需从 C 源码编译 MuPDF，编译时间较长；Windows 上 Visual Studio 兼容性有报告问题 |

**结论**：技术上最成熟的方案，但 **AGPL 许可证是重大障碍**。除非购买商业许可或项目以 AGPL 开源，否则不可用。

### 方案 B：`pdfium-render`（PDFium 的 Rust 绑定）

| 维度 | 评估 |
|------|------|
| **能力** | PDFium（Chromium 的 PDF 库）支持注释操作，包括 `FPDF_ANNOT_REDACT` 类型 |
| **文本提取+坐标** | 支持文本提取和搜索，可获取字符级坐标 |
| **Rust 绑定完整度** | v0.8.37（活跃维护），暴露了 436+ FPDF_* 函数。注释相关 API（`FPDFAnnot_*`）已绑定，但 redaction 的 apply 操作需要确认 |
| **许可证** | **BSD 3-Clause**（宽松许可），可自由商业使用 |
| **跨平台** | 需要 PDFium 动态库（.dylib/.dll），需随应用分发预编译的 PDFium 二进制 |
| **包体积** | PDFium 动态库约 20-30MB |

**PDFium 实现 Redaction 的具体路径**（无内置 Redact API，需自行组装）：
1. 用 `PdfPageText` / `PdfPageTextChar` 提取字符级文本 + bounding box
2. 用 `PdfPageObjects::remove_object()` 删除包含敏感文本的 text object
3. 用 `PdfPathObject` 创建黑色填充矩形并添加到页面
4. 内部调用 `FPDFPage_GenerateContent()` 重写 Content Stream

**注意**：PDFium 的删除是 object 级别而非 character 级别。如果一个 text object 同时包含敏感和非敏感文本，需要处理部分删除的情况。

**PDFium 预编译库**：可从 [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries/releases) 获取各平台预编译版本，也可用 `pdfium-auto` crate 自动下载。

**结论**：许可证友好，库能力强。需要自行实现 redaction 逻辑（中等工程量），PDFium 动态库约 30-50MB 需随应用分发。

### 方案 C：`lopdf`（纯 Rust）

| 维度 | 评估 |
|------|------|
| **能力** | 纯 Rust 的 PDF 操作库，可解析和修改 Content Stream、添加绘图操作 |
| **文本提取+坐标** | **不原生支持**。需自行解析文本操作符（`BT`/`ET`/`Tj`/`TJ`/`Td`/`Tm`/`Tf`）并计算字体矩阵来推算坐标，工程量大且容易出错 |
| **修改 Content Stream** | 支持。可以 decode → filter 操作符 → re-encode |
| **绘制黑色矩形** | 支持。通过 `re`（rectangle）+ `f`（fill）操作符实现 |
| **许可证** | **MIT**（最宽松） |
| **跨平台** | 纯 Rust，无外部依赖，编译即用 |
| **包体积** | 极小，增量约 1-2MB |
| **维护状态** | 活跃，v0.39.0（2025-01），4.6M+ 下载 |

**结论**：许可证和体积最优，但**文本定位是最大挑战**。需要自行实现完整的 PDF 文本布局引擎（解析 `Tm`/`Td`/`TD`/`T*`/`Tf`/`Tj`/`TJ`/`'`/`"` 等所有文本操作符 + 字体度量 + CMap/ToUnicode 映射 + CID/Type0 字体支持），工程量巨大（估计数周至数月），且对各种真实 PDF 的兼容性难以保证。

### 方案 D：`lopdf` + `pdf-extract`（组合方案）

| 维度 | 评估 |
|------|------|
| **思路** | 用 `pdf-extract`（基于 `pdf` crate）提取文本，用 `lopdf` 修改 Content Stream 和绘制黑框 |
| **文本提取** | `pdf-extract` 可提取文本，但**坐标精度有限** |
| **许可证** | 均为 MIT/Apache-2.0 |
| **可行性** | 中等。两个库对 PDF 内部结构的理解不同，对齐坐标系有难度 |

### 方案 E：渲染为图片再重建（最安全但有损）

| 维度 | 评估 |
|------|------|
| **思路** | PDF → 逐页渲染为高分辨率图片 → 在图片上涂黑 → 图片重组为新 PDF |
| **安全性** | **最高**。原始 PDF 的所有结构（文本层、元数据、隐藏对象、JS）全部销毁，只剩像素 |
| **代价** | 输出 PDF **不可搜索**、不可选中文本；文件体积可能增大；清晰度取决于渲染分辨率 |
| **实现** | 渲染用 `pdfium-render` 或 `mupdf-rs`，输出 PDF 用 `printpdf` 或 `lopdf` |
| **许可证** | 取决于选用的渲染库 |

---

## 三、方案对比总结

| 方案 | 安全性 | 实现难度 | 许可证 | 输出质量 | 推荐度 |
|------|--------|---------|--------|---------|--------|
| A. mupdf-rs | ★★★★★ | ★★☆ (需 unsafe FFI) | AGPL ❌ | 原生 PDF | 许可证不可接受 |
| B. pdfium-render | ★★★★★ | ★★★ | BSD ✅ | 原生 PDF | **推荐（首选）** |
| C. lopdf 纯 Rust | ★★★★☆ | ★★★★★ (极难) | MIT ✅ | 原生 PDF | 文本定位难度过高 |
| D. lopdf + pdf-extract | ★★★★☆ | ★★★★☆ | MIT ✅ | 原生 PDF | 坐标对齐有风险 |
| E. 渲染为图片 | ★★★★★+ | ★★☆ | 取决于渲染库 | 图片 PDF（不可搜索） | **推荐（最安全备选）** |

---

## 四、推荐方案

### 首选：方案 B — 基于 PDFium（`pdfium-render`）

**理由**：
1. PDFium 是 Chromium 使用的 PDF 引擎，成熟度极高
2. BSD 许可证，可自由商业使用
3. 支持文本提取 + 坐标、注释操作、页面渲染
4. Rust 绑定活跃维护（v0.8.37，2025-11 更新）
5. 即使高层 API 不完整，可通过 FFI 调用底层 PDFium 函数

**实现路径**：
1. 用 PDFium 打开 PDF，提取每页文本及其 bounding box
2. 将文本送入现有的三层识别引擎（regex → NER → dict）
3. 对识别出的敏感文本，通过 PDFium API 创建 Redact 注释并应用
4. 如果 PDFium 的 Redact 注释 apply 不可用，则退回到方案 E（渲染为图片）作为保底

**需要解决的问题**：
- PDFium 动态库的分发：需要为 macOS (arm64/x86_64) 和 Windows (x86_64) 预编译 PDFium 并随 Tauri 应用打包
- 验证 `pdfium-render` 中 Redact 注释的完整操作流程

### 保底：方案 E — 渲染为图片

如果 PDFium 的原生 Redaction API 不完整，可用 PDFium 渲染 + `printpdf` 重建的方式实现。此方案安全性最高，但输出的 PDF 不可搜索。

**实现路径**：
1. PDFium 逐页渲染为 300dpi PNG
2. 文本识别沿用方案 B 的流程
3. 在图片上用黑色矩形覆盖敏感区域
4. 用 `printpdf` 将图片重组为 PDF

---

## 五、与现有架构的集成点

当前项目 `FileContent` 枚举有 `Spreadsheet` 和 `Document` 两种变体，需要新增 PDF 相关的数据模型：

```
parser/
  ├── excel.rs
  ├── word.rs
  ├── word_export.rs
  ├── txt.rs
  └── pdf.rs          ← 新增：PDF 解析与导出
```

**数据模型扩展思路**：
- `FileContent` 新增 `Pdf` 变体，包含逐页的文本块（含坐标）
- `SensitiveItem` 现有的 `start`/`end` 偏移量需扩展为 `bbox`（bounding box）用于 PDF 定位
- 脱敏策略中，PDF 仅支持"涂黑"（Mask/Blackout），不支持替换或泛化（因为字体/排版无法精确复现）

---

## 六、下一步行动建议

1. **POC 验证**：基于 `pdfium-render` 编写一个最小可行原型，验证：
   - 能否提取文本 + bounding box
   - 能否创建并 apply Redact 注释
   - 若不行，验证渲染为图片再重建的流程
2. **PDFium 分发方案**：调研 Tauri 打包 PDFium 动态库的最佳实践（sidecar binary 或 resources）
3. **许可证复核**：确认 PDFium BSD 许可证在具体分发场景下的合规性

---

## 参考资源

- [mupdf-rs GitHub](https://github.com/messense/mupdf-rs)
- [mupdf-rs crates.io](https://crates.io/crates/mupdf)
- [pdfium-render GitHub](https://github.com/ajrcarey/pdfium-render)
- [pdfium-render crates.io](https://crates.io/crates/pdfium-render)
- [lopdf GitHub](https://github.com/J-F-Liu/lopdf)
- [lopdf crates.io](https://crates.io/crates/lopdf)
- [pdf-extract crates.io](https://crates.io/crates/pdf-extract)
- [PDFium 源码 (Chromium)](https://github.com/chromium/pdfium)
- [Artifex MuPDF 许可证说明](https://artifex.com/licensing)
- [Michael F. Bryan: Parsing PDFs in Rust](https://adventures.michaelfbryan.com/posts/parsing-pdfs-in-rust)
- [PDFium 预编译二进制 (bblanchon)](https://github.com/bblanchon/pdfium-binaries/releases)
- [pdfium-auto crate](https://crates.io/crates/pdfium-auto)
- [PyMuPDF Redaction 文档](https://pymupdf.readthedocs.io/en/latest/the-basics.html)
- [MuPDF Redaction API](https://webviewer-docs.mupdf.com/api-reference/redaction)
