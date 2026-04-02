# 数据脱敏工具 v0.1 技术实施方案

> 基于 PRD.md、ARCHITECTURE.md、USER-STORIES.md、PAGE-FLOW.md，细化技术实现方案，指导开发落地。

## 一、完整数据模型

### 1.1 SensitiveType 枚举（修正）

现有代码仅 7 种类型，PRD 要求 13 种。修正后完整枚举：

```rust
enum SensitiveType {
    // --- 规则引擎识别（8种）---
    Phone,           // 手机号
    IdCard,          // 身份证
    BankCard,        // 银行卡
    Email,           // 邮箱
    IpAddress,       // IP地址
    LandlinePhone,   // 固定电话
    LicensePlate,    // 车牌号
    CreditCode,      // 统一社会信用代码
    // --- NER 模型识别（4种）---
    PersonName,      // 人名
    OrgName,         // 机构名（含公司、政府、学校等所有组织机构）
    Address,         // 地址
    Title,           // 职位
    // --- 自定义词典 ---
    Custom(String),  // 自定义词条
}
```

前端显示名映射：`OrgName → "机构/公司名"`。

### 1.2 FileContent（修正为枚举）

现有实现用扁平 `cells` 统一描述 Excel 和 Word，但 Word 段落渲染需要不同结构。改回 ARCHITECTURE 原设计的枚举方案：

```rust
enum FileContent {
    Spreadsheet {
        file_name: String,
        file_type: FileType,  // Xlsx | Xls | Csv
        headers: Vec<String>,
        rows: Vec<Vec<String>>,  // rows[行][列] = 单元格文本
        row_count: usize,
        col_count: usize,
    },
    Document {
        file_name: String,
        file_type: FileType,  // Docx
        paragraphs: Vec<Paragraph>,
    },
}

struct Paragraph {
    index: usize,       // 段落序号
    text: String,       // 段落纯文本
    style: String,      // 段落样式（heading1/heading2/normal/listParagraph 等）
}
```

前端根据 `FileContent` 变体类型，选择表格渲染或段落渲染。

### 1.3 TaskRecord & MappingEntry（新增）

支撑 P4 历史任务列表、P5 反向还原的完整链路。

**存储方案**：每个脱敏任务存为独立 JSON 文件。
**路径**：`{app_data}/tasks/task_{id}.json`

```rust
/// 脱敏任务记录
struct TaskRecord {
    id: String,                  // 任务ID，格式: "task_20250115_143000_a1b2"（时间戳+4位随机）
    original_file_name: String,  // 原文件名: "合同_甲方.docx"
    file_type: FileType,         // 原文件类型
    created_at: String,          // 创建时间 ISO 8601: "2025-01-15T14:30:00"
    sensitive_count: usize,      // 识别到的敏感项总数（含所有类型）
    replaced_count: usize,       // 实际执行替换的条数
    mappings: Vec<MappingEntry>, // 映射表
}

/// 单条映射关系
struct MappingEntry {
    original_text: String,        // 真实数据: "张三"
    replaced_text: String,        // 脱敏后数据: "李明" 或 "138****5678"
    sensitive_type: SensitiveType, // 敏感类型
    strategy: StrategyType,       // 使用的策略: Mask / Replace / Generalize
    occurrences: usize,           // 该文本在文件中出现的次数
}

/// 策略类型（轻量版，仅用于映射记录）
enum StrategyType {
    Mask,
    Replace,
    Generalize,
}
```

**设计要点**：

- **`MappingEntry` 是去重的**：同一文本只存一条记录，用 `occurrences` 记录出现次数。如 "张三" 出现 5 次 → 一条 MappingEntry，occurrences=5
- **还原时只处理 `Replace` 策略的条目**：Mask 和 Generalize 的数据已不可逆（信息丢失），还原时跳过
- **`id` 用时间戳+随机数**：不引入 UUID 依赖，时间戳保证粗排序，随机后缀避免冲突

**本地文件结构**：

