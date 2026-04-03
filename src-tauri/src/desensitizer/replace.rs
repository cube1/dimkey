use crate::models::sensitive::SensitiveType;
use crate::models::strategy::ReplaceStyle;
use rand::Rng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

// 编译时嵌入 JSON 假数据
const PERSON_NAMES_JSON: &str = include_str!("../../resources/fake_data/zh/person_names.json");
const ORG_COMPONENTS_JSON: &str = include_str!("../../resources/fake_data/zh/org_components.json");
const TITLES_JSON: &str = include_str!("../../resources/fake_data/zh/titles.json");
const ADDRESS_COMPONENTS_JSON: &str = include_str!("../../resources/fake_data/zh/address_components.json");
const PATTERNS_JSON: &str = include_str!("../../resources/fake_data/zh/patterns.json");

#[derive(Deserialize)]
struct PersonNames {
    surnames: Vec<String>,
    given_names: Vec<String>,
}

#[derive(Deserialize)]
struct OrgComponents {
    prefixes: Vec<String>,
    industries: Vec<String>,
    suffixes: Vec<String>,
}

#[derive(Deserialize)]
struct AddressComponents {
    city_districts: Vec<String>,
    streets: Vec<String>,
    numbers: Vec<u32>,
}

#[derive(Deserialize)]
struct Patterns {
    phone_prefixes: Vec<String>,
    id_area_codes: Vec<String>,
    bank_card_prefixes: Vec<String>,
    landline_area_codes: Vec<String>,
    plate_provinces: Vec<String>,
    plate_letters: String,
    credit_code_areas: Vec<String>,
    credit_code_charset: String,
    email_names: Vec<String>,
    email_domains: Vec<String>,
}

/// 所有假数据，启动时解析一次
struct FakeData {
    person_names: PersonNames,
    org_components: OrgComponents,
    titles: Vec<String>,
    address_components: AddressComponents,
    patterns: Patterns,
}

static FAKE_DATA: OnceLock<FakeData> = OnceLock::new();

fn get_fake_data() -> &'static FakeData {
    FAKE_DATA.get_or_init(|| FakeData {
        person_names: serde_json::from_str(PERSON_NAMES_JSON).expect("解析 person_names.json 失败"),
        org_components: serde_json::from_str(ORG_COMPONENTS_JSON).expect("解析 org_components.json 失败"),
        titles: serde_json::from_str(TITLES_JSON).expect("解析 titles.json 失败"),
        address_components: serde_json::from_str(ADDRESS_COMPONENTS_JSON).expect("解析 address_components.json 失败"),
        patterns: serde_json::from_str(PATTERNS_JSON).expect("解析 patterns.json 失败"),
    })
}

/// 从切片中随机选取一项
fn pick<'a>(rng: &mut impl Rng, items: &'a [String]) -> &'a str {
    &items[rng.gen_range(0..items.len())]
}

/// 复姓表
const COMPOUND_SURNAMES: &[&str] = &[
    "欧阳", "太史", "端木", "上官", "司马", "东方", "独孤", "南宫",
    "万俟", "闻人", "夏侯", "诸葛", "尉迟", "公羊", "赫连", "澹台",
    "皇甫", "宗政", "濮阳", "公冶", "太叔", "申屠", "公孙", "慕容",
    "仲孙", "钟离", "长孙", "宇文", "司徒", "鲜于", "司空", "闾丘",
    "令狐", "百里", "呼延", "东郭", "南门", "西门", "左丘", "第五",
];

/// 组织后缀关键词表（只保留简短后缀，"有限公司"等长后缀会自动匹配到"公司"）
const ORG_SUFFIXES: &[&str] = &[
    "检察院", "基金会", "法院", "银行", "医院", "学校", "大学",
    "集团", "协会", "中心", "公司", "局", "委", "所", "院", "厂",
];

/// 从人名中提取姓氏（优先匹配复姓）
fn extract_surname(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() >= 2 {
        let two: String = chars[..2].iter().collect();
        if COMPOUND_SURNAMES.contains(&two.as_str()) {
            return two;
        }
    }
    if chars.is_empty() {
        return String::new();
    }
    chars[0].to_string()
}

/// 从组织名中提取后缀关键词（如 "公司"、"法院"）
fn extract_org_suffix(org: &str) -> String {
    for suffix in ORG_SUFFIXES {
        if org.ends_with(suffix) {
            return suffix.to_string();
        }
    }
    "单位".to_string()
}

