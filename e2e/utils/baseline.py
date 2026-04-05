"""Fixture 数据基线对照 — 验证检测结果是否匹配期望"""

import warnings

from utils.excel_manager import read_baseline


def assert_baseline(detected_texts: list[str], fixture_file: str) -> dict:
    """对照 Excel 基线验证检测结果

    Args:
        detected_texts: 页面上所有敏感高亮项的文本列表
        fixture_file: fixture 文件名（如 "sample.txt"）

    Returns:
        {
            "passed": bool,
            "hard_missing": [(value, type), ...],
            "soft_missing": [(value, type), ...],
            "hard_found": [(value, type), ...],
            "soft_found": [(value, type), ...],
            "total_expected": int,
            "total_found": int,
        }
    """
    baseline = read_baseline(fixture_file)
    if not baseline:
        warnings.warn(f"fixture {fixture_file!r} 在 Excel 中无基线数据，跳过对照")
        return {
            "passed": True,
            "hard_missing": [],
            "soft_missing": [],
            "hard_found": [],
            "soft_found": [],
            "total_expected": 0,
            "total_found": 0,
        }

    detected_set = set(detected_texts)

    hard_missing = []
    soft_missing = []
    hard_found = []
    soft_found = []

    for item in baseline:
        value = item["value"]
        type_name = item["type"]
        mode = item["assert_mode"]

        found = value in detected_set
        if mode == "hard":
            if found:
                hard_found.append((value, type_name))
            else:
                hard_missing.append((value, type_name))
        else:  # soft
            if found:
                soft_found.append((value, type_name))
            else:
                soft_missing.append((value, type_name))

    return {
        "passed": len(hard_missing) == 0 and len(soft_missing) == 0,
        "hard_missing": hard_missing,
        "soft_missing": soft_missing,
        "hard_found": hard_found,
        "soft_found": soft_found,
        "total_expected": len(baseline),
        "total_found": len(hard_found) + len(soft_found),
    }


def format_baseline_report(result: dict) -> str:
    """格式化基线对照报告"""
    lines = []
    total = result["total_expected"]
    found = result["total_found"]
    lines.append(f"基线对照: {found}/{total} 命中")

    if result["hard_missing"]:
        lines.append(f"  正则类未命中 ({len(result['hard_missing'])}):")
        for value, type_name in result["hard_missing"]:
            lines.append(f"     - {type_name}: {value}")

    if result["soft_missing"]:
        lines.append(f"  NER 类未命中 ({len(result['soft_missing'])}):")
        for value, type_name in result["soft_missing"]:
            lines.append(f"     - {type_name}: {value}")

    if result["passed"]:
        lines.append("  全部命中")
    else:
        parts = []
        if result["hard_missing"]:
            parts.append(f"正则类 {len(result['hard_missing'])} 项")
        if result["soft_missing"]:
            parts.append(f"NER 类 {len(result['soft_missing'])} 项")
        lines.append(f"  未命中: {', '.join(parts)}，测试失败")

    return "\n".join(lines)


def check_baseline(detected_texts: list[str], fixture_file: str) -> None:
    """一步完成基线对照 + 断言，fail 时输出漏检明细

    用法：
        check_baseline(detected_texts, "sample.txt")
    """
    result = assert_baseline(detected_texts, fixture_file)
    report = format_baseline_report(result)
    assert result["passed"], f"基线对照失败\n{report}"
