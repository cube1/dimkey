# 项目启动指南

## 环境准备

### macOS
```bash
# 1. 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. 安装 Node.js (推荐 v20+)
brew install node

# 3. 安装 Tauri CLI
cargo install tauri-cli --version "^2"

# 4. 安装系统依赖（macOS 通常不需要额外依赖）
```

### Windows
```powershell
# 1. 安装 Rust (需要 Visual Studio C++ Build Tools)
# 下载 https://rustup.rs

# 2. 安装 Node.js
# 下载 https://nodejs.org

# 3. 安装 Tauri CLI
cargo install tauri-cli --version "^2"

# 4. 安装 WebView2 (Windows 10/11 通常已内置)
```

## 项目初始化

```bash
# 用 Claude Code 执行以下步骤：

# 1. 创建项目目录
mkdir desensitize-tool && cd desensitize-tool

# 2. 把 CLAUDE.md 和 docs/ 放入项目根目录

# 3. 让 Claude Code 初始化 Tauri v2 项目
# 提示词: "根据 CLAUDE.md 初始化 Tauri v2 项目，使用 React + TypeScript 前端"
```

## 开发命令

```bash
# 开发模式（热重载）
cargo tauri dev

# 构建发布版本
cargo tauri build

# 仅前端开发
npm run dev

# Rust 检查
cd src-tauri && cargo check
```

## 给 Claude Code 的第一个提示词

把项目文件准备好后，在 Claude Code 中执行：

```
请阅读 CLAUDE.md 和 docs/ 下的所有文档，然后：
1. 初始化 Tauri v2 项目（React + TypeScript + TailwindCSS）
2. 创建 CLAUDE.md 中定义的项目目录结构
3. 配置 Cargo.toml 添加所需依赖
4. 实现一个最小可运行的骨架：文件拖入 → 调用 Rust → 返回结果显示在界面上
```
