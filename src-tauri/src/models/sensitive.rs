use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::task::MappingEntry;
use super::strategy::Strategy;

/// 单元格类型枚举（保留 Excel 原始类型，用于导出时还原）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CellType {
    Text,
    Integer,
    Float,
    Boolean,
    /// Excel 日期时间（保留序列号用于导出还原）
    DateTime { serial: f64 },
    DateTimeIso,
    DurationIso,
    Empty,
}

/// 单元格值（文本 + 原始类型）
#[derive(Debug, Clone, Serialize)]
pub struct CellValue {
    pub text: String,
    pub cell_type: CellType,
}

impl CellValue {
    /// 创建文本类型的 CellValue
    pub fn text(s: String) -> Self {
        Self { text: s, cell_type: CellType::Text }
    }
    /// 创建空单元格
    pub fn empty() -> Self {
        Self { text: String::new(), cell_type: CellType::Empty }
    }
}

impl PartialEq for CellValue {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl PartialEq<&str> for CellValue {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

impl PartialEq<String> for CellValue {
    fn eq(&self, other: &String) -> bool {
        self.text == *other
    }
}

impl std::fmt::Display for CellValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}

impl From<&str> for CellValue {
    fn from(s: &str) -> Self {
        CellValue::text(s.to_string())
    }
}

impl From<String> for CellValue {
    fn from(s: String) -> Self {
        CellValue::text(s)
    }
}

/// 自定义反序列化：兼容旧格式（纯字符串）和新格式（对象）
impl<'de> Deserialize<'de> for CellValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct CellValueVisitor;

        impl<'de> Visitor<'de> for CellValueVisitor {
            type Value = CellValue;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a string or a CellValue object")
            }

            // 旧格式：纯字符串 → CellValue::text
            fn visit_str<E: de::Error>(self, v: &str) -> Result<CellValue, E> {
                Ok(CellValue::text(v.to_string()))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<CellValue, E> {
                Ok(CellValue::text(v))
            }

            // 新格式：{ "text": "...", "cell_type": "..." }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<CellValue, M::Error> {
                let mut text: Option<String> = None;
                let mut cell_type: Option<CellType> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "text" => text = Some(map.next_value()?),
                        "cell_type" => cell_type = Some(map.next_value()?),
                        _ => { let _ = map.next_value::<serde::de::IgnoredAny>()?; }
                    }
                }

                Ok(CellValue {
                    text: text.unwrap_or_default(),
                    cell_type: cell_type.unwrap_or(CellType::Text),
                })
            }
        }

        deserializer.deserialize_any(CellValueVisitor)
    }
}

/// 敏感信息类型枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SensitiveType {
    // ---- 通用类型 ----
    /// 邮箱
    Email,
    /// IP 地址
    IpAddress,

    // ---- 中文特有 ----
    /// 手机号（中国）
    Phone,
    /// 身份证号（中国）
    IdCard,
    /// 银行卡号（中国）
    BankCard,
    /// 固定电话（中国）
    LandlinePhone,
    /// 车牌号（中国）
    LicensePlate,
    /// 统一社会信用代码（中国）
    CreditCode,

    // ---- 英文特有 ----
    /// 社会安全号码（美国 SSN）
    Ssn,
    /// 信用卡号（国际，Luhn 校验）
    CreditCard,
    /// 美国电话
    UsPhone,
    /// 英国电话
    UkPhone,
    /// 护照号
    Passport,
    /// 国际银行账号（IBAN）
    Iban,
    /// 邮政编码（美国 ZIP）
    ZipCode,
    /// 邮编（英国 Postcode）
    UkPostcode,
    /// 驾照号码
    DriversLicense,

    // ---- NER 实体（中英通用） ----
    /// 人名
    PersonName,
    /// 机构名
    OrgName,
    /// 地址
    Address,
    /// 职位
    Title,

    // ---- 自定义 ----
    /// 自定义词条
    Custom(String),
}

/// 识别来源
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DetectSource {
    /// 正则引擎
    Regex,
    /// NER 模型
    Ner,
    /// 自定义词典
    Dict,
    /// 用户手动标记
    Manual,
}

/// 手动框选涂黑区域（归一化屏幕坐标 0~1）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfBbox {
    pub page_index: usize,
    /// 左边界（0=页面左侧，1=页面右侧）
    pub left: f32,
    /// 上边界（0=页面顶部，1=页面底部）
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// 单条敏感信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitiveItem {
    /// 唯一标识
    pub id: String,
    /// 原始文本
    pub text: String,
    /// 敏感信息类型
    pub sensitive_type: SensitiveType,
    /// 识别来源
    pub source: DetectSource,
    /// 置信度 (0.0 ~ 1.0)
    pub confidence: f64,
    /// 在单元格/段落内的起始偏移
    pub start: usize,
    /// 在单元格/段落内的结束偏移
    pub end: usize,
    /// 所在行号（Excel/CSV 行，Word 段落序号）
    pub row: usize,
    /// 所在列号（Excel/CSV 列，Word 中为 0）
    pub col: usize,
    /// 所在 Sheet 索引（Excel 多 Sheet，Word/CSV 为 0）
    #[serde(default)]
    pub sheet_index: usize,
    /// PDF 手动涂黑区域（归一化屏幕坐标）。
    /// Vec 是因为多行文字选中需要每行一个 bbox（getClientRects 拆分），
    /// 单一矩形框选时长度为 1。
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub pdf_bboxes: Option<Vec<PdfBbox>>,
}