```
~/Library/Application Support/com.desensitize-tool/
├── config.json          # 策略配置
├── dict.json            # 自定义词典
└── tasks/               # 脱敏任务目录（新增）
    ├── task_20250115_143000_a1b2.json
    ├── task_20250114_091500_c3d4.json
    └── ...
```

---

## 二、IPC 接口契约

前端每个操作映射到 Rust command，入参/出参完整定义。

### 2.1 文件操作

```rust
// 导入文件 — P1 拖拽/选择文件后调用
#[tauri::command]
async fn import_file(file_path: String) -> Result<FileContent, String>
// 入参：文件绝对路径
// 出参：FileContent 枚举（Spreadsheet 或 Document）
// 错误："文件格式不支持" / "文件过大" / "文件损坏" / "文件有密码保护"

// 导出脱敏文件 — P3 点击"导出文件"
#[tauri::command]
async fn export_file(
    original_content: FileContent,     // 原始文件内容（用于保持格式）
    desensitized_content: FileContent, // 脱敏后内容
    output_path: String,               // 用户选择的保存路径
) -> Result<(), String>

// 导出还原文件 — P5 点击"导出文件"
#[tauri::command]
async fn export_restored_file(
    restored_content: FileContent,
    output_path: String,
) -> Result<(), String>
```

### 2.2 敏感识别

```rust
// 规则引擎 — P2 进入后立即调用
#[tauri::command]
async fn detect_by_regex(content: FileContent) -> Result<Vec<SensitiveItem>, String>

// NER 模型 — 规则引擎返回后异步调用
#[tauri::command]
async fn detect_by_ner(content: FileContent) -> Result<Vec<SensitiveItem>, String>

// 词典匹配 — 与规则引擎同步调用
#[tauri::command]
async fn detect_by_dict(content: FileContent) -> Result<Vec<SensitiveItem>, String>
// 词典数据由 Rust 端直接从 dict.json 读取，不需要前端传入
```

### 2.3 脱敏执行

```rust
// 执行脱敏 — P2 点击"开始脱敏"
#[tauri::command]
async fn apply_desensitize(
    content: FileContent,
    items: Vec<SensitiveItem>,                     // 用户确认的敏感项列表
    strategies: HashMap<SensitiveType, Strategy>,  // 各类型对应策略
) -> Result<DesensitizeResult, String>

/// 脱敏结果
struct DesensitizeResult {
    content: FileContent,            // 脱敏后的文件内容
    mappings: Vec<MappingEntry>,     // 映射表（用于保存任务和反向还原）
    summary: DesensitizeSummary,     // 统计汇总
}

struct DesensitizeSummary {
    total: usize,                              // 总替换处数
    by_type: HashMap<SensitiveType, usize>,    // 按类型统计
}
```

### 2.4 任务管理（新增）

```rust
// 保存脱敏任务 — P3 导出成功后自动调用
#[tauri::command]
async fn save_task(task: TaskRecord) -> Result<(), String>

// 获取历史任务列表 — P4 进入时调用，P1 最近任务也调用
#[tauri::command]
async fn list_tasks() -> Result<Vec<TaskRecord>, String>
// 返回按 created_at 倒序排列的任务列表

// 删除任务 — P4 点击"删除"确认后调用
#[tauri::command]
async fn delete_task(task_id: String) -> Result<(), String>
// 删除 tasks/task_{id}.json 文件

// 反向还原 — P5 进入时调用
#[tauri::command]
async fn restore_file(
    task_id: String,       // 对应哪个脱敏任务
    file_path: String,     // AI 结果文件路径
) -> Result<RestoreResult, String>

struct RestoreResult {
    original_content: FileContent,   // 还原前内容（AI结果）
    restored_content: FileContent,   // 还原后内容
    matched_count: usize,            // 成功匹配并还原的处数
}
```

### 2.5 配置管理

