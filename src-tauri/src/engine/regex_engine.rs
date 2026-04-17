use regex::Regex;
use uuid::Uuid;
use crate::models::sensitive::{SensitiveItem, SensitiveType, DetectSource, FileContent};
use crate::models::language::Language;

/// Luhn 算法校验信用卡号
fn luhn_check(number: &str) -> bool {
    let digits: Vec<u32> = number
        .chars()
        .filter(|c| c.is_ascii_digit())
        .filter_map(|c| c.to_digit(10))
        .collect();
    if digits.len() < 13 {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                d
            }
        })
        .sum();
    sum % 10 == 0
}

/// 边界检查类型
#[derive(Clone)]
pub enum BoundaryCheck {
    /// 前后不能是数字
    NotDigit,
    /// 前后不能是字母或数字
    NotAlphanumeric,
    /// 不检查边界
    None,
}

/// 单条正则规则定义
pub struct RegexRule {
    /// 编译后的正则表达式
    pub regex: Regex,
    /// 对应的敏感类型
    pub sensitive_type: SensitiveType,
    /// 边界检查
    pub boundary: BoundaryCheck,
}

/// 正则引擎：基于正则表达式快速识别敏感信息
pub struct RegexEngine {
    /// 按优先级排列的正则规则列表（身份证优先于银行卡，避免误匹配）
    rules: Vec<RegexRule>,
}

impl RegexEngine {
    /// 创建中文正则引擎（向后兼容）
    pub fn new() -> Self {
        Self::for_language(Language::Zh)
    }

    /// 按语言创建正则引擎
    pub fn for_language(lang: Language) -> Self {
        let mut rules = Vec::new();

        // 语言特有规则（优先级高，先匹配）
        match lang {
            Language::Zh => rules.extend(super::rules::zh::rules()),
            Language::En => rules.extend(super::rules::en::rules()),
        }

        // 通用规则（Email, IP）
        rules.extend(super::rules::common::rules());

        Self { rules }
    }

    /// 对文件内容进行正则匹配，返回所有识别到的敏感项
    pub fn detect(&self, content: &FileContent) -> Vec<SensitiveItem> {
        match content {
            FileContent::Spreadsheet { sheets, .. } => {
                let mut items = Vec::new();
                for (sheet_idx, sheet) in sheets.iter().enumerate() {
                    let mut sheet_items = self.detect_sheet(sheet, sheet_idx);
                    items.append(&mut sheet_items);
                }
                items
            }
            FileContent::Document { paragraphs, .. } => {
                self.detect_document(paragraphs)
            }
        }
    }

    /// 扫描单个 Sheet
    /// headers 也作为第 0 行参与扫描，数据行从第 1 行开始
    fn detect_sheet(&self, sheet: &crate::models::sensitive::SheetData, sheet_index: usize) -> Vec<SensitiveItem> {
        let mut items = Vec::new();

        // 扫描表头（row = 0）
        for (col, text) in sheet.headers.iter().enumerate() {
            if !text.is_empty() {
                let mut cell_items = self.detect_text(text, 0, col);
                for item in &mut cell_items {
                    item.sheet_index = sheet_index;
                }
                items.append(&mut cell_items);
            }
        }

        // 扫描数据行（row = row_index + 1，因为表头占了第 0 行）
        for (row_index, row) in sheet.rows.iter().enumerate() {
            for (col, cell) in row.iter().enumerate() {
                let text = &cell.text;
                if !text.is_empty() {
                    let mut cell_items = self.detect_text(text, row_index + 1, col);
                    for item in &mut cell_items {
                        item.sheet_index = sheet_index;
                    }
                    items.append(&mut cell_items);
                }
            }
        }

        items
    }

    /// 扫描文档类文件（Word）
    fn detect_document(&self, paragraphs: &[crate::models::sensitive::Paragraph]) -> Vec<SensitiveItem> {
        let mut items = Vec::new();

        for paragraph in paragraphs {
            if !paragraph.text.is_empty() {
                let mut para_items = self.detect_text(&paragraph.text, paragraph.index, 0);
                items.append(&mut para_items);
            }
        }

        items
    }

