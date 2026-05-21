# Regression Fixtures — 用户实际报来的"打开后不工作"文件

## 目的

每个用户报来的"测试过但实际打开 UI 不工作"案例，都在这里留一份 fixture，
配套 `src-tauri/tests/regression_no_passthrough.rs` 写一行测试，永不复发。

## 命名规范

```
issue_<编号>_<短描述>.<扩展名>
```

例如:
- `issue_001_pdf_skipped.txt`
- `issue_002_chinese_address_missed.csv`
- `issue_003_orgname_chained.docx`

## 准备 fixture 的流程

1. 用户原始文件可能含真实敏感信息 — **不要直接入库**
2. 用 dimkey 自身把它脱敏一遍（`Replace + Fake`），导出后作为 fixture
3. 验证脱敏后的 fixture 仍然能复现 bug（如果 bug 是"识别漏掉特定姓名"，
   要确保 fake 数据里仍有相似结构的姓名）
4. 放入本目录，加测试

## 不应放在这里的东西

- 真实客户数据（哪怕你自认为安全）
- 大于 5 MB 的文件（用最小复现样本）
- 与已有 `e2e/fixtures/sample.*` 重复的内容
