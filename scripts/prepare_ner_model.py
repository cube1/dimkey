#!/usr/bin/env python3
"""导出 Davlan/xlm-roberta-base-ner-hrl 为 ONNX INT8 格式"""
import json
import shutil
from pathlib import Path

def main():
    from optimum.onnxruntime import ORTModelForTokenClassification, ORTQuantizer
    from optimum.onnxruntime.configuration import AutoQuantizationConfig
    from transformers import AutoTokenizer

    model_name = "Davlan/xlm-roberta-base-ner-hrl"
    export_dir = Path("onnx_export")
    int8_dir = Path("onnx_export_int8")
    target_dir = Path("src-tauri/resources/ner")

    # 1. 导出 ONNX
    print(f"导出 {model_name} → ONNX...")
    model = ORTModelForTokenClassification.from_pretrained(model_name, export=True)
    tokenizer = AutoTokenizer.from_pretrained(model_name)
    model.save_pretrained(export_dir)
    tokenizer.save_pretrained(export_dir)

    # 2. INT8 动态量化
    print("INT8 动态量化...")
    quantizer = ORTQuantizer.from_pretrained(export_dir)
    qconfig = AutoQuantizationConfig.avx512_vnni(is_static=False, per_channel=False)
    quantizer.quantize(save_dir=int8_dir, quantization_config=qconfig)

    # 3. 复制到项目目录
    target_dir.mkdir(parents=True, exist_ok=True)

    # model.onnx
    quantized = int8_dir / "model_quantized.onnx"
    if not quantized.exists():
        quantized = int8_dir / "model.onnx"
    shutil.copy2(quantized, target_dir / "model.onnx")
    print(f"模型已复制: {target_dir / 'model.onnx'}")

    # tokenizer.json
    shutil.copy2(export_dir / "tokenizer.json", target_dir / "tokenizer.json")
    print(f"分词器已复制: {target_dir / 'tokenizer.json'}")

    # id2label.json — 从 config.json 提取
    config = json.loads((export_dir / "config.json").read_text())
    id2label = config.get("id2label", {})
    (target_dir / "id2label.json").write_text(json.dumps(id2label, indent=2, ensure_ascii=False))
    print(f"标签映射已写入: {target_dir / 'id2label.json'}")

    # 清理
    print("清理临时目录...")
    shutil.rmtree(export_dir, ignore_errors=True)
    shutil.rmtree(int8_dir, ignore_errors=True)
    print("完成!")

if __name__ == "__main__":
    main()