/// 数字转中文数词（用于某式序号：二、三、...、十一、...）
fn to_chinese_numeral(n: usize) -> String {
    const DIGITS: &[&str] = &["零", "一", "二", "三", "四", "五", "六", "七", "八", "九", "十"];
    if n >= 100 {
        return n.to_string();
    }
    if n <= 10 {
        return DIGITS[n].to_string();
    }
    if n < 20 {
        return format!("十{}", if n % 10 == 0 { "" } else { DIGITS[n % 10] });
    }
    let tens = n / 10;
    let ones = n % 10;
    if ones == 0 {
        format!("{}十", DIGITS[tens])
    } else {
        format!("{}十{}", DIGITS[tens], DIGITS[ones])
    }
}

/// 计算身份证号校验位（ISO 7064:1983 Mod 11-2）
fn calc_id_card_check_digit(id17: &str) -> char {
    const WEIGHTS: [u32; 17] = [7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
    const CHECK_CHARS: [char; 11] = ['1', '0', 'X', '9', '8', '7', '6', '5', '4', '3', '2'];
    let sum: u32 = id17
        .chars()
        .zip(WEIGHTS.iter())
        .map(|(c, &w)| c.to_digit(10).unwrap_or(0) * w)
        .sum();
    CHECK_CHARS[(sum % 11) as usize]
}

/// 替换状态：持有预洗牌序列和计数器，保证生成唯一性
pub struct ReplaceState {
    seed: u64,
    counters: HashMap<String, usize>,
    // 惰性初始化的洗牌索引序列
    name_indices: Option<Vec<u32>>,
    org_indices: Option<Vec<u32>>,
    address_indices: Option<Vec<u32>>,
    title_indices: Option<Vec<u32>>,
}

/// 类型偏移量，确保不同类型使用不同的 RNG 种子
const NAME_SEED_OFFSET: u64 = 0;
const ORG_SEED_OFFSET: u64 = 1;
const ADDRESS_SEED_OFFSET: u64 = 2;
const TITLE_SEED_OFFSET: u64 = 3;

impl ReplaceState {
    /// 从工作区数据构造
    pub fn new(seed: u64, counters: HashMap<String, usize>) -> Self {
        Self {
            seed,
            counters,
            name_indices: None,
            org_indices: None,
            address_indices: None,
            title_indices: None,
        }
    }

    /// 导出当前计数器供持久化
    pub fn export_counters(&self) -> HashMap<String, usize> {
        self.counters.clone()
    }

    /// 初始化洗牌索引序列
    fn init_shuffled_indices(seed: u64, offset: u64, pool_size: u32) -> Vec<u32> {
        let mut rng = StdRng::seed_from_u64(seed.wrapping_add(offset));
        let mut indices: Vec<u32> = (0..pool_size).collect();
        indices.shuffle(&mut rng);
        indices
    }

    /// 取下一个唯一姓名
    pub fn next_name(&mut self) -> String {
        let data = get_fake_data();
        let surname_count = data.person_names.surnames.len() as u32;
        let given_count = data.person_names.given_names.len() as u32;
        let pool_size = surname_count * given_count;

        let indices = self.name_indices.get_or_insert_with(|| {
            Self::init_shuffled_indices(self.seed, NAME_SEED_OFFSET, pool_size)
        });

        let counter = self.counters.entry("PersonName".to_string()).or_insert(0);
        let idx = indices[*counter % indices.len()] as usize;
        let wrap = *counter / indices.len();
        *counter += 1;

        let surname_idx = idx / given_count as usize;
        let given_idx = idx % given_count as usize;

        let name = format!(
            "{}{}",
            data.person_names.surnames[surname_idx],
            data.person_names.given_names[given_idx]
        );

        if wrap > 0 {
            format!("{}{}", name, wrap)
        } else {
            name
        }
    }

    /// 取下一个唯一机构名
    pub fn next_org(&mut self) -> String {
        let data = get_fake_data();
        let prefix_count = data.org_components.prefixes.len() as u32;
        let industry_count = data.org_components.industries.len() as u32;
        let suffix_count = data.org_components.suffixes.len() as u32;
        let pool_size = prefix_count * industry_count * suffix_count;

        let indices = self.org_indices.get_or_insert_with(|| {
            Self::init_shuffled_indices(self.seed, ORG_SEED_OFFSET, pool_size)
        });

        let counter = self.counters.entry("OrgName".to_string()).or_insert(0);
        let idx = indices[*counter % indices.len()] as usize;
        let wrap = *counter / indices.len();
        *counter += 1;

        let suffix_idx = idx % suffix_count as usize;
        let remaining = idx / suffix_count as usize;
        let industry_idx = remaining % industry_count as usize;
        let prefix_idx = remaining / industry_count as usize;

        let org = format!(
            "{}{}{}",
            data.org_components.prefixes[prefix_idx],
            data.org_components.industries[industry_idx],
            data.org_components.suffixes[suffix_idx]
        );

        if wrap > 0 {
            format!("{}{}", org, wrap)
        } else {
            org
        }
    }

    /// 取下一个唯一地址
    pub fn next_address(&mut self) -> String {
        let data = get_fake_data();
        let district_count = data.address_components.city_districts.len() as u32;
        let street_count = data.address_components.streets.len() as u32;
        let number_count = data.address_components.numbers.len() as u32;
        let pool_size = district_count * street_count * number_count;

        let indices = self.address_indices.get_or_insert_with(|| {
            Self::init_shuffled_indices(self.seed, ADDRESS_SEED_OFFSET, pool_size)
        });

        let counter = self.counters.entry("Address".to_string()).or_insert(0);
        let idx = indices[*counter % indices.len()] as usize;
        let wrap = *counter / indices.len();
        *counter += 1;

        let number_idx = idx % number_count as usize;
        let remaining = idx / number_count as usize;
        let street_idx = remaining % street_count as usize;
        let district_idx = remaining / street_count as usize;

        let addr = format!(
            "{}{}{}号",
            data.address_components.city_districts[district_idx],
            data.address_components.streets[street_idx],
            data.address_components.numbers[number_idx]
        );

        if wrap > 0 {
            format!("{}{}", addr, wrap)
        } else {
            addr
        }
    }

    /// 取下一个唯一职位
    pub fn next_title(&mut self) -> String {
        let data = get_fake_data();
        let pool_size = data.titles.len() as u32;

        let indices = self.title_indices.get_or_insert_with(|| {
            Self::init_shuffled_indices(self.seed, TITLE_SEED_OFFSET, pool_size)
        });

        let counter = self.counters.entry("Title".to_string()).or_insert(0);
        let idx = indices[*counter % indices.len()] as usize;
        let wrap = *counter / indices.len();
        *counter += 1;

        let title = data.titles[idx].clone();

        if wrap > 0 {
            format!("{}{}", title, wrap)
        } else {
            title
        }
    }

    /// 某式：人名替换（张某、张某二、李某...）
    pub fn next_mou_name(&mut self, original: &str) -> String {
        if original.is_empty() {
            return "某某".to_string();
        }
        let surname = extract_surname(original);
        let key = format!("mou_surname_{}", surname);
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        if *count == 1 {
            format!("{}某", surname)
        } else {
            format!("{}某{}", surname, to_chinese_numeral(*count))
        }
    }

    /// 某式：组织名替换（某公司、某法院、某公司二...）
    pub fn next_mou_org(&mut self, original: &str) -> String {
        let suffix = extract_org_suffix(original);
        let key = format!("mou_org_{}", suffix);
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        if *count == 1 {
            format!("某{}", suffix)
        } else {
            format!("某{}{}", suffix, to_chinese_numeral(*count))
        }
    }

    /// 某式：地址替换（某地、某地二...）
    pub fn next_mou_address(&mut self, _original: &str) -> String {
        let key = "mou_address".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        if *count == 1 {
            "某地".to_string()
        } else {
            format!("某地{}", to_chinese_numeral(*count))
        }
    }

    /// 某式：职务替换（某职务、某职务二...）
    pub fn next_mou_title(&mut self, _original: &str) -> String {
        let key = "mou_title".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        if *count == 1 {
            "某职务".to_string()
        } else {
            format!("某职务{}", to_chinese_numeral(*count))
        }
    }

    /// 天干序列，用于序号式人名
    const TIANGAN: &'static [&'static str] = &[
        "甲", "乙", "丙", "丁", "戊", "己", "庚", "辛", "壬", "癸",
    ];

    /// 序号式：人名替换（当事人一、当事人二、当事人三...）
    pub fn next_ordinal_name(&mut self) -> String {
        let key = "ordinal_name".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        format!("当事人{}", to_chinese_numeral(*count))
    }

    /// 序号式：组织名替换（甲公司、乙集团...）
    pub fn next_ordinal_org(&mut self, original: &str) -> String {
        let suffix = extract_org_suffix(original);
        let key = "ordinal_org".to_string();
        let count = self.counters.entry(key).or_insert(0);
        let prefix = if *count < Self::TIANGAN.len() {
            Self::TIANGAN[*count].to_string()
        } else {
            format!("{}", *count + 1)
        };
        *count += 1;
        format!("{}{}", prefix, suffix)
    }

    /// 序号式：地址替换（地址一、地址二、地址三...）
    pub fn next_ordinal_address(&mut self) -> String {
        let key = "ordinal_address".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        format!("地址{}", to_chinese_numeral(*count))
    }

    /// 序号式：职务替换（职务一、职务二...）
    pub fn next_ordinal_title(&mut self) -> String {
        let key = "ordinal_title".to_string();
        let count = self.counters.entry(key).or_insert(0);
        *count += 1;
        format!("职务{}", to_chinese_numeral(*count))
    }
}

