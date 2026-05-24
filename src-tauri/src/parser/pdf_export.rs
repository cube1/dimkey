use crate::models::sensitive::{FileContent, Paragraph, SensitiveItem};
use crate::parser::pdf::parse_pdf_with_pdfium;
use ::image::RgbaImage;
use pdfium_render::prelude::*;
use printpdf::{
    ColorBits, ColorSpace, Image, ImageTransform, ImageXObject, Mm,
    PdfDocument as PrintPdfDocument, Px,
};
use std::collections::HashMap;
use std::io::BufWriter;

/// 需要涂黑的 text object 信息
#[derive(Debug, Clone)]
pub struct RedactTarget {
    pub text: String,
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// 从段落和敏感项中计算需要涂黑的区域，按页分组
///
/// 返回 HashMap<page_index, Vec<RedactTarget>>
pub fn compute_redact_targets(
    paragraphs: &[Paragraph],
    sensitive_items: &[SensitiveItem],
) -> HashMap<usize, Vec<RedactTarget>> {
    let mut redact_by_page: HashMap<usize, Vec<RedactTarget>> = HashMap::new();

    for item in sensitive_items {
        // 已带 pdf_bboxes 的 item（手动框选 / 手动文字选中）由调用方按 bbox 路径单独处理；
        // 这里跳过避免基于 paragraphs.get(row) 的错位匹配在错段落画鬼影黑块
        if item.pdf_bboxes.is_some() {
            continue;
        }
        let para_index = item.row;
        let para = match paragraphs.get(para_index) {
            Some(p) => p,
            None => {
                eprintln!(
                    "警告：段落索引 {} 超出范围（共 {} 段），跳过",
                    para_index,
                    paragraphs.len()
                );
                continue;
            }
        };

        let pdf_pos = match &para.pdf_position {
            Some(pos) => pos,
            None => {
                eprintln!("警告：段落 {} 没有 PDF 坐标信息，跳过", para_index);
                continue;
            }
        };

        let sensitive_start = item.start;
        let sensitive_end = item.end;

        for text_obj in &pdf_pos.text_objects {
            let obj_char_start = text_obj.char_offset;
            let obj_char_end = obj_char_start + text_obj.text.chars().count();

            if obj_char_start < sensitive_end && obj_char_end > sensitive_start {
                let targets = redact_by_page.entry(pdf_pos.page_index).or_default();
                targets.push(RedactTarget {
                    text: text_obj.text.clone(),
                    left: text_obj.bbox.left,
                    top: text_obj.bbox.top,
                    right: text_obj.bbox.right,
                    bottom: text_obj.bbox.bottom,
                });
            }
        }
    }

    redact_by_page
}

/// 导出涂黑后的 PDF 文件
///
/// 实现逻辑：
/// 1. 重新解析原始 PDF 获取带坐标的段落信息
/// 2. 根据 sensitive_items 的 row/start/end 找到需要涂黑的 text object
/// 3. 在原始 PDF 上删除对应 text object 并画黑色矩形
/// 4. 保存为新文件
pub fn export_pdf_redacted(
    pdfium: &Pdfium,
    original_path: &str,
    sensitive_items: &[SensitiveItem],
    output_path: &str,
) -> Result<(), String> {
    if sensitive_items.is_empty() {
        std::fs::copy(original_path, output_path)
            .map_err(|e| format!("复制 PDF 文件失败：{}", e))?;
        return Ok(());
    }

    // 步骤 1：重新解析原始 PDF 获取带坐标的段落信息
    let file_content = parse_pdf_with_pdfium(pdfium, original_path)?;
    let paragraphs = match &file_content {
        FileContent::Document { paragraphs, .. } => paragraphs,
        _ => return Err("PDF 解析结果格式不正确".to_string()),
    };

    // 步骤 2：计算需要涂黑的区域，按页分组
    let mut redact_by_page = compute_redact_targets(paragraphs, sensitive_items);

    // 步骤 3：用 PDFium 重新打开原始 PDF，执行涂黑操作
    let doc = pdfium
        .load_pdf_from_file(original_path, None)
        .map_err(|e| format!("打开 PDF 文件失败：{}", e))?;

    // 额外收集 pdf_bboxes 类型的涂黑区域（用户手动框选 / 文字选中按行拆出的多 bbox）
    for item in sensitive_items {
        if let Some(ref bboxes) = item.pdf_bboxes {
            for bbox in bboxes {
                let page = match doc.pages().get(bbox.page_index as u16) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!(
                            "警告：跳过 bbox（页 {} 超出范围）：{}",
                            bbox.page_index + 1,
                            e
                        );
                        continue;
                    }
                };
                let pw = page.width().value;
                let ph = page.height().value;

                let targets = redact_by_page.entry(bbox.page_index).or_default();
                targets.push(RedactTarget {
                    text: String::new(),  // empty text marks this as a rect-select target
                    left: bbox.left * pw,
                    top: (1.0 - bbox.top) * ph,      // screen coords → PDF coords (Y flip)
                    right: bbox.right * pw,
                    bottom: (1.0 - bbox.bottom) * ph,
                });
            }
        }
    }

    if redact_by_page.is_empty() {
        // sensitive_items 在 line 89 已判空并提前返回；走到这里说明：用户期待脱敏，
        // 但所有 item 的 bbox 都超页 / row 都失效 → 报错而非静默 copy 原文件，
        // 避免用户拿到未脱敏 PDF 却看到"导出成功"
        return Err(format!(
            "无法定位任何待脱敏区域（{} 个敏感项均无效，可能源 PDF 已变更或敏感项 stale）",
            sensitive_items.len()
        ));
    }

    for (&page_index, targets) in &redact_by_page {
        let mut page = doc
            .pages()
            .get(page_index as u16)
            .map_err(|e| format!("读取第 {} 页失败：{}", page_index + 1, e))?;

        // 设置手动内容再生策略，批量操作后一次性 regenerate
        page.set_content_regeneration_strategy(
            PdfPageContentRegenerationStrategy::Manual,
        );

        // 收集需要删除的 object 索引和对应的 bbox
        let mut to_remove: Vec<(PdfPageObjectIndex, RedactTarget)> = Vec::new();
        let objects = page.objects();
        let obj_count = objects.len();

        for i in 0..obj_count {
            let obj = match objects.get(i) {
                Ok(o) => o,
                Err(_) => continue,
            };

            if let Some(text_obj) = obj.as_text_object() {
                let text = text_obj.text();
                let bounds = match obj.bounds() {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                // text object 中心点（PDF 坐标系：top > bottom）
                let obj_cx = (bounds.left().value + bounds.right().value) / 2.0;
                let obj_cy = (bounds.top().value + bounds.bottom().value) / 2.0;

                let tolerance = 0.5;
                for target in targets {
                    let matched = if target.text.is_empty() {
                        // rect-select / 手动文字选中（bbox 路径）：text object 中心
                        // 落在 bbox 内即删除——绕开 row/start-end 与 paragraph 索引
                        // 不一致的问题，让黑矩形下面的真文字也被删掉，避免 pdftotext
                        // 提取出"被涂黑"的原文
                        obj_cx >= target.left
                            && obj_cx <= target.right
                            && obj_cy >= target.bottom
                            && obj_cy <= target.top
                    } else {
                        // 文字路径：text 内容相同 + bbox 接近（tolerance 0.5）
                        text == target.text
                            && (bounds.left().value - target.left).abs() < tolerance
                            && (bounds.top().value - target.top).abs() < tolerance
                            && (bounds.right().value - target.right).abs() < tolerance
                            && (bounds.bottom().value - target.bottom).abs() < tolerance
                    };
                    if matched {
                        to_remove.push((i, target.clone()));
                        break;
                    }
                }
            }
        }

        // 按索引从大到小排序，先删除后面的，避免索引偏移
        to_remove.sort_by(|a, b| b.0.cmp(&a.0));

        // 去重（同一个 object 可能匹配多个 sensitive item）
        to_remove.dedup_by_key(|item| item.0);

        // 先收集所有需要画黑色矩形的 bbox
        let mut rects_to_draw: Vec<RedactTarget> =
            to_remove.iter().map(|(_, t)| t.clone()).collect();
        // Also add rect-select targets (empty text)
        rects_to_draw.extend(targets.iter().filter(|t| t.text.is_empty()).cloned());

        // 删除 text object（从大索引到小索引）
        for (idx, _) in &to_remove {
            match page.objects_mut().remove_object_at_index(*idx) {
                Ok(_removed_obj) => {
                    // object 被成功移除，它会在 drop 时释放内存
                }
                Err(e) => {
                    eprintln!(
                        "警告：删除第 {} 页第 {} 个 text object 失败：{}",
                        page_index + 1,
                        idx,
                        e
                    );
                }
            }
        }

        // 画黑色填充矩形覆盖被删除的区域
        for rect_target in &rects_to_draw {
            // PdfRect::new_from_values(bottom, left, top, right)
            let rect = PdfRect::new_from_values(
                rect_target.bottom,
                rect_target.left,
                rect_target.top,
                rect_target.right,
            );

            match page.objects_mut().create_path_object_rect(
                rect,
                None,        // stroke_color: 不需要边框
                None,        // stroke_width
                Some(PdfColor::BLACK), // fill_color: 黑色填充
            ) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!(
                        "警告：在第 {} 页画黑色矩形失败：{}",
                        page_index + 1,
                        e
                    );
                }
            }
        }

        // 手动触发内容再生
        page.regenerate_content()
            .map_err(|e| format!("第 {} 页内容再生失败：{}", page_index + 1, e))?;
    }

    // 步骤 4：保存为新文件
    doc.save_to_file(output_path)
        .map_err(|e| format!("保存涂黑后的 PDF 文件失败：{}", e))?;

    Ok(())
}

