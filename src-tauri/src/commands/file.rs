use std::path::Path;
use serde::Serialize;
use crate::models::sensitive::{FileContent, FileType, SheetData};
use crate::parser::excel::{parse_excel, parse_csv, parse_excel_from_bytes};
use crate::parser::txt::{parse_txt, export_txt};
use crate::parser::pdf::parse_pdf;
use crate::parser::word::parse_docx;
use crate::parser::word::parse_docx_from_bytes;
use crate::parser::word_export::export_docx;

/// PDF 页面渲染结果
#[derive(Debug, Clone, Serialize)]
pub struct PdfPageRender {
    /// 页码（0-based）
    pub page_index: usize,
    /// base64 编码的 PNG 图片
    pub image_base64: String,
    /// 图片宽度（像素）
    pub image_width: u32,
    /// 图片高度（像素）
    pub image_height: u32,
    /// PDF 页面宽度（PDF points）
    pub page_width: f32,
    /// PDF 页面高度（PDF points）
    pub page_height: f32,
    /// 页面内所有 text objects（用于前端透明文本层）
    pub text_objects: Vec<PdfTextObjectInfo>,
}

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

/// 文件大小上限：50MB
const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024;

/// 内部文件导入（供 restore_file 等命令复用）
pub fn import_file_internal(path: &str) -> Result<FileContent, String> {
    let file_path = Path::new(path);

    if !file_path.exists() {
        return Err(format!("文件不存在：{}", path));
    }

    let metadata = std::fs::metadata(path)
        .map_err(|e| format!("无法读取文件信息：{}", e))?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(format!(
            "文件大小超过限制（最大 50MB），当前文件大小：{:.1}MB",
            metadata.len() as f64 / 1024.0 / 1024.0
        ));
    }

    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "xlsx" | "xls" => parse_excel(path),
        "csv" | "tsv" => parse_csv(path),
        "docx" => parse_docx(path),
        "txt" => parse_txt(path),
        "pdf" => parse_pdf(path),
        _ => Err("不支持的文件格式，目前支持 Excel(.xlsx/.xls)、CSV(.csv/.tsv)、Word(.docx)、TXT(.txt)、PDF(.pdf)".to_string()),
    }
}

/// 导入文件，解析为 FileContent
#[tauri::command]
pub async fn import_file(
    file_path: String,
    pdfium_state: tauri::State<'_, crate::PdfiumState>,
    app_handle: tauri::AppHandle,
) -> Result<FileContent, String> {
    let fp = file_path.clone();
    let is_pdf = Path::new(&fp)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false);

    let result = if is_pdf {
        // PDF 使用 PdfiumState 中已有的实例，不在 spawn_blocking 中重新加载
        let pdfium_guard = pdfium_state.0.lock()
            .map_err(|e| format!("获取 PDFium 锁失败：{}", e))?;
        let pdfium = pdfium_guard.as_ref()
            .ok_or_else(|| "PDFium 不可用，无法解析 PDF 文件".to_string())?;
        crate::parser::pdf::parse_pdf_with_pdfium(pdfium, &fp)
    } else {
        tokio::task::spawn_blocking(move || import_file_internal(&fp))
            .await
            .map_err(|e| format!("文件导入任务失败: {}", e))?
    };

    if let Ok(ref content) = result {
        let ext = Path::new(&file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown")
            .to_lowercase();
        let row_count = match content {
            FileContent::Spreadsheet { sheets, .. } => sheets.iter().map(|s| s.row_count).sum(),
            FileContent::Document { paragraphs, .. } => paragraphs.len(),
        };
        crate::analytics::track(&app_handle, "file_imported", Some(serde_json::json!({
            "file_type": ext,
            "row_count": row_count,
        })));
    }

    result
}