```rust
// 加载策略配置
#[tauri::command]
async fn load_config() -> Result<AppConfig, String>

// 保存策略配置
#[tauri::command]
async fn save_config(config: AppConfig) -> Result<(), String>

// 加载词典 — M1 抽屉打开时调用
#[tauri::command]
async fn load_dict() -> Result<Vec<DictEntry>, String>

// 保存词典 — M1 抽屉关闭时调用
#[tauri::command]
async fn save_dict(entries: Vec<DictEntry>) -> Result<(), String>
```

### 2.6 前端操作 → IPC 调用映射表

| 用户操作 | 页面 | 调用命令 |
|----------|------|---------|
| 拖拽/选择文件 | P1 | `import_file` |
| 页面加载 | P2 | `detect_by_regex` + `detect_by_dict`（并行），然后 `detect_by_ner`（异步） |
| 点击"开始脱敏" | P2 | `apply_desensitize` |
| 点击"导出文件" | P3 | `export_file` → 成功后 `save_task` |
| 进入历史页 | P4 | `list_tasks` |
| 删除任务 | P4 | `delete_task` |
| 点击"还原" | P4/P1 | 选择文件 → `restore_file` |
| 导出还原文件 | P5 | `export_restored_file` |
| 打开策略配置 | M2 | `load_config` |
| 保存策略配置 | M2 | `save_config` |
| 打开词典管理 | M1 | `load_dict` |
| 关闭词典管理 | M1 | `save_dict` |

---

## 三、前端组件架构

### 3.1 视图切换方案

不引入 React Router，用 Zustand 状态驱动条件渲染。将现有 `step` 改为 `view`，直接对应 PAGE-FLOW 的 5 个视图：

```typescript
type ViewType = "home" | "preview" | "result" | "history" | "restore";
```

App.tsx 根据 `view` 值渲染对应页面组件：

```
view === "home"     → <HomePage />        // P1
view === "preview"  → <PreviewPage />     // P2
view === "result"   → <ResultPage />      // P3
view === "history"  → <HistoryPage />     // P4
view === "restore"  → <RestorePage />     // P5
```

### 3.2 组件树

```
<App>
├── <TopBar />                    // 全局顶栏（标题、返回、词典管理、历史任务）
│
├── <HomePage />                  // P1
│   ├── <FileDropZone />          // 拖拽/点击导入区
│   └── <RecentTasks />           // 最近脱敏任务卡片列表
│
├── <PreviewPage />               // P2
│   ├── <SummaryBar />            // 顶部汇总条（总数、分类标签、NER状态）
│   ├── <ContentRenderer />       // 文件内容渲染区
│   │   ├── <SpreadsheetView />   // Excel/CSV 虚拟滚动表格
│   │   └── <DocumentView />      // Word 段落渲染
│   └── <BottomBar />             // 底部栏（文件信息、开始脱敏按钮）
│
├── <ResultPage />                // P3
│   ├── <DiffPanel />             // 左右并排对比（同步滚动）
│   │   ├── <ContentRenderer />   // 左：原始内容
│   │   └── <ContentRenderer />   // 右：脱敏后内容
│   └── <ResultFooter />          // 底部汇总 + 返回修改/导出
│
├── <HistoryPage />               // P4
│   └── <TaskList />              // 任务卡片列表 + 空状态
│
├── <RestorePage />               // P5
│   ├── <DiffPanel />             // 复用 P3 的对比面板
│   └── <RestoreFooter />         // 底部汇总 + 导出
│
├── <DictDrawer />                // M1 词典管理右侧抽屉（全局）
├── <StrategyPanel />             // M2 策略配置右侧面板（P2 专属）
├── <SensitivePopover />          // M3 敏感项详情浮层（P2 专属）
└── <Toast />                     // 全局 Toast 通知（react-hot-toast）
```

**关键复用**：`ContentRenderer` 和 `DiffPanel` 在 P2/P3/P5 之间复用，`ContentRenderer` 内部根据 `FileContent` 变体自动选择 `SpreadsheetView` 或 `DocumentView`。

### 3.3 状态管理设计

将现有的单一 `appStore` 拆分为 3 个职责清晰的 store：

