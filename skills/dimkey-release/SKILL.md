---
name: dimkey-release
description: Use when user mentions "发版", "打包", "发布", "release", "升级版本", "新版本", or wants to release a new version of the Dimkey app. Covers the full release workflow including version bump, changelog generation, git tag, and macOS local build.
---

# Dimkey Release

Dimkey 应用完整发布流程。从版本号同步到 changelog 生成、tag 推送、本地构建上传，一条龙。

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
- 版本标题下方插入下载链接（URL 按命名规则预构造，无需等文件上传）
- 插入到 `CHANGELOG.md` 的 `# 更新日志` 之后、上一个版本之前

**下载链接模板：**

```markdown
## [vX.Y.Z] — YYYY-MM-DD

**下载：** [Windows 安装包](https://github.com/cube1/dimkey-site/releases/download/vX.Y.Z/Dimkey_X.Y.Z_x64-setup.exe) | [macOS DMG](https://github.com/cube1/dimkey-site/releases/download/vX.Y.Z/Dimkey_X.Y.Z_aarch64.dmg)
```

**文件名规则：**
- Windows: `Dimkey_{version}_x64-setup.exe`
- macOS: `Dimkey_{version}_aarch64.dmg`

**产物分发架构：**
- 私有仓库 `cube1/dimkey` Releases: 存放全部构建产物（内部备份）
- 公开仓库 `cube1/dimkey-site` Releases: 存放 DMG、EXE、updater 包（用户下载）
- `dimkey.com/latest.json`: 自动更新元数据（通过 GitHub Pages 提供）

生成后让用户确认/修改 changelog 内容，确认后写入文件。

### 3. 提交 + Tag + 推送

```bash
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json CHANGELOG.md
git commit -m "chore: release vX.Y.Z"
git tag vX.Y.Z
git push origin main
git push origin vX.Y.Z
```

tag 推送后 GitHub Actions 自动触发 Windows 构建。

### 4. macOS 本地构建 + 上传

直接执行发布脚本（环境变量已配置在 ~/.zshrc 中）：

```bash
source ~/.zshrc && ./scripts/release-macos.sh vX.Y.Z
```

脚本自动完成：
- 本地构建 macOS 应用（代码签名）
- xcrun notarytool 公证（自动重试 3 次）
- 生成 DMG + updater 产物
- 上传到私有仓库 GitHub Release
- 同步上传到公开仓库 dimkey-site Release
- 从 CHANGELOG.md 更新 Release 描述
- 等待 Windows CI 完成后触发 latest.json 生成并同步到 dimkey-site

超时时间设为 10 分钟（600000ms），构建+公证需要较长时间。

### 5. 验证

构建完成后验证：
```bash
# 私有仓库 Release
gh release view vX.Y.Z
# 公开仓库 Release
gh release view vX.Y.Z --repo cube1/dimkey-site
# 自动更新元数据
curl -s https://dimkey.com/latest.json | jq .
```

## 注意事项

- commit 消息用中文，tag 用 `vX.Y.Z` 格式
- 构建平台：macOS 本地构建（签名+公证），Windows 由 GitHub CI 构建
- changelog 中的下载链接 URL 是预构造的，基于固定命名规则，不依赖实际文件是否已上传
