# 表格列级脱敏优化设计

## 背景

当前表格脱敏与文档脱敏共用同一套流程：逐单元格识别 → 逐项展示 → 全自动脱敏。这对表格场景存在三个问题：

1. **缺乏列级控制** — 表格数据天然按列组织，用户需要按列设规则，而非逐单元格操作
2. **脱敏映射表过大** — 一致性映射按每个敏感项存储，大表（万行级）产生海量映射记录
3. **表格与文档体验无差异** — 表格有结构化优势，应该利用列结构提供更高效的交互

## 设计目标

- 表格导入后，按列推断敏感类型，用户在列头确认/调整后再执行脱敏
- 可还原列（名字/地址/机构）使用去重码本，映射体积缩减 20-30 倍
- 文档类型流程保持不变，仅表格走新流程

## 整体流程

```
导入表格文件
    ↓
采样识别（前100行或10%取较小值）
    ↓
列头显示推断标签（如 "手机号 95%"）
    ↓
用户通过列头下拉菜单确认/修正列规则
    ↓
用户点击"执行脱敏"
    ↓
按列批量脱敏 + 构建去重码本
    ↓
ComparisonView 双栏预览
    ↓
导出
```

与现有流程的区别：
- 不再逐单元格识别全表，而是采样推断列类型
- 不再自动执行脱敏，用户确认列规则后手动触发
- 文档类型（Word）保持现有全自动流程不变

## 列头下拉菜单（ColumnRulePopover）

点击表格列头弹出 Popover，包含：

- **敏感类型选择** — 预填自动推断结果，用户可修改（下拉选择所有 SensitiveType）
- **脱敏策略选择** — 掩码/替换/泛化，根据类型给出推荐默认值
- **可还原勾选框** — 仅替换/泛化策略可勾选，掩码时灰掉
- **不脱敏此列** — 一键跳过按钮
- **确认按钮** — 保存列规则

列头状态三种视觉态：
- 未识别（灰色）
- 已推断待确认（黄色标签）
- 已确认（绿色标签）

已确认的列整列加浅色背景。

## 去重码本（Codebook）

### 原理

对每个可还原列，提取所有唯一值，仅为唯一值生成假数据替换。同一值出现多次只存一条映射。

### 存储格式

独立文件：`{APP_DATA}/workspaces/{workspace_id}/{record_id}.codebook.json`

```json
{
  "version": 1,
  "columns": {
    "1": {
      "type": "PersonName",
      "strategy": "Replace",
      "mappings": {
        "张三": "李四",
        "王五": "赵六"
      }
    },
    "3": {
      "type": "Address",
      "strategy": "Replace",
      "mappings": {
        "北京市朝阳区": "上海市浦东新区"
      }
    }
  }
}
```

掩码列不写入码本（不可逆）。

### 体积对比（10,000 行表格）

| 列 | 唯一值 | 当前方案 | 码本方案 |
|---|---|---|---|
| 姓名 | ~300 | 10,000 条 | 300 条 |
| 地址 | ~200 | 10,000 条 | 200 条 |
| 手机号 | — | 10,000 条 | 0 条（掩码不存） |
| **合计** | | **~30,000 条** | **~500 条** |

### 还原流程

1. 用户选择历史记录点击"还原"
2. 加载对应 `.codebook.json`
3. 按列反向替换：假数据 → 原文
4. 掩码列无法还原，提示用户

## 后端接口

### 新增 Tauri Command

**`detect_columns`** — 采样识别列类型

```rust
#[tauri::command]
pub async fn detect_columns(
    content: FileContent,
    sample_size: Option<usize>,  // 默认 100
) -> Result<Vec<ColumnInference>, String>
```

逻辑：对每列采样 N 行，统计正则命中率，返回列级推断结果。

**`apply_desensitize_by_columns`** — 按列规则执行脱敏

```rust
#[tauri::command]
pub async fn apply_desensitize_by_columns(
    content: FileContent,
    column_rules: Vec<ColumnRule>,
    workspace_id: String,
) -> Result<DesensitizeResult, String>
```

逻辑：按列遍历，对整列批量脱敏，可还原列构建码本写入磁盘。

### 新增数据模型

```rust
struct ColumnInference {
    col: usize,
    header: String,
    inferred_type: Option<SensitiveType>,
    confidence: f64,
    sample_hits: usize,
    sample_total: usize,
}

struct ColumnRule {
    col: usize,
    sensitive_type: SensitiveType,
    strategy: Strategy,
    reversible: bool,
}
```

### 现有接口不变

文档类型继续走 `apply_desensitize`，表格走 `apply_desensitize_by_columns`。

## 前端变更

### SpreadsheetView 改造

- 列头区域新增推断类型标签 + 点击触发下拉菜单
- 列头三种视觉状态（未识别/已推断/已确认）
- 已确认列整列加浅色背景

### 新增组件

- **ColumnRulePopover** — 列头下拉菜单组件

### 流程控制（useAutoDesensitize 改造）

- 文档类型：保持现有全自动流程
- 表格类型：拆成两步
  - 第一步：导入 + `detect_columns` → 展示推断结果，暂停等用户确认
  - 第二步：用户点击"执行脱敏" → `apply_desensitize_by_columns`

### 新增"执行脱敏"按钮

- 仅表格模式下出现，放在表格上方工具栏
- 至少有一列被确认时才可点击
- 点击后显示进度，完成后切换到 ComparisonView

### ComparisonView

无需改造，已支持 Spreadsheet 类型双栏对比。
