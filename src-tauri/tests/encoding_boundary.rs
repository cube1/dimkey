mod common;

use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::excel::{parse_csv, parse_excel};

use common::*;

// ============================================================
// C33 — GBK 编码 CSV 导入与识别
// ============================================================

/// C33: GBK 编码的 CSV 文件应能正常导入，解析出 5 行数据
/// 注：当前 parse_csv 不支持 GBK 编码（仅 TXT 解析器有编码检测），此测试标记为 ignore
#[test]
fn test_gbk_csv_import() {
    let path = fixture_path("boundary/gbk_sample.csv");
    let content = parse_csv(&path).expect("GBK CSV 导入失败");

    let rows = get_rows(&content);
    assert_eq!(rows.len(), 5, "GBK CSV 应解析出 5 行数据");
}

/// C33: GBK CSV 中应识别出至少 5 个手机号
#[test]
fn test_gbk_csv_detect_phone() {
    let path = fixture_path("boundary/gbk_sample.csv");
    let content = parse_csv(&path).expect("GBK CSV 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 5,
        "GBK CSV 应识别出至少 5 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
}

/// C33: GBK CSV 中应识别出至少 5 个身份证号
#[test]
fn test_gbk_csv_detect_idcard() {
    let path = fixture_path("boundary/gbk_sample.csv");
    let content = parse_csv(&path).expect("GBK CSV 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 5,
        "GBK CSV 应识别出至少 5 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
}

/// C33: GBK CSV 中应识别出至少 5 个邮箱
#[test]
fn test_gbk_csv_detect_email() {
    let path = fixture_path("boundary/gbk_sample.csv");
    let content = parse_csv(&path).expect("GBK CSV 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 5,
        "GBK CSV 应识别出至少 5 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
}

// ============================================================
// C34 — UTF-8 BOM CSV 导入与识别
// ============================================================

/// C34: UTF-8 BOM 编码的 CSV 文件应能正常导入，解析出 5 行数据
#[test]
fn test_utf8bom_csv_import() {
    let path = fixture_path("boundary/utf8bom_sample.csv");
    let content = parse_csv(&path).expect("UTF-8 BOM CSV 导入失败");

    let rows = get_rows(&content);
    assert_eq!(rows.len(), 5, "UTF-8 BOM CSV 应解析出 5 行数据");
}

/// C34: UTF-8 BOM CSV 中应识别出至少 5 个手机号
#[test]
fn test_utf8bom_csv_detect_phone() {
    let path = fixture_path("boundary/utf8bom_sample.csv");
    let content = parse_csv(&path).expect("UTF-8 BOM CSV 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 5,
        "UTF-8 BOM CSV 应识别出至少 5 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
}

/// C34: UTF-8 BOM CSV 中应识别出至少 5 个身份证号
#[test]
fn test_utf8bom_csv_detect_idcard() {
    let path = fixture_path("boundary/utf8bom_sample.csv");
    let content = parse_csv(&path).expect("UTF-8 BOM CSV 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 5,
        "UTF-8 BOM CSV 应识别出至少 5 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
}

// ============================================================
// C35 — 全角数字边界测试
// ============================================================

/// C35: 全角数字 CSV 文件应能正常导入
#[test]
fn test_fullwidth_csv_import() {
    let path = fixture_path("boundary/fullwidth_digits.csv");
    let content = parse_csv(&path).expect("全角数字 CSV 导入失败");

    let rows = get_rows(&content);
    assert!(rows.len() >= 5, "全角数字 CSV 应至少有 5 行数据，实际: {}", rows.len());
}

/// C35: 半角手机号和身份证号应被正常识别，全角为边界情况
/// 第 2、4 行为半角手机号，半角身份证号至少 3 个
#[test]
fn test_fullwidth_halfwidth_detect() {
    let path = fixture_path("boundary/fullwidth_digits.csv");
    let content = parse_csv(&path).expect("全角数字 CSV 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    let phone_count = count_by_type(&items, &SensitiveType::Phone);
    assert!(
        phone_count >= 2,
        "至少应识别出 2 个半角手机号（第 2、4 行），实际: {}",
        phone_count
    );

    let idcard_count = count_by_type(&items, &SensitiveType::IdCard);
    assert!(
        idcard_count >= 3,
        "至少应识别出 3 个半角身份证号，实际: {}",
        idcard_count
    );
}

/// C35: 第 5 行手机号前有全角空格，"13644445555" 仍应被识别
#[test]
fn test_fullwidth_phone_with_leading_space() {
    let path = fixture_path("boundary/fullwidth_digits.csv");
    let content = parse_csv(&path).expect("全角数字 CSV 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    let phone_count = count_by_type(&items, &SensitiveType::Phone);
    assert!(
        phone_count >= 3,
        "含全角空格前缀的手机号 13644445555 也应被识别，Phone 总数应 >= 3，实际: {}",
        phone_count
    );
}

// ============================================================
// C38 — 大单元格（>1000 字符）识别测试
// ============================================================

/// C38: 大单元格 XLSX 文件应能正常导入，解析出 3 行数据
#[test]
fn test_large_cell_import() {
    let path = fixture_path("boundary/large_cell.xlsx");
    let content = parse_excel(&path).expect("大单元格 XLSX 导入失败");

    let rows = get_rows(&content);
    assert_eq!(rows.len(), 3, "大单元格 XLSX 应解析出 3 行数据");
}

/// C38: 大单元格中散布的手机号应全部被识别，至少 5 个
#[test]
fn test_large_cell_detect_phone() {
    let path = fixture_path("boundary/large_cell.xlsx");
    let content = parse_excel(&path).expect("大单元格 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Phone) >= 5,
        "大单元格应识别出至少 5 个手机号，实际: {}",
        count_by_type(&items, &SensitiveType::Phone)
    );
}

/// C38: 大单元格中散布的身份证号应被识别，至少 3 个
#[test]
fn test_large_cell_detect_idcard() {
    let path = fixture_path("boundary/large_cell.xlsx");
    let content = parse_excel(&path).expect("大单元格 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::IdCard) >= 3,
        "大单元格应识别出至少 3 个身份证号，实际: {}",
        count_by_type(&items, &SensitiveType::IdCard)
    );
}

/// C38: 大单元格中散布的邮箱应被识别，至少 3 个
#[test]
fn test_large_cell_detect_email() {
    let path = fixture_path("boundary/large_cell.xlsx");
    let content = parse_excel(&path).expect("大单元格 XLSX 导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(
        count_by_type(&items, &SensitiveType::Email) >= 3,
        "大单元格应识别出至少 3 个邮箱，实际: {}",
        count_by_type(&items, &SensitiveType::Email)
    );
}
