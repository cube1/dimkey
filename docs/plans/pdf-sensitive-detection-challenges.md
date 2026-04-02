# PDF 敏感数据识别问题深入分析

> 日期：2026-03-12
> 基础调研：见 `pdf-redaction-research.md`
> 本文聚焦：PDF 中敏感信息**识别**环节的具体问题与解决方案

---

## 一、问题总览

PDF 敏感数据识别面临的挑战远超 Excel/CSV/Word，根本原因在于 **PDF 是一种视觉呈现格式，而非结构化数据格式**。以下逐一分析核心问题。

---

## 二、六大核心问题

### 问题 1：文本提取的碎片化

**现象**：PDF 中一段完整的中文句子，底层可能被拆成多个 text object：

```
% 视觉上显示为："张三的身份证号是 110101199003071234"
% 但 Content Stream 中可能是：
BT /F1 12 Tf 72 700 Td (张三的身份证号是 ) Tj ET
BT /F1 12 Tf 230 700 Td (110101) Tj ET
BT /F1 12 Tf 280 700 Td (199003071234) Tj ET
```

**影响**：
- 正则引擎需要跨 text object 拼接才能匹配完整的身份证号
- NER 模型接收碎片化文本时，上下文不完整，识别准确率下降
- 字典匹配可能因词被拆分而漏报

**解决方案**：
1. **空间聚合算法**：根据字符 bounding box 的空间接近性（水平距离 < 字符宽度阈值），将相邻 text object 合并为逻辑文本行
2. **双层文本结构**：维护"原始 text object → 逻辑文本行"的映射关系，识别在逻辑行上进行，定位回溯到原始 object
3. 实现参考：PDFium 的 `FPDFText_GetText()` 已内置基础的文本流重组能力

### 问题 2：文本编码与字体映射

**现象**：PDF 中文本的存储编码不是 Unicode，而是字体内部的 glyph index：

```
% 同样是 "张三"，不同 PDF 可能存储为：
(张三)              % 直接 Unicode（少见）
<5F20 4E09>         % Unicode code points（hex）
<0012 0034>         % CID（Character ID，需要 CMap 映射）
<A1B2 C3D4>         % 自定义编码（需要 ToUnicode CMap）
```

**影响**：
- 如果 PDF 缺少 ToUnicode CMap，提取出的文本可能是乱码或 glyph index，无法进行任何敏感识别
- 嵌入子集字体（subset font）可能只包含文档中用到的字符，glyph ID 是重新编号的
- 某些 PDF 生成器（如部分扫描仪驱动）生成的 PDF 没有正确的字符映射

**解决方案**：
1. 使用 PDFium 等成熟库提取文本——它们内置了 CMap/ToUnicode 解析
2. 对于无法解码的文本区域，标记为"需 OCR"，回退到 OCR 流程
3. 在识别结果中标注置信度：来自正确 Unicode 映射的文本置信度高，来自猜测映射的置信度低

### 问题 3：扫描件与图片型 PDF

**现象**：PDF 页面内容实际是一张大图片，没有文本层：

```
% Content Stream 只有一个 image 操作：
q 595.28 0 0 841.89 0 0 cm /Im0 Do Q
```

**影响**：
- 文本提取返回空字符串，三层识别引擎完全失效
- 混合型 PDF（部分页面是文本，部分是扫描）更加复杂

**解决方案**：
1. **检测策略**：提取文本后检查每页文本量，如果字符数 < 阈值（如 50 字/页），判定为扫描页
2. **OCR 集成**（v0.1 不做，规划到后续版本）：
   - Rust 方案：`leptess` crate（Tesseract OCR 绑定），需随应用分发 Tesseract + 中文语言包（~30MB）
   - 轻量方案：用 PDFium 渲染页面为图片后，调用系统 OCR API（macOS Vision / Windows OCR）
3. **用户提示**：识别到扫描页时，提示用户"该页面为扫描件，无法提取文本，建议先使用 OCR 工具处理"

### 问题 4：文本空间定位与阅读顺序

**现象**：PDF 中文本没有逻辑阅读顺序，只有坐标位置：

```
% 两栏布局：左栏和右栏的文本在 Content Stream 中可能交错出现
BT 72 700 Td  (左栏第一行) Tj ET
BT 310 700 Td (右栏第一行) Tj ET
BT 72 685 Td  (左栏第二行) Tj ET
BT 310 685 Td (右栏第二行) Tj ET
```

**影响**：
- 简单拼接所有文本可能导致跨栏混合，如"左栏第一行右栏第一行左栏第二行…"
- NER 模型依赖上下文语序，错误的阅读顺序会严重影响识别准确率
- 表格中的文本定位更加复杂

