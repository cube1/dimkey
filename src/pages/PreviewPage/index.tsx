import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useAppStore } from "../../stores/appStore";
import { useDetectStore, useActiveItems, useAllActiveItems } from "../../stores/detectStore";
import { useConfigStore } from "../../stores/configStore";
import { SummaryBar } from "../../components/SummaryBar";
import { ContentRenderer } from "../../components/ContentRenderer";
import { SensitivePopover } from "../../components/SensitivePopover";
import { TextSelectionToolbar } from "../../components/TextSelectionToolbar";
import type { SensitiveItem, DesensitizeResult, StrategyConfig } from "../../types";

export function PreviewPage() {
  const { t } = useTranslation();
  const fileContent = useAppStore((s) => s.fileContent);
  const filePath = useAppStore((s) => s.filePath);
  const setView = useAppStore((s) => s.setView);
  const setDesensitizeResult = useAppStore((s) => s.setDesensitizeResult);
  const setItems = useDetectStore((s) => s.setItems);
  const appendItems = useDetectStore((s) => s.appendItems);
  const setNerStatus = useDetectStore((s) => s.setNerStatus);
  const activeItems = useActiveItems();
  const allActiveItems = useAllActiveItems();

  const setStrategyPanelOpen = useAppStore((s) => s.setStrategyPanelOpen);
  const [desensitizing, setDesensitizing] = useState(false);
  const [detecting, setDetecting] = useState(true);
  const contentAreaRef = useRef<HTMLDivElement>(null);

  /** SensitivePopover 状态 */
  const [popoverItem, setPopoverItem] = useState<SensitiveItem | null>(null);
  const [popoverAnchor, setPopoverAnchor] = useState<DOMRect | null>(null);

  const handleClickItem = useCallback(
    (item: SensitiveItem, event: React.MouseEvent) => {
      const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
      setPopoverItem(item);
      setPopoverAnchor(rect);
    },
    []
  );

  const handlePopoverClose = useCallback(() => {
    setPopoverItem(null);
    setPopoverAnchor(null);
  }, []);

  // 进入页面时触发识别
  useEffect(() => {
    if (!fileContent) return;

    let cancelled = false;

    setDetecting(true);
    const runDetection = async () => {
      try {
        // 规则引擎 + 词典匹配（并行）
        const [regexItems, dictItems] = await Promise.all([
          invoke<SensitiveItem[]>("detect_by_regex", { content: fileContent }),
          invoke<SensitiveItem[]>("detect_by_dict", { content: fileContent, dictEntries: [] }).catch(() => [] as SensitiveItem[]),
        ]);

        if (cancelled) return;
        setItems([...regexItems, ...dictItems]);
        setDetecting(false);

        // NER 异步识别
        setNerStatus("running");
        try {
          const nerItems = await invoke<SensitiveItem[]>("detect_by_ner", {
            content: fileContent,
          });
          if (!cancelled) {
            appendItems(nerItems);
            setNerStatus("done");
          }
        } catch {
          // NER 可能尚未实现，静默处理
          if (!cancelled) setNerStatus("done");
        }
      } catch (err) {
        const message = typeof err === "string" ? err : t("preview.detectError");
        toast.error(message);
        if (!cancelled) setDetecting(false);
      }
    };

    runDetection();
    return () => { cancelled = true; };
  }, [fileContent, setItems, appendItems, setNerStatus]);

  if (!fileContent) {
    return (
      <div className="flex-1 flex items-center justify-center text-gray-400">
        {t("preview.noContent")}
      </div>
    );
  }

  const fileName = fileContent.file_name;

  const handleStartDesensitize = async () => {
    if (allActiveItems.length === 0) {
      toast.error(t("preview.noSensitive"));
      return;
    }

    setDesensitizing(true);
    try {
      // 从 configStore 构建策略配置列表（排除 Custom，它不是合法的单元变体）
      const { strategies } = useConfigStore.getState();
      const strategyConfigs: StrategyConfig[] = Object.entries(strategies)
        .filter(([key]) => key !== "Custom")
        .map(([key, strategy]) => ({
          sensitive_type: key as StrategyConfig["sensitive_type"],
          strategy,
          consistent: true,
        }));

      const result = await invoke<DesensitizeResult>("apply_desensitize", {
        content: fileContent,
        items: allActiveItems,
        strategies: strategyConfigs,
      });

      setDesensitizeResult(result);
      setView("result");
    } catch (err) {
      const message = typeof err === "string" ? err : t("preview.desensitizeFailed");
      toast.error(message);
    } finally {
      setDesensitizing(false);
    }
  };

  return (
    <div className="flex-1 flex flex-col min-h-0">
      {/* 汇总条 */}
      <SummaryBar />

      {/* 内容区 */}
      <div ref={contentAreaRef} className="flex-1 min-h-0 relative">
        {detecting ? (
          <div className="p-6 space-y-3">
            {Array.from({ length: 8 }).map((_, i) => (
              <div key={i} className="animate-pulse flex gap-4">
                <div className="h-4 bg-gray-200 rounded flex-1" style={{ maxWidth: `${60 + (i % 3) * 15}%` }} />
              </div>
            ))}
            <p className="text-sm text-gray-400 mt-4">{t("preview.detecting")}</p>
          </div>
        ) : (
          <ContentRenderer
            content={fileContent}
            items={activeItems}
            onClickItem={handleClickItem}
          />
        )}

        {/* 脱敏执行中遮罩 */}
        {desensitizing && (
          <div className="absolute inset-0 bg-white/60 flex items-center justify-center z-10">
            <div className="flex flex-col items-center gap-2">
              <div className="w-8 h-8 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" />
              <span className="text-sm text-blue-600">{t("preview.processing")}</span>
            </div>
          </div>
        )}
      </div>

      {/* 手动标记工具条 */}
      <TextSelectionToolbar containerRef={contentAreaRef} />

      {/* 敏感项浮层 */}
      <SensitivePopover
        item={popoverItem}
        anchorRect={popoverAnchor}
        onClose={handlePopoverClose}
      />

      {/* 底部栏 */}
      <div className="bg-white border-t border-gray-200 px-6 py-3 flex items-center justify-between">
        <div className="text-sm text-gray-500">
          {t("previewPage.file")}: {fileName}
          {filePath && (
            <span className="ml-3 text-gray-400">
              {filePath}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => setStrategyPanelOpen(true)}
            className="px-3 py-2 text-gray-500 hover:text-gray-700 hover:bg-gray-100 rounded-lg transition-colors"
            title={t("topBar.strategyConfig")}
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
            </svg>
          </button>
          <button
            onClick={handleStartDesensitize}
            className="px-6 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50"
            disabled={allActiveItems.length === 0 || desensitizing}
          >
            {desensitizing ? t("preview.processing") : t("preview.startDesensitize")}
          </button>
        </div>
      </div>
    </div>
  );
}
