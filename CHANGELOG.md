# 更新日志

## [v0.5.2] — 2026-04-05

**下载：** [官网下载](https://dimkey.com/#download)

### 修复
- **NER 模型正确打包**：修复 v0.5.1 遗漏的模型文件，机构名、人名、地址识别恢复正常
- **DMG 添加 Applications 快捷方式**：macOS 安装包打开后可直接拖放到应用文件夹
- 发版脚本全角括号导致变量解析失败

### 新功能
- **NER 模型快速切换**：本地一条命令 `./scripts/use_ner_model.sh <chinese|multilingual>` 秒切两个模型，首次自动下载缓存
- **多语言 NER 模型**：切换到 `Davlan/xlm-roberta-base-ner-hrl`，支持中英文混排识别
- **基线断言收紧**：E2E 测试基线 hard 和 soft 全部命中才算通过，新增 `check_baseline` 辅助函数
- **Rust 全管道集成测试**：三层引擎（正则 + NER + 词典）端到端测试框架

### 重构
- 统一 NER 模型导出脚本 `scripts/export_ner_model.py`，参数化支持中文/多语言双模型

### CI
- 发版产物命名统一（去掉 DMG 文件名中的 `v` 前缀）
- Release workflow 改用 `use_ner_model.sh` 下载模型

### 其他
- 删除冗余的 `prepare_ner_model.py`
- gitignore 新增 `.ner_cache/` 模型缓存目录

## [v0.5.1] — 2026-04-04

**下载：** [官网下载](https://dimkey.com/#download)

### 新功能
- **E2E 测试体系**：Playwright + IPC Mock 双层测试架构，覆盖率 84% → 94%
- **macOS 代码签名与公证**：xcrun notarytool 自动公证
- **自动更新**：updater 端点和签名配置

### 修复
- **IPv6 正则优化**：去除裸 `::` 匹配，400 热线要求分隔符
- **8 个 Bug 修复**（P1×2 + P2×6）：正则引擎扩展、fixture 数据修正
- **4 个 Bug 修复**（P0×1 + P1×3）：词典死循环、正则遗漏、策略切换
- Unicode 空格处理、补全 read_testcases 字段
- E2E 测试文件导入方式、连接策略、路径配置修复

### 重构
- 拆分 E2E skill 为三层测试工作流（设计 / 生成 / 执行）
- 优化 dimkey-test-run skill + Bug 清单机制

### CI
- **优化发版流程**：release workflow + macOS 构建脚本

### 其他
- 更新应用图标（全平台）
- 清理旧仓库遗留，移除 Gitee 同步

## [v0.1.0] — 2026-04-02

首次发布。

- 支持 Excel (.xlsx/.xls)、CSV (.csv/.tsv)、Word (.docx) 文件导入
- 三层识别引擎：正则规则 → NER 模型 → 自定义词典
- 脱敏策略：掩码、替换（假数据）、泛化
- 一致性替换：同一敏感信息在文档中统一替换
- 高亮预览 + 脱敏前后对比
- 原格式导出
- 策略配置本地持久化
- 纯本地运行，零网络通信
