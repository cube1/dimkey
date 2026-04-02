use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write, Cursor};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
use quick_xml::Reader as XmlReader;
use quick_xml::Writer as XmlWriter;
use quick_xml::events::{Event, BytesStart, BytesText};
use crate::models::sensitive::Paragraph;
use crate::parser::word::parse_document_xml;

/// 导出脱敏后的 docx 文件
/// 读取原始 docx，替换 document.xml 中的段落文本，其他内容原封不动
pub fn export_docx(
    original_path: &str,
    paragraphs: &[Paragraph],
    output_path: &str,
) -> Result<(), String> {
    let file = File::open(original_path)
        .map_err(|e| format!("无法打开原始文件：{}", e))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| format!("无法读取原始 docx：{}", e))?;

    let out_file = File::create(output_path)
        .map_err(|e| format!("无法创建输出文件：{}", e))?;
    let mut zip_writer = ZipWriter::new(out_file);

    // 先读取原始 document.xml 解析原始段落，用于对比找出实际被修改的段落
    let original_xml = {
        let mut doc_entry = archive.by_name("word/document.xml")
            .map_err(|e| format!("读取 document.xml 失败：{}", e))?;
        let mut content = String::new();
        doc_entry.read_to_string(&mut content)
            .map_err(|e| format!("读取 document.xml 失败：{}", e))?;
        content
    };
    let original_paragraphs = parse_document_xml(&original_xml)?;
    let original_text_map: HashMap<usize, &str> = original_paragraphs
        .iter()
        .map(|p| (p.index, p.text.as_str()))
        .collect();

    // 只把实际被脱敏修改过的段落放入替换映射，未改变的段落保持原始 XML 不动
    let para_map: HashMap<usize, &str> = paragraphs
        .iter()
        .filter(|p| {
            original_text_map.get(&p.index)
                .map_or(true, |orig| *orig != p.text.as_str())
        })
        .map(|p| (p.index, p.text.as_str()))
        .collect();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| format!("读取 ZIP entry 失败：{}", e))?;

        let entry_name = entry.name().to_string();
        let options = SimpleFileOptions::default()
            .compression_method(entry.compression());

        if entry_name == "word/document.xml" {
            // 替换段落文本（使用已缓存的原始 XML）
            let new_xml = replace_paragraph_texts(&original_xml, &para_map)?;

            zip_writer.start_file(&entry_name, options)
                .map_err(|e| format!("写入 ZIP entry 失败：{}", e))?;
            zip_writer.write_all(new_xml.as_bytes())
                .map_err(|e| format!("写入 document.xml 失败：{}", e))?;
        } else {
            // 其他文件原封不动复制
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)
                .map_err(|e| format!("读取 ZIP entry 失败：{}", e))?;

            zip_writer.start_file(&entry_name, options)
                .map_err(|e| format!("写入 ZIP entry 失败：{}", e))?;
            zip_writer.write_all(&buf)
                .map_err(|e| format!("写入 ZIP entry 失败：{}", e))?;
        }
    }

    zip_writer.finish()
        .map_err(|e| format!("完成 ZIP 写入失败：{}", e))?;

    Ok(())
}

