# 技术架构

## 整体架构

```
┌─────────────────────────────────────────┐
│          React 前端 (WebView)            │
│  ┌──────────┐ ┌────────┐ ┌───────────┐  │
│  │FileDropZone│ │Preview │ │ResultView │  │
│  └──────────┘ └────────┘ └───────────┘  │
│  ┌──────────────┐ ┌─────────────────┐   │
│  │StrategyConfig│ │ DictManager     │   │
│  └──────────────┘ └─────────────────┘   │
└──────────────┬──────────────────────────┘
               │ Tauri IPC (invoke)
┌──────────────▼──────────────────────────┐
│          Rust 后端 (Native Process)      │
│                                         │
│  ┌─── Parser ───┐  ┌─── Engine ──────┐  │
│  │ excel.rs     │  │ regex_engine.rs │  │
│  │ word.rs      │  │ ner_engine.rs   │  │
│  │ csv (内置)    │  │ dict_engine.rs  │  │
│  └──────────────┘  └────────────────┘  │
│                                         │
│  ┌─── Desensitizer ─┐  ┌── Config ──┐  │
│  │ mask.rs          │  │ 本地JSON    │  │
│  │ replace.rs       │  │ 词典存储    │  │
│  │ generalize.rs    │  └────────────┘  │
│  └──────────────────┘                   │
│                                         │
│  ┌─── Models ─────┐                    │
│  │ NER ONNX模型    │                    │
│  │ (resources/)    │                    │
│  └────────────────┘                     │
└─────────────────────────────────────────┘
```

## Tauri IPC 命令设计

### 文件操作

```rust
#[tauri::command]
async fn import_file(file_path: String) -> Result<FileContent, String>
// 解析文件，返回结构化内容

#[tauri::command]
async fn export_file(content: DesensitizedContent, output_path: String) -> Result<(), String>
// 导出脱敏后的文件
```

### 敏感识别

```rust
#[tauri::command]
async fn detect_by_rules(content: FileContent) -> Result<Vec<SensitiveItem>, String>
// 规则引擎识别（同步快速返回）

#[tauri::command]
async fn detect_by_ner(content: FileContent) -> Result<Vec<SensitiveItem>, String>
// NER 模型识别（异步，较慢）

#[tauri::command]
async fn detect_by_dict(content: FileContent, dict: Vec<DictEntry>) -> Result<Vec<SensitiveItem>, String>
// 词典匹配
```

### 脱敏执行

```rust
#[tauri::command]
async fn desensitize(
    content: FileContent,
    items: Vec<SensitiveItem>,
    strategies: HashMap<SensitiveType, Strategy>
) -> Result<DesensitizedContent, String>
// 执行脱敏，返回脱敏后内容
```

### 配置管理

```rust
#[tauri::command]
async fn load_config() -> Result<AppConfig, String>

#[tauri::command]
async fn save_config(config: AppConfig) -> Result<(), String>

#[tauri::command]
async fn load_dict() -> Result<Vec<DictEntry>, String>

#[tauri::command]
async fn save_dict(entries: Vec<DictEntry>) -> Result<(), String>
```

## 核心数据模型

```rust
// 敏感信息类型
enum SensitiveType {
    Phone,          // 手机号
    IdCard,         // 身份证
    BankCard,       // 银行卡
    Email,          // 邮箱
    IpAddress,      // IP地址
    LandlinePhone,  // 固定电话
    LicensePlate,   // 车牌号
    CreditCode,     // 统一社会信用代码
    PersonName,     // 人名 (NER)
    OrgName,        // 机构名 (NER)
    Address,        // 地址 (NER)
    Title,          // 职位 (NER)
    Custom,         // 自定义词典
}

// 识别到的敏感信息
struct SensitiveItem {
    id: String,                 // 唯一标识
    text: String,               // 原始文本
    sensitive_type: SensitiveType,
    source: DetectSource,       // Rule / NER / Dict
    confidence: f32,            // 置信度 0-1
    position: Position,         // 在文件中的位置
}

// 脱敏策略
enum Strategy {
    Mask { keep_prefix: usize, keep_suffix: usize },  // 掩码
    Replace,                                            // 替换为假数据
    Generalize { level: GeneralizeLevel },              // 泛化
    Hash,                                               // 哈希
}

// 文件内容（统一抽象）
enum FileContent {
    Spreadsheet { sheets: Vec<Sheet> },    // Excel/CSV
    Document { paragraphs: Vec<Paragraph> }, // Word
}
```

## 渐进式识别流程

```
前端 invoke("import_file")
  → Rust 解析文件 → 返回 FileContent
  
前端 invoke("detect_by_rules")
  → Rust 正则扫描 → 毫秒级返回 → 前端立即渲染高亮
  
前端 invoke("detect_by_ner")  // 异步，不阻塞UI
  → Rust 加载ONNX模型 → 推理 → 秒级返回 → 前端追加高亮
  
前端 invoke("detect_by_dict")  // 如果有自定义词典
  → Rust 词典匹配 → 即时返回 → 前端追加高亮
```

## NER 模型选型建议

| 方案 | 模型大小 | 推理速度 | 精度 |
|------|---------|---------|------|
| chinese-bert-wwm-ner (量化) | ~20MB | ~2s/万字 | 较高 |
| onnx-msra-ner (轻量) | ~10MB | ~1s/万字 | 中等 |

优先选择量化后的BERT系列NER模型，平衡精度和体积。

## 本地存储

```
~/Library/Application Support/com.desensitize-tool/  (macOS)
%APPDATA%/com.desensitize-tool/                      (Windows)
├── config.json          # 脱敏策略配置
├── dict.json            # 自定义词典
└── preferences.json     # 界面偏好
```

## 开发分期建议

### Phase 1: 骨架搭建（1周）
- Tauri v2 项目初始化
- React 基础页面（文件拖入 → 空白预览）
- Rust 侧文件解析（先做CSV）
- 前后端 IPC 通路跑通

### Phase 2: 规则引擎（1周）
- 实现正则引擎（8种规则）
- 前端高亮标记展示
- 脱敏策略配置UI

### Phase 3: 脱敏执行（1周）
- 掩码/替换/泛化算法
- 一致性替换（映射表）
- 文件导出（保持格式）
- 前后对比预览

### Phase 4: NER + 词典（1周）
- ONNX 模型集成
- 异步识别 + 渐进式展示
- 自定义词典管理
- Excel/Word 解析完善

### Phase 5: 打磨（1周）
- 配置持久化
- 异常处理完善
- 跨平台测试
- 打包发布