/// 假数据替换脱敏：用随机生成的假数据替换原文
pub fn apply_replace(
    text: &str,
    sensitive_type: &SensitiveType,
    state: &mut ReplaceState,
    style: &ReplaceStyle,
) -> String {
    let data = get_fake_data();
    let mut rng = rand::thread_rng();

    match sensitive_type {
        SensitiveType::PersonName => match style {
            ReplaceStyle::Fake => state.next_name(),
            ReplaceStyle::Mou => state.next_mou_name(text),
            ReplaceStyle::Ordinal => state.next_ordinal_name(),
        },
        SensitiveType::OrgName => match style {
            ReplaceStyle::Fake => state.next_org(),
            ReplaceStyle::Mou => state.next_mou_org(text),
            ReplaceStyle::Ordinal => state.next_ordinal_org(text),
        },
        SensitiveType::Title => match style {
            ReplaceStyle::Fake => state.next_title(),
            ReplaceStyle::Mou => state.next_mou_title(text),
            ReplaceStyle::Ordinal => state.next_ordinal_title(),
        },
        SensitiveType::Address => match style {
            ReplaceStyle::Fake => state.next_address(),
            ReplaceStyle::Mou => state.next_mou_address(text),
            ReplaceStyle::Ordinal => state.next_ordinal_address(),
        },
        SensitiveType::Phone => {
            let p = &data.patterns;
            let prefix = pick(&mut rng, &p.phone_prefixes);
            let suffix: u32 = rng.gen_range(10000000..99999999);
            format!("{}{}", prefix, suffix)
        }
        SensitiveType::IdCard => {
            let p = &data.patterns;
            let area = pick(&mut rng, &p.id_area_codes);
            let year: u32 = rng.gen_range(1960..2000);
            let month: u32 = rng.gen_range(1..13);
            let day: u32 = rng.gen_range(1..29);
            let seq: u32 = rng.gen_range(100..999);
            let id17 = format!("{}{:04}{:02}{:02}{:03}", area, year, month, day, seq);
            let check = calc_id_card_check_digit(&id17);
            format!("{}{}", id17, check)
        }
        SensitiveType::BankCard => {
            let p = &data.patterns;
            let prefix = pick(&mut rng, &p.bank_card_prefixes);
            // 根据原文长度生成对应长度的假银行卡号
            let orig_len = text.chars().count();
            let target_len = if orig_len >= 16 && orig_len <= 19 { orig_len } else { 16 };
            let remaining = target_len - prefix.len();
            let mut digits = String::with_capacity(remaining);
            for _ in 0..remaining {
                digits.push(char::from(b'0' + rng.gen_range(0..10u8)));
            }
            format!("{}{}", prefix, digits)
        }
        SensitiveType::Email => {
            let p = &data.patterns;
            let name = pick(&mut rng, &p.email_names);
            let num: u32 = rng.gen_range(100..999);
            let domain = pick(&mut rng, &p.email_domains);
            format!("{}{}@{}", name, num, domain)
        }
        SensitiveType::IpAddress => {
            format!(
                "{}.{}.{}.{}",
                rng.gen_range(10..200),
                rng.gen_range(0..256),
                rng.gen_range(0..256),
                rng.gen_range(1..255)
            )
        }
        SensitiveType::LandlinePhone => {
            let p = &data.patterns;
            let area = pick(&mut rng, &p.landline_area_codes);
            let num: u32 = rng.gen_range(60000000..89999999);
            format!("{}-{}", area, num)
        }
        SensitiveType::LicensePlate => {
            let p = &data.patterns;
            let province = pick(&mut rng, &p.plate_provinces);
            let letters: Vec<char> = p.plate_letters.chars().collect();
            let letter = letters[rng.gen_range(0..letters.len())];
            let suffix: u32 = rng.gen_range(10000..99999);
            format!("{}{}{:05}", province, letter, suffix)
        }
        SensitiveType::CreditCode => {
            let p = &data.patterns;
            let area = pick(&mut rng, &p.credit_code_areas);
            let charset: Vec<char> = p.credit_code_charset.chars().collect();
            let mut code = format!("91{}", area);
            for _ in 0..10 {
                code.push(charset[rng.gen_range(0..charset.len())]);
            }
            code
        }
        SensitiveType::Ssn => {
            format!(
                "{:03}-{:02}-{:04}",
                rng.gen_range(100..899),
                rng.gen_range(10..99),
                rng.gen_range(1000..9999)
            )
        }
        SensitiveType::CreditCard => {
            // 生成 16 位假信用卡号（4开头，Visa 风格）
            let mut num = format!("4{:015}", rng.gen_range(0u64..999_999_999_999_999));
            num.truncate(16);
            num
        }
        SensitiveType::UsPhone => {
            format!(
                "({:03}) {:03}-{:04}",
                rng.gen_range(200..999),
                rng.gen_range(200..999),
                rng.gen_range(1000..9999)
            )
        }
        SensitiveType::UkPhone => {
            format!("+44 {:04} {:06}", rng.gen_range(1000..9999), rng.gen_range(100000..999999))
        }
        SensitiveType::Passport => {
            let letter = (b'A' + rng.gen_range(0..26u8)) as char;
            format!("{}{:08}", letter, rng.gen_range(10000000u32..99999999))
        }
        SensitiveType::Iban => {
            format!(
                "GB{:02}BANK{:014}",
                rng.gen_range(10..99),
                rng.gen_range(10000000000000u64..99999999999999)
            )
        }
        SensitiveType::ZipCode => {
            format!("{:05}", rng.gen_range(10000..99999))
        }
        SensitiveType::UkPostcode => {
            let letter1 = (b'A' + rng.gen_range(0..26u8)) as char;
            let letter2 = (b'A' + rng.gen_range(0..26u8)) as char;
            let letter3 = (b'A' + rng.gen_range(0..26u8)) as char;
            format!(
                "{}{}{} {}{}{}",
                letter1, letter2, rng.gen_range(1..9),
                rng.gen_range(1..9), letter3, (b'A' + rng.gen_range(0..26u8)) as char
            )
        }
        SensitiveType::DriversLicense => {
            let mut dl = String::with_capacity(12);
            for _ in 0..12 {
                if rng.gen_bool(0.5) {
                    dl.push((b'A' + rng.gen_range(0..26u8)) as char);
                } else {
                    dl.push((b'0' + rng.gen_range(0..10u8)) as char);
                }
            }
            dl
        }
        SensitiveType::Custom(_) => "[已替换]".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> ReplaceState {
        ReplaceState::new(42, HashMap::new())
    }

    #[test]
    fn test_replace_person_name() {
        let mut state = test_state();
        let result = apply_replace("张三", &SensitiveType::PersonName, &mut state, &ReplaceStyle::Fake);
        assert!(result.chars().count() >= 2);
    }

    #[test]
    fn test_replace_phone() {
        let mut state = test_state();
        let result = apply_replace("13812345678", &SensitiveType::Phone, &mut state, &ReplaceStyle::Fake);
        assert_eq!(result.len(), 11);
    }

    #[test]
    fn test_replace_email() {
        let mut state = test_state();
        let result = apply_replace("test@example.com", &SensitiveType::Email, &mut state, &ReplaceStyle::Fake);
        assert!(result.contains('@'));
    }

    #[test]
    fn test_replace_id_card() {
        let mut state = test_state();
        let result = apply_replace("110101199001011234", &SensitiveType::IdCard, &mut state, &ReplaceStyle::Fake);
        assert_eq!(result.len(), 18);
        // 校验位应为合法字符（0-9 或 X）
        let last = result.chars().last().unwrap();
        assert!(last.is_ascii_digit() || last == 'X');
    }

    #[test]
    fn test_replace_org() {
        let mut state = test_state();
        let result = apply_replace("某某有限公司", &SensitiveType::OrgName, &mut state, &ReplaceStyle::Fake);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_replace_ip() {
        let mut state = test_state();
        let result = apply_replace("192.168.1.1", &SensitiveType::IpAddress, &mut state, &ReplaceStyle::Fake);
        assert_eq!(result.split('.').count(), 4);
    }

    #[test]
    fn test_replace_bank_card() {
        let mut state = test_state();
        // 16 位原文 → 16 位结果
        let result = apply_replace("6222021234567890", &SensitiveType::BankCard, &mut state, &ReplaceStyle::Fake);
        assert_eq!(result.len(), 16);
        // 19 位原文 → 19 位结果
        let result19 = apply_replace("6222021234567890123", &SensitiveType::BankCard, &mut state, &ReplaceStyle::Fake);
        assert_eq!(result19.len(), 19);
    }

    #[test]
    fn test_replace_credit_code() {
        let mut state = test_state();
        let result = apply_replace("91110000MA0XXXXXXX", &SensitiveType::CreditCode, &mut state, &ReplaceStyle::Fake);
        assert_eq!(result.len(), 18);
    }

    #[test]
    fn test_uniqueness_names() {
        let mut state = test_state();
        let mut names: Vec<String> = Vec::new();
        for _ in 0..100 {
            names.push(state.next_name());
        }
        let unique: std::collections::HashSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), 100, "100 个姓名应全部唯一");
    }

    #[test]
    fn test_uniqueness_orgs() {
        let mut state = test_state();
        let mut orgs: Vec<String> = Vec::new();
        for _ in 0..100 {
            orgs.push(state.next_org());
        }
        let unique: std::collections::HashSet<&String> = orgs.iter().collect();
        assert_eq!(unique.len(), 100, "100 个机构名应全部唯一");
    }

    #[test]
    fn test_uniqueness_addresses() {
        let mut state = test_state();
        let mut addrs: Vec<String> = Vec::new();
        for _ in 0..100 {
            addrs.push(state.next_address());
        }
        let unique: std::collections::HashSet<&String> = addrs.iter().collect();
        assert_eq!(unique.len(), 100, "100 个地址应全部唯一");
    }

    #[test]
    fn test_deterministic_with_seed() {
        // 相同 seed + counter 产生相同结果
        let mut state1 = ReplaceState::new(123, HashMap::new());
        let mut state2 = ReplaceState::new(123, HashMap::new());

        let name1 = state1.next_name();
        let name2 = state2.next_name();
        assert_eq!(name1, name2, "相同 seed 应产生相同姓名");

        let org1 = state1.next_org();
        let org2 = state2.next_org();
        assert_eq!(org1, org2, "相同 seed 应产生相同机构名");

        let addr1 = state1.next_address();
        let addr2 = state2.next_address();
        assert_eq!(addr1, addr2, "相同 seed 应产生相同地址");
    }

    #[test]
    fn test_counter_resume() {
        // 模拟工作区恢复：从 counter=5 开始，应跳过前 5 个
        let mut state_fresh = ReplaceState::new(99, HashMap::new());
        let mut names_first_10: Vec<String> = Vec::new();
        for _ in 0..10 {
            names_first_10.push(state_fresh.next_name());
        }

        let mut counters = HashMap::new();
        counters.insert("PersonName".to_string(), 5);
        let mut state_resumed = ReplaceState::new(99, counters);

        // 恢复后产生的应与 fresh 的第 6~10 个相同
        for i in 5..10 {
            let name = state_resumed.next_name();
            assert_eq!(name, names_first_10[i], "恢复后第 {} 个应一致", i);
        }
    }

    // ========== 辅助函数测试 ==========

    #[test]
    fn test_extract_surname_single() {
        assert_eq!(extract_surname("张三"), "张");
        assert_eq!(extract_surname("李明华"), "李");
        assert_eq!(extract_surname("王"), "王");
    }

    #[test]
    fn test_extract_surname_compound() {
        assert_eq!(extract_surname("欧阳修"), "欧阳");
        assert_eq!(extract_surname("司马迁"), "司马");
        assert_eq!(extract_surname("上官婉儿"), "上官");
        assert_eq!(extract_surname("诸葛亮"), "诸葛");
    }

    #[test]
    fn test_extract_org_suffix() {
        assert_eq!(extract_org_suffix("腾讯科技有限公司"), "公司");
        assert_eq!(extract_org_suffix("北京市朝阳区人民法院"), "法院");
        assert_eq!(extract_org_suffix("中国人民银行"), "银行");
        assert_eq!(extract_org_suffix("北京大学"), "大学");
        assert_eq!(extract_org_suffix("某某机构"), "单位");
    }

    #[test]
    fn test_to_chinese_numeral() {
        assert_eq!(to_chinese_numeral(2), "二");
        assert_eq!(to_chinese_numeral(3), "三");
        assert_eq!(to_chinese_numeral(10), "十");
        assert_eq!(to_chinese_numeral(11), "十一");
        assert_eq!(to_chinese_numeral(20), "二十");
        assert_eq!(to_chinese_numeral(21), "二十一");
        assert_eq!(to_chinese_numeral(99), "九十九");
        // n >= 100 降级为阿拉伯数字
        assert_eq!(to_chinese_numeral(100), "100");
        assert_eq!(to_chinese_numeral(110), "110");
    }

    // ========== 某式测试 ==========

    #[test]
    fn test_mou_person_name() {
        let mut state = ReplaceState::new(42, HashMap::new());
        assert_eq!(state.next_mou_name("张三"), "张某");
        assert_eq!(state.next_mou_name("李四"), "李某");
        assert_eq!(state.next_mou_name("张四"), "张某二");
        assert_eq!(state.next_mou_name("欧阳修"), "欧阳某");
        // 空名字应返回 "某某"
        assert_eq!(state.next_mou_name(""), "某某");
    }

    #[test]
    fn test_mou_org_name() {
        let mut state = ReplaceState::new(42, HashMap::new());
        assert_eq!(state.next_mou_org("腾讯科技有限公司"), "某公司");
        assert_eq!(state.next_mou_org("北京市朝阳区人民法院"), "某法院");
        assert_eq!(state.next_mou_org("百度在线网络技术有限公司"), "某公司二");
    }

    #[test]
    fn test_mou_address() {
        let mut state = ReplaceState::new(42, HashMap::new());
        assert_eq!(state.next_mou_address("北京市朝阳区"), "某地");
        assert_eq!(state.next_mou_address("上海市浦东新区"), "某地二");
    }

    #[test]
    fn test_mou_title() {
        let mut state = ReplaceState::new(42, HashMap::new());
        assert_eq!(state.next_mou_title("总经理"), "某职务");
        assert_eq!(state.next_mou_title("副总裁"), "某职务二");
    }

    // ========== 序号式测试 ==========

    #[test]
    fn test_ordinal_person_name() {
        let mut state = ReplaceState::new(42, HashMap::new());
        assert_eq!(state.next_ordinal_name(), "当事人一");
        assert_eq!(state.next_ordinal_name(), "当事人二");
        assert_eq!(state.next_ordinal_name(), "当事人三");
    }

    #[test]
    fn test_ordinal_org_name() {
        let mut state = ReplaceState::new(42, HashMap::new());
        assert_eq!(state.next_ordinal_org("腾讯科技有限公司"), "甲公司");
        assert_eq!(state.next_ordinal_org("百度集团"), "乙集团");
        assert_eq!(state.next_ordinal_org("阿里巴巴有限公司"), "丙公司");
    }

    #[test]
    fn test_ordinal_address() {
        let mut state = ReplaceState::new(42, HashMap::new());
        assert_eq!(state.next_ordinal_address(), "地址一");
        assert_eq!(state.next_ordinal_address(), "地址二");
        assert_eq!(state.next_ordinal_address(), "地址三");
    }

    #[test]
    fn test_ordinal_title() {
        let mut state = ReplaceState::new(42, HashMap::new());
        assert_eq!(state.next_ordinal_title(), "职务一");
        assert_eq!(state.next_ordinal_title(), "职务二");
        assert_eq!(state.next_ordinal_title(), "职务三");
    }

    // ========== 风格不影响格式型实体 ==========

    #[test]
    fn test_style_does_not_affect_phone() {
        let mut state = ReplaceState::new(42, HashMap::new());
        let r = apply_replace("13812345678", &SensitiveType::Phone, &mut state, &ReplaceStyle::Mou);
        assert_eq!(r.len(), 11);
        assert!(r.chars().all(|c| c.is_ascii_digit()));
    }
}
