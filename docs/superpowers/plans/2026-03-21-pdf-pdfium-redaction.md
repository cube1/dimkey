# PDF PDFium 涂黑脱敏实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用 PDFium 替代 pdf_extract + printpdf，实现 PDF 原地涂黑脱敏，保留原始样式。

**Architecture:** 用 `pdfium-render` crate 通过 PDFium 动态库操作 PDF。解析阶段提取文本+坐标，导出阶段**重新打开原始 PDF 提取坐标**（因为 `pdf_position` 标记 `#[serde(skip)]` 不跨 IPC），根据 `SensitiveItem` 的 `row/start/end` 定位对应 text object，执行删除+画黑色矩形。操作失败时降级为 PDFium 渲染图片 + printpdf 重组。

**关键设计决策 — `pdf_position` 不跨 IPC**：`FileContent` 经过前端 IPC 往返后 `pdf_position` 会丢失（`#[serde(skip)]`）。因此导出时不依赖传入的 `pdf_position`，而是**重新解析原始 PDF 获取坐标**，再根据 `SensitiveItem` 的 `row`（段落序号）和 `start/end`（字符偏移）定位需要涂黑的 text object。这保证了数据流的正确性。

**Tech Stack:** Rust, pdfium-render, printpdf (仅降级路径), Tauri v2

**Spec:** `docs/superpowers/specs/2026-03-21-pdf-pdfium-redaction-design.md`

---

### Task 1: 添加 pdfium-render 依赖并下载 PDFium 动态库

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Create: `src-tauri/resources/pdfium/` (放置动态库)

- [ ] **Step 1: 下载 macOS arm64 PDFium 预编译库**

从 https://github.com/nickmsmith/pdfium-binaries-local/releases 或 https://github.com/nickmsmith/nickmsmith-pdfium-binaries/releases 下载 macOS arm64 版本，解压后将 `lib/libpdfium.dylib` 放入 `src-tauri/resources/pdfium/`。

```bash
# 在 src-tauri/resources/ 下创建 pdfium 目录
mkdir -p src-tauri/resources/pdfium
# 下载并解压（具体 URL 需确认，这里用占位）
# 将 libpdfium.dylib 放入 src-tauri/resources/pdfium/
```

> 注意：PDFium 动态库约 20-30MB，需要加入 .gitignore 或用 Git LFS 管理。先加入 .gitignore。

- [ ] **Step 2: 添加 pdfium-render 到 Cargo.toml**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 中添加：

```toml
pdfium-render = "0.8"
```

保留 `printpdf`（降级路径仍需要）和 `pdf-extract`（Task 10 清理阶段再移除）。

新增 `image` crate（PDFium 渲染结果处理需要）：

```toml
image = "0.25"
```

- [ ] **Step 3: 配置 tauri.conf.json 打包 PDFium 库**

修改 `src-tauri/tauri.conf.json` 的 `bundle.resources`，新增 PDFium 库：

```json
"resources": {
  "resources/ner/*": "ner/",
  "resources/pdfium/*": "pdfium/"
}
```

- [ ] **Step 4: 将 pdfium 目录加入 .gitignore**

在项目根目录 `.gitignore` 中添加：

```
src-tauri/resources/pdfium/
```

- [ ] **Step 5: 验证编译**

```bash
cd src-tauri && cargo check
```

