# Fixture 文件生成模式

生成测试用 fixture 文件的规范和示例。

## 通用原则

- 使用 Python 脚本生成（`e2e/.venv/bin/python`）
- 数据要**贴近真实业务场景**，不要用 "test1", "test2" 这类假数据
- 敏感值使用**合法格式的虚构数据**（能通过正则校验但非真实个人信息）
- 保存到 `e2e/fixtures/scenarios/{ext}/` 对应目录

## Excel (xlsx)

依赖: `openpyxl`

```python
import openpyxl
from openpyxl.styles import Font, Alignment, PatternFill, Border, Side

wb = openpyxl.Workbook()
ws = wb.active
ws.title = "员工花名册"

headers = ["工号", "姓名", "手机号", "身份证号", "邮箱", "银行卡号", "家庭住址"]
# 写表头 + 样式
for col, h in enumerate(headers, 1):
    cell = ws.cell(row=1, column=col, value=h)
    cell.font = Font(bold=True)

# 写数据行（每行包含多种敏感类型）
data = [
    ["EMP001", "张三", "13800138001", "110101199003076789",
     "zhangsan@qq.com", "6222021234567890123", "北京市海淀区中关村大街1号"],
    # ... 更多行
]
for row_idx, row_data in enumerate(data, 2):
    for col_idx, value in enumerate(row_data, 1):
        ws.cell(row=row_idx, column=col_idx, value=value)

wb.save("e2e/fixtures/scenarios/xlsx/文件名.xlsx")
```

**场景模式**: 员工花名册、客户通讯录、合同信息表、工单记录等。

## CSV

直接用 `csv` 模块，注意编码用 `utf-8`。

```python
import csv

with open("e2e/fixtures/scenarios/csv/文件名.csv", "w", newline="", encoding="utf-8") as f:
    writer = csv.writer(f)
    writer.writerow(["姓名", "手机号", "身份证号", "邮箱"])
    writer.writerow(["张三", "13800138001", "110101199003076789", "zhangsan@qq.com"])
```

**场景模式**: 员工信息导出、会议纪要（长文本列）、投诉工单。
**注意**: 会议纪要等非结构化场景，敏感值嵌在长文本段落中。

## Word (docx)

依赖: `python-docx`

```python
from docx import Document

doc = Document()
doc.add_heading("客户调研报告", level=0)
doc.add_paragraph(
    "走访客户: 北京星辰科技有限公司\n"
    "对接人: 李四，手机 15912345678，邮箱 lisi@163.com\n"
    "身份证号: 320106198507121234"
)
# 也可加表格
table = doc.add_table(rows=3, cols=4, style='Table Grid')
# ...
doc.save("e2e/fixtures/scenarios/docx/文件名.docx")
```

**场景模式**: 合同、调研报告、人事通知、病历、判决书。
**注意**: Word 文档中敏感值散布在段落和表格中，测试文档解析的完整性。

## 纯文本 (txt)

直接写文件。

```python
with open("e2e/fixtures/scenarios/txt/文件名.txt", "w", encoding="utf-8") as f:
    f.write("会议纪要\n\n参会人: 张三(13800138001)\n...")
```

## 敏感数据生成规则

| 类型 | 格式要求 | 示例 |
|------|----------|------|
| Phone | 1[3-9]开头，11位 | 13800138001 |
| IdCard | 6位地区码 + 8位生日 + 3位序号 + 1位校验 | 110101199003076789 |
| Email | 合法邮箱格式 | zhangsan@qq.com |
| BankCard | 62开头，16-19位 | 6222021234567890123 |
| PersonName | 2-4字中文姓名 | 张三、欧阳修 |
| Address | 省市区+街道+门牌 | 北京市海淀区中关村大街1号 |
| CreditCode | 18位统一社会信用代码 | 91110108MA01XXXXXX |
| Landline | 区号-号码 | 010-62345678 |

## 已有 fixture 参考

生成前先检查 `e2e/fixtures/scenarios/` 下已有文件，避免重复场景。已有生成脚本在 `e2e/fixtures/generators/` 可供参考。