/// 内部带密码文件导入
fn import_file_with_password_internal(path: &str, password: &str) -> Result<FileContent, String> {
    let file_path = Path::new(path);

    if !file_path.exists() {
        return Err(format!("文件不存在：{}", path));
    }

    let metadata = std::fs::metadata(path)
        .map_err(|e| format!("无法读取文件信息：{}", e))?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(format!(
            "文件大小超过限制（最大 50MB），当前文件大小：{:.1}MB",
            metadata.len() as f64 / 1024.0 / 1024.0
        ));
    }

    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .unwrap_or_default();

    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // 使用 office-crypto 解密
    let decrypted = office_crypto::decrypt_from_file(path, password)
        .map_err(|e| {
            let msg = e.to_string().to_lowercase();
            if msg.contains("password") || msg.contains("incorrect") || msg.contains("invalid") || msg.contains("wrong") {
                "WRONG_PASSWORD".to_string()
            } else if msg.contains("unsupported") || msg.contains("not supported") {
                format!("该文件的加密格式暂不支持解密：{}", e)
            } else {
                format!("文件解密失败：{}", e)
            }
        })?;

    match extension.as_str() {
        "xlsx" | "xls" => {
            let file_type = if extension == "xls" { FileType::Xls } else { FileType::Xlsx };
            parse_excel_from_bytes(decrypted, file_name, file_type)
        }
        "docx" => parse_docx_from_bytes(decrypted, file_name),
        _ => Err(format!("不支持对 {} 格式的文件进行解密", extension)),
    }
}

/// 带密码导入加密文件
#[tauri::command]
pub async fn import_file_with_password(
    file_path: String,
    password: String,
    app_handle: tauri::AppHandle,
) -> Result<FileContent, String> {
    let fp = file_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        import_file_with_password_internal(&fp, &password)
    })
    .await
    .map_err(|e| format!("文件导入任务失败: {}", e))?;

    if let Ok(ref content) = result {
        let ext = Path::new(&file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown")
            .to_lowercase();
        let row_count = match content {
            FileContent::Spreadsheet { sheets, .. } => sheets.iter().map(|s| s.row_count).sum(),
            FileContent::Document { paragraphs, .. } => paragraphs.len(),
        };
        crate::analytics::track(&app_handle, "file_imported_encrypted", Some(serde_json::json!({
            "file_type": ext,
            "row_count": row_count,
        })));
    }

    result
}

/// 导入粘贴板文本，解析为 FileContent（复用 TXT 段落逻辑）
#[tauri::command]
pub async fn import_clipboard_text(text: String, app_handle: tauri::AppHandle) -> Result<FileContent, String> {
    if text.is_empty() {
        return Err("粘贴板内容为空".to_string());
    }

    // 限制文本长度（10 万字符）
    if text.chars().count() > 100_000 {
        return Err("文本过长（超过 10 万字符），请缩短后重试".to_string());
    }

    let paragraphs: Vec<crate::models::sensitive::Paragraph> = text
        .lines()
        .enumerate()
        .map(|(i, line)| crate::models::sensitive::Paragraph {
            index: i,
            text: line.to_string(),
            style: "normal".to_string(),
            table_position: None,
            pdf_position: None,
        })
        .collect();

    let row_count = paragraphs.len();

    let content = FileContent::Document {
        file_name: "clipboard.txt".to_string(),
        file_type: FileType::Txt,
        paragraphs,
        encoding: Some("utf-8".to_string()),
    };

    crate::analytics::track(&app_handle, "clipboard_imported", Some(serde_json::json!({
        "row_count": row_count,
    })));

    Ok(content)
}

/// 检查文件是否存在
#[tauri::command]
pub async fn check_file_exists(file_path: String) -> Result<bool, String> {
    if Path::new(&file_path).exists() {
        Ok(true)
    } else {
        Err(format!("文件不存在：{}", file_path))
    }
}