Expected: 编译通过，无错误。pdfium-render 作为依赖被正确解析。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/tauri.conf.json .gitignore
git commit -m "feat: 添加 pdfium-render 依赖，移除 pdf-extract，配置 PDFium 库打包"
```

---

### Task 2: 数据模型扩展 — 添加 PDF 坐标类型

**Files:**
- Modify: `src-tauri/src/models/sensitive.rs:206-232`

- [ ] **Step 1: 在 sensitive.rs 中添加 BBox 和 PdfTextObject 和 PdfTextPosition 结构体**

在 `TablePosition` 结构体之前（约 line 206）添加：

```rust
/// PDF 坐标包围盒
#[derive(Debug, Clone)]
pub struct BBox {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// 单个 PDF text object 的信息
#[derive(Debug, Clone)]
pub struct PdfTextObject {
    /// 文本内容（用于导出时按内容+坐标重新匹配）
    pub text: String,
    /// 包围盒
    pub bbox: BBox,
    /// 该 object 的文本在所属段落中的字符偏移量（Unicode 字符计数）
    pub char_offset: usize,
}

/// PDF 文本块的页面坐标信息
#[derive(Debug, Clone)]
pub struct PdfTextPosition {
    /// 所在页码（0-based）
    pub page_index: usize,
    /// 组成该段落的所有 text object
    pub text_objects: Vec<PdfTextObject>,
    /// 段落整体包围盒
    pub bbox: BBox,
}
```

> 注意：这些结构体不需要 `Serialize`/`Deserialize`，因为它们标记 `#[serde(skip)]` 不跨 IPC。

- [ ] **Step 2: 在 Paragraph 中添加 pdf_position 字段**

修改 `Paragraph` 结构体（约 line 221），新增字段：

```rust
pub struct Paragraph {
    pub index: usize,
    pub text: String,
    pub style: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub table_position: Option<TablePosition>,
    /// PDF 坐标信息（仅 Rust 侧使用，不发送到前端）
    #[serde(skip)]
    #[serde(default)]
    pub pdf_position: Option<PdfTextPosition>,
}
```

- [ ] **Step 3: 修复所有构造 Paragraph 的地方**

由于 `Paragraph` 新增了字段，所有现有构造 `Paragraph` 的代码都需要加上 `pdf_position: None`。需要修改的文件：

- `src-tauri/src/parser/pdf.rs`
- `src-tauri/src/parser/word.rs`
- `src-tauri/src/parser/txt.rs`
- `src-tauri/src/commands/file.rs` — `import_clipboard_text`
- `src-tauri/src/commands/workspace.rs`
- `src-tauri/src/engine/dict_engine.rs`
- `src-tauri/src/engine/ner_engine.rs`
- `src-tauri/src/engine/regex_engine.rs`

在每个 `Paragraph { ... }` 构造中添加 `pdf_position: None,`。

- [ ] **Step 4: 验证编译**

```bash
cd src-tauri && cargo check
```

Expected: 编译通过。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/models/sensitive.rs src-tauri/src/parser/ src-tauri/src/commands/file.rs
git commit -m "feat: 添加 PDF 坐标数据模型（BBox, PdfTextObject, PdfTextPosition）"
```

---

### Task 3: PDFium 初始化 — 在 Tauri setup 中加载 PDFium 库

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 PdfiumState 类型并在 setup 中初始化**

在 `lib.rs` 中 `NerEngineState` 定义之后，添加：

```rust
use pdfium_render::prelude::*;

/// PDFium 库句柄全局状态
pub struct PdfiumState(pub Option<Pdfium>);
```

在 `setup` 闭包中（NER 引擎初始化之后），添加 PDFium 初始化逻辑：

```rust
// 初始化 PDFium（从 resources/pdfium/ 加载动态库）
let pdfium_dir = resource_dir.join("pdfium");
let pdfium_dir = if pdfium_dir.join("libpdfium.dylib").exists() || pdfium_dir.join("pdfium.dll").exists() {
    pdfium_dir
} else {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join("pdfium")
};

let pdfium_lib = if cfg!(target_os = "macos") {
    pdfium_dir.join("libpdfium.dylib")
} else {
    pdfium_dir.join("pdfium.dll")
};

let pdfium_state = match Pdfium::bind_to_library(pdfium_lib.to_string_lossy()) {
    Ok(bindings) => {
        println!("PDFium 已加载");
        PdfiumState(Some(Pdfium::new(bindings)))
    }
    Err(e) => {
        eprintln!("PDFium 加载失败，PDF 脱敏功能不可用: {}", e);
        PdfiumState(None)
    }
};
app.manage(pdfium_state);
```

- [ ] **Step 2: 验证编译**

```bash
cd src-tauri && cargo check
```

Expected: 编译通过。如果 PDFium 动态库不存在，运行时会降级（`PdfiumState(None)`）。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: 在 Tauri setup 中初始化 PDFium 库"
```

---

### Task 4: 重写 PDF 解析 — 用 PDFium 提取文本+坐标

**Files:**
- Modify: `src-tauri/src/parser/pdf.rs`

- [ ] **Step 1: 编写 PDFium 解析测试**

在 `src-tauri/src/parser/pdf.rs` 底部添加测试模块。需要先准备一个测试 PDF 文件（在 `src-tauri/tests/fixtures/` 下放一个简单的文本 PDF）。

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pdf_extracts_text() {
        // 需要 PDFium 库可用才能测试
        // 如果库不可用，跳过测试
        let pdfium = match create_test_pdfium() {
            Some(p) => p,
            None => {
                eprintln!("PDFium 不可用，跳过测试");
                return;
            }
        };

        let result = parse_pdf_with_pdfium(&pdfium, "tests/fixtures/sample.pdf");
        assert!(result.is_ok(), "解析失败: {:?}", result.err());

        if let Ok(FileContent::Document { paragraphs, .. }) = result {
            assert!(!paragraphs.is_empty(), "应提取到段落");
            // 验证第一个段落有 pdf_position
            assert!(paragraphs[0].pdf_position.is_some(), "应包含坐标信息");
        } else {
            panic!("应返回 Document 变体");
        }
    }

    fn create_test_pdfium() -> Option<pdfium_render::prelude::Pdfium> {
        let lib_path = if cfg!(target_os = "macos") {
            "resources/pdfium/libpdfium.dylib"
        } else {
            "resources/pdfium/pdfium.dll"
        };
        Pdfium::bind_to_library(lib_path)
            .ok()
            .map(|b| Pdfium::new(b))
    }
}
```

- [ ] **Step 2: 重写 parse_pdf 函数，新增 parse_pdf_with_pdfium**

重写 `src-tauri/src/parser/pdf.rs`：

```rust
use crate::models::sensitive::{
    BBox, FileContent, FileType, Paragraph, PdfTextObject, PdfTextPosition,
};
use pdfium_render::prelude::*;
use std::path::Path;

