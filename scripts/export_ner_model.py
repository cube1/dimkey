#!/usr/bin/env python3
"""
导出 shibing624/bert4ner-base-chinese 为 ONNX + INT8 量化
输出到 src-tauri/resources/ner/ 目录

使用方法:
  pip install optimum[onnxruntime] transformers torch
  python scripts/export_ner_model.py
"""

import json
import shutil
from pathlib import Path

MODEL_NAME = "shibing624/bert4ner-base-chinese"
OUTPUT_DIR = Path(__file__).parent.parent / "src-tauri" / "resources" / "ner"
TEMP_DIR = Path(__file__).parent / "_ner_export_tmp"


def export_onnx():
    """导出 FP32 ONNX 模型和 tokenizer"""
    from optimum.onnxruntime import ORTModelForTokenClassification
    from transformers import AutoTokenizer

    print(f"正在从 HuggingFace 下载并导出: {MODEL_NAME}")
    model = ORTModelForTokenClassification.from_pretrained(
        MODEL_NAME, export=True
    )
    model.save_pretrained(str(TEMP_DIR))

    # 单独保存 tokenizer（包含 vocab.txt）
    tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)
    tokenizer.save_pretrained(str(TEMP_DIR))
    print(f"FP32 ONNX + tokenizer 已导出到: {TEMP_DIR}")


def quantize():
    """INT8 动态量化"""
    from optimum.onnxruntime import ORTQuantizer
    from optimum.onnxruntime.configuration import AutoQuantizationConfig

    quantizer = ORTQuantizer.from_pretrained(str(TEMP_DIR))
    config = AutoQuantizationConfig.avx512_vnni(is_static=False, per_channel=False)
    quantizer.quantize(save_dir=str(TEMP_DIR / "quantized"), quantization_config=config)
    print("INT8 量化完成")


def build_id2label():
    """从模型 config 提取 id2label 映射"""
    config_path = TEMP_DIR / "config.json"
    with open(config_path) as f:
        config = json.load(f)

    id2label = config.get("id2label", {})
    print(f"标签数量: {len(id2label)}")
    print(f"标签列表: {list(id2label.values())}")
    return id2label


def copy_to_resources(id2label: dict):
    """将所需文件复制到 resources/ner/"""
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    # 优先使用量化版本
    quantized_model = TEMP_DIR / "quantized" / "model_quantized.onnx"
    fp32_model = TEMP_DIR / "model.onnx"

    if quantized_model.exists():
        shutil.copy(quantized_model, OUTPUT_DIR / "model.onnx")
        size_mb = quantized_model.stat().st_size / 1024 / 1024
        print(f"已复制量化模型 ({size_mb:.1f} MB)")
    else:
        shutil.copy(fp32_model, OUTPUT_DIR / "model.onnx")
        size_mb = fp32_model.stat().st_size / 1024 / 1024
        print(f"已复制 FP32 模型 ({size_mb:.1f} MB)（量化失败，回退）")

    # 复制词表
    shutil.copy(TEMP_DIR / "vocab.txt", OUTPUT_DIR / "vocab.txt")
    print("已复制 vocab.txt")

    # 写入 id2label.json
    with open(OUTPUT_DIR / "id2label.json", "w", encoding="utf-8") as f:
        json.dump(id2label, f, ensure_ascii=False, indent=2)
    print("已写入 id2label.json")


def cleanup():
    """清理临时文件"""
    if TEMP_DIR.exists():
        shutil.rmtree(TEMP_DIR)
        print("已清理临时文件")


def main():
    print("=" * 50)
    print("NER 模型导出工具")
    print(f"模型: {MODEL_NAME}")
    print(f"输出: {OUTPUT_DIR}")
    print("=" * 50)

    try:
        export_onnx()
        id2label = build_id2label()
        quantize()
        copy_to_resources(id2label)
        print("\n导出完成！模型文件已放入 src-tauri/resources/ner/")
        print("重新运行 cargo tauri dev 即可加载模型")
    finally:
        cleanup()


if __name__ == "__main__":
    main()
