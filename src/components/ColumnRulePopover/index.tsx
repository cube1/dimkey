import { useState, useEffect, useRef, useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { ColumnInference, ColumnRule, Strategy, StrategyType } from "../../types";
import {
  SENSITIVE_TYPE_CONFIG,
  STRATEGY_LABELS,
  getAllowedStrategies,
} from "../../types";
import { useConfigStore } from "../../stores/configStore";

interface ColumnRulePopoverProps {
  col: number;
  inference: ColumnInference | null;
  currentRule: ColumnRule | null;
  onConfirm: (rule: ColumnRule) => void;
  onSkip: () => void;
  onClose: () => void;
  anchorRect: DOMRect;
}

const TYPE_KEYS = Object.keys(SENSITIVE_TYPE_CONFIG).filter((k) => k !== "Custom");

function getStrategyType(strategy: Strategy): StrategyType {
  if (typeof strategy === "string") return strategy as StrategyType;
  if ("Mask" in strategy) return "Mask";
  if ("Replace" in strategy) return "Replace";
  return "Mask";
}

export function ColumnRulePopover({
  col,
  inference,
  currentRule,
  onConfirm,
  onSkip,
  onClose,
  anchorRect,
}: ColumnRulePopoverProps) {
  const { t } = useTranslation();
  const popoverRef = useRef<HTMLDivElement>(null);
  const replaceStyle = useConfigStore((s) => s.replaceStyle);

  // 初始化选中的类型
  const initialType = currentRule?.sensitive_type
    ?? (inference?.inferred_type
      ? (typeof inference.inferred_type === "string" ? inference.inferred_type : "Custom")
      : "Phone");

  const [selectedType, setSelectedType] = useState(initialType);
  const [selectedStrategy, setSelectedStrategy] = useState<StrategyType>(
    currentRule ? getStrategyType(currentRule.strategy) : "Mask"
  );
  const [keepPrefix, setKeepPrefix] = useState(
    currentRule?.strategy && typeof currentRule.strategy === "object" && "Mask" in currentRule.strategy
      ? currentRule.strategy.Mask.keep_prefix
      : 3
  );
  const [keepSuffix, setKeepSuffix] = useState(
    currentRule?.strategy && typeof currentRule.strategy === "object" && "Mask" in currentRule.strategy
      ? currentRule.strategy.Mask.keep_suffix
      : 4
  );
  const [reversible, setReversible] = useState(currentRule?.reversible ?? false);

  const allowedStrategies = getAllowedStrategies(selectedType);

  // 类型变更时重置策略为第一个允许的
  useEffect(() => {
    const allowed = getAllowedStrategies(selectedType);
    if (!allowed.includes(selectedStrategy)) {
      setSelectedStrategy(allowed[0]);
    }
  }, [selectedType]);

  // 点击外部关闭
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [onClose]);

  // Esc 关闭
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);

  const handleConfirm = useCallback(() => {
    let strategy: Strategy;
    if (selectedStrategy === "Mask") {
      strategy = { Mask: { keep_prefix: keepPrefix, keep_suffix: keepSuffix } };
    } else if (selectedStrategy === "Generalize") {
      strategy = "Generalize";
    } else {
      strategy = { Replace: { style: replaceStyle } };
    }

    onConfirm({
      col,
      sensitive_type: selectedType,
      strategy,
      reversible: selectedStrategy === "Mask" ? false : reversible,
      sheet_index: 0,
    });
  }, [col, selectedType, selectedStrategy, keepPrefix, keepSuffix, reversible, replaceStyle, onConfirm]);

  // 计算位置
  const top = anchorRect.bottom + 4;
  const left = Math.max(8, anchorRect.left);

  return (
    <div
      ref={popoverRef}
      className="fixed z-50 bg-white rounded-xl shadow-float border border-slate-200 p-4 w-72 animate-fade-in"
      style={{ top, left }}
    >
      {/* 列头信息 */}
      <div className="text-sm font-medium text-slate-700 mb-3">
        {t("comparison.columnHeader", { col: col + 1, header: inference?.header || `Col ${col + 1}` })}
        {inference && inference.inferred_type && (
          <span className="ml-2 text-xs text-amber-600">
            {t("comparison.inferenceConfidence", { confidence: (inference.confidence * 100).toFixed(0) })}
          </span>
        )}
      </div>

      {/* 敏感类型选择 */}
      <div className="mb-3">
        <label className="block text-xs text-slate-500 mb-1">{t("comparison.sensitiveType")}</label>
        <select
          value={selectedType}
          onChange={(e) => setSelectedType(e.target.value)}
          className="w-full px-2 py-1.5 text-sm border border-slate-300 rounded-md focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-400"
        >
          {TYPE_KEYS.map((key) => (
            <option key={key} value={key}>
              {SENSITIVE_TYPE_CONFIG[key].label}
            </option>
          ))}
        </select>
      </div>

      {/* 脱敏策略选择 */}
      <div className="mb-3">
        <label className="block text-xs text-slate-500 mb-1">{t("comparison.desensitizeStrategy")}</label>
        <div className="flex gap-2">
          {allowedStrategies.map((st) => (
            <button
              key={st}
              onClick={() => setSelectedStrategy(st)}
              className={`flex-1 px-2 py-1.5 text-xs rounded-md border transition-colors ${
                selectedStrategy === st
                  ? "bg-primary-50 border-primary-300 text-primary-700"
                  : "bg-white border-slate-300 text-slate-600 hover:bg-slate-50"
              }`}
            >
              {STRATEGY_LABELS[st]}
            </button>
          ))}
        </div>
      </div>

      {/* Mask 参数 */}
      {selectedStrategy === "Mask" && (
        <div className="mb-3 flex gap-3">
          <div className="flex-1">
            <label className="block text-xs text-slate-500 mb-1">{t("comparison.keepPrefix")}</label>
            <input
              type="number"
              min={0}
              max={10}
              value={keepPrefix}
              onChange={(e) => setKeepPrefix(Number(e.target.value))}
              className="w-full px-2 py-1 text-sm border border-slate-300 rounded-md focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-400"
            />
          </div>
          <div className="flex-1">
            <label className="block text-xs text-slate-500 mb-1">{t("comparison.keepSuffix")}</label>
            <input
              type="number"
              min={0}
              max={10}
              value={keepSuffix}
              onChange={(e) => setKeepSuffix(Number(e.target.value))}
              className="w-full px-2 py-1 text-sm border border-slate-300 rounded-md focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-400"
            />
          </div>
        </div>
      )}

      {/* 可还原选项 */}
      <div className="mb-4">
        <label className={`flex items-center gap-2 text-sm ${
          selectedStrategy === "Mask" ? "text-slate-400" : "text-slate-700"
        }`}>
          <input
            type="checkbox"
            checked={selectedStrategy === "Mask" ? false : reversible}
            disabled={selectedStrategy === "Mask"}
            onChange={(e) => setReversible(e.target.checked)}
            className="rounded border-slate-300"
          />
          {t("comparison.reversible")}
        </label>
        {selectedStrategy === "Mask" && (
          <p className="text-xs text-slate-400 mt-1 ml-6">{t("comparison.maskNotReversible")}</p>
        )}
      </div>

      {/* 操作按钮 */}
      <div className="flex items-center justify-between">
        <button
          onClick={onSkip}
          className="text-sm text-slate-500 hover:text-slate-700 transition-colors"
        >
          {t("comparison.skipColumn")}
        </button>
        <button
          onClick={handleConfirm}
          className="px-4 py-1.5 bg-primary-600 text-white text-sm font-medium rounded-md hover:bg-primary-700 shadow-sm shadow-primary-600/20 transition-colors"
        >
          {t("common.confirm")}
        </button>
      </div>
    </div>
  );
}