/// 解析 PDF 文件，提取文本内容和坐标（使用 PDFium）
pub fn parse_pdf_with_pdfium(pdfium: &Pdfium, path: &str) -> Result<FileContent, String> {
    let file_path = Path::new(path);
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.pdf")
        .to_string();

    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| format!("打开 PDF 文件失败：{}", e))?;

    let mut all_paragraphs: Vec<Paragraph> = Vec::new();
    let mut global_index = 0;

    for (page_idx, page) in document.pages().iter().enumerate() {
        let mut page_text_objects: Vec<PdfTextObject> = Vec::new();

        // 遍历页面上的所有对象，过滤 text object
        for object in page.objects().iter() {
            if let Some(text_obj) = object.as_text_object() {
                let text = text_obj.text();
                if text.trim().is_empty() {
                    continue;
                }
                let bounds = object.bounds()
                    .map_err(|e| format!("获取 text object 坐标失败：{}", e))?;
                page_text_objects.push(PdfTextObject {
                    text: text.clone(),
                    bbox: BBox {
                        left: bounds.left.value,
                        top: bounds.top.value,
                        right: bounds.right.value,
                        bottom: bounds.bottom.value,
                    },
                    char_offset: 0, // 先占位，合并段落时计算
                });
            }
        }

        if page_text_objects.is_empty() {
            continue;
        }

        // 按阅读顺序排序：从上到下（y 降序），从左到右（x 升序）
        page_text_objects.sort_by(|a, b| {
            let y_cmp = b.bbox.top.partial_cmp(&a.bbox.top).unwrap_or(std::cmp::Ordering::Equal);
            if y_cmp == std::cmp::Ordering::Equal {
                a.bbox.left.partial_cmp(&b.bbox.left).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                y_cmp
            }
        });

        // 合并为段落：y 坐标接近的 text object 合并
        let mut current_group: Vec<PdfTextObject> = vec![page_text_objects.remove(0)];

        for obj in page_text_objects {
            let last = current_group.last().unwrap();
            let line_height = (last.bbox.top - last.bbox.bottom).abs();
            let threshold = line_height * 1.2;
            let y_diff = (last.bbox.top - obj.bbox.top).abs();

            if y_diff <= threshold {
                // 同一段落
                current_group.push(obj);
            } else {
                // 新段落：先处理当前组
                let para = build_paragraph(page_idx, &mut current_group, global_index);
                all_paragraphs.push(para);
                global_index += 1;
                current_group = vec![obj];
            }
        }

        // 处理最后一组
        if !current_group.is_empty() {
            let para = build_paragraph(page_idx, &mut current_group, global_index);
            all_paragraphs.push(para);
            global_index += 1;
        }
    }

    if all_paragraphs.is_empty() {
        return Err(
            "PDF 文件中未提取到文本内容，可能是扫描件（图片型 PDF），暂不支持".to_string(),
        );
    }

    Ok(FileContent::Document {
        file_name,
        file_type: FileType::Pdf,
        paragraphs: all_paragraphs,
        encoding: None,
    })
}

/// 将一组 text object 合并为一个段落
fn build_paragraph(
    page_index: usize,
    objects: &mut Vec<PdfTextObject>,
    index: usize,
) -> Paragraph {
    // 按 x 坐标排序（同一行内从左到右）
    objects.sort_by(|a, b| {
        a.bbox.left.partial_cmp(&b.bbox.left).unwrap_or(std::cmp::Ordering::Equal)
    });

    // 计算 char_offset 并拼接文本
    let mut text = String::new();
    for obj in objects.iter_mut() {
        obj.char_offset = text.chars().count();
        text.push_str(&obj.text);
    }

    // 计算整体 bbox
    let bbox = BBox {
        left: objects.iter().map(|o| o.bbox.left).fold(f32::MAX, f32::min),
        top: objects.iter().map(|o| o.bbox.top).fold(f32::MIN, f32::max),
        right: objects.iter().map(|o| o.bbox.right).fold(f32::MIN, f32::max),
        bottom: objects.iter().map(|o| o.bbox.bottom).fold(f32::MAX, f32::min),
    };

    Paragraph {
        index,
        text,
        style: "normal".to_string(),
        table_position: None,
        pdf_position: Some(PdfTextPosition {
            page_index,
            text_objects: objects.clone(),
            bbox,
        }),
    }
}

