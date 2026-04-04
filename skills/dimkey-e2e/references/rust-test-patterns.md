# Rust 集成测试模式

## 文件位置

测试文件在 `src-tauri/tests/`，公共辅助在 `src-tauri/tests/common/mod.rs`。

## 测试模板

```rust
mod common;
use dimkey_lib::engine::regex_engine::RegexEngine;
use dimkey_lib::models::sensitive::*;
use dimkey_lib::parser::excel::{parse_csv, parse_xlsx};
use dimkey_lib::parser::word::parse_docx;
use common::*;

#[test]
fn test_scenario_name() {
    let path = test_data_path("文件名.csv");  // 自动从 e2e/fixtures/scenarios/{ext}/ 查找
    let content = parse_csv(&path).expect("导入失败");
    let engine = RegexEngine::new();
    let items = engine.detect(&content);

    assert!(items.len() > 0, "应检测到敏感信息");
    let phones = count_by_type(&items, &SensitiveType::Phone);
    assert!(phones >= 8, "应检测到至少 8 个手机号");
}
```

## 解析函数

| 格式 | 函数 |
|------|------|
| CSV | `parse_csv(&path)` |
| Excel | `parse_xlsx(&path)` |
| Word | `parse_docx(&path)` |

## common 辅助函数

| 函数 | 用途 |
|------|------|
| `test_data_path("文件名")` | 获取 fixture 路径（按扩展名自动查 scenarios/） |
| `count_by_type(&items, &SensitiveType::Phone)` | 按类型计数 |
| `desensitize_content(&content, &items, &strategies)` | 执行脱敏 |
| `restore_from_mappings(&mut content, &mappings)` | 还原 |
| `get_rows/get_headers/get_paragraphs(&content)` | 提取数据 |

## SensitiveType 枚举值

Phone, IdCard, Email, Address, PersonName, OrgName, BankCard, CreditCode, Custom(String)

## 现有测试模块

| 文件 | 数量 | 覆盖 |
|------|------|------|
| engine/regex_engine.rs (单元) | ~90+ | 正则引擎 |
| engine/dict_engine.rs (单元) | 5 | 字典匹配 |
| desensitize_csv.rs | 8 | CSV 全流程 |
| desensitize_excel.rs | 4 | Excel 全流程 |
| desensitize_word.rs | 4 | Word 全流程 |
| restore_roundtrip.rs | 5 | 脱敏→还原 |
| column_desensitize.rs | 3 | 列级规则 |
| consistency.rs | 3 | 一致性替换 |
| boundary.rs | 6 | 边界场景 |
| audit_dump.rs | 3 | 人工审查 |

## 执行命令

```bash
cd src-tauri && cargo test                        # 全部
cd src-tauri && cargo test test_name -- --nocapture  # 单个，含输出
```