**`useAppStore`** — 全局导航与文件状态

```typescript
interface AppState {
  // --- 视图导航 ---
  view: ViewType;                    // 当前视图
  previousView: ViewType | null;     // 上一个视图（用于返回）

  // --- 文件 ---
  fileContent: FileContent | null;   // 当前导入的文件内容
  filePath: string | null;           // 当前文件路径

  // --- 操作 ---
  setView: (view: ViewType) => void;
  goBack: () => void;
  importFile: (path: string) => void;
  reset: () => void;                 // 回到初始状态（处理新文件）
}
```

**`useDetectStore`** — P2 识别与编辑状态

```typescript
interface DetectState {
  // --- 识别结果 ---
  items: SensitiveItem[];              // 当前全部敏感项（规则+NER+词典合并）
  nerStatus: "idle" | "running" | "done";  // NER 异步状态

  // --- 用户编辑 ---
  removedIds: Set<string>;             // 用户取消标记的项 ID
  addedItems: SensitiveItem[];         // 用户手动标记的项
  itemOverrides: Map<string, Strategy>;// 单项策略覆盖（M3 浮层中修改的）

  // --- 撤销栈 ---
  undoStack: EditAction[];             // Cmd+Z 撤销用

  // --- 计算属性 ---
  activeItems: () => SensitiveItem[];  // items - removedIds + addedItems
  summaryByType: () => Map<SensitiveType, number>;  // 按类型统计

  // --- 操作 ---
  setItems: (items: SensitiveItem[]) => void;
  appendItems: (items: SensitiveItem[]) => void;  // NER 结果追加
  removeItem: (id: string) => void;    // 取消标记
  addItem: (item: SensitiveItem) => void;  // 手动标记
  overrideStrategy: (id: string, strategy: Strategy) => void;
  undo: () => void;
  resetDetect: () => void;
}

type EditAction =
  | { type: "remove"; item: SensitiveItem }
  | { type: "add"; item: SensitiveItem }
  | { type: "override"; id: string; previous: Strategy };
```

**`useConfigStore`** — 策略配置与词典

```typescript
interface ConfigState {
  // --- 策略 ---
  strategies: Map<SensitiveType, Strategy>;  // 各类型默认策略

  // --- 词典 ---
  dictEntries: DictEntry[];

  // --- 操作 ---
  loadConfig: () => Promise<void>;     // 启动时调用 load_config
  saveConfig: () => Promise<void>;
  updateStrategy: (type: SensitiveType, strategy: Strategy) => void;
  resetToDefault: () => void;
  loadDict: () => Promise<void>;
  saveDict: () => Promise<void>;
  addDictEntry: (entry: DictEntry) => void;
  updateDictEntry: (index: number, entry: DictEntry) => void;
  removeDictEntry: (index: number) => void;
}
```

**store 间协作关系**：

```
useAppStore          useDetectStore          useConfigStore
    │                     │                       │
    │  importFile()       │                       │
    │──────────────────→  │  setItems()           │
    │  setView("preview") │  (规则+词典结果)        │
    │                     │                       │
    │                     │  appendItems()        │
    │                     │  (NER异步结果)          │
    │                     │                       │
    │                     │  activeItems() ←──── strategies
    │                     │  (合并策略计算脱敏预览)    │
    │                     │                       │
    │  setView("result")  │                       │
    │  (脱敏完成)          │                       │
```

**设计要点**：

- `DetectStore` 不直接删除 items，而是用 `removedIds` 标记。撤销时只需从 Set 中移除 ID，不用恢复完整对象
- `itemOverrides` 存储用户在 M3 浮层中对单项的策略修改，未覆盖的项走 `ConfigStore` 的默认策略
- 脱敏执行时，最终策略 = `itemOverrides[id] ?? configStore.strategies[type]`
- P3/P5 的脱敏结果和还原结果是一次性数据，直接存为页面组件的 local state，不放入全局 store

---

## 四、关键技术决策

