#!/usr/bin/env python3
"""为 5 个新 fixture 写入测试用例和基线数据到 testcases.xlsx"""
import sys
sys.path.insert(0, "e2e")

from utils.excel_manager import add_testcase, add_baseline

# ══════════════════════════════════════════════
# 1. en/uk_customer_records.csv
# ══════════════════════════════════════════════
case_id = add_testcase({
    "category": "核心管道",
    "scenario": "英文场景脱敏: UK客户记录(UkPhone+CreditCard+UkPostcode+Email)",
    "precondition": "fixture文件 scenarios/en/uk_customer_records.csv 已就绪",
    "steps": "1. 导入 uk_customer_records.csv\n2. 启用全类型识别\n3. 执行脱敏\n4. 导出并比对基线",
    "expected": "UK电话号码、信用卡号、英国邮编、邮箱均被正确识别并脱敏",
    "fixture": "scenarios/en/uk_customer_records.csv",
    "priority": "P0",
    "note": "补充缺失类型: CreditCard, UkPhone",
})
print(f"用例 {case_id}: UK客户记录")

add_baseline("scenarios/en/uk_customer_records.csv", [
    # UkPhone (8条)
    {"value": "+44 7911 123456", "type": "UkPhone", "count": 1, "note": "移动号码", "assert_mode": "hard"},
    {"value": "+44 7700 900123", "type": "UkPhone", "count": 1, "note": "移动号码", "assert_mode": "hard"},
    {"value": "07456 789012", "type": "UkPhone", "count": 1, "note": "无国际前缀", "assert_mode": "hard"},
    {"value": "+44 20 7946 0958", "type": "UkPhone", "count": 1, "note": "伦敦座机", "assert_mode": "hard"},
    {"value": "07891 234567", "type": "UkPhone", "count": 1, "note": "移动号码", "assert_mode": "hard"},
    {"value": "+44 7534 567890", "type": "UkPhone", "count": 1, "note": "移动号码", "assert_mode": "hard"},
    {"value": "+44 161 496 0000", "type": "UkPhone", "count": 1, "note": "曼彻斯特座机", "assert_mode": "hard"},
    {"value": "07321 098765", "type": "UkPhone", "count": 1, "note": "移动号码", "assert_mode": "hard"},
    # CreditCard (8条)
    {"value": "4539 1488 0343 6467", "type": "CreditCard", "count": 1, "note": "Visa", "assert_mode": "hard"},
    {"value": "5425 2334 1102 8976", "type": "CreditCard", "count": 1, "note": "Mastercard", "assert_mode": "hard"},
    {"value": "4716 0912 3456 7890", "type": "CreditCard", "count": 1, "note": "Visa", "assert_mode": "hard"},
    {"value": "5543 2109 8765 4321", "type": "CreditCard", "count": 1, "note": "Mastercard", "assert_mode": "hard"},
    {"value": "4929 5678 9012 3456", "type": "CreditCard", "count": 1, "note": "Visa", "assert_mode": "hard"},
    {"value": "5187 6543 2109 8765", "type": "CreditCard", "count": 1, "note": "Mastercard", "assert_mode": "hard"},
    {"value": "4024 0071 2345 6789", "type": "CreditCard", "count": 1, "note": "Visa", "assert_mode": "hard"},
    {"value": "5264 1893 0574 6281", "type": "CreditCard", "count": 1, "note": "Mastercard", "assert_mode": "hard"},
    # UkPostcode (8条)
    {"value": "SW1A 1AA", "type": "UkPostcode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "EC2R 8AH", "type": "UkPostcode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "M1 1AE", "type": "UkPostcode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "B1 1BB", "type": "UkPostcode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "EH1 1YZ", "type": "UkPostcode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "LS1 1UR", "type": "UkPostcode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "CF10 1EP", "type": "UkPostcode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "G1 1DU", "type": "UkPostcode", "count": 1, "note": "", "assert_mode": "hard"},
    # Email (8条)
    {"value": "oliver.thompson@barclays.co.uk", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "charlotte.davies@hsbc.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "g.wilson@lloyds.co.uk", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "amelia.brown@natwest.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "harry.taylor@rbs.co.uk", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "isabella.martin@santander.co.uk", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "w.harris@virgin.co.uk", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "sophia.clark@metro.co.uk", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    # IBAN (1条, Notes里提到)
    {"value": "GB29 NWBK 6016 1331 9268 19", "type": "IBAN", "count": 1, "note": "Notes字段内", "assert_mode": "hard"},
    # DriversLicense (1条, Notes里提到)
    {"value": "WILSO703159GW9IJ", "type": "DriversLicense", "count": 1, "note": "UK驾照号Notes内", "assert_mode": "hard"},
    # PersonName (NER)
    {"value": "Oliver Thompson", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "Charlotte Davies", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "George Wilson", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "Amelia Brown", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
])
print(f"  基线: 42条")

# ══════════════════════════════════════════════
# 2. en/us_compliance_audit.xlsx
# ══════════════════════════════════════════════
case_id = add_testcase({
    "category": "核心管道",
    "scenario": "英文场景脱敏: US合规审计表(ZipCode+DriversLicense+CreditCard+SSN)",
    "precondition": "fixture文件 scenarios/en/us_compliance_audit.xlsx 已就绪",
    "steps": "1. 导入 us_compliance_audit.xlsx\n2. 启用全类型识别\n3. 执行脱敏\n4. 导出并比对基线",
    "expected": "ZIP编码、驾照号、信用卡号、SSN、美国电话均被正确识别并脱敏",
    "fixture": "scenarios/en/us_compliance_audit.xlsx",
    "priority": "P0",
    "note": "补充缺失类型: ZipCode, DriversLicense",
})
print(f"用例 {case_id}: US合规审计表")

add_baseline("scenarios/en/us_compliance_audit.xlsx", [
    # SSN (10条)
    {"value": "539-48-2671", "type": "SSN", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "218-73-9045", "type": "SSN", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "482-16-3759", "type": "SSN", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "671-24-8903", "type": "SSN", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "345-89-1627", "type": "SSN", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "723-51-4089", "type": "SSN", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "156-92-3748", "type": "SSN", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "894-37-5210", "type": "SSN", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "267-84-1935", "type": "SSN", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "418-63-7092", "type": "SSN", "count": 1, "note": "", "assert_mode": "hard"},
    # UsPhone (10条)
    {"value": "(415) 293-8847", "type": "UsPhone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "312-547-6183", "type": "UsPhone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "(212) 834-2196", "type": "UsPhone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "(617) 429-5731", "type": "UsPhone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "503-218-4765", "type": "UsPhone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "(202) 614-3982", "type": "UsPhone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "(310) 872-4591", "type": "UsPhone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "646-903-2178", "type": "UsPhone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "(773) 541-8629", "type": "UsPhone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "206-734-5918", "type": "UsPhone", "count": 1, "note": "", "assert_mode": "hard"},
    # DriversLicense (10条)
    {"value": "D123-4567-8901", "type": "DriversLicense", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "W456-7890-1234", "type": "DriversLicense", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "E789-0123-4567", "type": "DriversLicense", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "S012-3456-7890", "type": "DriversLicense", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "G234-5678-9012", "type": "DriversLicense", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "C567-8901-2345", "type": "DriversLicense", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "T890-1234-5678", "type": "DriversLicense", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "R123-4567-8901", "type": "DriversLicense", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "W345-6789-0123", "type": "DriversLicense", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "B678-9012-3456", "type": "DriversLicense", "count": 1, "note": "", "assert_mode": "hard"},
    # CreditCard (10条)
    {"value": "4539 1488 0343 6467", "type": "CreditCard", "count": 1, "note": "Visa", "assert_mode": "hard"},
    {"value": "5425 2334 1102 8976", "type": "CreditCard", "count": 1, "note": "Mastercard", "assert_mode": "hard"},
    {"value": "4716 0912 3456 7890", "type": "CreditCard", "count": 1, "note": "Visa", "assert_mode": "hard"},
    {"value": "5543 2109 8765 4321", "type": "CreditCard", "count": 1, "note": "Mastercard", "assert_mode": "hard"},
    {"value": "4929 5678 9012 3456", "type": "CreditCard", "count": 1, "note": "Visa", "assert_mode": "hard"},
    {"value": "5187 6543 2109 8765", "type": "CreditCard", "count": 1, "note": "Mastercard", "assert_mode": "hard"},
    {"value": "4024 0071 2345 6789", "type": "CreditCard", "count": 1, "note": "Visa", "assert_mode": "hard"},
    {"value": "5264 1893 0574 6281", "type": "CreditCard", "count": 1, "note": "Mastercard", "assert_mode": "hard"},
    {"value": "4485 3210 9876 5432", "type": "CreditCard", "count": 1, "note": "Visa", "assert_mode": "hard"},
    {"value": "5391 0567 8901 2345", "type": "CreditCard", "count": 1, "note": "Mastercard", "assert_mode": "hard"},
    # ZipCode (10条)
    {"value": "94102", "type": "ZipCode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "60611", "type": "ZipCode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "10022", "type": "ZipCode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "02108", "type": "ZipCode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "97205", "type": "ZipCode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "20006", "type": "ZipCode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "90028", "type": "ZipCode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "10006", "type": "ZipCode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "60606", "type": "ZipCode", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "98101", "type": "ZipCode", "count": 1, "note": "", "assert_mode": "hard"},
    # Email (10条)
    {"value": "m.johnson@corpx.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "j.williams@corpx.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "d.martinez@corpx.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "s.anderson@corpx.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "r.garcia@corpx.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "l.chen@corpx.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "j.thompson@corpx.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "m.rodriguez@corpx.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "k.white@corpx.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "e.brown@corpx.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    # Title (NER, 10条)
    {"value": "Chief Financial Officer", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "VP of Engineering", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "Senior Analyst", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "Director of Compliance", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "IT Security Manager", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "Head of HR", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "Software Engineer", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "General Counsel", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "Database Administrator", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "Product Manager", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
])
print(f"  基线: 80条")

# ══════════════════════════════════════════════
# 3. en/international_vendor_contacts.docx
# ══════════════════════════════════════════════
case_id = add_testcase({
    "category": "核心管道",
    "scenario": "英文场景脱敏: 国际供应商通讯录(CreditCard+IBAN+Passport+UkPhone+DriversLicense)",
    "precondition": "fixture文件 scenarios/en/international_vendor_contacts.docx 已就绪",
    "steps": "1. 导入 international_vendor_contacts.docx\n2. 启用全类型识别\n3. 执行脱敏\n4. 导出并比对基线",
    "expected": "信用卡号、IBAN、护照号、UK电话、驾照号均被正确识别；散布在段落中的敏感值不遗漏",
    "fixture": "scenarios/en/international_vendor_contacts.docx",
    "priority": "P0",
    "note": "补充: CreditCard+Passport+DriversLicense 在docx格式中",
})
print(f"用例 {case_id}: 国际供应商通讯录")

add_baseline("scenarios/en/international_vendor_contacts.docx", [
    # UkPhone
    {"value": "+44 7911 234567", "type": "UkPhone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "+44 20 7946 1234", "type": "UkPhone", "count": 1, "note": "紧急联系", "assert_mode": "hard"},
    # CreditCard
    {"value": "4539 7890 1234 5678", "type": "CreditCard", "count": 1, "note": "Visa", "assert_mode": "hard"},
    {"value": "5425 6789 0123 4567", "type": "CreditCard", "count": 1, "note": "Mastercard", "assert_mode": "hard"},
    {"value": "4716 5432 1098 7654", "type": "CreditCard", "count": 1, "note": "Visa", "assert_mode": "hard"},
    {"value": "5543 8765 4321 0987", "type": "CreditCard", "count": 1, "note": "Mastercard", "assert_mode": "hard"},
    # IBAN
    {"value": "GB82 WEST 1234 5698 7654 32", "type": "IBAN", "count": 2, "note": "出现2次", "assert_mode": "hard"},
    {"value": "DE89 3704 0044 0532 0130 00", "type": "IBAN", "count": 2, "note": "出现2次", "assert_mode": "hard"},
    {"value": "FR76 1234 5003 1234 5678 9012 345", "type": "IBAN", "count": 1, "note": "", "assert_mode": "hard"},
    # Passport
    {"value": "533410987", "type": "Passport", "count": 1, "note": "UK护照", "assert_mode": "hard"},
    {"value": "12AB34567", "type": "Passport", "count": 1, "note": "法国护照", "assert_mode": "hard"},
    {"value": "GA 1234567", "type": "Passport", "count": 1, "note": "加拿大护照", "assert_mode": "hard"},
    # DriversLicense
    {"value": "B072RRE2I55", "type": "DriversLicense", "count": 1, "note": "德国驾照", "assert_mode": "hard"},
    {"value": "D890-1234-5678", "type": "DriversLicense", "count": 1, "note": "美国驾照", "assert_mode": "hard"},
    # SSN
    {"value": "345-89-1627", "type": "SSN", "count": 1, "note": "", "assert_mode": "hard"},
    # UsPhone
    {"value": "(415) 555-0147", "type": "UsPhone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "+1 416-555-0198", "type": "UsPhone", "count": 1, "note": "加拿大", "assert_mode": "hard"},
    # ZipCode
    {"value": "94105", "type": "ZipCode", "count": 1, "note": "", "assert_mode": "hard"},
    # Email
    {"value": "j.fletcher@techbridge.co.uk", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "k.weber@muenchen-data.de", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "m.dupont@paris-analytics.fr", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "m.torres@dataflow.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "s.obrien@mapletech.ca", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "compliance@globalcorp.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    # PersonName (NER)
    {"value": "James Fletcher", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "Klaus Weber", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "Marie Dupont", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "Michael Torres", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
])
print(f"  基线: 28条")

# ══════════════════════════════════════════════
# 4. txt/IT运维事件报告.txt
# ══════════════════════════════════════════════
case_id = add_testcase({
    "category": "核心管道",
    "scenario": "TXT场景脱敏: IT运维事件报告(IpAddress增强+Title+Phone+Email)",
    "precondition": "fixture文件 scenarios/txt/IT运维事件报告.txt 已就绪",
    "steps": "1. 导入 IT运维事件报告.txt\n2. 启用全类型识别\n3. 执行脱敏\n4. 导出并比对基线",
    "expected": "IPv4/IPv6/公网IP、中文职位、手机号、邮箱均被正确识别并脱敏",
    "fixture": "scenarios/txt/IT运维事件报告.txt",
    "priority": "P0",
    "note": "补充: IpAddress多样化(IPv6+公网), Title(NER)",
})
print(f"用例 {case_id}: IT运维事件报告")

add_baseline("scenarios/txt/IT运维事件报告.txt", [
    # IpAddress — IPv4 内网
    {"value": "192.168.1.10", "type": "IpAddress", "count": 1, "note": "内网Web", "assert_mode": "hard"},
    {"value": "192.168.1.11", "type": "IpAddress", "count": 1, "note": "内网Web", "assert_mode": "hard"},
    {"value": "192.168.1.12", "type": "IpAddress", "count": 1, "note": "内网Web", "assert_mode": "hard"},
    {"value": "10.0.2.100", "type": "IpAddress", "count": 1, "note": "API网关", "assert_mode": "hard"},
    {"value": "172.16.0.1", "type": "IpAddress", "count": 2, "note": "DB主节点,出现2次", "assert_mode": "hard"},
    {"value": "172.16.0.2", "type": "IpAddress", "count": 1, "note": "DB从节点", "assert_mode": "hard"},
    {"value": "172.16.0.3", "type": "IpAddress", "count": 1, "note": "DB从节点", "assert_mode": "hard"},
    {"value": "10.0.0.1", "type": "IpAddress", "count": 1, "note": "跳板机", "assert_mode": "hard"},
    {"value": "10.0.3.50", "type": "IpAddress", "count": 1, "note": "ETL服务器", "assert_mode": "hard"},
    {"value": "192.168.10.100", "type": "IpAddress", "count": 1, "note": "备用服务器", "assert_mode": "hard"},
    # IpAddress — 公网
    {"value": "203.0.113.50", "type": "IpAddress", "count": 2, "note": "公网IP,出现2次", "assert_mode": "hard"},
    {"value": "198.51.100.23", "type": "IpAddress", "count": 1, "note": "CDN", "assert_mode": "hard"},
    {"value": "198.51.100.24", "type": "IpAddress", "count": 1, "note": "CDN", "assert_mode": "hard"},
    {"value": "8.8.8.8", "type": "IpAddress", "count": 1, "note": "DNS", "assert_mode": "hard"},
    # IpAddress — IPv6
    {"value": "2001:0db8:85a3:0000:0000:8a2e:0370:7334", "type": "IpAddress", "count": 1, "note": "IPv6日志服务器", "assert_mode": "hard"},
    {"value": "2001:4860:4860::8888", "type": "IpAddress", "count": 1, "note": "IPv6 DNS", "assert_mode": "hard"},
    {"value": "fd00::3:50", "type": "IpAddress", "count": 1, "note": "IPv6 ETL", "assert_mode": "hard"},
    {"value": "2001:db8::1", "type": "IpAddress", "count": 1, "note": "IPv6备用", "assert_mode": "hard"},
    # Phone
    {"value": "13855556666", "type": "Phone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "18712345678", "type": "Phone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "13900001234", "type": "Phone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "15800009999", "type": "Phone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "13711112222", "type": "Phone", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "18600001111", "type": "Phone", "count": 1, "note": "", "assert_mode": "hard"},
    # Email
    {"value": "chenzy@xxtech.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "zhaomh@xxtech.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "yangfan@xxtech.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    # Title (NER)
    {"value": "运维总监", "type": "Title", "count": 2, "note": "NER,出现2次", "assert_mode": "soft"},
    {"value": "技术总监", "type": "Title", "count": 2, "note": "NER,出现2次", "assert_mode": "soft"},
    {"value": "高级数据库管理员", "type": "Title", "count": 2, "note": "NER,出现2次", "assert_mode": "soft"},
    {"value": "前端技术经理", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "首席技术官", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    # PersonName (NER)
    {"value": "陈志远", "type": "PersonName", "count": 2, "note": "NER", "assert_mode": "soft"},
    {"value": "刘阳", "type": "PersonName", "count": 2, "note": "NER", "assert_mode": "soft"},
    {"value": "赵明华", "type": "PersonName", "count": 2, "note": "NER", "assert_mode": "soft"},
    {"value": "孙文博", "type": "PersonName", "count": 2, "note": "NER", "assert_mode": "soft"},
    {"value": "王建国", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
])
print(f"  基线: 37条")

# ══════════════════════════════════════════════
# 5. docx/集团高管通讯录.docx
# ══════════════════════════════════════════════
case_id = add_testcase({
    "category": "核心管道",
    "scenario": "DOCX场景脱敏: 集团高管通讯录(Title+PersonName+Phone+Email+IdCard+Landline)",
    "precondition": "fixture文件 scenarios/docx/集团高管通讯录.docx 已就绪",
    "steps": "1. 导入 集团高管通讯录.docx\n2. 启用全类型识别\n3. 执行脱敏\n4. 导出并比对基线",
    "expected": "职位头衔、人名、手机号、邮箱、身份证号、座机均被正确识别；Word表格中的数据不遗漏",
    "fixture": "scenarios/docx/集团高管通讯录.docx",
    "priority": "P1",
    "note": "补充: Title(NER)中文职位, 段落+表格混合",
})
print(f"用例 {case_id}: 集团高管通讯录")

add_baseline("scenarios/docx/集团高管通讯录.docx", [
    # Phone (15条: 4总部 + 5板块 + 6职能表格里的 + 紧急联络)
    {"value": "13900001234", "type": "Phone", "count": 1, "note": "王建国", "assert_mode": "hard"},
    {"value": "13800005678", "type": "Phone", "count": 1, "note": "李明远", "assert_mode": "hard"},
    {"value": "18600009876", "type": "Phone", "count": 1, "note": "张慧芳", "assert_mode": "hard"},
    {"value": "15900004321", "type": "Phone", "count": 1, "note": "陈大伟", "assert_mode": "hard"},
    {"value": "13700001111", "type": "Phone", "count": 1, "note": "赵鹏飞", "assert_mode": "hard"},
    {"value": "18500002222", "type": "Phone", "count": 1, "note": "孙丽华", "assert_mode": "hard"},
    {"value": "15600003333", "type": "Phone", "count": 1, "note": "周志强", "assert_mode": "hard"},
    {"value": "13600004444", "type": "Phone", "count": 1, "note": "吴婷婷", "assert_mode": "hard"},
    {"value": "17700005555", "type": "Phone", "count": 1, "note": "郑海波", "assert_mode": "hard"},
    {"value": "13800006666", "type": "Phone", "count": 1, "note": "林晓燕(表格)", "assert_mode": "hard"},
    {"value": "13900007777", "type": "Phone", "count": 2, "note": "黄国栋,紧急联络", "assert_mode": "hard"},
    {"value": "18700008888", "type": "Phone", "count": 1, "note": "马超(表格)", "assert_mode": "hard"},
    {"value": "15800009999", "type": "Phone", "count": 1, "note": "何芳(表格)", "assert_mode": "hard"},
    {"value": "13600001010", "type": "Phone", "count": 1, "note": "许文涛(表格)", "assert_mode": "hard"},
    {"value": "18900002020", "type": "Phone", "count": 1, "note": "方正(表格)", "assert_mode": "hard"},
    {"value": "13500003030", "type": "Phone", "count": 1, "note": "钱学军", "assert_mode": "hard"},
    # Email (16条)
    {"value": "wangjg@huaxing.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "limingyuan@huaxing.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "zhanghuifang@huaxing.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "chendawei@huaxing.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "zhaopengfei@huaxing.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "sunlihua@huaxing.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "zhouzhiqiang@huaxing.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "wutingting@huaxing.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "zhenghaibol@huaxing.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    {"value": "linxiaoyan@huaxing.com", "type": "Email", "count": 1, "note": "表格", "assert_mode": "hard"},
    {"value": "huangguodong@huaxing.com", "type": "Email", "count": 1, "note": "表格", "assert_mode": "hard"},
    {"value": "machao@huaxing.com", "type": "Email", "count": 1, "note": "表格", "assert_mode": "hard"},
    {"value": "hefang@huaxing.com", "type": "Email", "count": 1, "note": "表格", "assert_mode": "hard"},
    {"value": "xuwentao@huaxing.com", "type": "Email", "count": 1, "note": "表格", "assert_mode": "hard"},
    {"value": "fangzheng@huaxing.com", "type": "Email", "count": 1, "note": "表格", "assert_mode": "hard"},
    {"value": "qianxuejun@huaxing.com", "type": "Email", "count": 1, "note": "", "assert_mode": "hard"},
    # IdCard (9条)
    {"value": "110101196803151234", "type": "IdCard", "count": 1, "note": "王建国", "assert_mode": "hard"},
    {"value": "310101197205201234", "type": "IdCard", "count": 1, "note": "李明远", "assert_mode": "hard"},
    {"value": "440301198109121234", "type": "IdCard", "count": 1, "note": "张慧芳", "assert_mode": "hard"},
    {"value": "330106198512081234", "type": "IdCard", "count": 1, "note": "陈大伟", "assert_mode": "hard"},
    {"value": "510107199008150098", "type": "IdCard", "count": 1, "note": "赵鹏飞", "assert_mode": "hard"},
    {"value": "320102198706251234", "type": "IdCard", "count": 1, "note": "孙丽华", "assert_mode": "hard"},
    {"value": "430102199509121234", "type": "IdCard", "count": 1, "note": "周志强", "assert_mode": "hard"},
    {"value": "610102199004189012", "type": "IdCard", "count": 1, "note": "吴婷婷", "assert_mode": "hard"},
    {"value": "350102199202287654", "type": "IdCard", "count": 1, "note": "郑海波", "assert_mode": "hard"},
    # Landline (3条)
    {"value": "010-85001234", "type": "Landline", "count": 1, "note": "总机", "assert_mode": "hard"},
    {"value": "010-85001235", "type": "Landline", "count": 1, "note": "安保", "assert_mode": "hard"},
    {"value": "400-800-1234", "type": "Landline", "count": 1, "note": "IT热线", "assert_mode": "hard"},
    # Title (NER, 中文职位)
    {"value": "集团董事长兼首席执行官", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "集团总裁兼首席运营官", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "集团副总裁兼首席财务官", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "集团副总裁兼首席技术官", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "科技事业部总经理", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "金融事业部总经理", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "地产事业部总经理", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "教育事业部总经理", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "医疗健康事业部总经理", "type": "Title", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "人力资源总监", "type": "Title", "count": 1, "note": "NER,表格", "assert_mode": "soft"},
    {"value": "首席法律顾问", "type": "Title", "count": 1, "note": "NER,表格", "assert_mode": "soft"},
    {"value": "审计总监", "type": "Title", "count": 1, "note": "NER,表格", "assert_mode": "soft"},
    {"value": "战略发展总监", "type": "Title", "count": 1, "note": "NER,表格", "assert_mode": "soft"},
    {"value": "公关总监", "type": "Title", "count": 1, "note": "NER,表格", "assert_mode": "soft"},
    {"value": "首席信息官", "type": "Title", "count": 1, "note": "NER,表格", "assert_mode": "soft"},
    # PersonName (NER)
    {"value": "王建国", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "李明远", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "张慧芳", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "陈大伟", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "赵鹏飞", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
    {"value": "孙丽华", "type": "PersonName", "count": 1, "note": "NER", "assert_mode": "soft"},
])
print(f"  基线: 72条")

print("\n全部用例和基线写入完成！")