/// 替换 document.xml 中的段落文本
/// 策略：保留所有原始 XML 结构不变，仅替换 <w:t> 内的文本内容
/// - 第一个 <w:t>：写入完整的替换文本
/// - 后续 <w:t>：清空文本
/// 这样无论文档包含多复杂的结构（hyperlink、bookmarkStart、mc:AlternateContent 等）都不会丢失
fn replace_paragraph_texts(
    xml: &str,
    para_map: &HashMap<usize, &str>,
) -> Result<String, String> {
    let mut reader = XmlReader::from_str(xml);
    let mut output = Cursor::new(Vec::new());
    let mut writer = XmlWriter::new(&mut output);

    let mut para_index: usize = 0;
    let mut in_paragraph = false;
    let mut in_ppr = false;
    let mut replacing = false;      // 当前段落是否需要替换
    let mut in_run = false;         // 是否在 <w:r> 内
    let mut t_count: usize = 0;    // 当前段落内已遇到的 <w:t> 数量（pPr 外、run 内）
    let mut in_replacing_t = false; // 正在替换的 <w:t> 内部，需跳过原始文本

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = local_name(&name);

                if local == "p" && is_w_ns(&name) && !in_paragraph {
                    in_paragraph = true;
                    replacing = needs_replacement(para_index, para_map);
                    t_count = 0;
                    in_run = false;
                    writer.write_event(Event::Start(e.clone()))
                        .map_err(|e| format!("XML 写入失败：{}", e))?;
                } else if local == "pPr" && in_paragraph && !in_ppr {
                    in_ppr = true;
                    writer.write_event(Event::Start(e.clone()))
                        .map_err(|e| format!("XML 写入失败：{}", e))?;
                } else if local == "r" && is_w_ns(&name) && in_paragraph && !in_ppr {
                    in_run = true;
                    writer.write_event(Event::Start(e.clone()))
                        .map_err(|e| format!("XML 写入失败：{}", e))?;
                } else if local == "t" && is_w_ns(&name) && in_run && replacing && !in_ppr {
                    t_count += 1;
                    if t_count == 1 {
                        // 第一个 <w:t>：写入带 xml:space="preserve" 的开始标签 + 替换文本
                        let mut t_start = BytesStart::new("w:t");
                        t_start.push_attribute(("xml:space", "preserve"));
                        writer.write_event(Event::Start(t_start))
                            .map_err(|e| format!("XML 写入失败：{}", e))?;
                        let new_text = para_map.get(&para_index).unwrap_or(&"");
                        writer.write_event(Event::Text(BytesText::new(new_text)))
                            .map_err(|e| format!("XML 写入失败：{}", e))?;
                    } else {
                        // 后续 <w:t>：保留开始标签但不写文本（清空）
                        writer.write_event(Event::Start(e.clone()))
                            .map_err(|e| format!("XML 写入失败：{}", e))?;
                    }
                    in_replacing_t = true;
                } else {
                    writer.write_event(Event::Start(e.clone()))
                        .map_err(|e| format!("XML 写入失败：{}", e))?;
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = local_name(&name);

                if local == "p" && is_w_ns(&name) && in_paragraph {
                    // 回退路径：段落需要替换但不包含任何 <w:t>（如仅含图片的段落），创建一个新的 text run
                    if replacing && t_count == 0 {
                        write_text_run(&mut writer, &None, para_map.get(&para_index).unwrap_or(&""))?;
                    }
                    in_paragraph = false;
                    replacing = false;
                    in_run = false;
                    para_index += 1;
                    writer.write_event(Event::End(e.clone()))
                        .map_err(|e| format!("XML 写入失败：{}", e))?;
                } else if local == "pPr" && in_ppr {
                    in_ppr = false;
                    writer.write_event(Event::End(e.clone()))
                        .map_err(|e| format!("XML 写入失败：{}", e))?;
                } else if local == "r" && is_w_ns(&name) && in_run && !in_ppr {
                    in_run = false;
                    writer.write_event(Event::End(e.clone()))
                        .map_err(|e| format!("XML 写入失败：{}", e))?;
                } else if local == "t" && in_replacing_t {
                    in_replacing_t = false;
                    writer.write_event(Event::End(e.clone()))
                        .map_err(|e| format!("XML 写入失败：{}", e))?;
                } else {
                    writer.write_event(Event::End(e.clone()))
                        .map_err(|e| format!("XML 写入失败：{}", e))?;
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_replacing_t {
                    // 跳过原始文本（已在 Start 中写入替换文本或清空）
                } else {
                    writer.write_event(Event::Text(e.clone()))
                        .map_err(|e| format!("XML 写入失败：{}", e))?;
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = local_name(&name);

                // 注意：自闭合 <w:p/> 不递增 para_index，
                // 与 parser（word.rs）行为保持一致（Empty 事件不触发 End 中的 += 1）
                if replacing && in_run && !in_ppr
                    && (local == "br" || local == "tab")
                    && is_w_ns(&name)
                {
                    // 解析时 <w:br/> 已转换为 \n，<w:tab/> 转为 \t（见 word.rs），
                    // 替换文本中已包含这些字符，保留原始元素会导致重复换行/制表符
                } else {
                    writer.write_event(Event::Empty(e.clone()))
                        .map_err(|e| format!("XML 写入失败：{}", e))?;
                }
            }
            Ok(Event::Eof) => break,
            Ok(event) => {
                // 声明、PI、注释等原样写入
                writer.write_event(event)
                    .map_err(|e| format!("XML 写入失败：{}", e))?;
            }
            Err(e) => return Err(format!("解析 XML 失败：{}", e)),
        }
        buf.clear();
    }

    let result = output.into_inner();
    String::from_utf8(result)
        .map_err(|e| format!("XML 编码转换失败：{}", e))
}

