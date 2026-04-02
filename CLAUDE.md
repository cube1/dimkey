# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

Dimkey (Dimkey) — 本地文档脱敏工具，基于 Tauri v2 构建。用户在使用外部 AI 工具前，拖入文件（Excel/CSV/Word），自动识别敏感信息并脱敏，导出安全文件。纯本地运行，零网络通信。

架构设计见 `docs/plans/v0.1/ARCHITECTURE.md`。PRD 和需求文档已迁至工作管理目录 (`~/Workspace/4-工作管理/products/Dimkey/`)。

## 开发命令

```bash
# 开发模式（前端热重载 + Rust 自动编译）
cargo tauri dev

# 构建发布版本
cargo tauri build

# 仅前端开发（不启动 Rust 后端）
npm run dev

# Rust 类型检查
cd src-tauri && cargo check

# Rust 测试
cd src-tauri && cargo test

# Rust 单个模块测试
cd src-tauri && cargo test engine::regex_engine
```

## 技术栈

- **框架**: Tauri v2（前端 React + TypeScript + TailwindCSS，后端 Rust）
- **NER推理**: ONNX Runtime (`ort` crate)
- **目标平台**: macOS (Apple Silicon + Intel) / Windows (x86_64)

## 架构要点

**前后端分工**: 前端只做交互展示，Rust 承担所有文件解析、敏感识别、脱敏逻辑。通过 Tauri IPC (`invoke`) 通信，不使用 HTTP。

**三层识别引擎（渐进式）**:
1. 规则引擎（正则）→ 毫秒级，先出结果给前端渲染
2. NER 模型（ONNX）→ 秒级，异步补充，不阻塞 UI
3. 自定义词典 → 即时匹配，用户自维护

**核心数据流**: 文件导入 → 解析为 `FileContent` → 规则扫描(快) → NER推理(慢,异步) → 用户配置策略 → 执行脱敏(一致性替换) → 导出原格式文件

**Rust 后端模块**:
- `commands/` — Tauri command handlers，前端 invoke 的入口
- `engine/` — 三层识别引擎（regex_engine / ner_engine / dict_engine）
- `parser/` — 文件解析（excel / word / csv）
- `desensitizer/` — 脱敏算法（mask 掩码 / replace 假数据替换 / generalize 泛化）
- `models/` — 数据模型（SensitiveType 枚举、Strategy 枚举、FileContent 等）

**本地存储路径**:
- macOS: `~/Library/Application Support/com.dimkey/`
- Windows: `%APPDATA%/com.dimkey/`
- 存放 `config.json`（策略配置）、`dict.json`（自定义词典）

## 编码规范

### Rust
- Tauri command 使用 `#[tauri::command]` 宏，错误统一返回 `Result<T, String>`，错误信息用中文
- 文件操作用 async，避免阻塞 UI
- 敏感信息类型统一走 `SensitiveType` 枚举

### React/TypeScript
- 函数组件 + Hooks，状态管理用 Zustand
- 样式只用 TailwindCSS，不写自定义 CSS
- 与 Rust 通信统一封装 `invoke()` 调用

### 通用
- 中文注释，中文提交信息

## 当前版本 v0.1 边界

**做**: 单文件导入(xlsx/xls/csv/docx)、三层识别、高亮预览+手动修正、掩码/替换/泛化、一致性替换、原格式保持、脱敏前后对比、配置本地保存

**不做**: PDF/图片、批量文件、水印、自动更新、用户账号
