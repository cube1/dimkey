---
name: dimkey-test-run
description: Dimkey 测试执行与结果回写。支持多维度过滤（全量/分类/用例ID/优先级/fixture），执行 Rust 和 Playwright 测试，结果回写 Excel，更新 Bug 清单。触发词："跑测试"、"回归测试"、"测一下"、"验证脱敏"、"检查识别"。仅在涉及 Dimkey 测试执行时使用。
---

# Dimkey 测试执行

## 工作流

1. **解析意图** — 确定执行范围（见过滤维度）
2. **筛选用例** — `read_testcases()` → 按条件过滤 → 取 `test_file` 字段，跳过空值（提示先跑 codegen）→ 按 `.rs` / `.py` 分组
3. **执行测试** — Rust 和 Pytest 分别执行
4. **收集结果** — 解析输出，逐用例判断通过/失败
5. **回写 Excel** — `update_result(case_id, {...})`，详见 [references/excel-manager.md](references/excel-manager.md)
6. **更新 Bug 清单** — 读取已有 `e2e/bug-list.md`，已修复→关闭，仍失败→保留，新增→追加。详见 [references/bug-list-format.md](references/bug-list-format.md)
7. **汇报**

## 过滤维度

| 用户表达 | 解析方式 |
|----------|----------|
| "跑全部测试" | 全量读取 Excel，跳过 test_file 为空的用例，分组执行 |
| "跑核心管道" | Excel category="核心管道" |
| "跑 C01 到 C10" | Excel id 范围匹配 |
| "跑所有 P0" | Excel priority="P0" |
| "跑员工花名册" | Excel fixture 模糊匹配 |
| "跑 Rust 测试" | 只执行 Rust 层 |
| "跑 UI 测试" | 只执行 Pytest 层 |

## 执行命令

### Rust

```bash
cd src-tauri && cargo test --no-fail-fast                          # 全量
cd src-tauri && cargo test --test desensitize_csv --no-fail-fast   # 按模块
cd src-tauri && cargo test test_function_name -- --nocapture       # 单个
```

### Pytest

```bash
# 全量（排除 needs_backend）
DIMKEY_E2E=1 DIMKEY_TEST_URL=http://127.0.0.1:1420 \
  e2e/.venv/bin/pytest e2e/tests/ -v -m "not needs_backend"

# 指定文件
DIMKEY_E2E=1 DIMKEY_TEST_URL=http://127.0.0.1:1420 \
  e2e/.venv/bin/pytest e2e/tests/test_workspace_crud.py -v
```

Pytest 前置：检查 `http://127.0.0.1:1420` 是否可达，不可达则自动启动 `TAURI_DEV_HOST=127.0.0.1 npm run dev`（端口被占用时先清理残留进程）。

## 汇报格式

```
## 测试结果

执行范围: {描述}
Rust: {n} 通过 / {m} 失败
Pytest: {n} 通过 / {m} 失败

### 编译/环境错误（如有）
{错误摘要} → 需修复后重跑

### 失败用例
| ID | 场景 | 错误摘要 |
|----|------|----------|

### Bug 清单
已更新 e2e/bug-list.md: {n} 个活跃 / {m} 个关闭 / {k} 个环境问题

已回写 Excel ✓
```

## 不做的事

- **不创建测试用例** → 交给 `dimkey-test-design`
- **不写测试代码** → 交给 `dimkey-test-codegen`
- **不修改已有测试文件**
- **不修复编译错误或测试 bug** — 报告给用户，由用户决定下一步
- **不在执行过程中用 AI 判断测试结果** — 通过/失败完全由测试框架决定
