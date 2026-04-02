import { useEffect, useRef, useState, useMemo } from "react";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useDetectStore } from "../../stores/detectStore";
import { useConfigStore } from "../../stores/configStore";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useReDetectDict } from "../../hooks/useReDetectDict";
import type { SensitiveItem, Strategy, StrategyType, DictEntry } from "../../types";
import { getSensitiveTypeKey, getSensitiveTypeInfo, getAllowedStrategies, getStrategyType, STRATEGY_LABELS, REPLACE_STYLE_LABELS } from "../../types";

interface SensitivePopoverProps {
  item: SensitiveItem | null;
  anchorRect: DOMRect | null;
  onClose: () => void;
  /** 忽略此项（仅本次移除） */
  onRemoveItem?: (item: SensitiveItem) => void;
  /** 加入白名单（永久排除）后的回调 */
  onAddToWhitelist?: (item: SensitiveItem) => void;
}

/** 掩码预览：前 N 位 + *** + 后 N 位 */
function previewMask(
  text: string,
  keepPrefix: number,
  keepSuffix: number
): string {
  const chars = [...text];
  if (chars.length <= keepPrefix + keepSuffix) {
    return "*".repeat(chars.length);
  }
  const prefix = chars.slice(0, keepPrefix).join("");
  const suffix = keepSuffix > 0 ? chars.slice(-keepSuffix).join("") : "";
  const mid = "*".repeat(chars.length - keepPrefix - keepSuffix);
  return prefix + mid + suffix;
}

/** 从 Strategy 中提取 Mask 参数 */
function getMaskParams(strategy: Strategy): {
  keepPrefix: number;
  keepSuffix: number;
} {
  if (typeof strategy !== "string" && "Mask" in strategy) {
    return {
      keepPrefix: strategy.Mask.keep_prefix,
      keepSuffix: strategy.Mask.keep_suffix,
    };
  }
  return { keepPrefix: 1, keepSuffix: 1 };
}