/// 判断某段落是否需要替换
fn needs_replacement(para_index: usize, para_map: &HashMap<usize, &str>) -> bool {
    para_map.contains_key(&para_index)
}

/// 写入 <w:t xml:space="preserve">text</w:t>
/// 注意：text 是明文（未转义），XmlWriter 会在写入时自动进行 XML 转义
fn write_t_element<W: std::io::Write>(
    writer: &mut XmlWriter<W>,
    text: &str,
) -> Result<(), String> {
    let mut t_start = BytesStart::new("w:t");
    t_start.push_attribute(("xml:space", "preserve"));
    writer.write_event(Event::Start(t_start))
        .map_err(|e| format!("XML 写入失败：{}", e))?;
    writer.write_event(Event::Text(BytesText::new(text)))
        .map_err(|e| format!("XML 写入失败：{}", e))?;
    writer.write_event(Event::End(quick_xml::events::BytesEnd::new("w:t")))
        .map_err(|e| format!("XML 写入失败：{}", e))?;
    Ok(())
}

/// 写入一个完整的 text run（含可选 rPr + w:t）
fn write_text_run<W: std::io::Write>(
    writer: &mut XmlWriter<W>,
    rpr: &Option<Vec<u8>>,
    text: &str,
) -> Result<(), String> {
    // <w:r>
    writer.write_event(Event::Start(BytesStart::new("w:r")))
        .map_err(|e| format!("XML 写入失败：{}", e))?;
    // rPr（如果有）
    if let Some(ref rpr_data) = rpr {
        writer.get_mut().write_all(rpr_data)
            .map_err(|e| format!("XML 写入失败：{}", e))?;
    }
    // <w:t>text</w:t>
    write_t_element(writer, text)?;
    // </w:r>
    writer.write_event(Event::End(quick_xml::events::BytesEnd::new("w:r")))
        .map_err(|e| format!("XML 写入失败：{}", e))?;
    Ok(())
}

/// 提取 local name
fn local_name(name: &str) -> &str {
    match name.find(':') {
        Some(pos) => &name[pos + 1..],
        None => name,
    }
}

