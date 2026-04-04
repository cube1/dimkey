# CLAUDE.md

## 项目概述

Dimkey — 本地文档脱敏工具，基于 Tauri v2 构建。用户拖入文件，自动识别敏感信息并脱敏，导出安全文件。纯本地运行，零网络通信。

支持格式：xlsx / xls / csv / docx / pdf / txt

## 开发命令

```bash
cargo tauri dev          # 开发模式（前端热重载 + Rust 自动编译）
cargo tauri build        # 构建发布版本
npm run dev              # 仅前端开发（不启动 Rust 后端）
cd src-tauri && cargo check   # Rust 类型检查
cd src-tauri && cargo test    # Rust 全部测试（165 个：单元 + 集成）
cd src-tauri && cargo test engine::regex_engine  # 单模块测试

# UI E2E 测试（Playwright + IPC Mock，不需要 Tauri 后端）
python3.11 -m venv e2e/.venv && e2e/.venv/bin/pip install -r e2e/requirements.txt  # 首次安装
e2e/.venv/bin/python -m playwright install chromium  # 首次安装浏览器
TAURI_DEV_HOST=127.0.0.1 npm run dev &  # 先启动 Vite
DIMKEY_E2E=1 DIMKEY_TEST_URL=http://127.0.0.1:1420 e2e/.venv/bin/pytest e2e/tests/ -v -m "not needs_backend"
# needs_backend 标记的测试需要真实 Tauri 后端，macOS 暂无 WebView WebDriver 方案
```

## 技术栈

- **前端**: React 19 + TypeScript + TailwindCSS + Zustand
- **后端**: Rust (Tauri v2 IPC)
- **NER推理**: ONNX Runtime (`ort` crate)
- **平台**: macOS (Apple Silicon + Intel) / Windows (x86_64)

## 架构要点

- 前端只做交互展示，**所有文件解析、识别、脱敏逻辑在 Rust 侧**，通过 Tauri IPC (`invoke`) 通信
- **三层识别引擎（渐进式）**: 正则(毫秒级,先出) → NER/ONNX(秒级,异步补充) → 自定义词典(即时匹配)
- **数据流**: 文件导入 → 解析为 `FileContent` → 三层扫描 → 用户配置策略 → 一致性替换脱敏 → 导出原格式

## 编码规范

### Rust
- Tauri command 用 `#[tauri::command]` 宏，错误返回 `Result<T, String>`，错误信息用中文
- 文件操作用 async，避免阻塞 UI
- 敏感信息类型统一走 `SensitiveType` 枚举

### React/TypeScript
- 函数组件 + Hooks，状态管理用 Zustand（stores 在 `src/stores/`）
- 样式只用 TailwindCSS，不写自定义 CSS
- 与 Rust 通信统一封装 `invoke()` 调用

### 通用
- 中文注释，中文提交信息
- i18n 用 i18next，语言文件在 `src/locales/`

## 常见坑

- PDF 解析依赖 pdfium 动态库，构建时确保 `pdfium-render` 能找到对应平台的 pdfium binary
- NER 模型文件较大，不提交到 git；运行时从 `resources/` 加载 ONNX 模型
- 本地存储路径 macOS: `~/Library/Application Support/com.dimkey/`，Windows: `%APPDATA%/com.dimkey/`