export function SensitivePopover({
  item,
  anchorRect,
  onClose,
  onRemoveItem,
  onAddToWhitelist,
}: SensitivePopoverProps) {
  const { t } = useTranslation();
  const popoverRef = useRef<HTMLDivElement>(null);

  // store
  const overrideStrategy = useDetectStore((s) => s.overrideStrategy);
  const removeItem = useDetectStore((s) => s.removeItem);
  const itemOverrides = useDetectStore((s) => s.itemOverrides);
  const strategies = useConfigStore((s) => s.strategies);
  const replaceStyle = useConfigStore((s) => s.replaceStyle);

  const reDetectDict = useReDetectDict();

  const wsData = useWorkspaceStore((s) => s.activeWorkspaceData);
  const addDictEntryFromPopover = useWorkspaceStore((s) => s.addDictEntryFromPopover);
  const addWhitelistEntry = useWorkspaceStore((s) => s.addWhitelistEntry);
  const aliasLinkMode = useWorkspaceStore((s) => s.aliasLinkMode);
  const enterAliasLinkMode = useWorkspaceStore((s) => s.enterAliasLinkMode);
  const addAliasLinkMember = useWorkspaceStore((s) => s.addAliasLinkMember);
  const aliasGroups = useWorkspaceStore((s) => s.aliasGroups);
  const workspaceMode = wsData?.workspace.mode || "Desensitize";
  const isTemplateMode = workspaceMode === "TemplateReplace";

  // 获取当前生效的策略：先查 itemOverrides，再查 configStore 默认值
  const currentStrategy = useMemo<Strategy>(() => {
    if (!item) return { Mask: { keep_prefix: 1, keep_suffix: 1 } };
    const override = itemOverrides.get(item.id);
    if (override) return override;
    const typeKey = getSensitiveTypeKey(item.sensitive_type);
    return strategies[typeKey] ?? { Mask: { keep_prefix: 1, keep_suffix: 1 } };
  }, [item, itemOverrides, strategies]);

  const belongsToGroup = useMemo(() => {
    if (!item) return null;
    const tKey = getSensitiveTypeKey(item.sensitive_type);
    return aliasGroups.find((g) =>
      g.sensitive_type_key === tKey && g.members.includes(item.text)
    ) ?? null;
  }, [item, aliasGroups]);

  // 本地状态：策略类型与 Mask 参数
  const [strategyType, setStrategyType] = useState<StrategyType>("Mask");
  const [keepPrefix, setKeepPrefix] = useState(1);
  const [keepSuffix, setKeepSuffix] = useState(1);

  // 模版替换模式的替换值
  const [replacementValue, setReplacementValue] = useState("");
  // 已存在的词典条目（用于显示"已在词典中"标识）
  const [existingDictEntry, setExistingDictEntry] = useState<DictEntry | null>(null);

  // 当 currentStrategy 或 item 变化时同步本地状态
  useEffect(() => {
    if (!item) return;
    const st = getStrategyType(currentStrategy);
    setStrategyType(st);
    const params = getMaskParams(currentStrategy);
    setKeepPrefix(params.keepPrefix);
    setKeepSuffix(params.keepSuffix);
  }, [currentStrategy, item]);

  // item 变化时重置/预填 replacementValue 和 existingDictEntry
  useEffect(() => {
    if (!item) return;
    setReplacementValue("");
    setExistingDictEntry(null);
    if (isTemplateMode && wsData) {
      const dictEntry = wsData.workspace.dict_entries.find(
        (e) => e.text === item.text
      );
      if (dictEntry) {
        setExistingDictEntry(dictEntry);
        if (dictEntry.replacement) {
          setReplacementValue(dictEntry.replacement);
        }
      }
    }
  }, [item, isTemplateMode, wsData]);

  // 点击浮层外部关闭
  useEffect(() => {
    if (!item) return;
    function handleMouseDown(e: MouseEvent) {
      if (
        popoverRef.current &&
        !popoverRef.current.contains(e.target as Node)
      ) {
        onClose();
      }
    }
    document.addEventListener("mousedown", handleMouseDown);
    return () => document.removeEventListener("mousedown", handleMouseDown);
  }, [item, onClose]);

  // Escape 键关闭浮层
  useEffect(() => {
    if (!item) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [item, onClose]);

  // 滚动时关闭浮层（RI-9）
  useEffect(() => {
    if (!item) return;
    const handleScroll = () => onClose();
    window.addEventListener("scroll", handleScroll, true);
    return () => window.removeEventListener("scroll", handleScroll, true);
  }, [item, onClose]);

  // item 为 null 时不渲染
  if (!item || !anchorRect) return null;

  const typeKey = getSensitiveTypeKey(item.sensitive_type);
  const typeInfo = getSensitiveTypeInfo(item.sensitive_type);
  const allowedStrategies = getAllowedStrategies(typeKey);

  // 构建 Strategy 对象并同步到 store
  const commitStrategy = (
    newType: StrategyType,
    prefix: number,
    suffix: number
  ) => {
    let newStrategy: Strategy;
    if (newType === "Mask") {
      newStrategy = { Mask: { keep_prefix: prefix, keep_suffix: suffix } };
    } else if (newType === "Replace") {
      newStrategy = { Replace: { style: replaceStyle } };
    } else {
      newStrategy = "Generalize";
    }
    overrideStrategy(item.id, newStrategy);
  };

  // 策略类型下拉切换
  const handleStrategyChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const newType = e.target.value as StrategyType;
    setStrategyType(newType);
    commitStrategy(newType, keepPrefix, keepSuffix);
  };

  // Mask 参数变更
  const handlePrefixChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = Math.max(0, parseInt(e.target.value) || 0);
    setKeepPrefix(val);
    commitStrategy("Mask", val, keepSuffix);
  };

  const handleSuffixChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = Math.max(0, parseInt(e.target.value) || 0);
    setKeepSuffix(val);
    commitStrategy("Mask", keepPrefix, val);
  };

  // 忽略此项（仅本次移除，不持久化）
  const handleRemove = () => {
    if (onRemoveItem) {
      onRemoveItem(item);
    } else {
      removeItem(item.id);
    }
    onClose();
  };

  // 加入白名单（永久排除）
  const handleAddToWhitelist = async () => {
    if (!item) return;
    await addWhitelistEntry(item.text);
    // 从当前识别结果中移除所有同文本的项
    const store = useWorkspaceStore.getState();
    const filtered = store.currentSensitiveItems.filter(
      (i) => i.text !== item.text
    );
    store.setCurrentSensitiveItems(filtered);
    // 同步更新 rawSensitiveItems
    const rawFiltered = store.rawSensitiveItems.filter(
      (i) => i.text !== item.text
    );
    store.setRawSensitiveItems(rawFiltered);
    toast.success(t("popover.addedToWhitelist", { text: item.text }));
    onClose();
    // 触发重新脱敏，更新右侧对比视图
    if (onAddToWhitelist) {
      onAddToWhitelist(item);
    }
  };

  // 确认替换值（模版替换模式，使用简化的 store 方法）
  const handleConfirmReplacement = async () => {
    if (!item || !replacementValue.trim()) return;
    await addDictEntryFromPopover(
      item.text,
      item.sensitive_type,
      replacementValue.trim()
    );
    // 重新检测词典项
    await reDetectDict();
    onClose();
  };

  // 浮层定位：anchorRect 正下方偏左
  const popoverStyle: React.CSSProperties = {
    position: "fixed",
    top: anchorRect.bottom + 6,
    left: anchorRect.left,
    zIndex: 50,
  };

  // 关联模式下的简化浮框
  if (aliasLinkMode && !isTemplateMode) {
    return (
      <div ref={popoverRef} style={popoverStyle}>
        <div className="w-64 rounded-xl border border-indigo-200 bg-white shadow-float animate-slide-up overflow-hidden">
          <div className="px-4 py-3">
            <p className="text-sm font-medium text-slate-700">{item.text}</p>
            <p className="text-xs text-slate-400 mt-1">{typeInfo.label}</p>
          </div>
          <div className="border-t border-slate-100 px-3 py-2 flex justify-end gap-2">
            <button onClick={onClose}
              className="text-xs px-3 py-1.5 text-slate-400 hover:text-slate-600 rounded-lg hover:bg-slate-100 transition-colors">
              {t("common.cancel")}
            </button>
            <button
              onClick={() => { addAliasLinkMember(item); onClose(); }}
              className="text-xs px-3 py-1.5 bg-indigo-500 text-white rounded-lg hover:bg-indigo-600 transition-colors">
              {t("popover.joinLink")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  // 模版替换模式：简化的替换值输入 + 确认
  if (isTemplateMode) {
    return (
      <div ref={popoverRef} style={popoverStyle}>
        <div className="w-72 rounded-xl border border-slate-200/80 bg-white shadow-float animate-slide-up overflow-hidden">
          <div className="px-4 pt-3.5 pb-3 space-y-2.5">
            <div className="text-xs text-slate-500">
              <span className={`inline-block rounded-full px-1.5 py-0.5 text-[11px] font-medium ${typeInfo.bgClass} ${typeInfo.textClass} mr-1.5`}>
                {typeInfo.label}
              </span>
              <span className="font-medium text-slate-700">{item.text}</span>
              {existingDictEntry && (
                <span className="ml-1 text-teal-600 text-[11px]">({t("popover.inDict")})</span>
              )}
            </div>
            <div>
              <label className="text-[11px] text-slate-400 mb-1 block tracking-wider">{t("strategyPanel.replaceTo")}</label>
              <input
                value={replacementValue}
                onChange={(e) => setReplacementValue(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleConfirmReplacement()}
                className="w-full text-sm px-2.5 py-1.5 border border-slate-200 bg-slate-50/80 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-500 transition-colors"
                placeholder={t("popover.inputReplacement")}
                autoFocus
              />
            </div>
          </div>
          <div className="border-t border-slate-100 px-3 py-2 flex gap-2 justify-end bg-slate-50/50">
            <button onClick={onClose}
              className="text-xs px-3 py-1.5 text-slate-400 hover:text-slate-600 rounded-lg hover:bg-slate-100 transition-colors">
              {t("common.cancel")}
            </button>
            <button onClick={handleConfirmReplacement}
              disabled={!replacementValue.trim()}
              className="text-xs px-3 py-1.5 bg-teal-500 text-white rounded-lg hover:bg-teal-600 disabled:opacity-40 transition-colors">
              {t("common.confirm")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div ref={popoverRef} style={popoverStyle}>
      <div className="w-72 rounded-xl border border-slate-200/80 bg-white shadow-float animate-slide-up overflow-hidden">
        {/* 头部：原文 + 类型标签 */}
        <div className="px-4 pt-3.5 pb-2.5">
          <p className="text-sm font-semibold text-slate-800 break-all leading-relaxed">
            {item.text}
          </p>
          <span
            className={`mt-1.5 inline-block rounded-full px-2 py-0.5 text-[11px] font-medium ${typeInfo.bgClass} ${typeInfo.textClass}`}
          >
            {typeInfo.label}
          </span>
        </div>

        <div className="h-px bg-gradient-to-r from-transparent via-slate-200 to-transparent" />

        {/* 策略选择 */}
        <div className="px-4 py-3 space-y-2">
          <label className="block text-[11px] font-medium text-slate-400 tracking-wider">
            {t("popover.desensitizeStrategy")}
          </label>
          <select
            value={strategyType}
            onChange={handleStrategyChange}
            className="w-full rounded-lg border border-slate-200 bg-slate-50/80 px-2.5 py-1.5 text-sm text-slate-700 focus:border-primary-500 focus:outline-none focus:ring-2 focus:ring-primary-500/20 transition-colors"
          >
            {allowedStrategies.map((st) => (
              <option key={st} value={st}>
                {STRATEGY_LABELS[st]}
              </option>
            ))}
          </select>

          {/* Mask 参数 */}
          {strategyType === "Mask" && (
            <div className="flex gap-2">
              <div className="flex-1">
                <label className="mb-0.5 block text-[11px] text-slate-400">{t("popover.keepBefore")}</label>
                <input
                  type="number"
                  min={0}
                  value={keepPrefix}
                  onChange={handlePrefixChange}
                  className="w-full rounded-lg border border-slate-200 bg-slate-50/80 px-2.5 py-1.5 text-sm text-slate-700 focus:border-primary-500 focus:outline-none focus:ring-2 focus:ring-primary-500/20 transition-colors"
                />
              </div>
              <div className="flex-1">
                <label className="mb-0.5 block text-[11px] text-slate-400">{t("popover.keepAfter")}</label>
                <input
                  type="number"
                  min={0}
                  value={keepSuffix}
                  onChange={handleSuffixChange}
                  className="w-full rounded-lg border border-slate-200 bg-slate-50/80 px-2.5 py-1.5 text-sm text-slate-700 focus:border-primary-500 focus:outline-none focus:ring-2 focus:ring-primary-500/20 transition-colors"
                />
              </div>
            </div>
          )}
        </div>

        {/* 预览 */}
        <div className="mx-4 mb-3 rounded-lg bg-slate-50 border border-slate-100 px-3 py-2">
          <p className="text-[11px] text-slate-400 mb-0.5">{t("popover.preview")}</p>
          {strategyType === "Mask" && (
            <p className="break-all font-mono text-sm text-slate-700 tracking-wide">
              {previewMask(item.text, keepPrefix, keepSuffix)}
            </p>
          )}
          {strategyType === "Replace" && (
            <p className="text-sm text-slate-400 italic">
              {t("popover.replaceWith", { style: REPLACE_STYLE_LABELS[replaceStyle] })}
            </p>
          )}
          {strategyType === "Generalize" && (
            <p className="text-sm text-slate-400 italic">
              {t("popover.generalize")}
            </p>
          )}
        </div>

        {/* 底部操作栏 */}
        <div className="border-t border-slate-100 px-2 py-1.5 flex items-center flex-wrap gap-1 bg-slate-50/50">
          {/* 关联实体按钮（仅 OrgName/PersonName 类型、非模版模式下显示） */}
          {(typeKey === "OrgName" || typeKey === "PersonName") && (
            belongsToGroup ? (
              <span className="flex items-center gap-1 px-2.5 py-1.5 text-xs text-indigo-500">
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101" />
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M10.172 13.828a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.102 1.101" />
                </svg>
                {t("popover.linked")}
              </span>
            ) : (
              <button
                onClick={() => {
                  enterAliasLinkMode(item);
                  onClose();
                }}
                className="flex items-center gap-1 px-2.5 py-1.5 text-xs text-indigo-600 hover:bg-indigo-50 rounded-lg transition-colors"
                title="将此项与其他项关联为同一实体"
              >
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101" />
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M10.172 13.828a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.102 1.101" />
                </svg>
                {t("popover.linkEntity")}
              </button>
            )
          )}
          <button
            onClick={handleAddToWhitelist}
            className="flex items-center gap-1 px-2.5 py-1.5 text-xs text-amber-600 hover:bg-amber-50 rounded-lg transition-colors"
            title="加入白名单，不再识别此文本"
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
            </svg>
            {t("popover.addToWhitelist")}
          </button>
          <button
            onClick={handleRemove}
            className="flex items-center gap-1 px-2.5 py-1.5 text-xs text-slate-400 hover:text-red-500 hover:bg-red-50 rounded-lg transition-colors"
            title="忽略此项，仅本次生效"
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M6 18L18 6M6 6l12 12" />
            </svg>
            {t("popover.ignoreItem")}
          </button>
        </div>
      </div>
    </div>
  );
}
