#!/usr/bin/env python3
"""为类型过滤和字典补充边界用例"""
import sys
sys.path.insert(0, "e2e")
from utils.excel_manager import add_testcase

# ── 类型过滤边界 ──

cid = add_testcase({
    "category": "类型过滤",
    "scenario": "enabled_types 传 None: 不传参数时应全量识别（兼容旧调用）",
    "precondition": "fixture: scenarios/csv/员工信息表.csv",
    "steps": "1. 导入员工信息表.csv\n2. enabled_types = None（不传）\n3. 调用 detect_by_regex\n4. 检查结果",
    "expected": "识别结果与传入全部类型时一致，包含 Phone/IdCard/Email 等",
    "fixture": "scenarios/csv/员工信息表.csv",
    "priority": "P1",
    "note": "回归: enabled_types 参数为 None 时的默认行为",
})
print(f"{cid}: enabled_types 传 None")

cid = add_testcase({
    "category": "类型过滤",
    "scenario": "enabled_types 含不存在的类型名: 传入未知类型字符串不应报错",
    "precondition": "fixture: scenarios/csv/员工信息表.csv",
    "steps": '1. 导入员工信息表.csv\n2. enabled_types = ["Phone", "NonExistType"]\n3. 调用 detect_by_regex',
    "expected": "不报错；结果中仅包含 Phone 类型，未知类型被忽略",
    "fixture": "scenarios/csv/员工信息表.csv",
    "priority": "P2",
    "note": "防御: 前端传入脏数据的容错",
})
print(f"{cid}: 不存在的类型名")

cid = add_testcase({
    "category": "类型过滤",
    "scenario": "类型过滤后脱敏: 关闭 IdCard 后执行脱敏，导出中 IdCard 应保持原文",
    "precondition": "fixture: scenarios/csv/员工信息表.csv",
    "steps": '1. 导入员工信息表.csv\n2. enabled_types 去掉 IdCard → detect\n3. 用过滤后的 items 调用 apply_desensitize\n4. 检查导出内容',
    "expected": "导出文件中身份证号保持原文不变；手机号/邮箱等已脱敏",
    "fixture": "scenarios/csv/员工信息表.csv",
    "priority": "P1",
    "note": "端到端: 类型过滤 → 脱敏 → 导出",
})
print(f"{cid}: 类型过滤后脱敏端到端")

cid = add_testcase({
    "category": "类型过滤",
    "scenario": "workspace 持久化 enabled_types: 保存后重新加载工作区，启用类型不变",
    "precondition": "fixture: scenarios/csv/员工信息表.csv",
    "steps": '1. 创建工作区，设置 enabled_types = ["Phone", "Email"]\n2. update_workspace 保存\n3. load_workspace 重新加载\n4. 检查 enabled_types',
    "expected": '加载后 enabled_types 仍为 ["Phone", "Email"]',
    "fixture": "scenarios/csv/员工信息表.csv",
    "priority": "P1",
    "note": "持久化: enabled_types 保存和恢复",
})
print(f"{cid}: workspace 持久化 enabled_types")

# ── 字典边界 ──

cid = add_testcase({
    "category": "字典/白名单",
    "scenario": "字典条目跨格式生效: 同一词条在 CSV 和 TXT 中都应命中",
    "precondition": "fixture: sample.txt + scenarios/csv/员工信息表.csv",
    "steps": '1. 构造 DictEntry: text="张三", mode=Exact\n2. 分别对 sample.txt 和 员工信息表.csv 调用 detect_by_dict\n3. 检查两次结果',
    "expected": '两次结果中均包含 text="张三" 的 Dict 命中项',
    "fixture": "sample.txt",
    "priority": "P2",
    "note": "字典引擎对 Document 和 Spreadsheet 两种 FileContent 的一致性",
})
print(f"{cid}: 字典跨格式生效")

cid = add_testcase({
    "category": "字典/白名单",
    "scenario": "字典条目为空字符串: 不应崩溃或匹配所有文本",
    "precondition": "fixture: sample.txt",
    "steps": '1. 构造 DictEntry: text="", mode=Exact\n2. 调用 detect_by_dict',
    "expected": "不报错，识别结果为空（空字符串不应匹配任何内容）",
    "fixture": "sample.txt",
    "priority": "P2",
    "note": "防御: 空词条的容错处理",
})
print(f"{cid}: 空字符串字典条目")

cid = add_testcase({
    "category": "字典/白名单",
    "scenario": "字典语言过滤: zh 语言条目在 En 模式下不生效",
    "precondition": "fixture: sample.txt",
    "steps": '1. 构造 DictEntry: text="张三", language="zh"\n2. 设置 language_state = En\n3. 调用 detect_by_dict',
    "expected": "En 模式下该 zh 条目被过滤，识别结果中不包含该词条",
    "fixture": "sample.txt",
    "priority": "P2",
    "note": "字典按语言过滤的逻辑验证",
})
print(f"{cid}: 字典语言过滤")

cid = add_testcase({
    "category": "字典/白名单",
    "scenario": "白名单精确匹配: 白名单值 '138001380' 不应排除 '13800138000'",
    "precondition": "fixture: sample.txt（含手机号 13800138000）",
    "steps": '1. 全量识别 sample.txt\n2. 白名单 = ["138001380"]（子串，非完整值）\n3. 过滤',
    "expected": '"13800138000" 不被排除（白名单应精确匹配，不做子串匹配）',
    "fixture": "sample.txt",
    "priority": "P1",
    "note": "白名单精确匹配 vs 子串匹配的行为确认",
})
print(f"{cid}: 白名单精确匹配")

print("\n边界用例添加完成")
