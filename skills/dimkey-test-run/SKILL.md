---
name: dimkey-test-run
description: Dimkey 测试执行与结果回写。支持多维度过滤（全量/分类/用例ID/优先级/fixture），执行 Rust 和 Playwright 测试，结果回写 Excel。触发词："跑测试"、"回归测试"、"测一下"、"验证脱敏"、"检查识别"。仅在涉及 Dimkey 测试执行时使用。
---

# Dimkey 测试执行

## 定位

测试工作流**第三步** — 执行测试并收集结果。纯确定性操作，零 AI 判断。

## 工作流

```
Step 1: 解析用户意图，确定执行范围
    │
    ▼
Step 2: 读取 Excel，筛选目标用例
    找到对应的 test_file 字段
    跳过 coverage="未覆盖" 的用例（提示用户先跑 codegen）
    按测试层分组（Rust / Pytest）
    │
    ▼
Step 3: 执行测试
    Rust: cd src-tauri && cargo test {test_names}
    Pytest: e2e/.venv/bin/pytest {test_files} -v
    │
    ▼
Step 4: 收集结果
    解析 cargo test / pytest 输出
    逐用例判断通过/失败
    │
    ▼
Step 5: 回写 Excel
    update_result(case_id, {"exec_result": "通过/失败", ...})
    详见 [references/excel-manager.md](references/excel-manager.md)
    │
    ▼
Step 6: 汇报
```

## 过滤维度

| 用户表达 | 解析方式 | 示例 |
|----------|----------|------|
| "跑全部测试" | 不过滤，Rust + Pytest 全量 | `cargo test` + `pytest e2e/tests/ -v` |
| "跑核心管道" | Excel category="核心管道" | 找所有 C 前缀用例的 test_file |
| "跑 C01 到 C10" | Excel id 范围匹配 | 指定用例 ID |
| "跑所有 P0" | Excel priority="P0" | 找所有 P0 用例的 test_file |
| "跑员工花名册" | Excel fixture 模糊匹配 | 包含"员工花名册"的用例 |
| "跑 Rust 测试" | 只执行 Rust 层 | `cargo test` |
| "跑 UI 测试" | 只执行 Pytest 层 | `pytest e2e/tests/ -v` |

### 过滤逻辑

1. `read_testcases()` 获取全部用例
2. 按用户条件过滤
3. 取 `test_file` 字段（跳过空值，提示："用例 {id} 未覆盖，需先运行 dimkey-test-codegen 生成代码"）
4. 按测试层分组执行

## 执行命令

### Rust 测试

```bash
cd src-tauri && cargo test                              # 全量
cd src-tauri && cargo test test_function_name -- --nocapture  # 单个
cd src-tauri && cargo test --test desensitize_csv       # 按模块
```

### Pytest（UI 测试）

```bash
# 前提：Vite dev server 已启动
# TAURI_DEV_HOST=127.0.0.1 npm run dev

# 全量（排除 needs_backend）
DIMKEY_E2E=1 DIMKEY_TEST_URL=http://127.0.0.1:1420 \
  e2e/.venv/bin/pytest e2e/tests/ -v -m "not needs_backend"

# 指定文件
DIMKEY_E2E=1 DIMKEY_TEST_URL=http://127.0.0.1:1420 \
  e2e/.venv/bin/pytest e2e/tests/test_workspace_crud.py -v
```

### Pytest 前置检查

执行 Pytest 前确认：
1. Vite dev server 是否已启动（`http://127.0.0.1:1420` 可达）
2. 如未启动，提示用户先执行 `TAURI_DEV_HOST=127.0.0.1 npm run dev`
3. Python venv 和 playwright 浏览器是否已安装

## 汇报格式

```
## 测试结果

执行范围: {描述}
Rust: {n} 通过 / {m} 失败
Pytest: {n} 通过 / {m} 失败

### 未覆盖用例（需先 codegen）
{id}: {场景}

### 失败用例
| ID | 场景 | 错误摘要 |
|----|------|----------|
| C05 | ... | assertion failed: ... |

已回写 Excel ✓
```

## 不做的事

- **不创建测试用例** → 交给 `dimkey-test-design`
- **不写测试代码** → 交给 `dimkey-test-codegen`
- **不修改已有测试文件**
- **不在执行过程中用 AI 判断测试结果** — 通过/失败完全由测试框架决定