/// 兼容旧接口：无 PDFium 时的降级解析（仅提取文本，无坐标）
pub fn parse_pdf(path: &str) -> Result<FileContent, String> {
    Err("PDF 功能需要 PDFium 库支持，请确认 PDFium 动态库已正确安装".to_string())
}
```

- [ ] **Step 3: 更新 commands/file.rs 中的 PDF 导入逻辑**

**不修改 `import_file_internal` 的签名**（避免影响 `task.rs`、`workspace.rs` 等所有调用方）。改为让 `parse_pdf` 内部自行加载 PDFium：

修改 `parser/pdf.rs` 中的 `parse_pdf` 函数，使其内部创建 PDFium 实例：

```rust
/// 解析 PDF（对外接口，内部加载 PDFium）
pub fn parse_pdf(path: &str) -> Result<FileContent, String> {
    let pdfium = load_pdfium()?;
    parse_pdf_with_pdfium(&pdfium, path)
}

/// 加载 PDFium 动态库
pub fn load_pdfium() -> Result<Pdfium, String> {
    let lib_path = if cfg!(target_os = "macos") {
        // 尝试多个路径
        let candidates = [
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/libpdfium.dylib"),
        ];
        candidates.iter().find(|p| p.exists())
            .ok_or("未找到 PDFium 动态库")?
            .to_string_lossy().to_string()
    } else {
        // Windows
        let candidates = [
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        ];
        candidates.iter().find(|p| p.exists())
            .ok_or("未找到 PDFium 动态库")?
            .to_string_lossy().to_string()
    };

    let bindings = Pdfium::bind_to_library(&lib_path)
        .map_err(|e| format!("加载 PDFium 失败：{}", e))?;
    Ok(Pdfium::new(bindings))
}
```

这样 `import_file_internal` 的签名不变，`task.rs`、`workspace.rs` 等调用方无需修改。

> **注意**：虽然 Task 3 在 `AppState` 中也初始化了 `PdfiumState`，但 `parse_pdf` 作为纯函数不依赖全局状态。`PdfiumState` 主要用于导出 command 中复用。如果性能是问题（每次解析都重新加载 PDFium 库），后续可优化为通过全局状态传递。

- [ ] **Step 4: 验证编译**

```bash
cd src-tauri && cargo check
```

Expected: 编译通过。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/parser/pdf.rs src-tauri/src/commands/file.rs
git commit -m "feat: 用 PDFium 重写 PDF 解析，提取文本+坐标"
```

---

### Task 5: PDF 涂黑导出 — 删除 text object + 画黑色矩形

**Files:**
- Create: `src-tauri/src/parser/pdf_export.rs`
- Modify: `src-tauri/src/parser/mod.rs`
- Modify: `src-tauri/src/commands/file.rs`

- [ ] **Step 1: 创建 pdf_export.rs 模块**

创建 `src-tauri/src/parser/pdf_export.rs`：