/// 导出脱敏后的文件
#[tauri::command]
pub async fn export_file(
    content: FileContent,
    output_path: String,
    original_path: Option<String>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let start = std::time::Instant::now();
    let ext = std::path::Path::new(&output_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown")
        .to_lowercase();

    // 从 FileContent 中提取原文件类型
    let source_file_type = match &content {
        FileContent::Spreadsheet { file_type, .. } => format!("{:?}", file_type).to_lowercase(),
        FileContent::Document { file_type, .. } => format!("{:?}", file_type).to_lowercase(),
    };

    let result = tokio::task::spawn_blocking(move || {
        export_content(&content, &output_path, original_path.as_deref())
    })
    .await
    .map_err(|e| format!("文件导出任务失败: {}", e))?;

    if result.is_ok() {
        crate::analytics::track(&app_handle, "file_exported", Some(serde_json::json!({
            "source_file_type": source_file_type,
            "target_file_type": ext,
            "duration_ms": start.elapsed().as_millis() as u64,
        })));
    }

    result
}

/// 内部导出逻辑（供 export_file 和还原导出复用）
pub fn export_content(
    content: &FileContent,
    output_path: &str,
    original_path: Option<&str>,
) -> Result<(), String> {
    match content {
        FileContent::Spreadsheet {
            file_type,
            sheets,
            ..
        } => match file_type {
            FileType::Csv => {
                if let Some(sheet) = sheets.first() {
                    export_csv(output_path, &sheet.headers, &sheet.rows)
                } else {
                    export_csv(output_path, &[], &[])
                }
            }
            FileType::Xlsx | FileType::Xls => {
                // rust_xlsxwriter 只能写 xlsx 格式，当输出路径为 .xls 时自动修正
                let final_path = if matches!(file_type, FileType::Xls) {
                    let p = std::path::Path::new(output_path);
                    if p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("xls"))
                        .unwrap_or(false)
                    {
                        p.with_extension("xlsx")
                            .to_string_lossy()
                            .to_string()
                    } else {
                        output_path.to_string()
                    }
                } else {
                    output_path.to_string()
                };
                export_xlsx(&final_path, sheets)
            }
            _ => Err("不支持的导出格式".to_string()),
        },
        FileContent::Document { file_type, paragraphs, encoding, .. } => match file_type {
            FileType::Txt => export_txt(paragraphs, output_path, encoding.as_deref()),
            FileType::Pdf => {
                // PDF 涂黑导出需要走 export_pdf_redacted_cmd command
                // 此处提供简单的纯文本降级导出
                Err("PDF 导出请使用专用的涂黑导出功能".to_string())
            }
            _ => {
                let src = original_path
                    .ok_or_else(|| "导出 Word 文档需要提供原始文件路径".to_string())?;
                export_docx(src, paragraphs, output_path)
            }
        },
    }
}

/// 导出为 CSV 文件
fn export_csv(path: &str, headers: &[String], rows: &[Vec<crate::models::sensitive::CellValue>]) -> Result<(), String> {
    let mut writer = csv::Writer::from_path(path)
        .map_err(|e| format!("创建 CSV 文件失败: {}", e))?;

    writer
        .write_record(headers)
        .map_err(|e| format!("写入表头失败: {}", e))?;

    for row in rows {
        let string_row: Vec<&str> = row.iter().map(|cv| cv.text.as_str()).collect();
        writer
            .write_record(&string_row)
            .map_err(|e| format!("写入数据行失败: {}", e))?;
    }

    writer
        .flush()
        .map_err(|e| format!("写入文件失败: {}", e))?;

    Ok(())
}

