import { useState, useRef, useCallback } from "react";
import { ChevronDown, ChevronRight, Shield } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import type { Strategy, ReplaceStyle } from "../../types";
import { SENSITIVE_TYPE_CONFIG, getAllowedStrategies, STRATEGY_LABELS, REPLACE_STYLE_LABELS } from "../../types";

function getStrategyName(strategy: Strategy): string {
  if (typeof strategy === "string") return strategy;
  if ("Mask" in strategy) return "Mask";
  if ("Replace" in strategy) return "Replace";
  return "Mask";
}

const TYPE_KEYS = Object.keys(SENSITIVE_TYPE_CONFIG).filter((k) => k !== "Custom");

export function RulesSection() {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState(false);
  const wsData = useWorkspaceStore((s) => s.activeWorkspaceData);
  const updateStrategies = useWorkspaceStore((s) => s.updateStrategies);
  const clearConsistencyMappings = useWorkspaceStore((s) => s.clearConsistencyMappings);
  const clearTypeConsistencyMappings = useWorkspaceStore((s) => s.clearTypeConsistencyMappings);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const strategies = wsData?.workspace.strategies || {};
  const enabledTypes = wsData?.workspace.enabled_types || [];

  // 从当前策略中提取替换风格（取第一个 Replace 策略的 style，默认 Fake）
  const currentReplaceStyle: ReplaceStyle = (() => {
    for (const key in strategies) {
      const s = strategies[key];
      if (typeof s === "object" && "Replace" in s) {
        return s.Replace.style;
      }
    }
    return "Fake";
  })();

  const debouncedSave = useCallback(
    (newStrategies: Record<string, Strategy>) => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => {
        updateStrategies(newStrategies);
      }, 500);
    },
    [updateStrategies]
  );

  const handleStrategyChange = async (typeKey: string, value: string) => {
    let strategy: Strategy;
    if (value === "Mask") {
      strategy = { Mask: { keep_prefix: 3, keep_suffix: 4 } };
    } else if (value === "Generalize") {
      strategy = "Generalize";
    } else {
      strategy = { Replace: { style: currentReplaceStyle } };
    }
    const newStrategies = { ...strategies, [typeKey]: strategy };
    // 乐观更新 store
    if (wsData) {
      useWorkspaceStore.setState({
        activeWorkspaceData: {
          ...wsData,
          workspace: { ...wsData.workspace, strategies: newStrategies },
        },
      });
    }
    // 清除该类型的一致性映射（确保重新脱敏用新策略）
    await clearTypeConsistencyMappings(typeKey);
    debouncedSave(newStrategies);
  };

  /** 替换风格切换（同时清除一致性映射） */
  const handleReplaceStyleChange = async (style: ReplaceStyle) => {
    const newStrategies = { ...strategies };
    for (const key in newStrategies) {
      const s = newStrategies[key];
      if (typeof s === "object" && "Replace" in s) {
        newStrategies[key] = { Replace: { style } };
      }
    }
    // 乐观更新 store
    if (wsData) {
      useWorkspaceStore.setState({
        activeWorkspaceData: {
          ...wsData,
          workspace: { ...wsData.workspace, strategies: newStrategies },
        },
      });
    }
    await updateStrategies(newStrategies);
    await clearConsistencyMappings();
  };

  const handleMaskParamChange = async (
    typeKey: string,
    field: "keep_prefix" | "keep_suffix",
    val: number
  ) => {
    const current = strategies[typeKey];
    if (typeof current === "object" && "Mask" in current) {
      const strategy: Strategy = {
        Mask: { ...current.Mask, [field]: Math.max(0, val) },
      };
      const newStrategies = { ...strategies, [typeKey]: strategy };
      if (wsData) {
        useWorkspaceStore.setState({
          activeWorkspaceData: {
            ...wsData,
            workspace: { ...wsData.workspace, strategies: newStrategies },
          },
        });
      }
      // 清除该类型的一致性映射（确保重新脱敏用新参数）
      await clearTypeConsistencyMappings(typeKey);
      debouncedSave(newStrategies);
    }
  };

  return (
    <div className="border-b border-slate-100">
      <button
        onClick={() => setCollapsed(!collapsed)}
        className="w-full flex items-center gap-2 px-4 py-2.5 text-xs font-semibold text-slate-600 hover:bg-slate-50 transition-colors"
      >
        {collapsed ? (
          <ChevronRight className="w-3.5 h-3.5 text-slate-400" />
        ) : (
          <ChevronDown className="w-3.5 h-3.5 text-slate-400" />
        )}
        <Shield className="w-3.5 h-3.5 text-primary-400" />
        {t("strategyPanel.rules")}
        {enabledTypes.length > 0 && (
          <span className="text-[11px] font-normal text-slate-400">({enabledTypes.length})</span>
        )}
      </button>

      {!collapsed && (
        <div className="px-4 pb-4 space-y-3">
          {/* 替换风格选择器（仅当有类型使用 Replace 时显示） */}
          {Object.values(strategies).some(
            (s) => typeof s === "object" && "Replace" in s
          ) && (
            <div className="flex items-center gap-2">
              <span className="text-xs text-slate-500 whitespace-nowrap">{t("strategyPanel.replaceStyleLabel")}</span>
              <div className="flex gap-1 rounded-lg bg-slate-100 p-0.5">
                {(["Fake", "Mou", "Ordinal"] as ReplaceStyle[]).map((style) => (
                  <button
                    key={style}
                    onClick={() => handleReplaceStyleChange(style)}
                    className={`rounded-md px-2 py-0.5 text-xs transition-colors ${
                      currentReplaceStyle === style
                        ? "bg-white text-slate-900 shadow-sm"
                        : "text-slate-500 hover:text-slate-700"
                    }`}
                  >
                    {REPLACE_STYLE_LABELS[style]}
                  </button>
                ))}
              </div>
            </div>
          )}

          {TYPE_KEYS.filter((k) => enabledTypes.includes(k)).map((typeKey) => {
            const info = SENSITIVE_TYPE_CONFIG[typeKey];
            const strategy = strategies[typeKey] || { Mask: { keep_prefix: 3, keep_suffix: 4 } };
            const strategyName = getStrategyName(strategy);
            const allowed = getAllowedStrategies(typeKey);
            const isMask = typeof strategy === "object" && "Mask" in strategy;

            return (
              <div key={typeKey} className="rounded-lg border border-slate-200 bg-white p-2.5 shadow-xs hover:border-slate-300 transition-colors">
                <div className="flex items-center justify-between gap-2">
                  <span
                    className={`shrink-0 px-2 py-0.5 rounded text-xs font-medium ${info.bgClass} ${info.textClass}`}
                  >
                    {info.label}
                  </span>
                  <select
                    value={strategyName}
                    onChange={(e) => handleStrategyChange(typeKey, e.target.value)}
                    className="text-xs border border-slate-200 rounded px-1.5 py-1 text-slate-700 bg-white focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-400"
                  >
                    {allowed.map((s) => (
                      <option key={s} value={s}>
                        {STRATEGY_LABELS[s]}
                      </option>
                    ))}
                  </select>
                </div>

                {isMask && (
                  <div className="mt-2 flex items-center gap-2 text-xs text-slate-600">
                    <label className="flex items-center gap-1">
                      {t("strategyPanel.keepPrefix")}
                      <input
                        type="number"
                        min={0}
                        value={
                          (strategy as { Mask: { keep_prefix: number; keep_suffix: number } })
                            .Mask.keep_prefix
                        }
                        onChange={(e) =>
                          handleMaskParamChange(typeKey, "keep_prefix", parseInt(e.target.value) || 0)
                        }
                        className="w-10 border border-slate-200 rounded px-1 py-0.5 text-center text-xs text-slate-700 focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-400"
                      />
                    </label>
                    <label className="flex items-center gap-1">
                      {t("strategyPanel.keepSuffix")}
                      <input
                        type="number"
                        min={0}
                        value={
                          (strategy as { Mask: { keep_prefix: number; keep_suffix: number } })
                            .Mask.keep_suffix
                        }
                        onChange={(e) =>
                          handleMaskParamChange(typeKey, "keep_suffix", parseInt(e.target.value) || 0)
                        }
                        className="w-10 border border-slate-200 rounded px-1 py-0.5 text-center text-xs text-slate-700 focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-400"
                      />
                    </label>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
