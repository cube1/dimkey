---
name: dimkey-release
description: Use when user mentions "发版", "打包", "发布", "release", "升级版本", "新版本", or wants to release a new version of the Dimkey app. Covers the full release workflow including version bump, changelog generation, git tag, and macOS local build for both Chinese and English variants.
---

# Dimkey Release

Dimkey 应用完整发布流程。从版本号同步到 changelog 生成、tag 推送、本地构建上传，一条龙。

**重要：从 v0.7.1 起 Dimkey 分中文版（zh）和英文版（en）两个独立产物**：
- 中文版：`com.dimkey.cn` / `Dimkey-zh_*` / NER 模型 `chinese`（shibing624/bert4ner-base-chinese）
- 英文版：`com.dimkey.en` / `Dimkey-en_*` / NER 模型 `distilbert-ner`（dslim/distilbert-NER）
- 语言由前端 `VITE_DIMKEY_LANG` 和后端 Cargo feature `lang-zh|lang-en` 在编译期锁定，无运行时切换

## 流程 Checklist

按顺序逐步执行，每步完成后再进入下一步。

### 1. 确认版本号

向用户确认新版本号。参考当前版本（读 `src-tauri/tauri.conf.json` 的 `version` 字段）建议 patch/minor/major 升级。

同步修改三处（必须一致）：
- `package.json` → `"version": "X.Y.Z"`
- `src-tauri/Cargo.toml` → `version = "X.Y.Z"`
- `src-tauri/tauri.conf.json` → `"version": "X.Y.Z"`

### 2. 生成 Changelog

**获取 commit 列表：**

```bash
git log $(git describe --tags --abbrev=0)..HEAD --oneline
```

如果没有 tag，使用 `git log --oneline` 获取全部 commit。

**归类规则：**

| commit 前缀 | changelog 分类 |
|-------------|---------------|
| `feat:` | 新功能 |
| `fix:` | 修复 |
| `refactor:` | 重构 |
| `ci:` | CI |
| `chore:` / `docs:` / 其他 | 其他 |

**格式要求：**

- 中文描述，用粗体标注关键功能名
- 版本标题下方插入下载链接（中英文版各一个）
- 插入到 `CHANGELOG.md` 的 `# 更新日志` 之后、上一个版本之前

**下载链接模板：**

```markdown
## [vX.Y.Z] — YYYY-MM-DD

**下载：**
- 中文版：[官网下载](https://dimkey.com/#download)
- English: [Download](https://dimkey.com/en/#download)
```

**产物分发架构：**
- 私有仓库 `cube1/dimkey` Releases: 存放全部构建产物（内部备份）
- 公开仓库 `cube1/dimkey-site` Releases: 存放 DMG、EXE、updater 包（中英文各一份）
- `dimkey.com/#download`: 中文版下载页，JS 读取 `latest-zh.json` 动态生成下载链接
- `dimkey.com/en/#download`: 英文版下载页，JS 读取 `latest-en.json`
- `dimkey.com/latest-zh.json` + `dimkey.com/latest-en.json`: 两份自动更新元数据，分别供中英文版查询
- `dimkey.com/latest.json`: **过渡兼容文件**，内容等同于 `latest-zh.json`，供 v0.7.0 及之前的老客户端查询（让老用户能升级一次到新版）；可在所有老用户升级完成后停用

生成后让用户确认/修改 changelog 内容，确认后写入文件。

### 3. 提交 + Tag + 推送

```bash
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json CHANGELOG.md
git commit -m "chore: release vX.Y.Z"
git tag vX.Y.Z
git push origin main
git push origin vX.Y.Z
```

tag 推送后 GitHub Actions 自动触发 Windows 构建（matrix 同时构建 zh + en 两份）。

### 4. macOS 本地构建 + 上传（中英文各一次）

**重要：必须先跑中文版（`zh`）再跑英文版（`en`），顺序不能反**——脚本设计成中文版做 Release 描述更新，英文版做 latest-*.json 触发，避免重复执行。

```bash
source ~/.zshrc
# 中文版（先跑）
./scripts/release-macos.sh vX.Y.Z zh
# 英文版（后跑）
./scripts/release-macos.sh vX.Y.Z en
```

每次脚本自动完成：
- 切换 NER 模型（`chinese` / `distilbert-ner`）
- 临时改写 `tauri.conf.json` 的 identifier 和 updater endpoint
- 临时改写 `Cargo.toml` 的 `default = ["lang-xx"]`
- 设置 `VITE_DIMKEY_LANG` 环境变量
- 本地构建 macOS 应用（代码签名）
- xcrun notarytool 公证（自动重试 3 次）
- 生成对应 lang 后缀的 DMG（`Dimkey-zh_X.Y.Z_aarch64.dmg`）+ updater 产物
- 上传到私有仓库 + 公开仓库 dimkey-site Release
- 退出时自动还原 `tauri.conf.json` 和 `Cargo.toml`

第二次跑（`en`）时还会等待 Windows CI 完成 → 触发 latest-zh.json + latest-en.json 生成并同步到 dimkey-site。

每次超时设为 10 分钟（600000ms），构建+公证需要较长时间。

### 5. 验证

构建完成后验证两份产物 + 两份 latest.json：
```bash
# 私有仓库 Release（应该看到 4 个产物：Dimkey-{zh,en}_*.{dmg,app.tar.gz} + 各自 sig + Windows nsis）
gh release view vX.Y.Z

# 公开仓库 Release
gh release view vX.Y.Z --repo cube1/dimkey-site

# 自动更新元数据（两个文件）
curl -s https://dimkey.com/latest-zh.json | jq .
curl -s https://dimkey.com/latest-en.json | jq .
```

预期产物清单（每个 Release 共 8 个核心文件）：

| 产物 | 中文版 | 英文版 |
|------|--------|--------|
| macOS DMG | `Dimkey-zh_X.Y.Z_aarch64.dmg` | `Dimkey-en_X.Y.Z_aarch64.dmg` |
| macOS Updater | `Dimkey-zh_X.Y.Z_aarch64.app.tar.gz` (+`.sig`) | `Dimkey-en_X.Y.Z_aarch64.app.tar.gz` (+`.sig`) |
| Windows | `Dimkey-zh_X.Y.Z_x64-setup.exe` + `.nsis.zip` (+`.sig`) | `Dimkey-en_X.Y.Z_x64-setup.exe` + `.nsis.zip` (+`.sig`) |
| latest 元数据 | `latest-zh.json` | `latest-en.json` |

## 注意事项

- commit 消息用中文，tag 用 `vX.Y.Z` 格式
- 构建平台：macOS 本地构建（签名+公证），Windows 由 GitHub CI 构建（matrix 同时出 zh + en）
- changelog 中的下载链接 URL 是预构造的，基于固定命名规则，不依赖实际文件是否已上传
- **不要颠倒 zh / en 顺序**：脚本逻辑依赖中文版先跑（更新 Release 描述）+ 英文版后跑（触发 latest.json 生成）
- macOS 上中英文版可并存（bundle id 不同）；Windows 上默认安装路径相同（productName 都是 Dimkey），同一台机器只能装一种