/// 将文件复制到系统剪贴板（等效于 Finder/资源管理器中复制文件）
#[tauri::command]
pub async fn copy_file_to_clipboard(file_path: String) -> Result<(), String> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("文件不存在：{}", file_path));
    }

    #[cfg(target_os = "macos")]
    {
        let abs_path = path.canonicalize()
            .map_err(|e| format!("无法解析文件路径：{}", e))?;
        let abs_str = abs_path.to_string_lossy();
        // 通过 osascript 将文件引用写入剪贴板
        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg(format!("set the clipboard to (POSIX file \"{}\")", abs_str))
            .output()
            .map_err(|e| format!("执行剪贴板命令失败：{}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("复制到剪贴板失败：{}", stderr));
        }
    }

    #[cfg(target_os = "windows")]
    {
        let abs_path = path.canonicalize()
            .map_err(|e| format!("无法解析文件路径：{}", e))?;
        let abs_str = abs_path.to_string_lossy().replace("\\\\?\\", "");
        // 通过 PowerShell 将文件复制到剪贴板
        let output = std::process::Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(format!("Set-Clipboard -Path '{}'", abs_str))
            .output()
            .map_err(|e| format!("执行剪贴板命令失败：{}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("复制到剪贴板失败：{}", stderr));
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        return Err("当前平台不支持复制文件到剪贴板".to_string());
    }

    Ok(())
}

/// 导出涂黑后的 PDF 文件
/// 不需要传入 FileContent（pdf_position 经 IPC 会丢失），内部重新解析获取坐标
#[tauri::command]
pub async fn export_pdf_redacted_cmd(
    original_path: String,
    sensitive_items: Vec<crate::models::sensitive::SensitiveItem>,
    output_path: String,
    pdfium_state: tauri::State<'_, crate::PdfiumState>,
) -> Result<(), String> {
    let pdfium_guard = pdfium_state.0.lock()
        .map_err(|e| format!("获取 PDFium 锁失败：{}", e))?;

    let pdfium = pdfium_guard.as_ref()
        .ok_or_else(|| "PDFium 不可用，无法导出 PDF".to_string())?;

    // 直接使用图片方案导出（原地涂黑方案在 drop PdfPageObject 时会触发
    // FPDFPageObj_Destroy segfault，pdfium-render 0.8 的已知问题）
    crate::parser::pdf_export::export_pdf_as_images(
        pdfium, &original_path, &sensitive_items, &output_path,
    )
}

/// 预览涂黑矩形：根据敏感项计算每页的涂黑区域（归一化屏幕坐标 0~1）
#[derive(Debug, Clone, Serialize)]
pub struct RedactOverlayRect {
    pub page_index: usize,
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// 计算 PDF 涂黑预览矩形，供前端 RedactionOverlay 渲染
#[tauri::command]
pub async fn compute_pdf_redact_preview(
    file_path: String,
    sensitive_items: Vec<crate::models::sensitive::SensitiveItem>,
    pdfium_state: tauri::State<'_, crate::PdfiumState>,
) -> Result<Vec<RedactOverlayRect>, String> {
    let pdfium_guard = pdfium_state.0.lock()
        .map_err(|e| format!("获取 PDFium 锁失败：{}", e))?;
    let pdfium = pdfium_guard.as_ref()
        .ok_or_else(|| "PDFium 不可用".to_string())?;

    // 解析 PDF 获取段落坐标
    let file_content = crate::parser::pdf::parse_pdf_with_pdfium(pdfium, &file_path)?;
    let paragraphs = match &file_content {
        FileContent::Document { paragraphs, .. } => paragraphs,
        _ => return Ok(vec![]),
    };

    // 复用 compute_redact_targets 逻辑
    let redact_by_page = crate::parser::pdf_export::compute_redact_targets(paragraphs, &sensitive_items);

    // 加载 PDF 获取页面尺寸，转换为归一化屏幕坐标
    let doc = pdfium.load_pdf_from_file(&file_path, None)
        .map_err(|e| format!("打开 PDF 失败：{}", e))?;

    let mut result = Vec::new();

    for (&page_index, targets) in &redact_by_page {
        let page = doc.pages().get(page_index as u16)
            .map_err(|e| format!("读取第 {} 页失败：{}", page_index + 1, e))?;
        let pw = page.width().value;
        let ph = page.height().value;

        for target in targets {
            result.push(RedactOverlayRect {
                page_index,
                left: target.left / pw,
                top: 1.0 - target.top / ph,       // PDF Y → 屏幕 Y
                right: target.right / pw,
                bottom: 1.0 - target.bottom / ph,
            });
        }
    }

    // 额外收集 pdf_bbox 项（已经是归一化屏幕坐标）
    for item in &sensitive_items {
        if let Some(ref bbox) = item.pdf_bbox {
            result.push(RedactOverlayRect {
                page_index: bbox.page_index,
                left: bbox.left,
                top: bbox.top,
                right: bbox.right,
                bottom: bbox.bottom,
            });
        }
    }

    Ok(result)
}

/// 渲染 PDF 文件每一页为 PNG 图片（base64 编码），返回页面尺寸信息
#[tauri::command]
pub async fn render_pdf_pages(
    file_path: String,
    pdfium_state: tauri::State<'_, crate::PdfiumState>,
) -> Result<Vec<PdfPageRender>, String> {
    use base64::Engine;
    use pdfium_render::prelude::*;

    let pdfium_guard = pdfium_state.0.lock()
        .map_err(|e| format!("获取 PDFium 锁失败：{}", e))?;
    let pdfium = pdfium_guard.as_ref()
        .ok_or_else(|| "PDFium 不可用，无法渲染 PDF 页面".to_string())?;

    let doc = pdfium
        .load_pdf_from_file(&file_path, None)
        .map_err(|e| format!("打开 PDF 文件失败：{}", e))?;

    let page_count = doc.pages().len();
    if page_count == 0 {
        return Err("PDF 文件没有页面".to_string());
    }

    // 预览用途，目标宽度 1200px
    let render_config = PdfRenderConfig::new()
        .set_target_width(1200)
        .set_clear_color(PdfColor::WHITE);

    let mut results = Vec::with_capacity(page_count as usize);

    for page_idx in 0..page_count {
        let page = doc
            .pages()
            .get(page_idx as u16)
            .map_err(|e| format!("读取第 {} 页失败：{}", page_idx + 1, e))?;

        let page_width = page.width().value;
        let page_height = page.height().value;

        // 渲染为位图
        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| format!("渲染第 {} 页失败：{}", page_idx + 1, e))?;

        let dynamic_image = bitmap.as_image();
        let img_width = dynamic_image.width();
        let img_height = dynamic_image.height();

        // 编码为 PNG bytes
        let mut png_bytes: Vec<u8> = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_bytes);
        dynamic_image
            .write_to(&mut cursor, image::ImageFormat::Png)
            .map_err(|e| format!("第 {} 页 PNG 编码失败：{}", page_idx + 1, e))?;

        // base64 编码
        let image_base64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);

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

        results.push(PdfPageRender {
            page_index: page_idx as usize,
            image_base64,
            image_width: img_width,
            image_height: img_height,
            page_width,
            page_height,
            text_objects,
        });
    }

    Ok(results)
}

