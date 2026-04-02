# Phase 4 设计：NER 异步识别 + 词典管理 + 策略配置

> 基于 TECH-SPEC.md Phase 4 规划，经讨论确认的实施设计。

## 范围总览

| # | 工作块 | 说明 |
|---|--------|------|
| A | NER 引擎架构 | 添加 `ort` crate，搭好 ONNX 加载→分词→推理→后处理流水线。占位模型跑通链路，实际模型文件后续放入 |
| B | 词典引擎实现 | 简单遍历匹配，支持 Exact/Fuzzy，完成 `detect_by_dict` 命令 |
| C | DictDrawer (M1) | 右侧抽屉，词条 CRUD + 关闭时保存并重新触发词典匹配 |
| D | StrategyPanel (M2) | P2 底部栏"脱敏策略"按钮触发的右侧面板，按类型配置策略 |
| E | SensitivePopover (M3) | 点击高亮项弹出浮层：详情、策略切换、实时预览、取消标记 |

**不做**：AC 自动机、打包真实 NER 模型、P2 手动标记、撤销栈（后两者属 Phase 5）。

**依赖关系**：B 和 C 紧耦合，D 和 E 独立，A 独立。

---

## A. NER 引擎架构

### Cargo.toml

添加 `ort = "2"` 依赖。

### NerEngine 结构体

```rust
pub struct NerEngine {
    session: Option<ort::Session>,   // 模型加载成功时 Some，否则 None
    vocab: HashMap<String, i64>,     // 分词词表
    id2label: Vec<String>,           // 模型输出 ID → BIO 标签映射
}
```

### 生命周期

- `NerEngine::try_load(model_dir: &Path)` — 尝试从 `resources/ner/` 加载 `model.onnx` + `vocab.txt` + `id2label.json`。文件不存在时返回 `Ok(NerEngine { session: None, ... })`，不报错。
- `detect()` 方法：`session` 为 None 时直接返回空 Vec，静默降级。

### 推理流水线（模型存在时）

1. 遍历所有文本片段（单元格或段落）
2. 简易字符级分词 → 构建 input_ids / attention_mask
3. `session.run()` 推理 → 输出 BIO 标签序列
4. BIO 后处理：合并连续 B-/I- 标签为实体 span → 映射到 SensitiveType（PER→PersonName, ORG→OrgName, LOC→Address）
5. 构建 SensitiveItem 列表返回

### 模型文件约定

```
src-tauri/resources/ner/
├── model.onnx       # ONNX 模型文件
├── vocab.txt        # 词表
└── id2label.json    # {"0": "O", "1": "B-PER", "2": "I-PER", ...}
```

### detect_by_ner 命令

从 Tauri Managed State 获取 NerEngine 实例，调用 `detect()`。

### 关键设计点

整个 NER 模块可选降级——没有模型文件就静默跳过，有就自动生效，不阻塞其他功能。

---

## B. 词典引擎实现

### 匹配逻辑

```
对每个文本片段（单元格 or 段落）:
    对每个词典条目 entry:
        Exact: 区分大小写的子串匹配
        Fuzzy: 忽略大小写的子串匹配

        每个匹配位置 → SensitiveItem {
            id: 唯一 ID,
            text: 匹配文本,
            sensitive_type: Custom(entry.text),
            location: { row, col, paragraph_index, start, end },
            source: "dict",
        }
```

### detect_by_dict 命令

1. 从 `app_data_dir()/dict.json` 读取词典
2. 构建 DictEngine 实例
3. 调用 `engine.detect(&content)` 返回结果
4. 词典为空时直接返回空 Vec

### 与前端协作

- PreviewPage 已在 `Promise.all` 中并行调用 `detect_by_dict`，无需改动
- 词典更新后（DictDrawer 关闭时）前端重新调用 `detect_by_dict`，替换 detectStore 中 `source=="dict"` 的项

### 不做

- 跨单元格匹配
- 正则模式词条

---

## C. DictDrawer (M1)

### 触发

TopBar 上的"词典管理"按钮。

### 组件

- 右侧滑出抽屉，宽度 400px
- 用 `@headlessui/react` 的 `Dialog` + `Transition`
- 内容：词条列表，每条显示 `文本 | 类型标签 | 匹配模式 | 删除按钮`
- 底部：添加表单（输入文本 + 敏感类型下拉 + Exact/Fuzzy 切换 + 添加按钮）
- 支持行内编辑
- 空状态提示

### 数据流

```
打开抽屉 → configStore.loadDict()
编辑操作 → configStore 内存态修改
关闭抽屉 → configStore.saveDict()
         → 重新调用 detect_by_dict
         → 替换 detectStore 中 source=="dict" 的项
```

---

## D. StrategyPanel (M2)

### 触发

P2 底部栏"开始脱敏"左侧新增"脱敏策略"按钮，点击展开右侧滑出面板。

### 组件

- 右侧面板，宽度 360px
- 列出 13 种 SensitiveType，每种一行：`类型名 | 策略下拉`
- 选择 Mask 时展开：`保留前N位` + `保留后N位` 数字输入框
- 底部："恢复默认" + "保存"按钮
- 保存时调用 `configStore.saveConfig()`

### 策略下拉约束

| 类型 | 可选策略 |
|------|---------|
| PersonName / OrgName / Title | Replace, Mask |
| Address | Mask, Replace, Generalize |
| 其余规则类型 | Mask, Replace |

---

## E. SensitivePopover (M3)

### 触发

P2 中点击任意高亮文本片段。

### 组件

浮层定位在高亮项正下方，内容：

1. **原始文本**（加粗）
2. **敏感类型标签**（带颜色）
3. **策略下拉切换**（同 M2 约束规则）
4. **脱敏预览**：
   - Mask → 前端本地计算，用 `*` 填充中间部分
   - Replace → 显示"（替换为假数据）"
   - Generalize → 显示"（泛化处理）"
5. **"取消标记"按钮** → `detectStore.removeItem(id)`

点击浮层外部自动关闭。
