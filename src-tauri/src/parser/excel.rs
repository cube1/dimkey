use std::path::Path;
use std::io::{Read, Seek, Cursor};
use calamine::{open_workbook_auto, open_workbook_auto_from_rs, Reader, Sheets, Data};
use crate::models::sensitive::{FileContent, FileType, SheetData, CellValue, CellType};

/// 从已打开的 workbook 解析为 FileContent（公共逻辑）
fn parse_workbook<RS: Read + Seek>(
    mut workbook: Sheets<RS>,
    file_name: String,
    file_type: FileType,
) -> Result<FileContent, String> {
    let sheet_names = workbook.sheet_names().to_vec();
    if sheet_names.is_empty() {
        return Err("Excel 文件中没有工作表".to_string());
    }

    let mut sheets = Vec::new();

    for sheet_name in &sheet_names {
        let range = workbook
            .worksheet_range(sheet_name)
            .map_err(|e| format!("无法读取工作表 '{}'：{}", sheet_name, e))?;

        let mut all_rows: Vec<Vec<CellValue>> = Vec::new();

        for row in range.rows() {
            let value_row: Vec<CellValue> = row
                .iter()
                .map(|cell| cell_to_value(cell))
                .collect();
            all_rows.push(value_row);
        }

        if all_rows.is_empty() {
            sheets.push(SheetData {
                name: sheet_name.clone(),
                headers: vec![],
                rows: vec![],
                row_count: 0,
                col_count: 0,
            });
            continue;
        }

        // 第一行作为表头（提取文本），其余作为数据行
        let headers: Vec<String> = all_rows.remove(0).into_iter().map(|cv| cv.text).collect();
        let col_count = headers.len();
        let row_count = all_rows.len();

        sheets.push(SheetData {
            name: sheet_name.clone(),
            headers,
            rows: all_rows,
            row_count,
            col_count,
        });
    }

    Ok(FileContent::Spreadsheet {
        file_name,
        file_type,
        sheets,
    })
}

/// 解析 Excel 文件（xlsx/xls）为 FileContent
pub fn parse_excel(path: &str) -> Result<FileContent, String> {
    let file_path = Path::new(path);
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // 根据扩展名确定文件类型
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .unwrap_or_default();
    let file_type = match extension.as_str() {
        "xls" => FileType::Xls,
        _ => FileType::Xlsx,
    };

    // 使用 calamine 自动识别并打开工作簿
    let workbook = open_workbook_auto(path)
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("password") || msg.contains("Password") || msg.contains("encrypted") {
                format!("ENCRYPTED:{}", extension)
            } else if msg.contains("OLE") || msg.contains("Cfb") || msg.contains("CFB") {
                "该文件可能不是有效的 Excel 文件，或文件格式与扩展名不匹配。如果是 .xls 文件，请尝试用 Excel 另存为 .xlsx 格式后重试".to_string()
            } else {
                format!("无法打开 Excel 文件：{}", e)
            }
        })?;

    parse_workbook(workbook, file_name, file_type)
}

/// 从内存字节解析 Excel 文件（用于解密后的数据）
pub fn parse_excel_from_bytes(bytes: Vec<u8>, file_name: String, file_type: FileType) -> Result<FileContent, String> {
    let workbook = open_workbook_auto_from_rs(Cursor::new(bytes))
        .map_err(|e| format!("解密后文件解析失败：{}", e))?;
    parse_workbook(workbook, file_name, file_type)
}

/// 将 calamine 单元格数据转换为 CellValue（保留原始类型）
fn cell_to_value(cell: &Data) -> CellValue {
    match cell {
        Data::Empty => CellValue::empty(),
        Data::String(s) => CellValue::text(s.clone()),
        Data::Int(i) => CellValue {
            text: i.to_string(),
            cell_type: CellType::Integer,
        },
        Data::Float(f) => {
            let text = if f.fract() == 0.0 && f.abs() < i64::MAX as f64 {
                (*f as i64).to_string()
            } else {
                f.to_string()
            };
            CellValue {
                text,
                cell_type: CellType::Float,
            }
        }
        Data::Bool(b) => CellValue {
            text: b.to_string(),
            cell_type: CellType::Boolean,
        },
        Data::Error(e) => CellValue::text(format!("#ERROR: {:?}", e)),
        Data::DateTime(dt) => CellValue {
            text: dt.to_string(),
            cell_type: CellType::DateTime { serial: dt.as_f64() },
        },
        Data::DateTimeIso(s) => CellValue {
            text: s.clone(),
            cell_type: CellType::DateTimeIso,
        },
        Data::DurationIso(s) => CellValue {
            text: s.clone(),
            cell_type: CellType::DurationIso,
        },
    }
}

/// 解析 CSV/TSV 文件为 FileContent
pub fn parse_csv(path: &str) -> Result<FileContent, String> {
    let file_path = Path::new(path);
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // 判断分隔符：TSV 用 tab，CSV 用逗号
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .unwrap_or_default();
    let delimiter = if extension == "tsv" { b'\t' } else { b',' };

    // 构建 CSV reader
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(true)
        .flexible(true)  // 允许行列数不一致
        .from_path(path)
        .map_err(|e| format!("无法打开 CSV 文件：{}", e))?;

    // 读取表头
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| format!("无法读取 CSV 表头：{}", e))?
        .iter()
        .map(|s| s.to_string())
        .collect();

    let col_count = headers.len();

    // 读取数据行（CSV 无类型概念，全部标记为 Text）
    let mut rows: Vec<Vec<CellValue>> = Vec::new();
    for result in reader.records() {
        let record = result.map_err(|e| format!("读取 CSV 数据行出错：{}", e))?;
        let row: Vec<CellValue> = record.iter().map(|s| CellValue::text(s.to_string())).collect();
        rows.push(row);
    }

    let row_count = rows.len();

    Ok(FileContent::Spreadsheet {
        file_name,
        file_type: FileType::Csv,
        sheets: vec![SheetData {
            name: String::new(),
            headers,
            rows,
            row_count,
            col_count,
        }],
    })
}