### 4.1 Excel/CSV 表格渲染

使用 `@tanstack/react-virtual` 实现虚拟滚动：

```
<SpreadsheetView>
├── <thead> 固定表头（headers）
└── <div> 虚拟滚动容器
    └── virtualizer.getVirtualItems() → 只渲染可见行
        └── <tr> 每行
            └── <td> 每个单元格
                └── <HighlightedText />  // 行内高亮敏感项
```

**高亮渲染策略**：每个单元格拿到 `(row, col)` 后，从 `activeItems()` 中筛选该位置的敏感项，按 `start` 排序，将单元格文本切割为 `[普通文本, 高亮片段, 普通文本, ...]` 交替渲染。

`HighlightedText` 是核心复用组件，接收 `text: string` + `items: SensitiveItem[]`，输出带颜色标记的 JSX。Excel 和 Word 共用。

### 4.2 Word 段落渲染

```
<DocumentView>
└── <div> 虚拟滚动容器（段落数多时也虚拟化）
    └── paragraphs.map(p =>
        <ParagraphBlock style={p.style}>
            <HighlightedText text={p.text} items={该段落的敏感项} />
        </ParagraphBlock>
    )
```

段落样式映射：Rust 解析 Word 时提取段落的 `style`，前端映射为对应的 TailwindCSS 类名：

| Word style | TailwindCSS |
|------------|-------------|
| heading1 | `text-2xl font-bold` |
| heading2 | `text-xl font-semibold` |
| normal | `text-base` |
| listParagraph | `text-base pl-6 list-disc` |

不追求完美还原 Word 排版，保证可读性和高亮标记即可。

### 4.3 P3/P5 同步滚动

左右两个面板各用一个 `div ref`，监听任一面板的 `scroll` 事件，同步设置另一面板的 `scrollTop`：

```typescript
const syncScroll = (source: HTMLDivElement, target: HTMLDivElement) => {
  target.scrollTop = source.scrollTop;
};
```

加一个 `isSyncing` 标志位防止循环触发。两侧内容结构相同（同源 FileContent，只是文本值不同），高度天然一致，不需要额外对齐。

### 4.4 撤销/重做（P2 Cmd+Z）

基于 `DetectStore.undoStack` 实现：

- 用户取消标记 → push `{ type: "remove", item }` → `removedIds.add(id)`
- 用户手动标记 → push `{ type: "add", item }` → `addedItems.push(item)`
- 用户改单项策略 → push `{ type: "override", id, previous }` → `itemOverrides.set(id, strategy)`
- Cmd+Z → pop 栈顶，反向执行

v0.1 只做撤销，不做重做。栈深度限制 50 步。

### 4.5 Toast 通知

使用 `react-hot-toast`，全局配置：

```typescript
<Toaster
  position="top-right"
  toastOptions={{
    success: { duration: 3000 },   // 成功自动消失
    error: { duration: Infinity },  // 错误需手动关闭
  }}
/>
```

对齐 PAGE-FLOW 中"成功 3 秒消失，错误需手动关闭"的规范。

### 4.6 键盘快捷键

使用前端 `useEffect` 监听 `keydown`，不引入全局快捷键插件：

| 快捷键 | 实现方式 | 说明 |
|--------|---------|------|
| Cmd+O | 前端 keydown → 调用 Tauri 文件对话框 | 仅 P1 生效 |
| Cmd+S | 前端 keydown → 触发导出流程 | 仅 P3/P5 生效 |
| Cmd+Z | 前端 keydown → `detectStore.undo()` | 仅 P2 生效 |
| Esc | 前端 keydown → 关闭当前浮层/抽屉/面板 | 全局 |

根据当前 `view` 判断快捷键是否生效，避免页面间冲突。

### 4.7 敏感类型颜色映射

每种 SensitiveType 分配固定颜色，用于高亮标记和汇总条标签：

