# Dimkey Dimkey

> 数据不出本机，隐去敏感痕迹。

Dimkey是一款本地文档脱敏工具，帮助你在使用外部 AI 工具前，快速清除文档中的敏感信息。拖入文件即可自动识别并脱敏，导出安全文件。**纯本地运行，零网络通信**。

基于 Tauri v2 构建，前端 React + TailwindCSS，后端 Rust。

<!-- 可在此处添加应用截图 -->
<!-- ![应用截图](docs/screenshots/main.png) -->

## 功能特性

### 三层渐进式识别引擎

| 引擎 | 识别类型 | 速度 | 说明 |
|------|---------|------|------|
| **正则规则** | 手机号、身份证、银行卡、邮箱、IP 地址、固定电话、车牌号、统一社会信用代码 | 毫秒级 | 先出结果，前端立即渲染 |
| **NER 模型** | 人名、机构名、地址、职位 | 秒级 | ONNX Runtime 推理，异步补充，不阻塞 UI |
| **自定义词典** | 用户自定义关键词 | 即时 | 支持精确/模糊匹配，用户自维护 |

### 脱敏策略

- **掩码 (Mask)**: 部分隐藏，如 `138****1234`
- **替换 (Replace)**: 生成同类型假数据，保持一致性替换（同一实体全文使用相同假数据）
- **泛化 (Generalize)**: 精确值转为范围或类别

### 支持文件格式

- Excel (.xlsx / .xls)
- CSV (.csv / .tsv)
- Word (.docx)

导出时保持原格式（Excel 样式、Word 排版、CSV 编码）。

### 其他亮点

- 脱敏前后对比视图，Diff 高亮显示变化
- 支持手动修正识别结果
- 一致性替换 — 同一敏感实体在全文中映射为相同假数据
- 基于 Codebook 映射表的反向还原
- 工作区管理，支持跨文件一致性映射（v0.2）
- 列级脱敏 — 表格按列批量脱敏（v0.2）
- 配置本地持久化保存

## 技术架构

```
┌─────────────────────────────────┐
│  前端  React 19 + TailwindCSS   │
│  状态管理 Zustand / 虚拟滚动     │
└──────────────┬──────────────────┘
               │ Tauri IPC (invoke)
┌──────────────▼──────────────────┐
│  Rust 后端                       │
│  ├── commands/   IPC 命令入口    │
│  ├── engine/     三层识别引擎    │
│  ├── parser/     文件解析/导出   │
│  ├── desensitizer/ 脱敏算法      │
│  └── models/     数据模型        │
└─────────────────────────────────┘
```

- **前端**只负责交互展示，所有文件解析、识别、脱敏逻辑均在 **Rust 后端**完成
- NER 推理使用 ONNX Runtime (`ort` crate)，模型缺失时优雅降级
- 大型表格使用 `@tanstack/react-virtual` 虚拟滚动

## 快速开始

### 环境要求

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://www.rust-lang.org/tools/install) 1.70+
- macOS 12+ 或 Windows 10+

### 安装与运行

```bash
# 克隆仓库
git clone https://github.com/cube1/desensitize-tool.git
cd desensitize-tool

# 安装前端依赖
npm install

# 开发模式（前端热重载 + Rust 自动编译）
cargo tauri dev
```

> 首次编译约 420+ 个 crate，需要几分钟，请耐心等待。

### 构建发布版本

```bash
cargo tauri build
```

## 项目结构

```
desensitize-tool/
├── src/                    # React 前端
│   ├── components/         # UI 组件
│   ├── stores/             # Zustand 状态管理
│   ├── hooks/              # React Hooks
│   └── types/              # TypeScript 类型
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── commands/       # Tauri IPC 命令
│   │   ├── engine/         # 三层识别引擎
│   │   ├── parser/         # 文件解析器
│   │   ├── desensitizer/   # 脱敏算法
│   │   └── models/         # 数据模型
│   ├── tests/              # 集成测试（33 个用例）
│   └── resources/ner/      # NER 模型文件
├── docs/                   # 项目文档
│   ├── v0.1/               # v0.1 PRD、架构、技术规格
│   └── v0.2/               # v0.2 版本文档
└── .github/workflows/      # CI/CD 发布流程
```

## 开发指南

```bash
# 仅前端开发（不启动 Rust 后端）
npm run dev

# Rust 类型检查
cd src-tauri && cargo check

# 运行全部 Rust 测试
cd src-tauri && cargo test

# 运行单个模块测试
cd src-tauri && cargo test engine::regex_engine
```

### 编码规范

- **Rust**: Tauri command 使用 `#[tauri::command]` 宏，错误统一返回 `Result<T, String>`，错误信息用中文
- **React**: 函数组件 + Hooks，样式只用 TailwindCSS，状态管理用 Zustand
- **通用**: 中文注释，中文提交信息

## 发布流程

推送 `v*.*.*` 格式的 Git 标签即触发 GitHub Actions 自动构建：

- macOS (Apple Silicon) → `.dmg` 安装包
- Windows (x86_64) → `.msi` 安装包
- 自动生成 `latest.json` 支持应用内更新检查

## 路线图

- [x] v0.1 — 单文件脱敏（正则 + NER + 词典，掩码/替换/泛化，对比预览，还原）
- [x] v0.2 — 工作区管理 + 列级脱敏 + Codebook 还原
- [ ] v0.3 — 批量文件、PDF 支持、脱敏模板、报告导出

## 目标用户

- 律师、会计、审计人员 — 处理合同、财务报表中的敏感信息
- 咨询顾问 — 交付物脱敏
- 普通职场人 — 上传 AI 工具前快速脱敏

## 许可证

<!-- 请根据实际情况选择许可证 -->
<!-- [MIT](LICENSE) -->
