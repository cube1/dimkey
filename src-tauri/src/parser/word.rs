use std::fs::File;
use std::io::{Read, Seek, Cursor};
use std::path::Path;
use zip::ZipArchive;
use quick_xml::Reader as XmlReader;
use quick_xml::events::Event;
use crate::models::sensitive::{FileContent, FileType, Paragraph, TablePosition};

/// 从已打开的 ZipArchive 解析为 FileContent（公共逻辑）
fn parse_docx_archive<R: Read + Seek>(
    mut archive: ZipArchive<R>,
    file_name: String,
) -> Result<FileContent, String> {
    let mut xml_content = String::new();
    {
        let mut doc_file = archive
            .by_name("word/document.xml")
            .map_err(|e| format!("docx 文件格式异常，缺少 document.xml：{}", e))?;
        doc_file
            .read_to_string(&mut xml_content)
            .map_err(|e| format!("读取 document.xml 失败：{}", e))?;
    }

    let paragraphs = parse_document_xml(&xml_content)?;

    Ok(FileContent::Document {
        file_name,
        file_type: FileType::Docx,
        paragraphs,
        encoding: None,
    })
}

/// 解析 Word 文件（docx）为 FileContent
pub fn parse_docx(path: &str) -> Result<FileContent, String> {
    let file_path = Path::new(path);
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let file = File::open(path)
        .map_err(|e| format!("无法打开文件：{}", e))?;

    let mut archive = ZipArchive::new(file)
        .map_err(|e| format!("无法解析 docx 文件（非有效的 ZIP 格式）：{}", e))?;

    // 检测加密文件：加密的 docx 会包含 EncryptedPackage 或 EncryptionInfo
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            let name = entry.name().to_string();
            if name == "EncryptedPackage" || name == "EncryptionInfo" {
                return Err("ENCRYPTED:docx".to_string());
            }
        }
    }

    parse_docx_archive(archive, file_name)
}

/// 从内存字节解析 docx 文件（用于解密后的数据）
pub fn parse_docx_from_bytes(bytes: Vec<u8>, file_name: String) -> Result<FileContent, String> {
    let archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("解密后文件解析失败：{}", e))?;
    parse_docx_archive(archive, file_name)
}