| 类型 | 颜色 | TailwindCSS |
|------|------|-------------|
| Phone | 蓝色 | `bg-blue-100 text-blue-800` |
| IdCard | 红色 | `bg-red-100 text-red-800` |
| BankCard | 橙色 | `bg-orange-100 text-orange-800` |
| Email | 紫色 | `bg-purple-100 text-purple-800` |
| IpAddress | 灰色 | `bg-gray-100 text-gray-800` |
| LandlinePhone | 青色 | `bg-cyan-100 text-cyan-800` |
| LicensePlate | 黄色 | `bg-yellow-100 text-yellow-800` |
| CreditCode | 粉色 | `bg-pink-100 text-pink-800` |
| PersonName | 绿色 | `bg-green-100 text-green-800` |
| OrgName | 靛蓝 | `bg-indigo-100 text-indigo-800` |
| Address | 棕色 | `bg-amber-100 text-amber-800` |
| Title | 石灰 | `bg-lime-100 text-lime-800` |
| Custom | 蓝灰 | `bg-slate-100 text-slate-800` |

---

## 五、分阶段实施计划

原则：每个阶段交付一个可端到端测试的里程碑，纵向切片优先于横向分层。

### Phase 1：数据模型修正 + 核心正向链路骨架

**目标**：拖入 Excel/CSV → 规则识别高亮 → 能看到 P2 预览

**Rust 后端**：
- 修正 `SensitiveType` 枚举（7→13 种）
- 重构 `FileContent` 为枚举（Spreadsheet / Document）
- 完善 `regex_engine`（补齐 IpAddress、LandlinePhone、LicensePlate、CreditCode 4 条正则）
- 调整 `import_file` 返回新 FileContent 结构
- 调整 `detect_by_regex` 适配新模型

**前端**：
- 修正 `types/index.ts` 对齐 Rust 模型
- 拆分 store（appStore / detectStore / configStore）
- `App.tsx` 改为 view 驱动的视图切换
- `TopBar` 全局顶栏（标题、返回按钮、词典/历史入口占位）
- `HomePage`（P1）：增强现有 FileDropZone，暂无 RecentTasks
- `PreviewPage`（P2）：SummaryBar + SpreadsheetView + HighlightedText + BottomBar
- 安装 `@tanstack/react-virtual`、`react-hot-toast`、`@headlessui/react`
- Toast 全局配置

**交付物**：拖入一个 CSV/Excel 文件 → 规则引擎识别 → P2 表格高亮展示 + 汇总条统计

---

### Phase 2：脱敏执行 + 导出 + 任务保存

**目标**：完整走通"导入 → 识别 → 脱敏 → 对比 → 导出"正向流程

**Rust 后端**：
- 完善 `mask.rs`（全部 13 种类型的掩码规则）
- 完善 `replace.rs`（假数据生成：假人名库、假机构名库等）
- 完善 `generalize.rs`（地址泛化、年龄泛化等）
- `apply_desensitize` 返回新的 `DesensitizeResult`（含映射表 + 汇总）
- 一致性替换逻辑：同一文本全文统一映射
- `export_file` 命令：写回 xlsx/csv 格式
- 新增 `save_task` 命令：将 TaskRecord 写入 `tasks/` 目录
- 新增 `TaskRecord` / `MappingEntry` 模型

**前端**：
- `ResultPage`（P3）：DiffPanel 左右对比 + 同步滚动 + ResultFooter
- `ContentRenderer` 抽象组件（P2/P3 共用）
- 导出文件流程：调用 Tauri 保存对话框 → `export_file` → `save_task` → Toast 成功
- "返回修改"：从 P3 回到 P2，保留 detectStore 状态

**交付物**：完整正向脱敏流程可用，导出的 Excel/CSV 格式正确，任务自动保存到本地

---

### Phase 3：历史任务 + 反向还原

**目标**：完整走通"查看历史 → 导入 AI 结果 → 还原 → 导出"反向流程

**Rust 后端**：
- `list_tasks` 命令：读取 `tasks/` 目录，解析所有 JSON，按时间倒序
- `delete_task` 命令：删除指定 JSON 文件
- `restore_file` 命令：加载映射表 → 导入新文件 → 反向替换 Replace 类条目
- `export_restored_file` 命令