```rust
use crate::models::sensitive::{BBox, Paragraph, PdfTextPosition, SensitiveItem};
use pdfium_render::prelude::*;

/// PDF 涂黑导出：重新解析原始 PDF 获取坐标，根据 sensitive_items 涂黑
pub fn export_pdf_redacted(
    pdfium: &Pdfium,
    original_path: &str,
    sensitive_items: &[SensitiveItem],
    output_path: &str,
) -> Result<(), String> {
    // 重新解析原始 PDF 获取带坐标的段落（因为 pdf_position 不跨 IPC）
    let content = crate::parser::pdf::parse_pdf_with_pdfium(pdfium, original_path)?;
    let paragraphs = match &content {
        FileContent::Document { paragraphs, .. } => paragraphs,
        _ => return Err("解析结果类型错误".to_string()),
    };

    let document = pdfium
        .load_pdf_from_file(original_path, None)
        .map_err(|e| format!("打开原始 PDF 失败：{}", e))?;

    // 按页分组需要涂黑的区域
    let redactions = compute_redaction_areas(paragraphs, sensitive_items)?;

    for (page_idx, page_redactions) in &redactions {
        let page = document.pages().get(*page_idx)
            .map_err(|e| format!("获取第 {} 页失败：{}", page_idx, e))?;

        // 收集需要删除的 object 及其 bbox
        let mut objects_to_remove: Vec<(usize, BBox)> = Vec::new();

        for object_idx in (0..page.objects().len()).rev() {
            let object = page.objects().get(object_idx)
                .map_err(|e| format!("获取 object 失败：{}", e))?;

            if let Some(text_obj) = object.as_text_object() {
                let text = text_obj.text();
                let bounds = object.bounds()
                    .map_err(|e| format!("获取坐标失败：{}", e))?;

                let obj_bbox = BBox {
                    left: bounds.left.value,
                    top: bounds.top.value,
                    right: bounds.right.value,
                    bottom: bounds.bottom.value,
                };

                // 检查是否与任何涂黑区域匹配（text + bbox 容差匹配）
                for redaction in page_redactions {
                    if text_matches(&text, &redaction.text)
                        && bbox_overlaps(&obj_bbox, &redaction.bbox, 0.1)
                    {
                        objects_to_remove.push((object_idx, redaction.bbox.clone()));
                        break;
                    }
                }
            }
        }

        // 按索引从大到小删除（避免索引偏移）
        objects_to_remove.sort_by(|a, b| b.0.cmp(&a.0));

        for (obj_idx, bbox) in &objects_to_remove {
            // 删除 text object
            page.objects().remove_object_at_index(*obj_idx)
                .map_err(|e| format!("删除 text object 失败：{}", e))?;

            // 在对应位置画黑色矩形
            draw_black_rect(&page, bbox)?;
        }

        // 重写页面 Content Stream
        page.objects().create_objects()
            .map_err(|e| format!("重写页面失败：{}", e))?;
    }

    // 清除文档元数据
    // TODO: pdfium-render 是否暴露元数据清除 API，需实现时确认

    // 保存为新文件
    document.save_to_file(output_path)
        .map_err(|e| format!("保存 PDF 文件失败：{}", e))?;

    Ok(())
}

/// 涂黑区域信息
struct RedactionArea {
    text: String,
    bbox: BBox,
}

/// 根据段落坐标和敏感项，计算需要涂黑的区域
fn compute_redaction_areas(
    paragraphs: &[Paragraph],
    sensitive_items: &[SensitiveItem],
) -> Result<std::collections::HashMap<usize, Vec<RedactionArea>>, String> {
    let mut redactions: std::collections::HashMap<usize, Vec<RedactionArea>> =
        std::collections::HashMap::new();

    for item in sensitive_items {
        let para = paragraphs.get(item.row)
            .ok_or_else(|| format!("段落索引 {} 超出范围", item.row))?;

        let pdf_pos = para.pdf_position.as_ref()
            .ok_or_else(|| format!("段落 {} 缺少 PDF 坐标信息", item.row))?;

        // 找到包含敏感文本的 text object
        for obj in &pdf_pos.text_objects {
            let obj_start = obj.char_offset;
            let obj_end = obj_start + obj.text.chars().count();

            // 检查该 object 是否与敏感项有重叠
            if item.start < obj_end && item.end > obj_start {
                redactions
                    .entry(pdf_pos.page_index)
                    .or_default()
                    .push(RedactionArea {
                        text: obj.text.clone(),
                        bbox: obj.bbox.clone(),
                    });
            }
        }
    }

    Ok(redactions)
}

/// 文本匹配（完全匹配）
fn text_matches(actual: &str, expected: &str) -> bool {
    actual == expected
}

/// bbox 重叠检查（带容差）
fn bbox_overlaps(a: &BBox, b: &BBox, tolerance: f32) -> bool {
    (a.left - b.left).abs() < tolerance
        && (a.top - b.top).abs() < tolerance
        && (a.right - b.right).abs() < tolerance
        && (a.bottom - b.bottom).abs() < tolerance
}

/// 在页面上画黑色填充矩形
fn draw_black_rect(page: &PdfPage, bbox: &BBox) -> Result<(), String> {
    // 使用 pdfium-render 的 path object API 创建黑色矩形
    // 具体 API 需实现时根据 pdfium-render 版本确认
    // 基本思路：创建 PdfPagePathObject → 设置填充色为黑色 → 添加矩形路径段 → 添加到页面
    todo!("实现黑色矩形绘制 — 需根据 pdfium-render API 确认具体调用方式")
}
```

> **重要说明**：上述代码是框架性的。`pdfium-render` 的具体 API（如 `remove_object_at_index`、path object 创建、`save_to_file` 等）需要在实现时查阅 `pdfium-render` 的最新文档确认方法签名。代码中标注了 `todo!()` 的部分需要根据实际 API 补全。

- [ ] **Step 2: 在 parser/mod.rs 中注册新模块**

```rust
pub mod excel;
pub mod pdf;
pub mod pdf_export;  // 新增
pub mod txt;
pub mod word;
pub mod word_export;
```

- [ ] **Step 3: 更新 commands/file.rs 的导出逻辑**

修改 `export_content` 函数中的 PDF 分支：

```rust
FileType::Pdf => {
    let src = original_path
        .ok_or_else(|| "导出 PDF 需要提供原始文件路径".to_string())?;
    // TODO: 需要获取 pdfium 引用和 sensitive_items
    // 当前 export_content 签名需要扩展，或者 PDF 导出走单独路径
    export_pdf_fallback(paragraphs, output_path)
}
```

> 注意：当前 `export_content` 的签名不包含 `pdfium` 和 `sensitive_items`。PDF 涂黑导出可能需要一个独立的 Tauri command，而不是复用 `export_file`。这需要在实现时决定具体的调用路径。一个选项是新增 `export_pdf_redacted` command。

- [ ] **Step 4: 验证编译**

```bash
cd src-tauri && cargo check
```

