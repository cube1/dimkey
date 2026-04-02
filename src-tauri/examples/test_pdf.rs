use dimkey_lib::parser::pdf::{load_pdfium, parse_pdf_with_pdfium};

fn main() {
    let pdf_path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("用法: cargo run --example test_pdf <pdf_path>");
        std::process::exit(1);
    });

    let pdfium = match load_pdfium() {
        Ok(p) => { println!("PDFium 加载成功"); p }
        Err(e) => { eprintln!("PDFium 加载失败: {}", e); return; }
    };

    match parse_pdf_with_pdfium(&pdfium, &pdf_path) {
        Ok(content) => {
            if let dimkey_lib::models::sensitive::FileContent::Document { paragraphs, .. } = &content {
                println!("共 {} 个段落\n", paragraphs.len());
                for (i, p) in paragraphs.iter().take(30).enumerate() {
                    let display: String = p.text.chars().take(60).collect();
                    let pos = p.pdf_position.as_ref().map(|pos| format!("p{} objs={}", pos.page_index, pos.text_objects.len())).unwrap_or("?".into());
                    println!("[{:3}] ({}) {}", i, pos, display);
                }
            }
        }
        Err(e) => eprintln!("解析失败: {}", e),
    }
}
