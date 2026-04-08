mod common;

use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::txt::parse_txt;

use common::*;

/// 测试 TXT 导入后的结构正确性（Document 类型，包含段落）
#[test]
fn test_txt_meeting_import_structure() {
    let path = test_data_path("会议纪要.txt");
    let content = parse_txt(&path).expect("TXT 导入失败");

    if let FileContent::Document { paragraphs, .. } = &content {
        assert!(
            !paragraphs.is_empty(),
            "应解析出至少一个段落"
        );
    } else {
        panic!("期望 Document 类型");
    }
}