**解决方案**：
1. **空间排序算法**：
   - 按 Y 坐标（从上到下）分组为"行带"（Y 坐标相近的归为一行）
   - 每个行带内按 X 坐标（从左到右）排序
   - 检测多栏布局：如果文本块在 X 方向有明显的间隔带，按栏分别处理
2. **PDFium 内置支持**：`FPDFText_GetText()` 已实现基础的阅读顺序重建
3. **分区识别**：对于复杂布局，按页面区域（zone）分别提取文本并独立运行识别引擎

### 问题 5：PDF 中的非文本敏感信息

**现象**：敏感信息可能不在可见文本中，而在 PDF 的其他结构中：

| 位置 | 示例 | 风险 |
|------|------|------|
| **元数据（Metadata）** | 作者、创建者、标题、主题 | 泄露文档作者姓名、公司名 |
| **书签（Bookmarks/Outlines）** | 目录项："张三的合同" | 泄露人名 |
| **注释（Annotations）** | 批注、高亮注释的内容 | 泄露审批意见中的敏感信息 |
| **表单字段（AcroForm）** | 填写的姓名、地址、身份证号 | 最常见的敏感数据来源 |
| **附件（Embedded Files）** | 内嵌的 Excel/Word 文件 | 附件中的敏感数据 |
| **JavaScript** | 表单脚本中硬编码的数据 | 罕见但可能 |
| **XMP 元数据** | XML 格式的扩展元数据 | 可能包含编辑历史 |

**解决方案**：
1. **全量扫描**：不仅提取页面文本，还需遍历元数据、书签、注释、表单字段
2. PDFium 提供了对应的 API：
   - `FPDF_GetMetaText()` — 元数据
   - `FPDFBookmark_*()` — 书签
   - `FPDFAnnot_*()` — 注释
   - `FPDF_GetFormType()` + `FPDFDoc_*()` — 表单
3. **清洗策略**：脱敏时，可选择直接清除所有元数据和注释（最安全），或逐一扫描并脱敏

### 问题 6：性能与大文件处理

**现象**：PDF 可能有数百页，每页包含大量文本和图形对象。

**影响**：
- 逐页提取文本 + 字符坐标的开销比 Excel/Word 大得多
- NER 模型处理大量文本时耗时长，如果按页提交则需要处理跨页实体
- 用户需要等待时间更长

**解决方案**：
1. **渐进式加载**：先提取前几页并展示结果，后续页面异步处理
2. **分页并行**：多页可并行提取文本（PDFium 支持多线程访问不同页面）
3. **流式反馈**：复用现有架构（规则引擎先出快速结果 → NER 异步补充），对每一页先返回正则结果
4. **页面范围选择**：允许用户指定需要脱敏的页面范围，避免处理无关页面

---

## 三、与现有三层引擎的集成方案

### 数据流设计

```
PDF 文件
  ↓
PDFium 打开文档
  ↓
逐页提取 → PdfPageContent {
    page_index: usize,
    text_blocks: Vec<TextBlock>,   // 带 bbox 的文本块
    full_text: String,             // 合并后的完整文本（用于识别引擎）
    metadata: HashMap<String, String>,  // 元数据
    annotations: Vec<AnnotationText>,   // 注释文本
    form_fields: Vec<FormFieldText>,    // 表单字段
}
  ↓
文本送入现有三层引擎
  ├── regex_engine.detect() → Vec<SensitiveItem>  （毫秒级）
  ├── ner_engine.detect()   → Vec<SensitiveItem>  （秒级，异步）
  └── dict_engine.detect()  → Vec<SensitiveItem>  （即时）
  ↓
SensitiveItem 中的 start/end 偏移量 → 映射回 TextBlock 的 bbox
  ↓
前端按页渲染 PDF 预览，在 bbox 位置叠加高亮标注
```

### 模型扩展

```rust
// FileContent 新增变体
enum FileContent {
    Spreadsheet { sheets: Vec<SheetData> },
    Document { paragraphs: Vec<Paragraph>, encoding: Option<String> },
    Pdf { pages: Vec<PdfPageData> },  // ← 新增
}

// PDF 页面数据
struct PdfPageData {
    page_index: usize,
    width: f64,
    height: f64,
    text_blocks: Vec<PdfTextBlock>,
    full_text: String,  // 重组后的完整文本
}

// PDF 文本块（带位置信息）
struct PdfTextBlock {
    text: String,
    bbox: BoundingBox,       // 在页面上的位置
    char_offsets: Vec<usize>, // 每个字符在 full_text 中的偏移
}

struct BoundingBox {
    x: f64,      // 左下角 X
    y: f64,      // 左下角 Y
    width: f64,
    height: f64,
}

// SensitiveItem 扩展
struct SensitiveItem {
    // ... 现有字段 ...
    page_index: Option<usize>,    // PDF 页码
    bbox: Option<BoundingBox>,    // PDF 中的位置
}
```