/// 导出为 Excel 文件（支持多 Sheet，按 CellType 分发写入）
fn export_xlsx(path: &str, sheets: &[SheetData]) -> Result<(), String> {
    use rust_xlsxwriter::{Workbook, Format, ExcelDateTime};
    use crate::models::sensitive::CellType;

    let mut workbook = Workbook::new();

    // 预建日期/时间格式
    let date_format = Format::new().set_num_format("yyyy-mm-dd");
    let datetime_format = Format::new().set_num_format("yyyy-mm-dd hh:mm:ss");

    for sheet in sheets {
        let worksheet = workbook.add_worksheet();

        // 设置 Sheet 名称（非空时设置）
        if !sheet.name.is_empty() {
            worksheet
                .set_name(&sheet.name)
                .map_err(|e| format!("设置工作表名称失败: {}", e))?;
        }

        for (col, header) in sheet.headers.iter().enumerate() {
            worksheet
                .write_string(0, col as u16, header)
                .map_err(|e| format!("写入表头失败: {}", e))?;
        }

        for (row_idx, row) in sheet.rows.iter().enumerate() {
            let excel_row = (row_idx + 1) as u32;
            for (col_idx, cell) in row.iter().enumerate() {
                let excel_col = col_idx as u16;
                match &cell.cell_type {
                    CellType::Empty => {
                        // 空单元格不写入
                    }
                    CellType::Integer => {
                        if let Ok(n) = cell.text.parse::<i64>() {
                            worksheet.write_number(excel_row, excel_col, n as f64)
                                .map_err(|e| format!("写入数据失败: {}", e))?;
                        } else {
                            worksheet.write_string(excel_row, excel_col, &cell.text)
                                .map_err(|e| format!("写入数据失败: {}", e))?;
                        }
                    }
                    CellType::Float => {
                        if let Ok(f) = cell.text.parse::<f64>() {
                            worksheet.write_number(excel_row, excel_col, f)
                                .map_err(|e| format!("写入数据失败: {}", e))?;
                        } else {
                            worksheet.write_string(excel_row, excel_col, &cell.text)
                                .map_err(|e| format!("写入数据失败: {}", e))?;
                        }
                    }
                    CellType::Boolean => {
                        let b = cell.text == "true";
                        worksheet.write_boolean(excel_row, excel_col, b)
                            .map_err(|e| format!("写入数据失败: {}", e))?;
                    }
                    CellType::DateTime { serial } => {
                        match ExcelDateTime::from_serial_datetime(*serial) {
                            Ok(dt) => {
                                let fmt = if serial.fract() == 0.0 { &date_format } else { &datetime_format };
                                worksheet.write_datetime_with_format(excel_row, excel_col, &dt, fmt)
                                    .map_err(|e| format!("写入数据失败: {}", e))?;
                            }
                            Err(_) => {
                                worksheet.write_string(excel_row, excel_col, &cell.text)
                                    .map_err(|e| format!("写入数据失败: {}", e))?;
                            }
                        }
                    }
                    CellType::DateTimeIso => {
                        match ExcelDateTime::parse_from_str(&cell.text) {
                            Ok(dt) => {
                                worksheet.write_datetime_with_format(excel_row, excel_col, &dt, &datetime_format)
                                    .map_err(|e| format!("写入数据失败: {}", e))?;
                            }
                            Err(_) => {
                                worksheet.write_string(excel_row, excel_col, &cell.text)
                                    .map_err(|e| format!("写入数据失败: {}", e))?;
                            }
                        }
                    }
                    CellType::Text | CellType::DurationIso => {
                        worksheet.write_string(excel_row, excel_col, &cell.text)
                            .map_err(|e| format!("写入数据失败: {}", e))?;
                    }
                }
            }
        }
    }

    workbook
        .save(path)
        .map_err(|e| format!("保存 Excel 文件失败: {}", e))?;

    Ok(())
}
