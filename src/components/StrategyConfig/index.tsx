import { Dialog, DialogPanel, DialogTitle, Transition, TransitionChild } from "@headlessui/react";
import { useTranslation } from "react-i18next";
import { useConfigStore } from "../../stores/configStore";
import type { Strategy, ReplaceStyle } from "../../types";
import { SENSITIVE_TYPE_CONFIG, getAllowedStrategies, STRATEGY_LABELS, REPLACE_STYLE_LABELS } from "../../types";

/** 获取当前策略的下拉选中值 */
function getStrategyName(strategy: Strategy): string {
  if (typeof strategy === "string") return strategy;
  if ("Mask" in strategy) return "Mask";
  if ("Replace" in strategy) return "Replace";
  return "Mask";
}

/** 需要展示的敏感类型 key（排除 Custom） */
const TYPE_KEYS = Object.keys(SENSITIVE_TYPE_CONFIG).filter(
  (k) => k !== "Custom"
);

interface StrategyConfigProps {
  open: boolean;
  onClose: () => void;
}

export function StrategyConfig({ open, onClose }: StrategyConfigProps) {
  const { t } = useTranslation();
  const strategies = useConfigStore((s) => s.strategies);
  const updateStrategy = useConfigStore((s) => s.updateStrategy);
  const saveConfig = useConfigStore((s) => s.saveConfig);
  const resetToDefault = useConfigStore((s) => s.resetToDefault);
  const replaceStyle = useConfigStore((s) => s.replaceStyle);
  const updateReplaceStyle = useConfigStore((s) => s.updateReplaceStyle);

  /** 处理策略下拉变更 */
  const handleStrategyChange = (typeKey: string, value: string) => {
    if (value === "Mask") {
      updateStrategy(typeKey, { Mask: { keep_prefix: 3, keep_suffix: 4 } });
    } else if (value === "Generalize") {
      updateStrategy(typeKey, "Generalize");
    } else {
      updateStrategy(typeKey, { Replace: { style: replaceStyle } });
    }
  };

  /** 处理 Mask 参数变更 */
  const handleMaskParamChange = (
    typeKey: string,
    field: "keep_prefix" | "keep_suffix",
    val: number
  ) => {
    const current = strategies[typeKey];
    if (typeof current === "object" && "Mask" in current) {
      updateStrategy(typeKey, {
        Mask: { ...current.Mask, [field]: Math.max(0, val) },
      });
    }
  };

  /** 保存并关闭 */
  const handleSave = async () => {
    await saveConfig();
    onClose();
  };

  return (
    <Transition appear show={open}>
      <Dialog as="div" className="relative z-50" onClose={onClose}>
        {/* 背景遮罩 */}
        <TransitionChild
          enter="ease-out duration-200"
          enterFrom="opacity-0"
          enterTo="opacity-100"
          leave="ease-in duration-150"
          leaveFrom="opacity-100"
          leaveTo="opacity-0"
        >
          <div className="fixed inset-0 bg-black/25" />
        </TransitionChild>

        {/* 面板容器 */}
        <div className="fixed inset-0 flex justify-end">
          <TransitionChild
            enter="ease-out duration-300"
            enterFrom="translate-x-full"
            enterTo="translate-x-0"
            leave="ease-in duration-200"
            leaveFrom="translate-x-0"
            leaveTo="translate-x-full"
          >
            <DialogPanel className="w-[360px] h-full bg-white shadow-xl flex flex-col">
              {/* 标题栏 */}
              <div className="flex items-center justify-between px-5 py-4 border-b border-gray-200">
                <DialogTitle className="text-lg font-semibold text-gray-800">
                  {t("strategyConfig.title")}
                </DialogTitle>
                <button
                  onClick={onClose}
                  className="p-1 text-gray-400 hover:text-gray-600 rounded transition-colors"
                >
                  <svg
                    className="w-5 h-5"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M6 18L18 6M6 6l12 12"
                    />
                  </svg>
                </button>
              </div>

              {/* 策略列表 */}
              <div className="flex-1 overflow-auto px-5 py-4 space-y-4">
                {/* 替换风格选择器（仅当有类型使用 Replace 时显示） */}
                {Object.values(strategies).some(
                  (s) => typeof s === "object" && "Replace" in s
                ) && (
                  <div className="mb-4 flex items-center gap-3">
                    <span className="text-sm text-gray-500 whitespace-nowrap">{t("strategyConfig.replaceStyle")}</span>
                    <div className="flex gap-1 rounded-lg bg-gray-100 p-1">
                      {(["Fake", "Mou", "Ordinal"] as ReplaceStyle[]).map((style) => (
                        <button
                          key={style}
                          onClick={() => updateReplaceStyle(style)}
                          className={`rounded-md px-3 py-1 text-sm transition-colors ${
                            replaceStyle === style
                              ? "bg-white text-gray-900 shadow-sm"
                              : "text-gray-500 hover:text-gray-700"
                          }`}
                        >
                          {REPLACE_STYLE_LABELS[style]}
                        </button>
                      ))}
                    </div>
                  </div>
                )}

                {TYPE_KEYS.map((typeKey) => {
                  const info = SENSITIVE_TYPE_CONFIG[typeKey];
                  const strategy = strategies[typeKey];
                  const strategyName = getStrategyName(strategy);
                  const allowed = getAllowedStrategies(typeKey);
                  const isMask =
                    typeof strategy === "object" && "Mask" in strategy;

                  return (
                    <div
                      key={typeKey}
                      className="rounded-lg border border-gray-100 p-3"
                    >
                      {/* 类型标签 + 策略下拉 */}
                      <div className="flex items-center justify-between gap-3">
                        <span
                          className={`shrink-0 px-2 py-0.5 rounded text-xs font-medium ${info.bgClass} ${info.textClass}`}
                        >
                          {info.label}
                        </span>
                        <select
                          value={strategyName}
                          onChange={(e) =>
                            handleStrategyChange(typeKey, e.target.value)
                          }
                          className="text-sm border border-gray-200 rounded-md px-2 py-1 text-gray-700 bg-white focus:outline-none focus:ring-1 focus:ring-blue-400"
                        >
                          {allowed.map((s) => (
                            <option key={s} value={s}>
                              {STRATEGY_LABELS[s]}
                            </option>
                          ))}
                        </select>
                      </div>

                      {/* Mask 参数输入 */}
                      {isMask && (
                        <div className="mt-2 flex items-center gap-3 text-xs text-gray-600">
                          <label className="flex items-center gap-1">
                            {t("strategyConfig.keepPrefix")}
                            <input
                              type="number"
                              min={0}
                              value={
                                (strategy as { Mask: { keep_prefix: number; keep_suffix: number } })
                                  .Mask.keep_prefix
                              }
                              onChange={(e) =>
                                handleMaskParamChange(
                                  typeKey,
                                  "keep_prefix",
                                  parseInt(e.target.value) || 0
                                )
                              }
                              className="w-12 border border-gray-200 rounded px-1.5 py-0.5 text-center text-sm text-gray-700 focus:outline-none focus:ring-1 focus:ring-blue-400"
                            />
                            {t("strategyConfig.chars")}
                          </label>
                          <label className="flex items-center gap-1">
                            {t("strategyConfig.keepSuffix")}
                            <input
                              type="number"
                              min={0}
                              value={
                                (strategy as { Mask: { keep_prefix: number; keep_suffix: number } })
                                  .Mask.keep_suffix
                              }
                              onChange={(e) =>
                                handleMaskParamChange(
                                  typeKey,
                                  "keep_suffix",
                                  parseInt(e.target.value) || 0
                                )
                              }
                              className="w-12 border border-gray-200 rounded px-1.5 py-0.5 text-center text-sm text-gray-700 focus:outline-none focus:ring-1 focus:ring-blue-400"
                            />
                            {t("strategyConfig.chars")}
                          </label>
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>

              {/* 底部按钮 */}
              <div className="flex items-center justify-between px-5 py-4 border-t border-gray-200">
                <button
                  onClick={resetToDefault}
                  className="px-4 py-2 text-sm text-gray-500 hover:text-gray-700 hover:bg-gray-100 rounded-lg transition-colors"
                >
                  {t("strategyConfig.restoreDefault")}
                </button>
                <button
                  onClick={handleSave}
                  className="px-5 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-lg transition-colors"
                >
                  {t("strategyConfig.save")}
                </button>
              </div>
            </DialogPanel>
          </TransitionChild>
        </div>
      </Dialog>
    </Transition>
  );
}
