---
name: dimkey-test-codegen
description: Dimkey 测试代码生成与同步。读取 Excel 未覆盖用例 → 参考已有测试模板 → 生成 Rust/Pytest 测试代码 → 更新 Excel 覆盖状态。触发词："生成测试代码"、"同步用例"、"覆盖率"、"补充代码"。仅在涉及 Dimkey 测试代码生成时使用。
---

# Dimkey 测试代码生成

## 定位

测试工作流**第二步** — 将 Excel 用例翻译为可执行的测试代码。只负责"怎么测"，不负责"测什么"和"跑测试"。

**AI 只做生成时工作，生成的代码需用户 review 后才算完成。**

## 工作流

```
Step 1: 扫描 Excel 未覆盖用例
    读取 coverage="未覆盖" 或 "部分覆盖" 的用例
    按优先级排序：P0 > P1 > P2
    │
    ▼
Step 2: 确定测试层
    根据用例分类自动选择 Rust 或 Pytest（见映射表）
    │
    ▼
Step 2.5: 校验 .baseline.json 与 fixture 一致性
    读取 fixture 对应的 .baseline.json sidecar 文件
    验证 sidecar 中每个 hard 值在 fixture 文件中确实存在
    不一致则停止生成，报告差异
    │
    ▼
Step 3: 读取已有测试文件
    Rust: 读 src-tauri/tests/ 中同分类的已有测试
    Pytest: 读 e2e/tests/ 中同分类的已有测试
    学习命名规范、import 模式、断言风格
    │
    ▼
Step 4: 生成测试代码
    参考已有模板 + .baseline.json 的基线数据
    Rust 测试使用 common::assert_baseline_from_sidecar() 自动加载断言
    同时生成按类型的 count 数量断言作为 smoke test
    │
    ▼
Step 5: 回写 Excel（必须）
    对每个生成了测试代码的用例，调用：
    update_result(case_id, {"test_file": "xxx.rs", "coverage": "已覆盖"})
    test_file 为测试文件名（不含路径前缀），coverage 为覆盖状态
    │
    ▼
Step 6: 输出汇报，等待用户 review
    列出新增/修改的测试文件及覆盖的用例 ID
```

## 分类 → 测试层映射

| 分类 | 测试层 | 原因 |
|------|--------|------|
| 核心管道(C)、策略切换(S)、类型过滤(T) | Rust | 后端逻辑 |
| 字典/白名单(D)、列级规则(L)、一致性替换(K) | Rust | 后端逻辑 |
| 还原(R)、批量处理(B) | Rust | 后端逻辑 |
| 工作区管理(W)、UI交互(U) | Pytest | 前端交互 |

## 追加 vs 新建文件

- **追加**: 同分类已有测试文件且文件 < 200 行 → 追加新测试函数
- **新建**: 同分类无已有文件，或已有文件 > 200 行 → 新建文件，命名跟随已有模式

## 代码生成规范

### Rust 测试

详见 [references/rust-test-patterns.md](references/rust-test-patterns.md)。

关键原则：
- 测试文件放 `src-tauri/tests/`，使用 `common::fixture_path()` 定位 fixture
- **基线断言**：
  - 全类型扫描 → `assert_baseline_from_sidecar(&items, &path)` — 检查 baseline 中所有类型
  - 类型过滤测试 → `assert_baseline_from_sidecar_filtered(&items, &path, Some(&[SensitiveType::Phone]))` — 只检查启用的类型，未启用类型自动跳过
- 同时保留按类型的 `count_by_type()` 数量断言作为 smoke test
- 引擎暂不支持的类型/格式：对应测试标记 `#[ignore = "原因"]`，baseline_coverage 测试也标记 ignore
- 函数名 `test_{snake_case_scenario}`，注释标注用例 ID

### Pytest（UI 测试）

详见 [references/ui-test-patterns.md](references/ui-test-patterns.md)。

关键原则：
- 测试文件放 `e2e/tests/`，使用 `utils/helpers.py` 封装函数
- data-testid 选择器定位元素，needs_backend 标记需要后端的测试
- 一个 class 对应一个功能模块，一个 method 对应一个用例

## Excel 操作

详见 [references/excel-manager.md](references/excel-manager.md)。

回写字段：
- `test_file`: 生成的测试文件名
- `coverage`: "已覆盖" / "部分覆盖"

## 不做的事

- **不创建测试用例** → 交给 `dimkey-test-design`
- **不执行测试** → 交给 `dimkey-test-run`
- **不修改 fixture 文件**
- **不在测试运行时引入 AI 判断** — 断言值来自 .baseline.json（由 dimkey-test-design 生成）
