# 模版替换功能设计文档

> 日期: 2026-03-05
> 状态: 已确认

## 背景与动机

律师使用合同模版为新客户生成合同时，需要将模版中旧客户的人名、机构名等替换为新客户信息。Dimkey的敏感信息识别能力可以自动找出这些需要替换的内容，结合字典的替换值功能，实现「模版替换」。

## 核心设计决策

| 决策项 | 选择 | 理由 |
|--------|------|------|
| 替换值来源 | 复用现有字典，DictEntry 增加可选 replacement 字段 | 最小改动，复用已有 UI 和数据结构 |
| 处理范围 | 只替换字典中有替换值的项，其余保持原样 | 律师只关心特定替换，不需要处理所有敏感项 |
| 产品形态 | 新增「模版替换」模式（工作区级别切换） | 脱敏和模版替换是两种不同意图，分离更清晰 |
| 识别引擎 | 三层引擎全开 | 用户可以从识别结果中点击添加字典词条 |
| 一致性映射 | 模版替换模式不使用一致性映射 | 字典本身就是唯一映射源，天然保证一致性 |
| 未匹配提示 | 静默忽略 | 用户已知只替换字典定义的项 |
| 导出格式 | 复用现有导出逻辑 | 保持原格式（docx/xlsx/csv） |

## 数据模型变更

### DictEntry 扩展

```rust
// src-tauri/src/models/strategy.rs
pub struct DictEntry {
    pub keyword: String,
    pub sensitive_type: SensitiveType,
    pub match_mode: MatchMode,
    #[serde(default)]
    pub replacement: Option<String>,  // 新增：可选的替换值
}
```

### Workspace 扩展

```rust
// src-tauri/src/models/workspace.rs
#[derive(Default)]
pub enum WorkspaceMode {
    #[default]
    Desensitize,       // 脱敏模式（默认）
    TemplateReplace,   // 模版替换模式
}

pub struct WorkspaceData {
    // ...现有字段不变
    #[serde(default)]
    pub mode: WorkspaceMode,  // 新增：工作区模式
}
```

### 前端类型

```typescript
// src/types/index.ts
interface DictEntry {
  keyword: string;
  sensitive_type: SensitiveTypeKey;
  match_mode: MatchMode;
  replacement?: string;  // 新增
}

type WorkspaceMode = "Desensitize" | "TemplateReplace";
```

## 用户流程

```
1. 创建/进入工作区 → 切换为「模版替换」模式
2. 拖入合同模版文件
3. 系统三层引擎识别 → 展示所有敏感项
4. 用户点击敏感项 → 弹窗中输入替换值 → 自动加入字典
   （或提前在字典中定义好映射）
5. 字典中有替换值的项用特殊颜色高亮，其余正常高亮
6. 点击执行 → 只替换有替换值的项，其余不动
7. 导出新合同文件
```

## 与现有脱敏流程的差异

| 步骤 | 脱敏模式 | 模版替换模式 |
|------|---------|-------------|
| 识别引擎 | 三层全开 | 三层全开 |
| 预览展示 | 所有敏感项高亮（橙/红色） | 所有敏感项高亮，字典有替换值的用蓝/绿色区分 |
| 点击敏感项 | 修改策略/取消标记 | **设置替换值/加入字典** |
| 策略应用 | 按类型配置的策略（掩码/替换/泛化） | **直接用字典的 replacement 值** |
| 一致性映射 | 启用，保证跨文件一致 | **不使用**，字典本身保证一致 |
| 无替换值项 | 按策略处理 | **保持原样不动** |
| 导出 | 原格式 | 原格式（复用） |

## 前端界面设计

### 工作区模式切换

在工作区头部区域增加模式切换（下拉或 toggle）：

```
┌─────────────────────────────────────────────┐
│  📁 房产合同模版  [脱敏模式 ▾]              │
│                   ┌──────────────┐          │
│                   │ ● 脱敏模式   │          │
│                   │ ○ 模版替换   │          │
│                   └──────────────┘          │
└─────────────────────────────────────────────┘
```

切换模式时清除当前文件的识别结果和脱敏结果。

### 右侧面板（模版替换模式）

简化为两个区域：

```
┌─ 替换字典 ────────────────────────┐
│                                    │
│  张三 → 李四              [×]     │
│  类型: 人名 | 精确匹配            │
│                                    │
│  某某科技公司 → 新客户有限公司 [×] │
│  类型: 机构名 | 精确匹配          │
│                                    │
│  [+ 添加替换词条]                  │
│                                    │
├─ 输出设置 ────────────────────────┤
│  输出目录: ~/Desktop    [选择]     │
└────────────────────────────────────┘
```

- 隐藏「识别规则」区域
- 字典区域突出显示 `原值 → 替换值` 格式
- 没有 replacement 的字典词条不展示或灰色提示

### 字典编辑弹窗增强

添加/编辑词条时增加「替换为」输入框：

```
┌─ 添加词条 ─────────────────────┐
│  关键词:   [张三          ]     │
│  类型:     [人名 ▾        ]     │
│  匹配模式: [精确匹配 ▾    ]     │
│  替换为:   [李四          ]  ← 新增，可选
│                                 │
│       [取消]    [确认]          │
└─────────────────────────────────┘
```

### 预览区高亮

- 字典有替换值的项：蓝色/绿色高亮（区别于脱敏的橙/红色）
- 其余敏感项：正常高亮但不做处理
- hover tooltip 显示：`张三 → 李四`

### SensitivePopover 增强（模版替换模式）

点击敏感项时的弹窗改为：

```
┌────────────────────────────────┐
│  张三                          │
│  [人名]                        │
│                                │
│  替换为: [李四          ]      │
│                                │
│  [加入字典并替换]  [跳过]      │
└────────────────────────────────┘
```

## 后端实现要点

### 修改的文件

1. `src-tauri/src/models/strategy.rs` — DictEntry 增加 replacement 字段
2. `src-tauri/src/models/workspace.rs` — 增加 WorkspaceMode 枚举和字段
3. `src-tauri/src/commands/desensitize.rs` — apply_desensitize 增加模版替换分支
4. `src-tauri/src/commands/workspace.rs` — update_workspace 支持 mode 字段

### 脱敏执行逻辑

```rust
match workspace.mode {
    WorkspaceMode::Desensitize => {
        // 现有逻辑不变：按策略脱敏所有项
    }
    WorkspaceMode::TemplateReplace => {
        // 1. 遍历敏感项
        // 2. 查找字典中是否有对应的替换值
        // 3. 有替换值 → 直接替换
        // 4. 无替换值 → 跳过，保持原文
        // 5. 不使用一致性映射
    }
}
```

### 序列化兼容性

```rust
// DictEntry 的 replacement 使用 serde default，旧数据自动为 None
#[serde(default)]
pub replacement: Option<String>,

// WorkspaceMode 使用 serde default，旧工作区自动为 Desensitize
#[serde(default)]
pub mode: WorkspaceMode,
```

### 不需要新增的 Tauri 命令

复用现有命令，通过工作区 mode 字段区分行为：
- `detect_sensitive` — 两种模式都三层引擎全开，无差异
- `apply_desensitize` — 根据 mode 走不同替换逻辑
- `export_file` — 无需改动

## YAGNI — 不做的事

- 不做模版库管理（工作区+字典组合已足够）
- 不做批量映射导入/导出
- 不做替换值的格式校验
- 不做脱敏→模版替换的自动转换
- 不做模版替换的"还原"功能