Expected: 编译通过（`todo!()` 不影响编译）。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/parser/pdf_export.rs src-tauri/src/parser/mod.rs src-tauri/src/commands/file.rs
git commit -m "feat: 添加 PDF 涂黑导出模块（框架）"
```

---

### Task 6: 图片降级方案 — PDFium 渲染 + printpdf 重组

**Files:**
- Modify: `src-tauri/src/parser/pdf_export.rs`

- [ ] **Step 1: 在 pdf_export.rs 中添加降级导出函数**

```rust
/// 降级方案：PDF → 逐页渲染为图片 → 涂黑 → printpdf 重组
pub fn export_pdf_as_images(
    pdfium: &Pdfium,
    original_path: &str,
    sensitive_items: &[SensitiveItem],
    output_path: &str,
) -> Result<(), String> {
    // 重新解析获取坐标（同 export_pdf_redacted）
    let content = crate::parser::pdf::parse_pdf_with_pdfium(pdfium, original_path)?;
    let paragraphs = match &content {
        FileContent::Document { paragraphs, .. } => paragraphs,
        _ => return Err("解析结果类型错误".to_string()),
    };

    let document = pdfium
        .load_pdf_from_file(original_path, None)
        .map_err(|e| format!("打开原始 PDF 失败：{}", e))?;

    let redactions = compute_redaction_areas(paragraphs, sensitive_items)?;

    // 收集每页的渲染图片
    let mut page_images: Vec<(Vec<u8>, f32, f32)> = Vec::new(); // (png_bytes, width_mm, height_mm)

    for (page_idx, page) in document.pages().iter().enumerate() {
        // 渲染为 300dpi 位图
        let render_config = PdfRenderConfig::new()
            .set_target_width(2480) // A4 @ 300dpi ≈ 2480px
            .set_maximum_height(3508);

        let bitmap = page.render_with_config(&render_config)
            .map_err(|e| format!("渲染第 {} 页失败：{}", page_idx, e))?;

        let mut image = bitmap.as_image();

        // 在图片上画黑色矩形
        if let Some(page_redactions) = redactions.get(&page_idx) {
            let page_width = page.width().value;
            let page_height = page.height().value;
            let img_width = image.width() as f32;
            let img_height = image.height() as f32;

            for redaction in page_redactions {
                // 将 PDF 坐标转换为图片像素坐标
                let x = (redaction.bbox.left / page_width * img_width) as u32;
                let y = ((page_height - redaction.bbox.top) / page_height * img_height) as u32;
                let w = ((redaction.bbox.right - redaction.bbox.left) / page_width * img_width) as u32;
                let h = ((redaction.bbox.top - redaction.bbox.bottom) / page_height * img_height) as u32;

                // 画黑色矩形
                for px in x..x.saturating_add(w).min(image.width()) {
                    for py in y..y.saturating_add(h).min(image.height()) {
                        image.put_pixel(px, py, image::Rgba([0, 0, 0, 255]));
                    }
                }
            }
        }

        // 编码为 PNG bytes
        let mut png_bytes = Vec::new();
        image.write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        ).map_err(|e| format!("编码 PNG 失败：{}", e))?;

        // 计算页面尺寸（毫米）
        let width_mm = page.width().value * 25.4 / 72.0; // PDF points to mm
        let height_mm = page.height().value * 25.4 / 72.0;

        page_images.push((png_bytes, width_mm, height_mm));
    }

    // 用 printpdf 将图片重组为 PDF
    assemble_image_pdf(&page_images, output_path)?;

    Ok(())
}

/// 用 printpdf 将图片列表组装为 PDF
fn assemble_image_pdf(
    pages: &[(Vec<u8>, f32, f32)],
    output_path: &str,
) -> Result<(), String> {
    use printpdf::*;

    if pages.is_empty() {
        return Err("没有可导出的页面".to_string());
    }

    let (first_png, first_w, first_h) = &pages[0];
    let (doc, page1, layer1) =
        PdfDocument::new("脱敏文档", Mm(*first_w), Mm(*first_h), "Layer 1");

    // 嵌入第一页图片
    embed_png_on_layer(
        &doc.get_page(page1).get_layer(layer1),
        first_png,
        *first_w,
        *first_h,
    )?;

    // 嵌入后续页面
    for (png_bytes, w, h) in &pages[1..] {
        let (page, layer) = doc.add_page(Mm(*w), Mm(*h), "Layer 1");
        embed_png_on_layer(&doc.get_page(page).get_layer(layer), png_bytes, *w, *h)?;
    }

    doc.save(&mut std::io::BufWriter::new(
        std::fs::File::create(output_path)
            .map_err(|e| format!("创建 PDF 文件失败：{}", e))?,
    ))
    .map_err(|e| format!("保存 PDF 文件失败：{}", e))?;

    Ok(())
}