/// 降级方案：PDF → 逐页渲染为高清图片 → 涂黑敏感区域 → printpdf 重组为新 PDF
///
/// 当原生涂黑方案（删除 text object + 画矩形）不可用时，
/// 使用此方案将每页渲染为位图，在位图上涂黑，再用 printpdf 重组为 PDF。
/// 优点：不依赖 PDFium 的 text object 删除能力，兼容性更好。
/// 缺点：输出文件较大，文本不可选中/搜索。
pub fn export_pdf_as_images(
    pdfium: &Pdfium,
    original_path: &str,
    sensitive_items: &[SensitiveItem],
    output_path: &str,
) -> Result<(), String> {
    if sensitive_items.is_empty() {
        std::fs::copy(original_path, output_path)
            .map_err(|e| format!("复制 PDF 文件失败：{}", e))?;
        return Ok(());
    }

    // 步骤 1：重新解析获取坐标信息
    let file_content = parse_pdf_with_pdfium(pdfium, original_path)?;
    let paragraphs = match &file_content {
        FileContent::Document { paragraphs, .. } => paragraphs,
        _ => return Err("PDF 解析结果格式不正确".to_string()),
    };

    // 步骤 2：计算涂黑区域（复用 compute_redact_targets）
    let mut redact_by_page = compute_redact_targets(paragraphs, sensitive_items);

    // 步骤 3：打开原始 PDF 用于渲染
    let doc = pdfium
        .load_pdf_from_file(original_path, None)
        .map_err(|e| format!("打开 PDF 文件失败：{}", e))?;

    // 额外收集 pdf_bboxes 类型的涂黑区域（手动框选 / 文字选中按行拆出）
    for item in sensitive_items {
        if let Some(ref bboxes) = item.pdf_bboxes {
            for bbox in bboxes {
                let page = match doc.pages().get(bbox.page_index as u16) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!(
                            "警告：跳过 bbox（页 {} 超出范围）：{}",
                            bbox.page_index + 1,
                            e
                        );
                        continue;
                    }
                };
                let pw = page.width().value;
                let ph = page.height().value;

                let targets = redact_by_page.entry(bbox.page_index).or_default();
                targets.push(RedactTarget {
                    text: String::new(),
                    left: bbox.left * pw,
                    top: (1.0 - bbox.top) * ph,
                    right: bbox.right * pw,
                    bottom: (1.0 - bbox.bottom) * ph,
                });
            }
        }
    }

    let page_count = doc.pages().len();
    if page_count == 0 {
        return Err("PDF 文件没有页面".to_string());
    }

    // 渲染目标宽度：A4 @ 300dpi ≈ 2480px
    let target_width: i32 = 2480;
    let render_config = PdfRenderConfig::new()
        .set_target_width(target_width)
        .set_clear_color(pdfium_render::prelude::PdfColor::WHITE);

    // 获取第一页尺寸来初始化 printpdf 文档
    let first_page = doc
        .pages()
        .get(0)
        .map_err(|e| format!("读取第 1 页失败：{}", e))?;
    let first_page_width_pt = first_page.width().value;
    let first_page_height_pt = first_page.height().value;

    let (pdf_doc, first_pdf_page, first_layer) = PrintPdfDocument::new(
        "脱敏导出",
        Mm(first_page_width_pt * 25.4 / 72.0),
        Mm(first_page_height_pt * 25.4 / 72.0),
        "图层 1",
    );

    // 逐页处理
    for page_idx in 0..page_count {
        let page = doc
            .pages()
            .get(page_idx as u16)
            .map_err(|e| format!("读取第 {} 页失败：{}", page_idx + 1, e))?;

        let page_width_pt = page.width().value;
        let page_height_pt = page.height().value;

        // 渲染为位图
        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| format!("渲染第 {} 页失败：{}", page_idx + 1, e))?;

        let mut dynamic_image = bitmap.as_image();
        let img_width = dynamic_image.width();
        let img_height = dynamic_image.height();

        // 在位图上涂黑敏感区域
        if let Some(targets) = redact_by_page.get(&(page_idx as usize)) {
            // 转为可修改的 RGBA 图片
            let rgba_img = dynamic_image.as_mut_rgba8().ok_or_else(|| {
                format!("第 {} 页图片转换为 RGBA 失败", page_idx + 1)
            })?;

            for target in targets {
                draw_black_rect_on_image(
                    rgba_img,
                    img_width,
                    img_height,
                    page_width_pt,
                    page_height_pt,
                    target,
                );
            }
        }

        // 将 DynamicImage 转为 RGB 字节（printpdf 需要 RGB，不需要 alpha）
        let rgb_image = dynamic_image.to_rgb8();
        let rgb_data = rgb_image.as_raw().clone();

        // 构建 printpdf 的 ImageXObject（手动构建，避免 image crate 版本冲突）
        let image_xobj = ImageXObject {
            width: Px(img_width as usize),
            height: Px(img_height as usize),
            color_space: ColorSpace::Rgb,
            bits_per_component: ColorBits::Bit8,
            interpolate: true,
            image_data: rgb_data,
            image_filter: None,
            smask: None,
            clipping_bbox: None,
        };
        let printpdf_image = Image::from(image_xobj);

        // 页面尺寸（mm）
        let page_width_mm = page_width_pt * 25.4 / 72.0;
        let page_height_mm = page_height_pt * 25.4 / 72.0;

        // 计算 DPI：图片像素 / 页面物理尺寸（英寸）
        let dpi_x = img_width as f32 / (page_width_mm / 25.4);
        let dpi_y = img_height as f32 / (page_height_mm / 25.4);
        // 使用较大的 DPI 值，确保图片不会超出页面
        let dpi = dpi_x.max(dpi_y);

        if page_idx == 0 {
            // 第一页已在 PrintPdfDocument::new 中创建
            let layer = pdf_doc.get_page(first_pdf_page).get_layer(first_layer);
            printpdf_image.add_to_layer(
                layer,
                ImageTransform {
                    translate_x: Some(Mm(0.0)),
                    translate_y: Some(Mm(0.0)),
                    dpi: Some(dpi),
                    ..Default::default()
                },
            );
        } else {
            // 后续页面需要新建
            let (new_page, new_layer) = pdf_doc.add_page(
                Mm(page_width_mm),
                Mm(page_height_mm),
                &format!("图层 {}", page_idx + 1),
            );
            let layer = pdf_doc.get_page(new_page).get_layer(new_layer);
            printpdf_image.add_to_layer(
                layer,
                ImageTransform {
                    translate_x: Some(Mm(0.0)),
                    translate_y: Some(Mm(0.0)),
                    dpi: Some(dpi),
                    ..Default::default()
                },
            );
        }

        // 此处 bitmap、dynamic_image 等局部变量在循环结束时自动释放，避免内存累积
    }

    // 保存 PDF
    let output_file = std::fs::File::create(output_path)
        .map_err(|e| format!("创建输出文件失败：{}", e))?;
    pdf_doc
        .save(&mut BufWriter::new(output_file))
        .map_err(|e| format!("保存降级 PDF 文件失败：{}", e))?;

    Ok(())
}

