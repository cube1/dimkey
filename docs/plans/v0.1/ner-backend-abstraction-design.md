# NER 后端抽象设计

## 目标

将 NER 引擎从 ONNX Runtime 硬编码改为可插拔的后端架构，支持后续切换推理引擎（candle、llama.cpp 等），同时先用 shibing624/bert4ner-base-chinese + INT8 量化跑通流程。

## 核心 Trait

```rust
/// 原始实体（后端输出的统一中间格式）
pub struct RawEntity {
    pub text: String,
    pub label: String,      // 原始标签，如 "PER"、"LOC"、"ORG"
    pub start: usize,       // 字符偏移起始
    pub end: usize,         // 字符偏移结束
    pub confidence: f32,
}

/// NER 推理后端 trait
pub trait NerBackend: Send {
    fn detect_text(&mut self, text: &str) -> Result<Vec<RawEntity>, String>;
    fn is_loaded(&self) -> bool;
}
```

设计决策：
- `RawEntity.label` 用 String —— 不同模型标签名不同，映射留给 NerEngine
- `detect_text` 只接受 `&str` —— 后端不需要知道 FileContent 结构
- `Send` 约束 —— 需放进 Mutex 跨线程使用
- 不在 trait 定义 `load` —— 不同后端初始化参数差异大

## NerEngine 统一入口

```rust
pub struct NerEngine {
    backend: Option<Box<dyn NerBackend>>,
    label_map: HashMap<String, SensitiveType>,
}
```

职责：
1. FileContent → 文本片段拆分（所有后端共用）
2. RawEntity → SensitiveItem 映射（标签归一化 + 位置信息填充）

公开接口 `detect(&mut self, content: &FileContent)` 不变，调用方无需改动。

## OnnxBackend 实现

```rust
pub struct OnnxBackend {
    session: Session,
    vocab: HashMap<String, i64>,
    id2label: Vec<String>,
}
```

- `try_load(model_dir) -> Result<Option<Self>, String>` —— None 代表文件缺失，Err 代表加载出错
- BIO 后处理留在 OnnxBackend 内部（BIO 是 BERT token-classification 特有格式）
- 提供 `build_label_map()` 从 id2label.json 自动提取标签映射

## 文件结构

```
engine/
  mod.rs
  ner_engine.rs           // NerEngine + NerBackend trait + RawEntity
  backends/
    mod.rs
    onnx_backend.rs       // OnnxBackend
```

## 初始化流程（lib.rs）

```rust
let ner_dir = resource_dir.join("ner");
let engine = match OnnxBackend::try_load(&ner_dir) {
    Ok(Some(backend)) => {
        let label_map = backend.build_label_map();
        NerEngine::new(Box::new(backend), label_map)
    }
    Ok(None) => NerEngine::degraded(),
    Err(e) => { eprintln!("NER 引擎加载警告: {}", e); NerEngine::degraded() }
};
```

## 不变的部分

- `NerEngineState` 定义
- `detect.rs` 中 `detect_by_ner` 命令
- 前端 `invoke("detect_by_ner")` 调用

## 模型选型

首选 shibing624/bert4ner-base-chinese + INT8 量化：
- F1 95.25%，PER/LOC/ORG 实体类型
- Apache 2.0 许可证
- 简体中文原生训练
- 量化后 ~100MB
