# 测试框架可信度提升 — 设计文档

**日期**: 2026-05-01
**作者**: 谭泽顺 + Claude
**目标**: 让"测试过 = 真的能工作"这件事可信

## 背景

当前测试格局：
- **Rust 集成测试（165 个）**：纯 Rust 调用 `full_pipeline`，验证识别/脱敏/导出逻辑（不碰 UI）
- **Playwright E2E（`e2e/tests/`）**：跑在普通 Chromium，**所有 Tauri IPC 都是 mock 的**
- **真后端 + 真 UI 集成**：`needs_backend` 标记的几乎全跳过（macOS 没 WebView WebDriver 方案）

## 痛点

用户实际遇到的两个具体场景：
1. **"测试都通过，但打开 UI 发现全部都没有替换"** —
   Playwright 当前只断言 `data-testid` 存在/可见（如 `view-comparison`），从不验证脱敏后的内容是否真的与原文不同。组件渲染了 ≠ 业务正确。
2. **"打包后连模型文件都没带，都不能识别"** —
   release 流程没有任何 artifact 校验环节，ONNX / pdfium 资源能否正确进入 `.app` bundle 完全靠运气。

## 方案三件套

### 方向 ① — Rust regression fixture 基础设施

**目的**：每个真实 bug 一个 fixture + 一个测试，永不复发。

- 新增目录 `src-tauri/tests/regression/`，下设 `fixtures/` 与 `cases/`
- 每个 case 命名为 `regression_<issue_id>_<short_desc>.rs`，使用 `full_pipeline`，断言**真实业务行为**（替换发生、识别命中、还原一致）
- 文档 `e2e/bug-list.md` 维护"bug → fixture → test"对应表

### 方向 ② — Playwright 行为断言

**目的**：从"组件存在"升级为"功能发生"。

- 新增 helper `assert_desensitization_applied(page)` —
  从 `window.__DIMKEY_STORE__` 读取真实 state，断言：
  - `currentResult !== null`
  - `currentResult.summary.total > 0`
  - `currentResult.content` 与 `currentFileContent` 的可比对字段实际不同
- 新增 helper `assert_ipc_called(page, cmd_name)` — 用 `window.__E2E_IPC_LOG__` 验证用户操作触发了正确的 IPC
- 新增专项测试 `test_no_silent_passthrough.py` — 复现"UI 没替换"症状，作为回归基线

### 方向 ④ — 打包 artifact smoke 校验

**目的**：build 完不能直接发，必须先验证关键资源在 bundle 里。

- 新增 `scripts/verify-bundle.sh`，对一个 `.app` 路径执行：
  - 校验 `Contents/Resources/ner/model.onnx` 存在且 ≥ 30 MB
  - 校验 `Contents/Resources/ner/tokenizer.json` / `id2label.json` / `model_config.json` 全部存在
  - 校验 `Contents/Resources/pdfium/` 内有动态库
  - 校验 `Contents/MacOS/Dimkey` 二进制可执行
  - 任何一项失败立刻 exit 1
- 同时校验 **`.app.tar.gz`（updater 包）和 `.dmg`** 解压/挂载后的内容（避免打包过程丢失）
- 在 `scripts/release-macos.sh` 的 build 完成后立即调用 `verify-bundle.sh`，校验失败拒绝发布

## 不做的事（YAGNI）

- ❌ 方向 ③（Rust 生成 IPC mock 喂 Playwright）：维护成本高，效果不如 ④
- ❌ macOS WebDriver 方案：技术上不可行
- ❌ AppleScript / 计算机视觉真 app 自动化：脆弱，留作未来增强

## 成功标准

1. 用户报"打开 UI 没替换"这类 bug 时，能快速写 fixture + 测试复现，下次自动拦截
2. 跑 `pytest e2e/tests/` 通过 = 用户实际打开 UI 也能正常工作
3. release 流程在模型文件丢失场景下能拒绝发布，保护用户

## 落地顺序

1. 方向 ② — Playwright 行为断言 helper + 复现测试（最高 ROI，立即解决用户痛点 #1）
2. 方向 ④ — bundle 校验脚本 + release 接入（解决用户痛点 #2，发版前必跑）
3. 方向 ① — regression 基础设施（长期机制，低优先）