**前端**：
- `HistoryPage`（P4）：TaskList 卡片列表 + 展开映射摘要 + 删除确认弹窗 + 空状态
- `RestorePage`（P5）：复用 DiffPanel + RestoreFooter
- `RecentTasks` 组件：P1 首页显示最近 2-3 条任务
- P1 卡片上的 [还原] 快捷入口
- 删除确认弹窗（headlessui Dialog）

**交付物**：脱敏 → 查看历史 → 还原，完整闭环可用

---

### Phase 4：NER 异步识别 + 词典管理 + 策略配置

**目标**：三层识别引擎全部就位，所有配置面板可用

**Rust 后端**：
- `ner_engine` 集成 ONNX Runtime（`ort` crate）
- NER 模型加载 + 推理（PersonName / OrgName / Address / Title）
- `detect_by_dict` 从 `dict.json` 读取词典执行匹配
- `load_dict` / `save_dict` 命令完善

**前端**：
- P2 异步 NER 流程：规则结果先渲染 → 调用 `detect_by_ner` → `appendItems` 追加高亮 → SummaryBar 状态从 "NER识别中..." 变为消失
- `DictDrawer`（M1）：右侧抽屉，词条列表 + 添加/编辑/删除表单，关闭时保存 + 重新触发词典匹配
- `StrategyPanel`（M2）：右侧面板，按类型配置策略下拉 + 掩码保留位数 + 保存/恢复默认
- `SensitivePopover`（M3）：点击高亮项弹出浮层，显示详情 + 切换策略 + 实时预览 + 取消标记
- `configStore` 的 `loadConfig`/`saveConfig` 调通

**交付物**：三层引擎完整识别，用户可管理词典、配置策略、逐项微调

---

### Phase 5：Word 支持 + 交互打磨

**目标**：补齐 Word 格式支持，完善交互体验，达到发布标准

**Rust 后端**：
- `word.rs` 解析器完善：提取段落文本 + 样式标签
- Word 导出：写回 docx 格式（保持基本排版）
- 错误处理完善（密码保护检测、损坏文件检测）

**前端**：
- `DocumentView` 组件：段落渲染 + 样式映射 + HighlightedText 复用
- P2 手动标记功能：选中文本 → 浮动工具条 → 选择敏感类型
- 撤销栈（Cmd+Z）实现
- 键盘快捷键（Cmd+O/S/Z、Esc）
- 拖拽视觉反馈（边框高亮变色、"松开导入"提示）
- 加载态：文件解析骨架屏、脱敏执行进度条
- 按类型筛选/全选/取消（SummaryBar 复选框）
- 跨平台测试（macOS ARM/Intel）

**交付物**：v0.1 功能完整，可打包发布

---

### 阶段依赖关系

```
Phase 1 ──→ Phase 2 ──→ Phase 3
                │
                └──→ Phase 4 ──→ Phase 5
```

Phase 3（历史/还原）和 Phase 4（NER/词典/策略）可并行开发，都依赖 Phase 2 的脱敏核心。Phase 5 依赖 Phase 4 完成后再整体打磨。

---

## 附录：新增依赖清单

### 前端 npm 包

| 包名 | 用途 | 阶段 |
|------|------|------|
| `@tanstack/react-virtual` | Excel/CSV 虚拟滚动表格 | Phase 1 |
| `react-hot-toast` | Toast 通知 | Phase 1 |
| `@headlessui/react` | 弹窗、抽屉、下拉等无样式 UI 组件 | Phase 1 |

### Rust crate（已在 Cargo.toml 或待添加）

| crate | 用途 | 阶段 |
|-------|------|------|
| `ort` | ONNX Runtime NER 推理 | Phase 4 |
| `rand` | 任务 ID 随机后缀、假数据生成 | Phase 2 |
| `chrono` | 时间格式化（TaskRecord.created_at） | Phase 2 |