/// 在 printpdf layer 上嵌入 PNG 图片
fn embed_png_on_layer(
    layer: &PdfLayerReference,
    png_bytes: &[u8],
    width_mm: f32,
    height_mm: f32,
) -> Result<(), String> {
    use printpdf::*;

    let image = Image::from(
        image_crate::load_from_memory(png_bytes)
            .map_err(|e| format!("解码 PNG 失败：{}", e))?,
    );

    image.add_to_layer(
        layer.clone(),
        ImageTransform {
            translate_x: Some(Mm(0.0)),
            translate_y: Some(Mm(0.0)),
            scale_x: Some(width_mm / image.image.width.into_pt(300.0).0 * 72.0),
            scale_y: Some(height_mm / image.image.height.into_pt(300.0).0 * 72.0),
            ..Default::default()
        },
    );

    Ok(())
}
```

> **注意**：上述代码中 printpdf 的 Image API 和 pdfium-render 的渲染 API 的具体签名需要在实现时查阅文档确认。代码展示的是整体逻辑流程。可能需要额外依赖 `image` crate。

- [ ] **Step 2: 验证编译**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/parser/pdf_export.rs
git commit -m "feat: 添加 PDF 图片降级导出方案"
```

---

### Task 7: 集成 — 串联解析→识别→涂黑→导出完整流程

**Files:**
- Modify: `src-tauri/src/commands/file.rs`

- [ ] **Step 1: 更新 import_file command 传入 PDFium**

修改 `import_file` command，从 `State<PdfiumState>` 获取 PDFium 实例并传给 `import_file_internal`：

```rust
#[tauri::command]
pub async fn import_file(
    file_path: String,
    pdfium_state: tauri::State<'_, crate::PdfiumState>,
    app_handle: tauri::AppHandle,
) -> Result<FileContent, String> {
    let pdfium_ref = pdfium_state.0.as_ref();
    let fp = file_path.clone();

    // 由于 Pdfium 不是 Send，需要在主线程处理 PDF
    // 或者对非 PDF 文件仍用 spawn_blocking
    let extension = Path::new(&fp)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    let result = if extension == "pdf" {
        // PDF 在当前线程处理（PDFium 可能不是 Send）
        import_file_internal(&fp, pdfium_ref)
    } else {
        tokio::task::spawn_blocking(move || import_file_internal(&fp, None))
            .await
            .map_err(|e| format!("文件导入任务失败: {}", e))?
    };

    // ... analytics 逻辑不变 ...
    result
}
```

- [ ] **Step 2: 更新 export_file command 处理 PDF 涂黑**

PDF 导出需要原始路径和敏感项信息。当前 `export_file` 的参数不包含敏感项。两种方案：

**方案 A**：新增专用 command `export_pdf_redacted`
**方案 B**：在现有 `export_file` 中通过 `original_path` + 重新从 FileContent 获取坐标

推荐方案 A，新增 command：

```rust
/// 导出涂黑后的 PDF 文件
/// 不需要传入 FileContent（pdf_position 经 IPC 会丢失），内部重新解析获取坐标
#[tauri::command]
pub async fn export_pdf_redacted(
    original_path: String,
    sensitive_items: Vec<SensitiveItem>,
    output_path: String,
    pdfium_state: tauri::State<'_, crate::PdfiumState>,
) -> Result<(), String> {
    let pdfium = pdfium_state.0.as_ref()
        .ok_or_else(|| "PDFium 不可用，无法导出 PDF".to_string())?;

    // 尝试原地涂黑（内部会重新解析原始 PDF 获取坐标）
    match crate::parser::pdf_export::export_pdf_redacted(
        pdfium, &original_path, &sensitive_items, &output_path,
    ) {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("PDF 原地涂黑失败，降级为图片方案: {}", e);
            // 降级为图片方案（同样内部重新解析）
            crate::parser::pdf_export::export_pdf_as_images(
                pdfium, &original_path, &sensitive_items, &output_path,
            )
        }
    }
}
```

- [ ] **Step 3: 在 lib.rs 注册新 command**

在 `invoke_handler` 的 `generate_handler!` 中添加 `export_pdf_redacted`。

- [ ] **Step 4: 更新现有 export_content 中的 PDF 分支**

保留 `export_content` 中的 PDF 分支作为简单 fallback（无涂黑的纯文本导出），或改为返回错误提示使用新 command：

```rust
FileType::Pdf => {
    Err("PDF 导出请使用 export_pdf_redacted 命令".to_string())
}
```

