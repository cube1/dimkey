# 匿名使用统计设计方案

## 概述

通过 Aptabase（免费第三方分析平台）采集基础功能使用统计，用于了解用户使用习惯、指导产品迭代。纯后端埋点，不采集任何用户身份或文件内容。

## 技术选型

- **平台**: [Aptabase](https://aptabase.com/) 免费版（2 万事件/月）
- **集成方式**: `tauri-plugin-aptabase` 官方 Tauri 插件
- **离线缓存**: 依赖插件内置的离线队列 + 批量上报机制，不额外实现

## 架构

```
用户操作 → 前端 invoke → Rust Command Handler → 执行业务逻辑
                                                  ↓ (成功后)
                                            Aptabase SDK 上报事件
                                                  ↓
                                         插件内置缓存 & 批量上报
                                                  ↓
                                         Aptabase Cloud Dashboard
```

设计原则：
- 后端埋点为主，事件在 Rust command 中触发
- 只统计成功完成的操作
- 不采集任何用户标识或文件内容
- 上报失败静默忽略，绝不影响正常使用

## 事件定义

共 8 个事件：

| 事件名 | 触发位置 | 属性 |
|---|---|---|
| `app_launched` | `main.rs` 应用启动时 | `version` |
| `file_imported` | `commands/file.rs` 导入成功后 | `file_type`, `row_count` |
| `detection_completed` | `commands/detect.rs` 识别完成后 | `engine`, `sensitive_count` |
| `desensitize_applied` | `commands/desensitize.rs` 脱敏成功后 | `strategy`, `cell_count` |
| `file_exported` | `commands/file.rs` 导出成功后 | `file_type`, `duration_ms` |
| `restore_used` | `commands/workspace.rs` 还原成功后 | 无 |
| `workspace_created` | `commands/workspace.rs` 创建成功后 | 无 |
| `dict_updated` | `commands/config.rs` 词典保存成功后 | `entry_count` |

属性说明：
- `version` — 从 tauri.conf.json 读取，自动跟随版本号
- `os` / `arch` — Aptabase 插件自动采集，无需手动传
- `file_type` — `"xlsx"` / `"csv"` / `"docx"`
- `engine` — `"regex"` / `"ner"` / `"dict"`
- `strategy` — `"mask"` / `"replace"` / `"generalize"`
- 数值属性均为精确整数

## 代码改动范围

仅 Rust 后端 + 少量前端（告知弹窗和设置开关）：

```
src-tauri/Cargo.toml                  — 添加 tauri-plugin-aptabase 依赖
src-tauri/tauri.conf.json             — 注册插件权限
src-tauri/src/main.rs                 — 插件初始化（~3 行）
src-tauri/src/commands/file.rs        — 2 个埋点（导入、导出）
src-tauri/src/commands/detect.rs      — 1 个埋点（识别完成）
src-tauri/src/commands/desensitize.rs — 1 个埋点（脱敏执行）
src-tauri/src/commands/workspace.rs   — 2 个埋点（创建、还原）
src-tauri/src/commands/config.rs      — 1 个埋点（词典更新）
src/components/AnalyticsConsent.tsx   — 首次启动告知弹窗（新增）
src/pages/SettingsPage/              — 设置页增加统计开关（如已有设置页则在其中添加）
```

## 隐私保障

- Aptabase 不使用 cookie、不生成设备指纹、不追踪跨会话用户身份
- 不传任何文件内容、文件名、路径、用户名
- 所有数值属性为聚合统计用数字，无法反推具体数据

## 用户告知与控制

- 首次启动弹窗告知："我们会收集匿名使用统计以改进产品，不包含任何文件内容或个人信息"
- 设置页提供关闭开关，用户可随时关闭
- 开关状态存在本地配置文件中（`analytics_enabled`），默认开启
- Rust 端每次 `track_event` 前检查配置项，为 `false` 则跳过

## 埋点代码示例

```rust
// commands/file.rs — import_file 成功后
app.track_event("file_imported", json!({
    "file_type": "xlsx",
    "row_count": content.rows.len()
}));
```
