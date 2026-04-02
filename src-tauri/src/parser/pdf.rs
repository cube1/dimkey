use crate::models::sensitive::{BBox, FileContent, FileType, Paragraph, PdfTextObject, PdfTextPosition};
use pdfium_render::prelude::*;
use std::path::Path;

/// 从 resources/pdfium/ 加载 PDFium 动态库
pub fn load_pdfium() -> Result<Pdfium, String> {
    let pdfium_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources").join("pdfium");

    let lib_path = if cfg!(target_os = "macos") {
        pdfium_dir.join("libpdfium.dylib")
    } else if cfg!(target_os = "windows") {
        pdfium_dir.join("pdfium.dll")
    } else {
        pdfium_dir.join("libpdfium.so")
    };

    if !lib_path.exists() {
        return Err(format!(
            "PDFium 动态库未找到：{}，请将 PDFium 库放入 resources/pdfium/ 目录",
            lib_path.display()
        ));
    }

    let bindings = Pdfium::bind_to_library(&lib_path)
        .map_err(|e| format!("加载 PDFium 动态库失败：{}", e))?;

    Ok(Pdfium::new(bindings))
}

/// 解析 PDF 文件，提取文本内容为段落（自动创建 PDFium 实例）
pub fn parse_pdf(path: &str) -> Result<FileContent, String> {
    let pdfium = load_pdfium()?;
    parse_pdf_with_pdfium(&pdfium, path)
}

/// 单个 text object 的中间表示（用于排序和合并）
struct RawTextObj {
    text: String,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    font_size: f32,
    page_index: usize,
}

/// 用已有的 PDFium 实例解析 PDF 文件，提取文本+坐标
pub fn parse_pdf_with_pdfium(pdfium: &Pdfium, path: &str) -> Result<FileContent, String> {
    let file_path = Path::new(path);
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.pdf")
        .to_string();

    let doc = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| format!("打开 PDF 文件失败：{}", e))?;

    let mut all_text_objs: Vec<RawTextObj> = Vec::new();

    // 遍历每一页
    for page_index in 0..doc.pages().len() {
        let page = doc.pages().get(page_index)
            .map_err(|e| format!("读取第 {} 页失败：{}", page_index + 1, e))?;

        let objects = page.objects();
        for i in 0..objects.len() {
            let obj = match objects.get(i) {
                Ok(o) => o,
                Err(_) => continue,
            };

            if let Some(text_obj) = obj.as_text_object() {
                let text = text_obj.text();
                if text.trim().is_empty() {
                    continue;
                }

                let font_size = text_obj.scaled_font_size().value;

                // 获取 bounding box
                let bounds = match obj.bounds() {
                    Ok(b) => b,
                    Err(_) => continue,
                };

                all_text_objs.push(RawTextObj {
                    text,
                    left: bounds.left().value,
                    top: bounds.top().value,
                    right: bounds.right().value,
                    bottom: bounds.bottom().value,
                    font_size,
                    page_index: page_index as usize,
                });
            }
        }
    }

    if all_text_objs.is_empty() {
        return Err(
            "PDF 文件中未提取到文本内容，可能是扫描件（图片型 PDF），暂不支持".to_string(),
        );
    }

    // 按阅读顺序排序：先按页码，再按 y 降序（PDF 坐标系 y 轴向上），再按 x 升序
    all_text_objs.sort_by(|a, b| {
        a.page_index
            .cmp(&b.page_index)
            .then(
                b.top
                    .partial_cmp(&a.top)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(
                a.left
                    .partial_cmp(&b.left)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    // 合并段落：同一页内，y 坐标差值小于行高 1.2 倍的 text object 合并为同一段落
    let mut paragraphs: Vec<Paragraph> = Vec::new();
    let mut current_group: Vec<&RawTextObj> = Vec::new();
    let mut current_page: usize = usize::MAX;
    let mut current_top: f32 = f32::MAX;

    let flush_group = |group: &[&RawTextObj], para_index: usize| -> Paragraph {
        // 组内按 x 排序（已排序，但确保一致性）
        let mut sorted: Vec<&RawTextObj> = group.to_vec();
        sorted.sort_by(|a, b| {
            a.left
                .partial_cmp(&b.left)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let page_index = sorted[0].page_index;

        // 拼接文本，计算每个 object 在段落中的 char_offset
        let mut para_text = String::new();
        let mut text_objects: Vec<PdfTextObject> = Vec::new();

        for (i, obj) in sorted.iter().enumerate() {
            // 判断是否需要在 text object 之间插入空格
            // 如果当前 object 的左边界与上一个 object 的右边界有间隙，插入空格
            if i > 0 {
                let prev = sorted[i - 1];
                let gap = obj.left - prev.right;
                // 间隙大于字体大小的 0.15 倍时插入空格（经验值）
                let space_threshold = obj.font_size * 0.15;
                if gap > space_threshold {
                    para_text.push(' ');
                }
            }

            let char_offset = para_text.chars().count();
            text_objects.push(PdfTextObject {
                text: obj.text.clone(),
                bbox: BBox {
                    left: obj.left,
                    top: obj.top,
                    right: obj.right,
                    bottom: obj.bottom,
                },
                char_offset,
            });
            para_text.push_str(&obj.text);
        }

        // 段落整体包围盒
        let bbox = BBox {
            left: sorted.iter().map(|o| o.left).fold(f32::MAX, f32::min),
            top: sorted.iter().map(|o| o.top).fold(f32::MIN, f32::max),
            right: sorted.iter().map(|o| o.right).fold(f32::MIN, f32::max),
            bottom: sorted.iter().map(|o| o.bottom).fold(f32::MAX, f32::min),
        };

        Paragraph {
            index: para_index,
            text: para_text,
            style: "normal".to_string(),
            table_position: None,
            pdf_position: Some(PdfTextPosition {
                page_index,
                text_objects,
                bbox,
            }),
        }
    };

    for obj in &all_text_objs {
        // 计算行高阈值：使用当前 text object 的 font_size * 1.2
        let line_threshold = obj.font_size * 1.2;

        // 与当前行的基准 y 坐标比较（用第一个 object 的 top，而非上一个）
        let should_break = obj.page_index != current_page
            || (current_top - obj.top).abs() > line_threshold;

        if should_break && !current_group.is_empty() {
            let para = flush_group(&current_group, paragraphs.len());
            paragraphs.push(para);
            current_group.clear();
        }

        if current_group.is_empty() {
            // 新行开始，用第一个 object 的 top 作为基准
            current_top = obj.top;
        }
        current_group.push(obj);
        current_page = obj.page_index;
    }

    // 最后一组
    if !current_group.is_empty() {
        let para = flush_group(&current_group, paragraphs.len());
        paragraphs.push(para);
    }

    if paragraphs.is_empty() {
        return Err(
            "PDF 文件中未提取到文本内容，可能是扫描件（图片型 PDF），暂不支持".to_string(),
        );
    }

    Ok(FileContent::Document {
        file_name,
        file_type: FileType::Pdf,
        paragraphs,
        encoding: None,
    })
}
