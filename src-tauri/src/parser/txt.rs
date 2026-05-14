use crate::models::sensitive::{FileContent, FileType, Paragraph};
use std::path::Path;

/// 解析 TXT 文件为 FileContent
///
/// 编码检测顺序：UTF-8（含 BOM）→ GBK/GB2312
/// 每行映射为一个 Paragraph，空行保留以维持行号对应关系
pub fn parse_txt(path: &str) -> Result<FileContent, String> {
    let file_path = Path::new(path);
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.txt")
        .to_string();

    let bytes = std::fs::read(path)
        .map_err(|e| format!("读取文件失败：{}", e))?;

    let (text, encoding) = decode_text(&bytes)?;

    let paragraphs: Vec<Paragraph> = text
        .lines()
        .enumerate()
        .map(|(i, line)| Paragraph {
            index: i,
            text: line.to_string(),
            style: "normal".to_string(),
            table_position: None,
            pdf_position: None,
        })
        .collect();

    Ok(FileContent::Document {
        file_name,
        file_type: FileType::Txt,
        paragraphs,
        encoding: Some(encoding),
    })
}

/// 解码文本字节，返回 (解码后文本, 编码名称)
///
/// 检测顺序：
/// 1. UTF-8 BOM → 跳过 BOM 后按 UTF-8 解码
/// 2. 有效 UTF-8 → 直接使用
/// 3. GBK 尝试解码
fn decode_text(bytes: &[u8]) -> Result<(String, String), String> {
    // UTF-8 BOM 检测
    let data = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    };

    // 尝试 UTF-8
    if let Ok(text) = std::str::from_utf8(data) {
        return Ok((text.to_string(), "utf-8".to_string()));
    }

    // 尝试 GBK
    let (decoded, _, had_errors) = encoding_rs::GBK.decode(data);
    if !had_errors {
        return Ok((decoded.into_owned(), "gbk".to_string()));
    }

    Err("不支持的文件编码，请将文件转换为 UTF-8 编码后重试".to_string())
}

/// 导出脱敏后的 TXT 文件
///
/// 按原始编码写回，段落之间用换行符连接
/// 当 `watermark` 为 `Some(text)` 时，在文件首行写入 `# {text}` 作为试用过期水印。
pub fn export_txt(
    paragraphs: &[Paragraph],
    output_path: &str,
    encoding: Option<&str>,
    watermark: Option<&str>,
) -> Result<(), String> {
    let body: String = paragraphs
        .iter()
        .map(|p| p.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let text = match watermark {
        Some(wm) => format!("# {}\n{}", wm, body),
        None => body,
    };

    let bytes = match encoding.unwrap_or("utf-8") {
        "gbk" => {
            let (encoded, _, had_errors) = encoding_rs::GBK.encode(&text);
            if had_errors {
                return Err("部分字符无法用 GBK 编码，建议导出为 UTF-8 格式".to_string());
            }
            encoded.into_owned()
        }
        _ => text.into_bytes(), // UTF-8
    };

    std::fs::write(output_path, bytes)
        .map_err(|e| format!("写入文件失败：{}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_utf8() {
        let text = "你好世界\n第二行";
        let bytes = text.as_bytes();
        let (decoded, enc) = decode_text(bytes).unwrap();
        assert_eq!(decoded, text);
        assert_eq!(enc, "utf-8");
    }

    #[test]
    fn test_decode_utf8_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("你好".as_bytes());
        let (decoded, enc) = decode_text(&bytes).unwrap();
        assert_eq!(decoded, "你好");
        assert_eq!(enc, "utf-8");
    }

    #[test]
    fn test_decode_gbk() {
        // "你好" 的 GBK 编码
        let (encoded, _, _) = encoding_rs::GBK.encode("你好世界");
        let (decoded, enc) = decode_text(&encoded).unwrap();
        assert_eq!(decoded, "你好世界");
        assert_eq!(enc, "gbk");
    }

    #[test]
    fn test_parse_txt_paragraphs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "第一行\n第二行\n\n第四行").unwrap();

        let content = parse_txt(path.to_str().unwrap()).unwrap();
        match content {
            FileContent::Document { paragraphs, encoding, .. } => {
                assert_eq!(paragraphs.len(), 4);
                assert_eq!(paragraphs[0].text, "第一行");
                assert_eq!(paragraphs[1].text, "第二行");
                assert_eq!(paragraphs[2].text, ""); // 空行保留
                assert_eq!(paragraphs[3].text, "第四行");
                assert_eq!(encoding, Some("utf-8".to_string()));
            }
            _ => panic!("应返回 Document 变体"),
        }
    }

    #[test]
    fn test_export_txt_utf8() {
        let paragraphs = vec![
            Paragraph { index: 0, text: "第一行".to_string(), style: "normal".to_string(), table_position: None, pdf_position: None },
            Paragraph { index: 1, text: "第二行".to_string(), style: "normal".to_string(), table_position: None, pdf_position: None },
        ];
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.txt");

        export_txt(&paragraphs, path.to_str().unwrap(), Some("utf-8"), None).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "第一行\n第二行");
    }

    #[test]
    fn test_export_txt_with_watermark() {
        let paragraphs = vec![
            Paragraph { index: 0, text: "第一行".to_string(), style: "normal".to_string(), table_position: None, pdf_position: None },
            Paragraph { index: 1, text: "第二行".to_string(), style: "normal".to_string(), table_position: None, pdf_position: None },
        ];
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.txt");

        export_txt(&paragraphs, path.to_str().unwrap(), Some("utf-8"), Some("Dimkey trial 水印")).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("# Dimkey trial 水印\n"), "实际内容: {}", content);
        assert!(content.contains("第一行"));
        assert!(content.contains("第二行"));
    }

    #[test]
    fn test_export_txt_gbk() {
        let paragraphs = vec![
            Paragraph { index: 0, text: "你好".to_string(), style: "normal".to_string(), table_position: None, pdf_position: None },
        ];
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.txt");

        export_txt(&paragraphs, path.to_str().unwrap(), Some("gbk"), None).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let (decoded, _, _) = encoding_rs::GBK.decode(&bytes);
        assert_eq!(decoded, "你好");
    }
}