/// 解析 document.xml 提取段落列表（支持表格结构识别）
pub fn parse_document_xml(xml: &str) -> Result<Vec<Paragraph>, String> {
    let mut reader = XmlReader::from_str(xml);
    let mut paragraphs: Vec<Paragraph> = Vec::new();

    // 段落状态追踪
    let mut in_paragraph = false;
    let mut in_run = false;
    let mut in_text = false;
    let mut in_ppr = false;       // <w:pPr>

    let mut current_text = String::new();
    let mut current_style = String::new();
    let mut para_index: usize = 0;

    // 表格嵌套追踪
    let mut table_depth: usize = 0;        // 表格嵌套深度
    let mut table_index: usize = 0;        // 当前文档中的第几个表格
    let mut in_table_row = false;           // 是否在 <w:tr> 内
    let mut in_table_cell = false;          // 是否在 <w:tc> 内
    let mut current_table_row: usize = 0;   // 当前表格行号
    let mut current_table_col: usize = 0;   // 当前表格列号
    let mut current_row_col_count: usize = 0;  // 当前行的列数
    // 记录当前行起始的 paragraphs 索引，用于 </w:tr> 时回填 col_count
    let mut row_start_para_idx: usize = 0;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name_bytes = e.name().as_ref().to_vec();
                let local = local_name(&name_bytes);
                match local {
                    b"tbl" if is_w_namespace(&name_bytes) => {
                        table_depth += 1;
                        if table_depth == 1 {
                            current_table_row = 0;
                        }
                    }
                    b"tr" if table_depth == 1 && is_w_namespace(&name_bytes) => {
                        in_table_row = true;
                        current_table_col = 0;
                        current_row_col_count = 0;
                        row_start_para_idx = paragraphs.len();
                    }
                    b"tc" if in_table_row && table_depth == 1 && is_w_namespace(&name_bytes) => {
                        in_table_cell = true;
                        current_row_col_count += 1;
                    }
                    b"p" if is_w_namespace(&name_bytes) => {
                        in_paragraph = true;
                        current_text.clear();
                        current_style.clear();
                    }
                    b"pPr" if in_paragraph => {
                        in_ppr = true;
                    }
                    b"pStyle" if in_ppr => {
                        // 读取 w:val 属性
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.as_ref()) == b"val" {
                                current_style = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    b"r" if in_paragraph => {
                        in_run = true;
                    }
                    b"t" if in_run => {
                        in_text = true;
                    }
                    b"tab" if in_run => {
                        current_text.push('\t');
                    }
                    b"br" if in_run => {
                        current_text.push('\n');
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_text {
                    let text = e.unescape()
                        .map_err(|err| format!("XML 文本解码失败：{}", err))?;
                    current_text.push_str(&text);
                }
            }
            Ok(Event::End(ref e)) => {
                let name_bytes = e.name().as_ref().to_vec();
                let local = local_name(&name_bytes);
                match local {
                    b"p" if in_paragraph => {
                        // 段落结束，保存（跳过纯空段落）
                        let trimmed = current_text.trim();
                        if !trimmed.is_empty() {
                            let style = normalize_style(&current_style);
                            let table_position = if table_depth == 1 && in_table_cell {
                                Some(TablePosition {
                                    table_index,
                                    row: current_table_row,
                                    col: current_table_col,
                                    col_count: 0, // 在 </w:tr> 时回填
                                })
                            } else {
                                None
                            };
                            paragraphs.push(Paragraph {
                                index: para_index,
                                text: current_text.clone(),
                                style,
                                table_position,
                                pdf_position: None,
                            });
                        }
                        para_index += 1;
                        in_paragraph = false;
                    }
                    b"tc" if in_table_cell && table_depth == 1 => {
                        in_table_cell = false;
                        current_table_col += 1;
                    }
                    b"tr" if in_table_row && table_depth == 1 => {
                        // 回填该行所有段落的 col_count
                        let col_count = current_row_col_count;
                        for p in &mut paragraphs[row_start_para_idx..] {
                            if let Some(ref mut tp) = p.table_position {
                                tp.col_count = col_count;
                            }
                        }
                        in_table_row = false;
                        current_table_row += 1;
                    }
                    b"tbl" if table_depth > 0 => {
                        table_depth -= 1;
                        if table_depth == 0 {
                            table_index += 1;
                        }
                    }
                    b"pPr" => {
                        in_ppr = false;
                    }
                    b"pStyle" => {}
                    b"r" => {
                        in_run = false;
                    }
                    b"t" => {
                        in_text = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("解析 XML 失败：{}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(paragraphs)
}

/// 提取 XML 标签的 local name（去掉命名空间前缀）
/// 例如 "w:p" → "p", "w:pPr" → "pPr"
fn local_name(name: &[u8]) -> &[u8] {
    match name.iter().position(|&b| b == b':') {
        Some(pos) => &name[pos + 1..],
        None => name,
    }
}

/// 检查是否为 w: 命名空间的元素（简单匹配）
fn is_w_namespace(name: &[u8]) -> bool {
    name.starts_with(b"w:") || !name.contains(&b':')
}

/// 将 Word 段落样式标识规范化为前端可用的样式名
fn normalize_style(style: &str) -> String {
    let lower = style.to_lowercase();
    match lower.as_str() {
        // 标题样式
        "heading1" | "1" => "heading1".to_string(),
        "heading2" | "2" => "heading2".to_string(),
        "heading3" | "3" => "heading3".to_string(),
        "heading4" | "4" => "heading4".to_string(),
        // 列表
        "listparagraph" | "a3" => "listParagraph".to_string(),
        // 默认
        _ => "normal".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_document_xml_basic() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:body>
                <w:p>
                    <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
                    <w:r><w:t>标题一</w:t></w:r>
                </w:p>
                <w:p>
                    <w:r><w:t>这是正文内容。</w:t></w:r>
                </w:p>
                <w:p>
                    <w:r><w:t>包含</w:t></w:r>
                    <w:r><w:t>多个 run 的段落</w:t></w:r>
                </w:p>
                <w:p></w:p>
            </w:body>
        </w:document>"#;

        let paragraphs = parse_document_xml(xml).unwrap();
        assert_eq!(paragraphs.len(), 3);
        assert_eq!(paragraphs[0].text, "标题一");
        assert_eq!(paragraphs[0].style, "heading1");
        assert!(paragraphs[0].table_position.is_none());
        assert_eq!(paragraphs[1].text, "这是正文内容。");
        assert_eq!(paragraphs[1].style, "normal");
        assert_eq!(paragraphs[2].text, "包含多个 run 的段落");
    }

    #[test]
    fn test_parse_empty_document() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:body></w:body>
        </w:document>"#;

        let paragraphs = parse_document_xml(xml).unwrap();
        assert_eq!(paragraphs.len(), 0);
    }

    #[test]
    fn test_normalize_style() {
        assert_eq!(normalize_style("Heading1"), "heading1");
        assert_eq!(normalize_style("heading2"), "heading2");
        assert_eq!(normalize_style("ListParagraph"), "listParagraph");
        assert_eq!(normalize_style(""), "normal");
        assert_eq!(normalize_style("SomeCustomStyle"), "normal");
    }

    #[test]
    fn test_parse_document_xml_with_table() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:body>
                <w:p>
                    <w:r><w:t>表格前的段落</w:t></w:r>
                </w:p>
                <w:tbl>
                    <w:tr>
                        <w:tc><w:p><w:r><w:t>姓名</w:t></w:r></w:p></w:tc>
                        <w:tc><w:p><w:r><w:t>电话</w:t></w:r></w:p></w:tc>
                        <w:tc><w:p><w:r><w:t>地址</w:t></w:r></w:p></w:tc>
                    </w:tr>
                    <w:tr>
                        <w:tc><w:p><w:r><w:t>张三</w:t></w:r></w:p></w:tc>
                        <w:tc><w:p><w:r><w:t>13800001111</w:t></w:r></w:p></w:tc>
                        <w:tc><w:p><w:r><w:t>北京市朝阳区</w:t></w:r></w:p></w:tc>
                    </w:tr>
                </w:tbl>
                <w:p>
                    <w:r><w:t>表格后的段落</w:t></w:r>
                </w:p>
            </w:body>
        </w:document>"#;

        let paragraphs = parse_document_xml(xml).unwrap();
        // 1 段落 + 6 表格单元格 + 1 段落 = 8 个
        assert_eq!(paragraphs.len(), 8);

        // 第一个段落：普通段落
        assert_eq!(paragraphs[0].text, "表格前的段落");
        assert!(paragraphs[0].table_position.is_none());

        // 表格第一行
        let tp1 = paragraphs[1].table_position.as_ref().unwrap();
        assert_eq!(paragraphs[1].text, "姓名");
        assert_eq!(tp1.table_index, 0);
        assert_eq!(tp1.row, 0);
        assert_eq!(tp1.col, 0);
        assert_eq!(tp1.col_count, 3);

        let tp2 = paragraphs[2].table_position.as_ref().unwrap();
        assert_eq!(paragraphs[2].text, "电话");
        assert_eq!(tp2.col, 1);
        assert_eq!(tp2.col_count, 3);

        let tp3 = paragraphs[3].table_position.as_ref().unwrap();
        assert_eq!(paragraphs[3].text, "地址");
        assert_eq!(tp3.col, 2);

        // 表格第二行
        let tp4 = paragraphs[4].table_position.as_ref().unwrap();
        assert_eq!(paragraphs[4].text, "张三");
        assert_eq!(tp4.row, 1);
        assert_eq!(tp4.col, 0);

        let tp5 = paragraphs[5].table_position.as_ref().unwrap();
        assert_eq!(paragraphs[5].text, "13800001111");
        assert_eq!(tp5.row, 1);
        assert_eq!(tp5.col, 1);

        // 最后一个段落：普通段落
        assert_eq!(paragraphs[7].text, "表格后的段落");
        assert!(paragraphs[7].table_position.is_none());

        // para_index 应该全局连续
        assert_eq!(paragraphs[0].index, 0);
        assert_eq!(paragraphs[1].index, 1);
        assert_eq!(paragraphs[7].index, 7);
    }
}