### 脱敏策略限制

| 策略 | Excel/Word/CSV | PDF |
|------|---------------|-----|
| 掩码（Mask） | ✅ 文本替换为 `***` | ⚠️ 需删除原文本 object + 绘制黑色矩形 |
| 替换（Replace） | ✅ 替换为假数据 | ❌ 不可靠——字体可能不包含替换字符的 glyph |
| 泛化（Generalize） | ✅ 如"北京市朝阳区" → "北京市" | ❌ 同上，且无法保证排版对齐 |
| 涂黑（Blackout） | 不需要 | ✅ **PDF 专属策略**——删除文本 + 画黑框 |

**结论**：PDF 脱敏策略应限定为**涂黑（Blackout）**，这是唯一能保证安全且不引入排版问题的方式。

---

## 四、实施优先级建议

### Phase 1：基础 PDF 文本识别（最小可行）

1. 集成 `pdfium-render`，实现 `parser/pdf.rs`
2. 提取页面文本（使用 PDFium 内置的文本流重组）
3. 复用三层识别引擎扫描文本
4. 前端展示：逐页 PDF 预览 + 高亮标注（基于 bbox）
5. 脱敏：仅支持涂黑策略
6. 扫描件页面：检测并提示用户，暂不支持 OCR

### Phase 2：增强功能

1. 元数据/书签/注释/表单的扫描与清洗
2. 分页并行处理优化
3. 支持加密 PDF（输入密码后解密）

### Phase 3：OCR 支持（远期）

1. 集成 OCR 引擎（Tesseract 或系统 OCR）
2. 自动检测扫描页并触发 OCR
3. OCR 结果与文本层结果合并

---

## 五、补充调研：其他可选 Rust 库

除了 `pdf-redaction-research.md` 中已评估的库，还有以下值得关注的选项：

| 库 | 特点 | 许可证 | 适用场景 |
|---|------|--------|---------|
| `pdf_oxide` | 号称 100% 通过 3830 个测试 PDF，平均 0.8ms 提取，5x 快于 PyMuPDF | 待确认 | 高性能文本提取 |
| `pdfplumber-rs` | Python pdfplumber 的 Rust 移植，保留字符位置、词边界、表格结构 | 待确认 | 布局感知的文本提取 |
| `ocrs` | 纯 Rust OCR，使用 ONNX 模型（RTen 引擎），无 C 依赖 | MIT | 与项目现有 ONNX 基础设施天然匹配 |
| `oxidize-pdf` | 纯 Rust PDF 核心 + 可选 Tesseract OCR，759 个真实 PDF 测试 98.8% 成功率 | **AGPL** | 许可证不可接受 |

**特别注意 `ocrs`**：该库使用 ONNX 模型进行 OCR，与本项目已有的 `ort` crate 基础设施高度契合。但目前仅支持拉丁字母，中文支持待确认。

---

## 六、安全警示：PDF 脱敏的常见失败案例

根据安全研究（Argelius Labs 2025），对 75 个安全机构的近 40,000 份 PDF 进行分析发现：**65% 的"已脱敏"文件仍然暴露了隐藏信息**。2025 年 Epstein 文件公开事件也暴露了同样的问题。

常见的**不安全做法**（必须避免）：
1. **仅画黑框覆盖**：底层文本仍可通过复制粘贴、PDF 对象浏览器提取
2. **修改字体颜色为白色**：文本仍在，选中即可显示
3. **使用高亮注释遮盖**：注释可被删除，露出原文
4. **增量保存**：旧内容保留在文件中，可通过取消增量更新恢复

**正确的脱敏必须**：
- 从 Content Stream 中**删除**文本绘制指令
- 清理元数据、书签、注释、表单、XMP 中的残留
- 使用**完整重写**（非增量保存）消除孤立对象
- 删除已脱敏字符的 ToUnicode CMap（防止通过 glyph 映射反推字符）
- 输出后**验证**：对结果 PDF 再次尝试文本提取，确认敏感内容已不可恢复

---

## 七、风险与注意事项

1. **PDFium 动态库体积**：约 20-30MB，会显著增加应用体积。可考虑按需下载或可选安装
2. **跨平台编译**：需为 macOS (arm64 + x86_64) 和 Windows (x86_64) 分别准备 PDFium 预编译库
3. **文本提取准确率**：无法保证 100% 准确——某些 PDF 生成器产出的文档结构非标准
4. **安全性**：PDF 脱敏必须使用"真涂黑"（删除底层文本），仅视觉遮盖是**不安全的**
5. **前端预览**：PDF 预览可用 `pdf.js` 或直接用 PDFium 渲染为图片，需权衡加载速度和交互性
6. **工程量评估**：PDF 支持的实现难度**远超 Excel/Word/CSV 之和**——建议充分 POC 验证后再规划排期
