# TXT 纯文本脱敏支持设计

## 概述

为Dimkey(Dimkey)增加 TXT 纯文本文件的脱敏支持。复用现有 `FileContent::Document` 变体，将 TXT 每行映射为一个 `Paragraph`，共享检测引擎和前端预览逻辑。

## 设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 数据结构 | 复用 `FileContent::Document` | TXT 和 DOCX 本质都是段落序列，复用可避免改动检测引擎和前端 |
| 编码支持 | UTF-8 + GBK/GB2312 自动检测 | 覆盖中文用户最常见场景 |
| 导出编码 | 保持原编码 | 符合"原格式保持"设计原则 |
| 编码检测库 | `encoding_rs` | Mozilla 出品，Rust 生态最成熟的编码处理库 |

## 改动清单

### 1. 数据模型 — `src-tauri/src/models/sensitive.rs`

- `FileType` 枚举新增 `Txt` 变体
- `FileContent::Document` 新增可选字段 `encoding: Option<String>`（DOCX 为 `None`，TXT 存 `"utf-8"` 或 `"gbk"`）

### 2. TXT 解析器 — `src-tauri/src/parser/txt.rs`（新文件）

- `parse_txt(path: &str) -> Result<FileContent, String>`
- 流程：读取字节 → 编码检测（先试 UTF-8，失败试 GBK）→ 按行分割为 Paragraph → 返回 Document
- 空行保留（text 为空字符串），保持行号对应关系

### 3. TXT 导出 — `src-tauri/src/parser/txt.rs`

- `export_txt(paragraphs: &[Paragraph], output_path: &str, encoding: Option<&str>) -> Result<(), String>`
- 流程：段落 text 用 `\n` 拼接 → 按原编码编码 → 写入文件

### 4. 解析器模块注册 — `src-tauri/src/parser/mod.rs`

- 新增 `pub mod txt;`

### 5. 导入路由 — `src-tauri/src/commands/file.rs`

- `import_file_internal` 的 match 中增加 `"txt"` 分支

### 6. 导出路由 — `src-tauri/src/commands/file.rs`

- `export_content` 中 Document 的 match 增加 `FileType::Txt` 分支

### 7. 依赖 — `src-tauri/Cargo.toml`

- 新增 `encoding_rs` 依赖

### 8. 前端 — `src/components/FileDropZone/index.tsx`

- `SUPPORTED_EXTENSIONS` 加入 `".txt"`

### 9. 前端文件类型映射

- 如有文件类型图标/标签显示，增加 TXT 映射

## 不需要改动

- 三层检测引擎（regex/ner/dict）— 已支持 Document 变体
- 脱敏器（desensitizer）— 与文件类型无关
- 策略配置 — 与文件类型无关
- 前端预览组件 — Document 预览已存在，直接复用

## 边界情况

- **非 UTF-8 且非 GBK 的文件**: 返回错误提示用户"不支持的文件编码，请转为 UTF-8 后重试"
- **空文件**: 返回空 paragraphs 列表，前端正常显示空内容
- **大文件**: 复用已有 50MB 限制
- **BOM 头**: UTF-8 BOM (`\xEF\xBB\xBF`) 需要跳过处理