/// 文件类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileType {
    Xlsx,
    Xls,
    Csv,
    Docx,
    Txt,
    Pdf,
}

/// PDF 坐标包围盒
#[derive(Debug, Clone)]
pub struct BBox {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// 单个 PDF text object 的信息
#[derive(Debug, Clone)]
pub struct PdfTextObject {
    /// 文本内容（用于导出时按内容+坐标重新匹配）
    pub text: String,
    /// 包围盒
    pub bbox: BBox,
    /// 该 object 的文本在所属段落中的字符偏移量（Unicode 字符计数，非字节数）
    pub char_offset: usize,
}

/// PDF 文本块的页面坐标信息
#[derive(Debug, Clone)]
pub struct PdfTextPosition {
    /// 所在页码（0-based）
    pub page_index: usize,
    /// 组成该段落的所有 text object
    pub text_objects: Vec<PdfTextObject>,
    /// 段落整体包围盒
    pub bbox: BBox,
}

/// 段落在表格中的位置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TablePosition {
    /// 该表格在文档中的序号（第几个表格，从 0 开始）
    pub table_index: usize,
    /// 行号（从 0 开始）
    pub row: usize,
    /// 列号（从 0 开始）
    pub col: usize,
    /// 所在行的总列数
    pub col_count: usize,
}

/// Word 文档段落
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paragraph {
    /// 段落序号（全局连续，所有 <w:p> 共享一个计数器）
    pub index: usize,
    /// 段落文本
    pub text: String,
    /// 段落样式
    pub style: String,
    /// 表格位置信息（None 表示普通段落）
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub table_position: Option<TablePosition>,
    /// PDF 坐标信息（仅 Rust 侧使用，不发送到前端）
    #[serde(skip)]
    #[serde(default)]
    pub pdf_position: Option<PdfTextPosition>,
}

/// 单个 Sheet 的数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetData {
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<CellValue>>,
    pub row_count: usize,
    pub col_count: usize,
}

/// 解析后的文件内容（区分表格类和文档类）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FileContent {
    /// 表格类文件（Excel / CSV）
    Spreadsheet {
        file_name: String,
        file_type: FileType,
        sheets: Vec<SheetData>,
    },
    /// 文档类文件（Word / TXT）
    Document {
        file_name: String,
        file_type: FileType,
        paragraphs: Vec<Paragraph>,
        /// 原始文件编码（TXT 用于导出时保持编码，Word 为 None）
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(default)]
        encoding: Option<String>,
    },
}

/// 脱敏结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesensitizeResult {
    /// 脱敏后的文件内容
    pub content: FileContent,
    /// 脱敏映射记录
    pub mappings: Vec<MappingEntry>,
    /// 脱敏统计摘要
    pub summary: DesensitizeSummary,
}

/// 脱敏统计摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesensitizeSummary {
    /// 脱敏总数
    pub total: usize,
    /// 按类型统计（key 为类型名字符串，如 "Phone"、"IdCard"）
    pub by_type: HashMap<String, usize>,
}

/// 单条还原位置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreItem {
    pub row: usize,
    pub col: usize,
    pub start: usize,
    pub end: usize,
    /// 该侧显示的文本
    pub text: String,
    /// 对侧显示的文本
    pub replaced_text: String,
    pub sensitive_type: SensitiveType,
    /// 所在 Sheet 索引
    #[serde(default)]
    pub sheet_index: usize,
}

/// 还原结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    /// 原始内容（脱敏后文件）
    pub original_content: FileContent,
    /// 还原后内容
    pub restored_content: FileContent,
    /// 匹配到的还原数
    pub matched_count: usize,
    /// 还原后内容的高亮位置（右侧）
    pub restore_items: Vec<RestoreItem>,
    /// 脱敏后内容的高亮位置（左侧）
    pub original_items: Vec<RestoreItem>,
    /// 用户选择的脱敏后文件路径（用于 Word 导出模板）
    pub file_path: String,
}

/// 列类型推断结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInference {
    /// 列索引
    pub col: usize,
    /// 列头名称
    pub header: String,
    /// 推断的敏感类型（None 表示未识别）
    pub inferred_type: Option<SensitiveType>,
    /// 推断置信度
    pub confidence: f64,
    /// 采样命中数
    pub sample_hits: usize,
    /// 采样总数
    pub sample_total: usize,
    /// 所在 Sheet 索引
    #[serde(default)]
    pub sheet_index: usize,
}

/// 列级脱敏规则（前端确认后传入）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnRule {
    /// 列索引
    pub col: usize,
    /// 敏感类型
    pub sensitive_type: String,
    /// 脱敏策略
    pub strategy: Strategy,
    /// 是否可还原
    pub reversible: bool,
    /// 所在 Sheet 索引
    #[serde(default)]
    pub sheet_index: usize,
}

/// 码本（可还原列的映射记录）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Codebook {
    /// 版本号
    pub version: u32,
    /// 各列的映射（key 为列头名称）
    pub columns: HashMap<String, CodebookColumn>,
}

/// 码本中单列的映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebookColumn {
    /// 敏感类型
    pub sensitive_type: String,
    /// 脱敏策略
    pub strategy: String,
    /// 原文 → 脱敏后 映射
    pub mappings: HashMap<String, String>,
}
