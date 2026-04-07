#!/usr/bin/env python3
"""
导出 NER 模型为 ONNX INT8 格式，输出到 .ner_cache/<name>/

使用方法:
  pip install optimum[onnxruntime] transformers torch huggingface_hub
  python scripts/export_ner_model.py chinese
  python scripts/export_ner_model.py multilingual
"""

import json
import shutil
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
CACHE_ROOT = REPO_ROOT / ".ner_cache"

MODELS = {
    "chinese": {
        "hf_id": "shibing624/bert4ner-base-chinese",
        # BERT 模型：tokenizer 从模型自身保存
        "tokenizer_source": "self",
    },
    "multilingual": {
        "hf_id": "Davlan/xlm-roberta-base-ner-hrl",
        # XLM-R：transformers 4.49+ 转 fast tokenizer 有 tiktoken bug，
        # 直接从 xlm-roberta-base 借用 tokenizer.json（共享词表）
        "tokenizer_source": "xlm-roberta-base",
    },
    "distilbert-ner": {
        "hf_id": "dslim/distilbert-NER",
        "tokenizer_source": "self",
    },
}


def export_model(name: str):
    if name not in MODELS:
        raise SystemExit(f"未知模型: {name}，可选: {list(MODELS.keys())}")

    from optimum.onnxruntime import ORTModelForTokenClassification, ORTQuantizer
    from optimum.onnxruntime.configuration import AutoQuantizationConfig
    from huggingface_hub import hf_hub_download

    cfg = MODELS[name]
    hf_id = cfg["hf_id"]
    target_dir = CACHE_ROOT / name
    export_tmp = CACHE_ROOT / f"_tmp_{name}_fp32"
    int8_tmp = CACHE_ROOT / f"_tmp_{name}_int8"

    print(f"[{name}] 导出 {hf_id} → ONNX...")
    model = ORTModelForTokenClassification.from_pretrained(hf_id, export=True)
    model.save_pretrained(export_tmp)

    print(f"[{name}] INT8 动态量化...")
    quantizer = ORTQuantizer.from_pretrained(export_tmp)
    qconfig = AutoQuantizationConfig.avx512_vnni(is_static=False, per_channel=False)
    quantizer.quantize(save_dir=int8_tmp, quantization_config=qconfig)

    target_dir.mkdir(parents=True, exist_ok=True)

    quantized = int8_tmp / "model_quantized.onnx"
    if not quantized.exists():
        quantized = int8_tmp / "model.onnx"
    shutil.copy2(quantized, target_dir / "model.onnx")
    print(f"[{name}] 模型已复制: {target_dir / 'model.onnx'}")

    if cfg["tokenizer_source"] == "self":
        # BERT：用 AutoTokenizer 保存 fast tokenizer（会生成 tokenizer.json）
        from transformers import AutoTokenizer
        tok = AutoTokenizer.from_pretrained(hf_id, use_fast=True)
        tok.save_pretrained(target_dir)
        # 清理 BERT 保存的多余文件，只保留 tokenizer.json
        for f in target_dir.iterdir():
            if f.name not in {"model.onnx", "tokenizer.json", "id2label.json"}:
                f.unlink()
        if not (target_dir / "tokenizer.json").exists():
            raise SystemExit(f"[{name}] tokenizer.json 生成失败")
    else:
        # XLM-R：从基础模型下载 tokenizer.json
        src = cfg["tokenizer_source"]
        tok_path = hf_hub_download(src, "tokenizer.json")
        shutil.copy2(tok_path, target_dir / "tokenizer.json")
    print(f"[{name}] 分词器已复制: {target_dir / 'tokenizer.json'}")

    # id2label.json — 从 config.json 提取
    config = json.loads((export_tmp / "config.json").read_text())
    id2label = config.get("id2label", {})
    (target_dir / "id2label.json").write_text(
        json.dumps(id2label, indent=2, ensure_ascii=False)
    )
    print(f"[{name}] 标签映射已写入: {target_dir / 'id2label.json'}")

    # model_config.json — 自动生成模型配置
    # 检测标注方案
    labels = list(id2label.values())
    has_b_prefix = any(l.startswith("B-") for l in labels)
    tagging_scheme = "bio" if has_b_prefix else "ionly"

    # 提取实体标签（去掉 B-/I- 前缀，去重）
    entity_labels = set()
    for l in labels:
        if l.startswith("B-") or l.startswith("I-"):
            entity_labels.add(l[2:])

    # 默认标签映射
    default_map = {
        "PER": "PersonName", "PERSON": "PersonName",
        "ORG": "OrgName", "ORGANIZATION": "OrgName",
        "LOC": "Address", "LOCATION": "Address", "GPE": "Address",
        "TITLE": "Title",
    }

    label_map = {}
    for entity in sorted(entity_labels):
        label_map[entity] = default_map.get(entity)

    model_config = {
        "name": name,
        "tagging_scheme": tagging_scheme,
        "label_map": label_map,
    }
    (target_dir / "model_config.json").write_text(
        json.dumps(model_config, indent=2, ensure_ascii=False)
    )
    print(f"[{name}] 模型配置已写入: {target_dir / 'model_config.json'}")

    print(f"[{name}] 清理临时目录...")
    shutil.rmtree(export_tmp, ignore_errors=True)
    shutil.rmtree(int8_tmp, ignore_errors=True)
    print(f"[{name}] 完成！")


def main():
    if len(sys.argv) != 2:
        raise SystemExit(f"用法: python {sys.argv[0]} <{'|'.join(MODELS.keys())}>")
    export_model(sys.argv[1])


if __name__ == "__main__":
    main()