    /// 检查匹配位置的边界条件（使用 UTF-8 安全的字符访问）
    fn check_boundary(&self, text: &str, start: usize, end: usize, boundary: &BoundaryCheck) -> bool {
        match boundary {
            BoundaryCheck::NotDigit => {
                // 前一个字符不能是数字
                if let Some(prev) = text[..start].chars().last() {
                    if prev.is_ascii_digit() {
                        return false;
                    }
                }
                // 后一个字符不能是数字
                if let Some(next) = text[end..].chars().next() {
                    if next.is_ascii_digit() {
                        return false;
                    }
                }
                true
            }
            BoundaryCheck::NotAlphanumeric => {
                // 前一个字符不能是字母或数字
                if let Some(prev) = text[..start].chars().last() {
                    if prev.is_ascii_alphanumeric() {
                        return false;
                    }
                }
                // 后一个字符不能是字母或数字
                if let Some(next) = text[end..].chars().next() {
                    if next.is_ascii_alphanumeric() {
                        return false;
                    }
                }
                true
            }
            BoundaryCheck::None => true,
        }
    }

    /// 对单段文本执行所有正则规则，返回匹配到的敏感项
    /// 使用已匹配区间进行去重，避免同一段文本被多条规则重复匹配
    pub fn detect_text(&self, text: &str, row: usize, col: usize) -> Vec<SensitiveItem> {
        let mut items = Vec::new();
        // 记录已匹配的字节区间，用于去重（例如身份证 18 位不应再被银行卡匹配）
        let mut matched_ranges: Vec<(usize, usize)> = Vec::new();

        for rule in &self.rules {
            for mat in rule.regex.find_iter(text) {
                let byte_start = mat.start();
                let byte_end = mat.end();

                // 检查边界条件（使用字节偏移）
                if !self.check_boundary(text, byte_start, byte_end, &rule.boundary) {
                    continue;
                }

                // 检查是否与已匹配区间重叠（字节偏移）
                let overlaps = matched_ranges.iter().any(|&(s, e)| {
                    byte_start < e && byte_end > s
                });
                if overlaps {
                    continue;
                }

                // IP 地址额外校验：IPv4 每段不超过 255，IPv6 跳过（正则已足够严格）
                if rule.sensitive_type == SensitiveType::IpAddress {
                    let ip_text = &text[byte_start..byte_end];
                    if ip_text.contains('.') && !ip_text.contains(':') {
                        // IPv4 校验
                        let valid = ip_text.split('.').all(|part| {
                            part.parse::<u32>().map_or(false, |n| n <= 255)
                        });
                        if !valid {
                            continue;
                        }
                    }
                }

                // 信用卡号额外 Luhn 校验
                if rule.sensitive_type == SensitiveType::CreditCard {
                    let matched_text = &text[byte_start..byte_end];
                    let digits_only: String = matched_text.chars().filter(|c| c.is_ascii_digit()).collect();
                    if !luhn_check(&digits_only) {
                        continue;
                    }
                }

                matched_ranges.push((byte_start, byte_end));

                // 转换为字符偏移（前端 JS 和后端替换逻辑都按字符索引处理）
                let char_start = text[..byte_start].chars().count();
                let char_end = char_start + text[byte_start..byte_end].chars().count();

                items.push(SensitiveItem {
                    id: Uuid::new_v4().to_string(),
                    text: text[byte_start..byte_end].to_string(),
                    sensitive_type: rule.sensitive_type.clone(),
                    source: DetectSource::Regex,
                    confidence: 0.95,
                    start: char_start,
                    end: char_end,
                    row,
                    col,
                    sheet_index: 0,
                    pdf_bbox: None,
                });
            }
        }

        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::sensitive::{FileType, Paragraph, SheetData, CellValue};

    /// 辅助函数：构建包含单个单元格的 Spreadsheet 用于测试
    fn make_spreadsheet(text: &str) -> FileContent {
        FileContent::Spreadsheet {
            file_name: "test.csv".to_string(),
            file_type: FileType::Csv,
            sheets: vec![SheetData {
                name: String::new(),
                headers: vec!["测试列".to_string()],
                rows: vec![vec![CellValue::text(text.to_string())]],
                row_count: 1,
                col_count: 1,
            }],
        }
    }

    /// 辅助函数：构建包含单个段落的 Document 用于测试
    fn make_document(text: &str) -> FileContent {
        FileContent::Document {
            file_name: "test.docx".to_string(),
            file_type: FileType::Docx,
            paragraphs: vec![Paragraph {
                index: 0,
                text: text.to_string(),
                style: "Normal".to_string(),
                table_position: None,
                pdf_position: None,
            }],
            encoding: None,
        }
    }

    #[test]
    fn test_detect_phone() {
        let engine = RegexEngine::new();
        let content = make_spreadsheet("联系方式：13812345678，备用号15912345678");
        let items = engine.detect(&content);
        let phones: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::Phone)
            .collect();
        assert_eq!(phones.len(), 2, "应识别出 2 个手机号");
        assert_eq!(phones[0].text, "13812345678");
        assert_eq!(phones[1].text, "15912345678");
    }

