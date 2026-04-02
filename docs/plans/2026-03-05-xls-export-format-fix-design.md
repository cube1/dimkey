# 修复 .xls 文件脱敏导出格式错误 — 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复 .xls 文件脱敏后导出为"伪 .xls"（XLSX 内容 + .xls 扩展名）导致无法还原/打开的问题。

**Architecture:** 前后端双保障 — 前端保存对话框自动将 .xls 转为 .xlsx，后端 export_content 兜底修正路径，并增强 OLE 错误提示。

**Tech Stack:** React + TypeScript（前端），Rust + calamine + rust_xlsxwriter（后端）

---

## 根因

1. `rust_xlsxwriter` 只能写 XLSX（ZIP/XML）格式
2. 前端保存对话框保留原始 `.xls` 扩展名
3. calamine 的 `open_workbook_auto()` 根据 `.xls` 扩展名选择 OLE 解析器，解析 ZIP 内容失败
4. 错误信息：`XIserror:Cfberror:Invalid OLE signature (not an office document?)`

---

### Task 1: 后端 — export_content 兜底修正 .xls 输出路径

**Files:**
- Modify: `src-tauri/src/commands/file.rs:254-262`

**Step 1: 修改 export_content 中 Xls 分支**

将 `file.rs` 第 262 行的：

```rust
FileType::Xlsx | FileType::Xls => export_xlsx(output_path, sheets),
```

替换为：

```rust
FileType::Xlsx | FileType::Xls => {
    // rust_xlsxwriter 只能写 xlsx 格式，当输出路径为 .xls 时自动修正
    let final_path = if matches!(file_type, FileType::Xls) {
        let p = std::path::Path::new(output_path);
        if p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("xls"))
            .unwrap_or(false)
        {
            p.with_extension("xlsx")
                .to_string_lossy()
                .to_string()
        } else {
            output_path.to_string()
        }
    } else {
        output_path.to_string()
    };
    export_xlsx(&final_path, sheets)
}
```

**Step 2: 验证编译通过**

Run: `cd src-tauri && cargo check`
Expected: 编译成功，无错误

**Step 3: Commit**

```bash
git add src-tauri/src/commands/file.rs
git commit -m "fix: 后端兜底修正 xls 导出路径为 xlsx"
```

---

### Task 2: 后端 — 增强 OLE 解析错误提示

**Files:**
- Modify: `src-tauri/src/parser/excel.rs:88-95`

**Step 1: 修改错误处理逻辑**

将 `excel.rs` 第 87-95 行的 `map_err` 替换为：

```rust
let workbook = open_workbook_auto(path)
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("password") || msg.contains("Password") || msg.contains("encrypted") {
            format!("ENCRYPTED:{}", extension)
        } else if msg.contains("OLE") || msg.contains("Cfb") || msg.contains("CFB") {
            "该文件可能不是有效的 Excel 文件，或文件格式与扩展名不匹配。如果是 .xls 文件，请尝试用 Excel 另存为 .xlsx 格式后重试".to_string()
        } else {
            format!("无法打开 Excel 文件：{}", e)
        }
    })?;
```

**Step 2: 验证编译通过**

Run: `cd src-tauri && cargo check`
Expected: 编译成功，无错误

**Step 3: Commit**

```bash
git add src-tauri/src/parser/excel.rs
git commit -m "fix: 增强 OLE 解析错误的中文提示信息"
```

---

### Task 3: 前端 — ResultPage 保存对话框扩展名修正

**Files:**
- Modify: `src/pages/ResultPage/index.tsx:159-165`

**Step 1: 修改 handleExport 中的扩展名逻辑**

将 `ResultPage/index.tsx` 第 159-165 行替换为：

```typescript
      const fileName = fileContent.file_name;
      const rawExt = fileName.split(".").pop()?.toLowerCase() || "csv";
      // rust_xlsxwriter 只能写 xlsx 格式，xls 自动转为 xlsx
      const ext = rawExt === "xls" ? "xlsx" : rawExt;
      const defaultName = fileName.replace(/\.[^.]+$/, `_脱敏.${ext}`);

      const outputPath = await save({
        defaultPath: defaultName,
        filters: [{ name: "脱敏文件", extensions: [ext] }],
      });
```

**Step 2: Commit**

```bash
git add src/pages/ResultPage/index.tsx
git commit -m "fix: ResultPage 导出对话框 xls 自动转为 xlsx 扩展名"
```

---

### Task 4: 前端 — RestorePage 保存对话框扩展名修正

**Files:**
- Modify: `src/pages/RestorePage/index.tsx:50-56`

**Step 1: 修改 handleExport 中的扩展名逻辑**

将 `RestorePage/index.tsx` 第 50-56 行替换为：

```typescript
      const fileName = restored_content.file_name;
      const rawExt = fileName.split(".").pop()?.toLowerCase() || "csv";
      // rust_xlsxwriter 只能写 xlsx 格式，xls 自动转为 xlsx
      const ext = rawExt === "xls" ? "xlsx" : rawExt;
      const defaultName = fileName.replace(/\.[^.]+$/, `_还原.${ext}`);

      const outputPath = await save({
        defaultPath: defaultName,
        filters: [{ name: "还原文件", extensions: [ext] }],
      });
```

**Step 2: Commit**

```bash
git add src/pages/RestorePage/index.tsx
git commit -m "fix: RestorePage 导出对话框 xls 自动转为 xlsx 扩展名"
```

---

### Task 5: 端到端验证

**Step 1: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: 编译成功

**Step 2: 手动测试（使用 test-data 中的 xlsx 文件验证不影响正常流程）**

Run: `cargo tauri dev`

测试步骤：
1. 导入一个 .xlsx 文件 → 脱敏 → 导出 → 确认保存对话框默认 .xlsx → 还原 → 应正常工作
2. 如有 .xls 测试文件：导入 → 脱敏 → 导出 → 确认保存对话框默认 .xlsx（而非 .xls） → 还原 → 应正常工作

**Step 3: 最终 Commit（如有调整）**

---

## 不做

- 不添加 XLS（OLE）写入能力（Rust 生态没有成熟的 xls writer）
- 不做文件魔数检测来重新识别格式