/// 检查是否为 w: 命名空间
fn is_w_ns(name: &str) -> bool {
    name.starts_with("w:") || !name.contains(':')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_paragraph_texts() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>原始标题</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>包含</w:t></w:r>
      <w:r><w:t>敏感信息的段落</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>不需要替换的段落</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let mut para_map = HashMap::new();
        para_map.insert(1, "包含***信息的段落");

        let result = replace_paragraph_texts(xml, &para_map).unwrap();

        assert!(result.contains("原始标题"), "未替换的段落应保持原文");
        assert!(result.contains("包含***信息的段落"), "替换段落应包含新文本");
        assert!(result.contains("不需要替换的段落"), "未替换段落应保持原文");
        // 新策略：第一个 <w:t> 包含完整替换文本，第二个 <w:t> 被清空
        assert!(!result.contains("敏感信息的段落"), "原始文本应被替换");
    }

    #[test]
    fn test_replace_empty_map() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

        let para_map = HashMap::new();
        let result = replace_paragraph_texts(xml, &para_map).unwrap();
        assert!(result.contains("Hello"));
    }

    #[test]
    fn test_xml_special_chars_escaped() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>A &amp; B 公司</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let mut para_map = HashMap::new();
        para_map.insert(0, "A & B <脱敏>");

        let result = replace_paragraph_texts(xml, &para_map).unwrap();
        assert!(result.contains("A &amp; B &lt;脱敏&gt;"), "XML 特殊字符应被正确转义，实际结果: {}", result);
        // 验证输出是合法 XML
        let mut reader = XmlReader::from_str(&result);
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Eof) => break,
                Err(e) => panic!("输出 XML 不合法: {}", e),
                _ => {}
            }
            buf.clear();
        }
    }

    #[test]
    fn test_unchanged_paragraphs_preserve_structure() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:rPr><w:b/></w:rPr><w:t>标题</w:t></w:r>
    </w:p>
    <w:p>
      <w:r>
        <w:rPr><w:noProof/></w:rPr>
        <w:drawing><wp:inline>image_data</wp:inline></w:drawing>
      </w:r>
    </w:p>
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>单元格</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    <w:p>
      <w:r><w:t>尾部段落</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let para_map = HashMap::new();
        let result = replace_paragraph_texts(xml, &para_map).unwrap();

        assert!(result.contains("w:drawing"), "图片元素应保留");
        assert!(result.contains("image_data"), "图片数据应保留");
        assert!(result.contains("w:tbl"), "表格应保留");
        assert!(result.contains("单元格"), "表格内容应保留");
        assert!(result.contains("尾部段落"), "尾部段落应保留");
        assert!(result.contains("<w:b/>") || result.contains("<w:b />"), "格式属性应保留");
    }

    #[test]
    fn test_complex_structure_with_hyperlink() {
        // 模拟 Windows Word 生成的复杂结构：包含超链接、书签等
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:bookmarkStart w:id="0" w:name="_GoBack"/>
      <w:bookmarkEnd w:id="0"/>
      <w:r><w:rPr><w:b/></w:rPr><w:t>标题内容</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>请访问</w:t></w:r>
      <w:hyperlink r:id="rId5">
        <w:r><w:rPr><w:color w:val="0000FF"/></w:rPr><w:t>链接文字</w:t></w:r>
      </w:hyperlink>
      <w:r><w:t>了解详情</w:t></w:r>
    </w:p>
    <w:p>
      <w:proofErr w:type="spellStart"/>
      <w:r><w:t>张三</w:t></w:r>
      <w:proofErr w:type="spellEnd"/>
      <w:r><w:t>的手机号是</w:t></w:r>
      <w:r><w:t>13800001111</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let mut para_map = HashMap::new();
        // 替换第 2 个段落（含敏感信息的段落）
        para_map.insert(2, "***的手机号是***");

        let result = replace_paragraph_texts(xml, &para_map).unwrap();

        // 标题和超链接段落不受影响
        assert!(result.contains("标题内容"), "标题应保留");
        assert!(result.contains("w:hyperlink"), "超链接结构应保留");
        assert!(result.contains("请访问"), "超链接前文字应保留");
        assert!(result.contains("链接文字"), "超链接文字应保留");
        assert!(result.contains("了解详情"), "超链接后文字应保留");
        assert!(result.contains("w:bookmarkStart"), "书签应保留");
        assert!(result.contains("w:bookmarkEnd"), "书签应保留");

        // 敏感段落的文本被替换
        assert!(result.contains("***的手机号是***"), "敏感段落应包含替换文本");
        assert!(!result.contains("13800001111"), "原始敏感信息应被替换");
        // proofErr 等元素应保留
        assert!(result.contains("w:proofErr"), "拼写检查标记应保留");
    }

    #[test]
    fn test_mc_alternate_content() {
        // 模拟 Windows Word 的 mc:AlternateContent 结构
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
  <w:body>
    <w:p>
      <mc:AlternateContent>
        <mc:Choice Requires="w14">
          <w:r><w:t>现代内容</w:t></w:r>
        </mc:Choice>
        <mc:Fallback>
          <w:r><w:t>兼容内容</w:t></w:r>
        </mc:Fallback>
      </mc:AlternateContent>
      <w:r><w:t>普通文本</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>第二段</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let mut para_map = HashMap::new();
        para_map.insert(0, "脱敏后的内容");

        let result = replace_paragraph_texts(xml, &para_map).unwrap();

        // mc:AlternateContent 结构应完整保留
        assert!(result.contains("mc:AlternateContent"), "AlternateContent 应保留");
        assert!(result.contains("mc:Choice"), "Choice 应保留");
        assert!(result.contains("mc:Fallback"), "Fallback 应保留");
        // 替换文本应出现在第一个 <w:t> 中
        assert!(result.contains("脱敏后的内容"), "替换文本应存在");
        // 第二段不受影响
        assert!(result.contains("第二段"), "未替换段落应保留");
    }

    #[test]
    fn test_replace_text_inside_table() {
        // 测试替换表格内段落的文本
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>表格前段落</w:t></w:r></w:p>
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>姓名</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>13800001111</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    <w:p><w:r><w:t>表格后段落</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

        let mut para_map = HashMap::new();
        // 替换表格内第 2 个单元格（para_index=2）
        para_map.insert(2, "138****1111");

        let result = replace_paragraph_texts(xml, &para_map).unwrap();

        assert!(result.contains("表格前段落"), "表格前段落应保留");
        assert!(result.contains("姓名"), "未替换的表格单元格应保留");
        assert!(result.contains("138****1111"), "表格内敏感信息应被替换");
        assert!(!result.contains("13800001111"), "原始手机号应被替换");
        assert!(result.contains("表格后段落"), "表格后段落应保留");
        assert!(result.contains("w:tbl"), "表格结构应保留");
    }

    #[test]
    fn test_replace_with_empty_text() {
        // 测试用空字符串替换段落文本
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>需要清空的段落</w:t></w:r></w:p>
    <w:p><w:r><w:t>保留的段落</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

        let mut para_map = HashMap::new();
        para_map.insert(0, "");

        let result = replace_paragraph_texts(xml, &para_map).unwrap();

        assert!(!result.contains("需要清空的段落"), "原文应被清空");
        assert!(result.contains("保留的段落"), "未替换段落应保留");
        // 验证输出是合法 XML
        let mut reader = XmlReader::from_str(&result);
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Eof) => break,
                Err(e) => panic!("输出 XML 不合法: {}", e),
                _ => {}
            }
            buf.clear();
        }
    }

    #[test]
    fn test_paragraph_with_drawing_preserved() {
        // 含图片的段落在替换时保留 drawing 元素
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r>
        <w:rPr><w:noProof/></w:rPr>
        <w:drawing><wp:inline>IMG_DATA</wp:inline></w:drawing>
      </w:r>
      <w:r><w:t>图片说明文字</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let mut para_map = HashMap::new();
        para_map.insert(0, "脱敏说明");

        let result = replace_paragraph_texts(xml, &para_map).unwrap();

        // 图片结构应完整保留
        assert!(result.contains("w:drawing"), "drawing 元素应保留");
        assert!(result.contains("IMG_DATA"), "图片数据应保留");
        assert!(result.contains("w:noProof"), "noProof 应保留");
        // 文本被替换
        assert!(result.contains("脱敏说明"), "替换文本应存在");
        assert!(!result.contains("图片说明文字"), "原始文本应被替换");
    }
}