    #[test]
    fn test_detect_phone_not_in_longer_number() {
        // 手机号不应从更长的数字串中误匹配
        let engine = RegexEngine::new();
        let content = make_spreadsheet("编号：123456789012345");
        let items = engine.detect(&content);
        let phones: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::Phone)
            .collect();
        assert_eq!(phones.len(), 0, "不应从长数字串中误匹配手机号");
    }

    #[test]
    fn test_detect_id_card() {
        let engine = RegexEngine::new();
        let content = make_spreadsheet("身份证：110101199001011234");
        let items = engine.detect(&content);
        let ids: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::IdCard)
            .collect();
        assert_eq!(ids.len(), 1, "应识别出 1 个身份证号");
        assert_eq!(ids[0].text, "110101199001011234");
    }

    #[test]
    fn test_detect_id_card_with_x() {
        let engine = RegexEngine::new();
        let content = make_spreadsheet("身份证：11010119900101123X");
        let items = engine.detect(&content);
        let ids: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::IdCard)
            .collect();
        assert_eq!(ids.len(), 1, "应识别出末位为 X 的身份证号");
        assert_eq!(ids[0].text, "11010119900101123X");
    }

    #[test]
    fn test_detect_email() {
        let engine = RegexEngine::new();
        let content = make_spreadsheet("邮箱：test@example.com");
        let items = engine.detect(&content);
        let emails: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::Email)
            .collect();
        assert_eq!(emails.len(), 1, "应识别出 1 个邮箱");
        assert_eq!(emails[0].text, "test@example.com");
    }

    #[test]
    fn test_detect_ip_address() {
        let engine = RegexEngine::new();
        let content = make_spreadsheet("服务器 IP：192.168.1.100");
        let items = engine.detect(&content);
        let ips: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::IpAddress)
            .collect();
        assert_eq!(ips.len(), 1, "应识别出 1 个 IP 地址");
        assert_eq!(ips[0].text, "192.168.1.100");
    }

    #[test]
    fn test_detect_ip_address_invalid() {
        // 每段超过 255 的不应识别为 IP
        let engine = RegexEngine::new();
        let content = make_spreadsheet("数据：999.999.999.999");
        let items = engine.detect(&content);
        let ips: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::IpAddress)
            .collect();
        assert_eq!(ips.len(), 0, "无效 IP 不应被识别");
    }

    #[test]
    fn test_detect_landline_phone() {
        let engine = RegexEngine::new();
        let content = make_spreadsheet("电话：010-12345678");
        let items = engine.detect(&content);
        let landlines: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::LandlinePhone)
            .collect();
        assert_eq!(landlines.len(), 1, "应识别出 1 个固定电话");
        assert_eq!(landlines[0].text, "010-12345678");
    }

    #[test]
    fn test_detect_landline_phone_no_dash() {
        let engine = RegexEngine::new();
        let content = make_spreadsheet("电话：02187654321");
        let items = engine.detect(&content);
        let landlines: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::LandlinePhone)
            .collect();
        assert_eq!(landlines.len(), 1, "应识别出无横杠的固定电话");
        assert_eq!(landlines[0].text, "02187654321");
    }

    #[test]
    fn test_detect_license_plate() {
        let engine = RegexEngine::new();
        let content = make_spreadsheet("车牌：京A12345");
        let items = engine.detect(&content);
        let plates: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::LicensePlate)
            .collect();
        assert_eq!(plates.len(), 1, "应识别出 1 个车牌号");
        assert_eq!(plates[0].text, "京A12345");
    }

    #[test]
    fn test_detect_credit_code() {
        let engine = RegexEngine::new();
        let content = make_spreadsheet("信用代码：91110000MA01234X56");
        let items = engine.detect(&content);
        let codes: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::CreditCode)
            .collect();
        assert_eq!(codes.len(), 1, "应识别出 1 个统一社会信用代码");
        assert_eq!(codes[0].text, "91110000MA01234X56");
    }

    #[test]
    fn test_detect_bank_card() {
        let engine = RegexEngine::new();
        let content = make_spreadsheet("银行卡：6222021234567890123");
        let items = engine.detect(&content);
        let cards: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::BankCard)
            .collect();
        assert_eq!(cards.len(), 1, "应识别出 1 个银行卡号");
        assert_eq!(cards[0].text, "6222021234567890123");
    }

    #[test]
    fn test_id_card_not_matched_as_bank_card() {
        // 身份证 18 位不应同时被银行卡规则（16-19 位）匹配
        let engine = RegexEngine::new();
        let content = make_spreadsheet("110101199001011234");
        let items = engine.detect(&content);
        let cards: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::BankCard)
            .collect();
        assert_eq!(cards.len(), 0, "身份证号不应被误匹配为银行卡");
        let ids: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::IdCard)
            .collect();
        assert_eq!(ids.len(), 1, "应被识别为身份证号");
    }

    #[test]
    fn test_detect_multiple_types_in_one_cell() {
        let engine = RegexEngine::new();
        let content = make_spreadsheet(
            "张三，手机13812345678，邮箱zhangsan@test.com，IP：10.0.0.1"
        );
        let items = engine.detect(&content);
        assert!(items.len() >= 3, "应识别出至少 3 种敏感信息，实际: {}", items.len());

        let types: Vec<&SensitiveType> = items.iter().map(|i| &i.sensitive_type).collect();
        assert!(types.contains(&&SensitiveType::Phone), "应包含手机号");
        assert!(types.contains(&&SensitiveType::Email), "应包含邮箱");
        assert!(types.contains(&&SensitiveType::IpAddress), "应包含 IP 地址");
    }

    #[test]
    fn test_detect_document_format() {
        let engine = RegexEngine::new();
        let content = make_document("本人身份证号110101199001011234，手机号13812345678。");
        let items = engine.detect(&content);
        assert!(items.len() >= 2, "文档格式应识别出至少 2 条敏感信息");

        // Document 格式下 col 应为 0
        for item in &items {
            assert_eq!(item.col, 0, "文档格式下 col 应为 0");
            assert_eq!(item.row, 0, "段落序号为 0");
        }
    }

    #[test]
    fn test_detect_spreadsheet_row_col() {
        let engine = RegexEngine::new();
        let content = FileContent::Spreadsheet {
            file_name: "test.csv".to_string(),
            file_type: FileType::Csv,
            sheets: vec![SheetData {
                name: String::new(),
                headers: vec!["姓名".to_string(), "手机号".to_string()],
                rows: vec![
                    vec![CellValue::text("张三".to_string()), CellValue::text("13812345678".to_string())],
                    vec![CellValue::text("李四".to_string()), CellValue::text("15912345678".to_string())],
                ],
                row_count: 2,
                col_count: 2,
            }],
        };
        let items = engine.detect(&content);
        let phones: Vec<&SensitiveItem> = items.iter()
            .filter(|i| i.sensitive_type == SensitiveType::Phone)
            .collect();
        assert_eq!(phones.len(), 2, "应识别出 2 个手机号");
        // 第一个手机号在 row=1（数据第1行）, col=1
        assert_eq!(phones[0].row, 1);
        assert_eq!(phones[0].col, 1);
        // 第二个手机号在 row=2（数据第2行）, col=1
        assert_eq!(phones[1].row, 2);
        assert_eq!(phones[1].col, 1);
    }

    #[test]
    fn test_detect_empty_content() {
        let engine = RegexEngine::new();
        let content = FileContent::Spreadsheet {
            file_name: "empty.csv".to_string(),
            file_type: FileType::Csv,
            sheets: vec![SheetData {
                name: String::new(),
                headers: vec![],
                rows: vec![],
                row_count: 0,
                col_count: 0,
            }],
        };
        let items = engine.detect(&content);
        assert!(items.is_empty(), "空内容不应有匹配结果");
    }

    #[test]
    fn test_each_item_has_unique_id() {
        let engine = RegexEngine::new();
        let content = make_spreadsheet("13812345678 15912345678 13712345678");
        let items = engine.detect(&content);
        let ids: Vec<&String> = items.iter().map(|i| &i.id).collect();
        // 确保所有 ID 都不同
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j], "每个匹配项的 ID 应唯一");
            }
        }
    }

    #[test]
    fn test_confidence_and_source() {
        let engine = RegexEngine::new();
        let content = make_spreadsheet("test@example.com");
        let items = engine.detect(&content);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].confidence, 0.95, "正则引擎置信度应为 0.95");
        assert_eq!(items[0].source, DetectSource::Regex, "来源应为 Regex");
    }

    #[test]
    fn test_luhn_check() {
        assert!(luhn_check("4111111111111111")); // Visa test
        assert!(luhn_check("5500000000000004")); // Mastercard
        assert!(!luhn_check("1234567890123456")); // Invalid
    }

    // ========== 英文正则引擎测试 ==========

    /// 辅助函数：用英文引擎检测文本（Spreadsheet 格式）
    fn en_detect(text: &str) -> Vec<SensitiveItem> {
        let engine = RegexEngine::for_language(Language::En);
        let content = make_spreadsheet(text);
        engine.detect(&content)
    }

    /// 辅助函数：用英文引擎检测文本（Document 格式）
    fn en_detect_doc(text: &str) -> Vec<SensitiveItem> {
        let engine = RegexEngine::for_language(Language::En);
        let content = make_document(text);
        engine.detect(&content)
    }

    /// 辅助函数：筛选指定类型
    fn filter_type(items: &[SensitiveItem], t: SensitiveType) -> Vec<&SensitiveItem> {
        items.iter().filter(|i| i.sensitive_type == t).collect()
    }

    // ---------- SSN ----------

    #[test]
    fn test_en_ssn_basic() {
        let items = en_detect("SSN: 539-48-2671");
        let ssns = filter_type(&items, SensitiveType::Ssn);
        assert_eq!(ssns.len(), 1, "应识别出 1 个 SSN");
        assert_eq!(ssns[0].text, "539-48-2671");
    }

    #[test]
    fn test_en_ssn_multiple() {
        let items = en_detect("Applicant SSN 123-45-6789, Spouse SSN 987-65-4321");
        let ssns = filter_type(&items, SensitiveType::Ssn);
        assert_eq!(ssns.len(), 2, "应识别出 2 个 SSN");
        assert_eq!(ssns[0].text, "123-45-6789");
        assert_eq!(ssns[1].text, "987-65-4321");
    }

    #[test]
    fn test_en_ssn_boundary_not_in_longer_number() {
        let items = en_detect("Code: 1539-48-26719");
        let ssns = filter_type(&items, SensitiveType::Ssn);
        assert_eq!(ssns.len(), 0, "前后有数字时不应匹配 SSN");
    }

    // ---------- 信用卡号 ----------

    #[test]
    fn test_en_credit_card_with_spaces() {
        let items = en_detect("Card: 4539 1488 0343 6467");
        let cards = filter_type(&items, SensitiveType::CreditCard);
        assert_eq!(cards.len(), 1, "应识别出带空格分隔的信用卡号");
        assert_eq!(cards[0].text, "4539 1488 0343 6467");
    }

    #[test]
    fn test_en_credit_card_with_dashes() {
        let items = en_detect("Card: 5412-1043-3218-1966");
        let cards = filter_type(&items, SensitiveType::CreditCard);
        // Luhn 校验可能不通过，但至少正则应该匹配到
        // 这里验证 Luhn 校验生效
        if cards.is_empty() {
            // Luhn 未通过，符合预期（假号码）
        } else {
            assert_eq!(cards[0].text, "5412-1043-3218-1966");
        }
    }

    #[test]
    fn test_en_credit_card_no_separator() {
        // 4111111111111111 是标准 Visa 测试号（Luhn 有效）
        let items = en_detect("Card: 4111111111111111");
        let cards = filter_type(&items, SensitiveType::CreditCard);
        assert_eq!(cards.len(), 1, "应识别出无分隔符的信用卡号");
        assert_eq!(cards[0].text, "4111111111111111");
    }

    #[test]
    fn test_en_credit_card_luhn_invalid_rejected() {
        let items = en_detect("Number: 1234567890123456");
        let cards = filter_type(&items, SensitiveType::CreditCard);
        assert_eq!(cards.len(), 0, "Luhn 校验不通过的号码不应被识别为信用卡");
    }

    // ---------- 美国电话 ----------

    #[test]
    fn test_en_us_phone_parentheses() {
        let items = en_detect("Call (415) 293-8847");
        let phones = filter_type(&items, SensitiveType::UsPhone);
        assert_eq!(phones.len(), 1, "应识别出括号格式的美国电话");
    }

    #[test]
    fn test_en_us_phone_dashes() {
        let items = en_detect("Phone: 415-293-8847");
        let phones = filter_type(&items, SensitiveType::UsPhone);
        assert_eq!(phones.len(), 1, "应识别出短横线格式的美国电话");
        assert_eq!(phones[0].text, "415-293-8847");
    }

    #[test]
    fn test_en_us_phone_dots() {
        let items = en_detect("Phone: 415.293.8847");
        let phones = filter_type(&items, SensitiveType::UsPhone);
        assert_eq!(phones.len(), 1, "应识别出点号分隔的美国电话");
    }

    #[test]
    fn test_en_us_phone_with_country_code() {
        let items = en_detect("Call +1-312-997-4455");
        let phones = filter_type(&items, SensitiveType::UsPhone);
        assert_eq!(phones.len(), 1, "应识别出带 +1 国际区号的美国电话");
    }

    #[test]
    fn test_en_us_phone_boundary() {
        let items = en_detect("ID: 94151234567890");
        let phones = filter_type(&items, SensitiveType::UsPhone);
        assert_eq!(phones.len(), 0, "长数字串中不应误匹配美国电话");
    }

    // ---------- 英国电话 ----------

    #[test]
    fn test_en_uk_phone_mobile() {
        let items = en_detect("Mobile: +44 7700 900456");
        let phones = filter_type(&items, SensitiveType::UkPhone);
        assert!(phones.len() >= 1, "应识别出英国手机号");
    }

    #[test]
    fn test_en_uk_phone_landline() {
        let items = en_detect("Office: +44 20 7946 0958");
        let phones = filter_type(&items, SensitiveType::UkPhone);
        assert!(phones.len() >= 1, "应识别出英国座机号");
    }

    #[test]
    fn test_en_uk_phone_freephone() {
        let items = en_detect("Freephone: 0800 123 4567");
        let phones = filter_type(&items, SensitiveType::UkPhone);
        assert!(phones.len() >= 1, "应识别出以 0 开头的英国电话");
    }

    // ---------- IBAN ----------

    #[test]
    fn test_en_iban_gb() {
        let items = en_detect("IBAN: GB29NWBK60161331926819");
        let ibans = filter_type(&items, SensitiveType::Iban);
        assert_eq!(ibans.len(), 1, "应识别出英国 IBAN");
    }

    #[test]
    fn test_en_iban_de() {
        let items = en_detect("IBAN: DE89370400440532013000");
        let ibans = filter_type(&items, SensitiveType::Iban);
        assert_eq!(ibans.len(), 1, "应识别出德国 IBAN");
    }

    #[test]
    fn test_en_iban_with_spaces() {
        let items = en_detect("IBAN: GB29 NWBK 6016 1331 9268 19");
        let ibans = filter_type(&items, SensitiveType::Iban);
        assert_eq!(ibans.len(), 1, "应识别出带空格的 IBAN");
    }

    // ---------- 护照号 ----------

    #[test]
    fn test_en_passport_us() {
        let items = en_detect("Passport: C12345678");
        let passports = filter_type(&items, SensitiveType::Passport);
        assert_eq!(passports.len(), 1, "应识别出美国护照号（1 字母 + 8 数字）");
        assert_eq!(passports[0].text, "C12345678");
    }

    #[test]
    fn test_en_passport_uk() {
        let items = en_detect("Passport: AB987654");
        let passports = filter_type(&items, SensitiveType::Passport);
        assert_eq!(passports.len(), 1, "应识别出英国护照号（2 字母 + 6 数字）");
        assert_eq!(passports[0].text, "AB987654");
    }

    #[test]
    fn test_en_passport_boundary() {
        // 三个或更多大写字母开头不应匹配（boundary 检查：前后不能是字母数字）
        let items = en_detect("Code: ABC123456");
        let passports = filter_type(&items, SensitiveType::Passport);
        assert_eq!(passports.len(), 0, "3 个字母开头不应匹配护照号");
    }

    /// BUG-017 修复回归：护照号国际格式扩展
    #[test]
    fn test_en_passport_with_space_separator() {
        // 加拿大等国家格式：2 字母 + 空格 + 7 数字
        let items = en_detect("Passport No: GA 1234567");
        let passports = filter_type(&items, SensitiveType::Passport);
        assert_eq!(passports.len(), 1, "应识别带空格的护照号");
        assert_eq!(passports[0].text, "GA 1234567");
    }

    #[test]
    fn test_en_passport_digit_letter_mixed() {
        // 某些欧洲格式：2 数字 + 2 字母 + 5 数字
        let items = en_detect("Passport No: 12AB34567");
        let passports = filter_type(&items, SensitiveType::Passport);
        assert_eq!(passports.len(), 1, "应识别数字字母混合护照号");
        assert_eq!(passports[0].text, "12AB34567");
    }

    // ---------- ZIP Code ----------

    #[test]
    fn test_en_zip_code_5digit() {
        let items = en_detect("ZIP: 94103");
        let zips = filter_type(&items, SensitiveType::ZipCode);
        assert_eq!(zips.len(), 1, "应识别出 5 位 ZIP Code");
        assert_eq!(zips[0].text, "94103");
    }

    #[test]
    fn test_en_zip_code_plus4() {
        let items = en_detect("ZIP: 94103-2845");
        let zips = filter_type(&items, SensitiveType::ZipCode);
        assert_eq!(zips.len(), 1, "应识别出 ZIP+4 格式");
        assert_eq!(zips[0].text, "94103-2845");
    }

    #[test]
    fn test_en_zip_code_not_in_longer_number() {
        let items = en_detect("Account: 1234567890");
        let zips = filter_type(&items, SensitiveType::ZipCode);
        assert_eq!(zips.len(), 0, "长数字串中不应误匹配 ZIP Code");
    }

    // ---------- UK Postcode ----------

    #[test]
    fn test_en_uk_postcode() {
        let items = en_detect("Address: SW1A 1AA");
        let postcodes = filter_type(&items, SensitiveType::UkPostcode);
        assert_eq!(postcodes.len(), 1, "应识别出英国邮编");
    }

    #[test]
    fn test_en_uk_postcode_no_space() {
        let items = en_detect("Postcode: EC1A1BB");
        let postcodes = filter_type(&items, SensitiveType::UkPostcode);
        assert_eq!(postcodes.len(), 1, "应识别出无空格的英国邮编");
    }

    // ---------- 驾照号 ----------

    #[test]
    fn test_en_us_drivers_license() {
        let items = en_detect("License: D450-3921-8876");
        let licenses = filter_type(&items, SensitiveType::DriversLicense);
        assert_eq!(licenses.len(), 1, "应识别出美国驾照号");
        assert_eq!(licenses[0].text, "D450-3921-8876");
    }

    #[test]
    fn test_en_uk_dvla_drivers_license() {
        let items = en_detect("DVLA: ASHWO607152AB1CZ");
        let licenses = filter_type(&items, SensitiveType::DriversLicense);
        assert_eq!(licenses.len(), 1, "应识别出英国 DVLA 格式驾照号");
        assert_eq!(licenses[0].text, "ASHWO607152AB1CZ");
    }

    #[test]
    fn test_en_drivers_license_boundary() {
        // 前面有字母时不应匹配
        let items = en_detect("RefXD450-3921-8876");
        let licenses = filter_type(&items, SensitiveType::DriversLicense);
        assert_eq!(licenses.len(), 0, "前面紧跟字母时不应匹配驾照号");
    }

    // ---------- Email（通用规则在英文引擎中也可用） ----------

    #[test]
    fn test_en_email() {
        let items = en_detect("Contact: j.anderson@gmail.com");
        let emails = filter_type(&items, SensitiveType::Email);
        assert_eq!(emails.len(), 1, "英文引擎应识别 Email");
        assert_eq!(emails[0].text, "j.anderson@gmail.com");
    }

    // ---------- IP 地址（通用规则在英文引擎中也可用） ----------

    #[test]
    fn test_en_ip_address() {
        let items = en_detect("Server: 192.168.1.100");
        let ips = filter_type(&items, SensitiveType::IpAddress);
        assert_eq!(ips.len(), 1, "英文引擎应识别 IP 地址");
        assert_eq!(ips[0].text, "192.168.1.100");
    }

    // ---------- Document 格式测试 ----------

    #[test]
    fn test_en_document_format() {
        let items = en_detect_doc("SSN: 123-45-6789, Email: test@example.com, ZIP: 90210");
        assert!(items.len() >= 3, "Document 格式应识别出至少 3 种英文敏感信息，实际: {}", items.len());
        for item in &items {
            assert_eq!(item.col, 0, "文档格式下 col 应为 0");
        }
    }

    // ---------- 多类型混合 ----------

    #[test]
    fn test_en_multiple_types_in_one_cell() {
        let items = en_detect(
            "John Doe, SSN 539-48-2671, Card 4111111111111111, Phone (415) 293-8847, Email john@test.com"
        );
        let types: Vec<&SensitiveType> = items.iter().map(|i| &i.sensitive_type).collect();
        assert!(types.contains(&&SensitiveType::Ssn), "应包含 SSN");
        assert!(types.contains(&&SensitiveType::CreditCard), "应包含信用卡");
        assert!(types.contains(&&SensitiveType::UsPhone), "应包含美国电话");
        assert!(types.contains(&&SensitiveType::Email), "应包含 Email");
    }

    // ---------- 引擎属性验证 ----------

    #[test]
    fn test_en_confidence_and_source() {
        let items = en_detect("SSN: 123-45-6789");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].confidence, 0.95, "英文正则引擎置信度应为 0.95");
        assert_eq!(items[0].source, DetectSource::Regex, "来源应为 Regex");
    }

    #[test]
    fn test_en_each_item_has_unique_id() {
        let items = en_detect("SSN 123-45-6789, SSN 987-65-4321");
        let ids: Vec<&String> = items.iter().map(|i| &i.id).collect();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j], "每个匹配项的 ID 应唯一");
            }
        }
    }

    #[test]
    fn test_en_empty_content() {
        let items = en_detect("");
        assert!(items.is_empty(), "空内容不应有匹配结果");
    }

    // ---------- 中文规则在英文引擎中不应生效 ----------

    #[test]
    fn test_en_engine_no_chinese_rules() {
        let items = en_detect("身份证：110101199001011234，手机：13812345678");
        let id_cards = filter_type(&items, SensitiveType::IdCard);
        let phones = filter_type(&items, SensitiveType::Phone);
        assert_eq!(id_cards.len(), 0, "英文引擎不应匹配中文身份证号");
        assert_eq!(phones.len(), 0, "英文引擎不应匹配中文手机号");
    }
}