- [ ] **Step 5: 验证编译**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/file.rs src-tauri/src/lib.rs
git commit -m "feat: 集成 PDF 涂黑导出完整流程，新增 export_pdf_redacted command"
```

---

### Task 8: 前端适配 — PDF 策略限制和降级提示

**Files:**
- Modify: `src/hooks/useAutoDesensitize.ts` (或处理导出逻辑的文件)
- Modify: `src/types/index.ts`

- [ ] **Step 1: 前端调用新的 PDF 导出 command**

在处理 PDF 文件导出时，调用 `export_pdf_redacted` 而非 `export_file`：

```typescript
if (fileType === "Pdf") {
  // PDF 导出不传 content（坐标信息不跨 IPC），后端会重新解析原始 PDF
  await invoke("export_pdf_redacted", {
    originalPath: originalFilePath,
    sensitiveItems: confirmedItems,
    outputPath: outputPath,
  });
} else {
  await invoke("export_file", { ... });
}
```

需要修改的前端调用点（所有调用 `invoke("export_file", ...)` 的地方，需加 PDF 条件分支）：
- `src/hooks/useAutoDesensitize.ts`
- 其他调用 `export_file` 的组件（需 grep 确认全部位置）

```bash
# 实现时先 grep 所有调用点
grep -rn 'export_file' src/
```

- [ ] **Step 2: PDF 文件的策略限制**

在策略选择 UI 中，当文件类型为 PDF 时，仅显示"涂黑"策略，禁用"替换"和"泛化"：

```typescript
const availableStrategies = fileType === "Pdf"
  ? [{ value: "mask", label: "涂黑" }]
  : allStrategies;
```

- [ ] **Step 3: Commit**

```bash
git add src/
git commit -m "feat: 前端适配 PDF 涂黑导出和策略限制"
```

---

### Task 9: 端到端测试和验证

**Files:**
- Create: `src-tauri/tests/fixtures/sample.pdf` (测试用 PDF)
- Modify: `src-tauri/src/parser/pdf.rs` (补充测试)

- [ ] **Step 1: 准备测试 PDF 文件**

用 printpdf 或其他方式创建一个包含已知文本的测试 PDF（如包含手机号 "13812345678" 的文档），放入 `src-tauri/tests/fixtures/sample.pdf`。

- [ ] **Step 2: 编写集成测试**

在 `src-tauri/src/parser/pdf.rs` 的测试模块中：

```rust
#[test]
fn test_redacted_pdf_text_removed() {
    let pdfium = match create_test_pdfium() {
        Some(p) => p,
        None => { eprintln!("跳过"); return; }
    };

    // 解析
    let content = parse_pdf_with_pdfium(&pdfium, "tests/fixtures/sample.pdf").unwrap();
    let paragraphs = match &content {
        FileContent::Document { paragraphs, .. } => paragraphs,
        _ => panic!("应返回 Document"),
    };

    // 模拟敏感项
    let items = vec![SensitiveItem {
        id: "test-1".to_string(),
        text: "13812345678".to_string(),
        sensitive_type: SensitiveType::Phone,
        source: DetectSource::Regex,
        confidence: 1.0,
        start: 0, // 根据实际位置调整
        end: 11,
        row: 0,   // 根据实际段落调整
        col: 0,
        sheet_index: 0,
    }];

    // 导出
    let output = "/tmp/test_redacted.pdf";
    crate::parser::pdf_export::export_pdf_redacted(
        &pdfium, "tests/fixtures/sample.pdf", paragraphs, &items, output,
    ).unwrap();

    // 验证：重新提取文本，敏感内容应被删除
    let redacted = parse_pdf_with_pdfium(&pdfium, output).unwrap();
    let redacted_text = match &redacted {
        FileContent::Document { paragraphs, .. } =>
            paragraphs.iter().map(|p| p.text.as_str()).collect::<String>(),
        _ => panic!("应返回 Document"),
    };

    assert!(!redacted_text.contains("13812345678"), "敏感文本应已被删除");

    // 清理
    let _ = std::fs::remove_file(output);
}
```

- [ ] **Step 3: 运行测试**

```bash
cd src-tauri && cargo test parser::pdf
```

Expected: 所有测试通过。

- [ ] **Step 4: 手动验证**

```bash
cargo tauri dev
```

导入一个包含敏感信息的 PDF → 确认识别正常 → 导出 → 用 PDF 阅读器打开导出文件，验证：
1. 原始布局保留
2. 敏感文本被黑色矩形覆盖
3. 无法选中/复制被涂黑的文本

- [ ] **Step 5: Commit**

```bash
git add src-tauri/tests/ src-tauri/src/parser/pdf.rs
git commit -m "test: 添加 PDF 涂黑脱敏集成测试"
```

---

### Task 10: 清理旧代码

**Files:**
- Modify: `src-tauri/src/commands/file.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: 移除旧的 export_pdf 函数和相关辅助函数**

从 `commands/file.rs` 中删除：
- `export_pdf` 函数（line 372-430）
- `wrap_text` 函数（line 432-446）
- `load_cjk_font_bytes` 函数（line 448-478）— 如果仅 PDF 导出使用

- [ ] **Step 2: 从 Cargo.toml 移除 pdf-extract**

确认 `pdf-extract` 不再被任何代码引用后，从 `Cargo.toml` 删除：

```toml
# 删除：
# pdf-extract = "0.10"
```

- [ ] **Step 3: 验证编译和测试**

```bash
cd src-tauri && cargo check && cargo test
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/file.rs src-tauri/Cargo.toml
git commit -m "refactor: 清理旧 PDF 导出代码，移除 pdf-extract 依赖"
```