/// 在 RGBA 图片上绘制黑色矩形，覆盖指定的 PDF 坐标区域
///
/// PDF 坐标系：原点在左下角，y 轴向上
/// 图片坐标系：原点在左上角，y 轴向下
fn draw_black_rect_on_image(
    img: &mut RgbaImage,
    img_width: u32,
    img_height: u32,
    page_width_pt: f32,
    page_height_pt: f32,
    target: &RedactTarget,
) {
    // PDF 坐标 → 图片像素坐标
    let scale_x = img_width as f32 / page_width_pt;
    let scale_y = img_height as f32 / page_height_pt;

    // PDF 的 left/right 直接映射到 x
    let px_left = (target.left * scale_x) as i32;
    let px_right = (target.right * scale_x) as i32;

    // PDF 的 y 需要翻转：img_y = img_height - pdf_y * scale_y
    // PDF bottom < top，翻转后 top 变大（靠近图片底部）
    let px_top = (img_height as f32 - target.top * scale_y) as i32;
    let px_bottom = (img_height as f32 - target.bottom * scale_y) as i32;

    // 确保坐标在图片范围内
    let x_start = px_left.max(0) as u32;
    let x_end = (px_right as u32).min(img_width);
    let y_start = px_top.max(0) as u32;
    let y_end = (px_bottom as u32).min(img_height);

    // 稍微扩展一点边距（1px），确保完全覆盖
    let x_start = x_start.saturating_sub(1);
    let x_end = (x_end + 1).min(img_width);
    let y_start = y_start.saturating_sub(1);
    let y_end = (y_end + 1).min(img_height);

    // 填充黑色像素
    let black = ::image::Rgba([0u8, 0, 0, 255]);
    for y in y_start..y_end {
        for x in x_start..x_end {
            img.put_pixel(x, y, black);
        }
    }
}
