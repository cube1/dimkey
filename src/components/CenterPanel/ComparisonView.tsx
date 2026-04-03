import { useState, useRef, useCallback, useMemo, useEffect } from "react";
import { RefreshCw } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useAutoDesensitize } from "../../hooks/useAutoDesensitize";
import { ContentRenderer } from "../ContentRenderer";
import { PdfPreviewView } from "../PdfPreviewView";
import type { PdfPageRender } from "../PdfPreviewView";
import { ColumnRulePopover } from "../ColumnRulePopover";
import { SensitivePopover } from "../SensitivePopover";
import { TextSelectionToolbar } from "../TextSelectionToolbar";
import type { SpreadsheetViewHandle } from "../SpreadsheetView";
import { useReDetectDict } from "../../hooks/useReDetectDict";
import type { ProcessingRecord, SensitiveItem, MappingEntry, ColumnRule } from "../../types";
import { SENSITIVE_TYPE_CONFIG } from "../../types";

const COL_DEFAULT_WIDTH = 150;

/** 缩放级别预设（px） */
const FONT_SIZE_LEVELS = [12, 14, 16, 18, 20];
const DEFAULT_FONT_SIZE_INDEX = 1; // 14px

/** PDF 缩放级别预设（%） */
const PDF_ZOOM_LEVELS = [50, 75, 100, 125, 150];
const DEFAULT_PDF_ZOOM = 100;

/**
 * 根据原始 items 和 mappings 计算脱敏后内容中的高亮位置
 * 同一单元格/段落内多处替换会导致偏移漂移，需逐项累计
 */
function buildDesensitizedItems(
  originalItems: SensitiveItem[],
  mappings: MappingEntry[]
): SensitiveItem[] {
  // original_text -> replaced_text 查找表
  const replaceMap = new Map<string, string>();
  for (const m of mappings) {
    replaceMap.set(m.original_text, m.replaced_text);
  }

  // 按 (sheet_index, row, col) 分组，组内按 start 排序
  const grouped = new Map<string, SensitiveItem[]>();
  for (const item of originalItems) {
    const key = `${item.sheet_index}:${item.row}:${item.col}`;
    if (!grouped.has(key)) grouped.set(key, []);
    grouped.get(key)!.push(item);
  }

  const result: SensitiveItem[] = [];

  for (const [, cellItems] of grouped) {
    const sorted = [...cellItems].sort((a, b) => a.start - b.start);
    let shift = 0; // 累计偏移量

    for (const item of sorted) {
      const replacedText = replaceMap.get(item.text);
      if (!replacedText) continue;

      const origLen = item.end - item.start;
      const newLen = replacedText.length;
      const newStart = item.start + shift;
      const newEnd = newStart + newLen;

      result.push({
        ...item,
        id: `desen_${item.id}`,
        text: replacedText,
        start: newStart,
        end: newEnd,
      });

      shift += newLen - origLen;
    }
  }

  return result;
}

export function ComparisonView() {
  const { t } = useTranslation();
  const reDetectDict = useReDetectDict();
  const currentFileContent = useWorkspaceStore((s) => s.currentFileContent);
  const currentResult = useWorkspaceStore((s) => s.currentResult);
  const currentSensitiveItems = useWorkspaceStore((s) => s.currentSensitiveItems);
  const activeRecordId = useWorkspaceStore((s) => s.activeRecordId);
  const wsData = useWorkspaceStore((s) => s.activeWorkspaceData);
  const setCenterView = useWorkspaceStore((s) => s.setCenterView);
  const currentFilePath = useWorkspaceStore((s) => s.currentFilePath);
  const columnInferences = useWorkspaceStore((s) => s.columnInferences);
  const isColumnMode = useWorkspaceStore((s) => s.isColumnMode);
  const confirmedColumnRules = useWorkspaceStore((s) => s.confirmedColumnRules);
  const setColumnRule = useWorkspaceStore((s) => s.setColumnRule);
  const activeSheetIndex = useWorkspaceStore((s) => s.activeSheetIndex);
  const setActiveSheetIndex = useWorkspaceStore((s) => s.setActiveSheetIndex);

  const { reDesensitizeColumn, undoColumnDesensitize, desensitizeManualItems, reDesensitizeWithFilteredItems, reprocessFromRecord } = useAutoDesensitize();

  // 左右联动选中状态（存储原始项的 id）
  const [selectedOriginalId, setSelectedOriginalId] = useState<string | null>(null);

  // 敏感项编辑 Popover 状态
  const [sensitivePopoverItem, setSensitivePopoverItem] = useState<SensitiveItem | null>(null);
  const [sensitivePopoverAnchor, setSensitivePopoverAnchor] = useState<DOMRect | null>(null);

  // 列头 Popover 状态
  const [popoverCol, setPopoverCol] = useState<number | null>(null);
  const [anchorRect, setAnchorRect] = useState<DOMRect | null>(null);

  const handleHeaderClick = useCallback((col: number, rect: DOMRect) => {
    setPopoverCol(col);
    setAnchorRect(rect);
  }, []);

  const handlePopoverConfirm = useCallback(
    (rule: ColumnRule) => {
      if (popoverCol !== null) {
        const ruleWithSheet = { ...rule, sheet_index: activeSheetIndex };
        const ruleKey = `${activeSheetIndex}:${popoverCol}`;
        setColumnRule(ruleKey, ruleWithSheet);
        reDesensitizeColumn(ruleWithSheet);
      }
      setPopoverCol(null);
      setAnchorRect(null);
    },
    [popoverCol, activeSheetIndex, setColumnRule, reDesensitizeColumn]
  );

  const handlePopoverSkip = useCallback(() => {
    if (popoverCol !== null) {
      const ruleKey = `${activeSheetIndex}:${popoverCol}`;
      setColumnRule(ruleKey, null);
      undoColumnDesensitize(popoverCol, activeSheetIndex);
    }
    setPopoverCol(null);
    setAnchorRect(null);
  }, [popoverCol, activeSheetIndex, setColumnRule, undoColumnDesensitize]);

  const handlePopoverClose = useCallback(() => {
    setPopoverCol(null);
    setAnchorRect(null);
  }, []);

  // 监听 enabled_types 和策略变化，重新执行脱敏（300ms 防抖）
  const enabledTypes = wsData?.workspace.enabled_types || [];
  const enabledTypesKey = enabledTypes.join(",");
  const strategiesKey = JSON.stringify(wsData?.workspace.strategies || {});
  const isFirstRender = useRef(true);

  useEffect(() => {
    // 跳过首次渲染（processFile 已经完成了初次脱敏）
    if (isFirstRender.current) {
      isFirstRender.current = false;
      return;
    }
    if (!currentResult) return;

    const timer = setTimeout(() => {
      reDesensitizeWithFilteredItems();
    }, 300);
    return () => clearTimeout(timer);
  }, [enabledTypesKey, strategiesKey]);

  const [colWidths, setColWidths] = useState<number[]>([]);
  const [fontSizeIndex, setFontSizeIndex] = useState(DEFAULT_FONT_SIZE_INDEX);
  const [reprocessing, setReprocessing] = useState(false);

  const [pdfZoom, setPdfZoom] = useState(DEFAULT_PDF_ZOOM);
  const [pdfPages, setPdfPages] = useState<PdfPageRender[] | null>(null);
  const [_pdfLoading, setPdfLoading] = useState(false);
  const [rectSelectMode, setRectSelectMode] = useState(false);
  const [pdfOverlayRects, setPdfOverlayRects] = useState<Array<{page_index: number; left: number; top: number; right: number; bottom: number}>>([]);

  // PDF 滚动同步
  const [pdfLeftScroll, setPdfLeftScroll] = useState({ left: 0, top: 0 });
  const [pdfRightScroll, setPdfRightScroll] = useState({ left: 0, top: 0 });
  const pdfSyncing = useRef(false);

  // PDF 缩放时保持滚动比例
  const pdfLeftRef = useRef<HTMLDivElement>(null);
  const prevZoom = useRef(DEFAULT_PDF_ZOOM);

  const leftRef = useRef<SpreadsheetViewHandle>(null);
  const rightRef = useRef<SpreadsheetViewHandle>(null);
  const leftPanelRef = useRef<HTMLDivElement>(null);
  const isSyncing = useRef(false);

  const setCurrentSensitiveItems = useWorkspaceStore((s) => s.setCurrentSensitiveItems);

  // 手动添加敏感项回调（用于 TextSelectionToolbar）：添加后立即执行脱敏
  const handleManualAddItem = useCallback(async (item: SensitiveItem) => {
    const store = useWorkspaceStore.getState();
    const existing = store.currentSensitiveItems;
    setCurrentSensitiveItems([...existing, item]);
    // 同步更新 rawSensitiveItems，防止 reDesensitizeWithFilteredItems 丢掉手动项
    store.setRawSensitiveItems([...store.rawSensitiveItems, item]);
    // 若命中白名单，自动移除对应条目
    const wsData = store.activeWorkspaceData;
    if (wsData) {
      const whitelist = wsData.workspace.whitelist || [];
      const matchIdx = whitelist.findIndex((w) =>
        w.match_mode === "Exact" ? w.text === item.text : w.text.toLowerCase() === item.text.toLowerCase()
      );
      if (matchIdx >= 0) {
        await store.removeWhitelistEntry(matchIdx);
      }
    }
    // 标记后立即执行脱敏，无需手动点击按钮
    desensitizeManualItems();
  }, [setCurrentSensitiveItems, desensitizeManualItems]);

  // 左侧（原始内容）点击：高亮两侧对应项 + 弹出编辑浮层
  const handleLeftClickItem = useCallback((item: SensitiveItem, event: React.MouseEvent) => {
    setSelectedOriginalId((prev) => (prev === item.id ? null : item.id));
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    setSensitivePopoverItem(item);
    setSensitivePopoverAnchor(rect);
  }, []);

  // 右侧（脱敏内容）点击：从 desen_ 前缀提取原始 id + 弹出编辑浮层
  const handleRightClickItem = useCallback((item: SensitiveItem, event: React.MouseEvent) => {
    const originalId = item.id.startsWith("desen_") ? item.id.slice(6) : item.id;
    setSelectedOriginalId((prev) => (prev === originalId ? null : originalId));
    // 找到原始项以传给 Popover
    const store = useWorkspaceStore.getState();
    const originalItem = store.currentSensitiveItems.find((i) => i.id === originalId);
    if (originalItem) {
      const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
      setSensitivePopoverItem(originalItem);
      setSensitivePopoverAnchor(rect);
    }
  }, []);

  // 关闭敏感项编辑浮层
  const handleSensitivePopoverClose = useCallback(() => {
    setSensitivePopoverItem(null);
    setSensitivePopoverAnchor(null);
  }, []);

  // 忽略此项（仅本次）：从 items 中移除 + 重新脱敏
  const handleRemoveItem = useCallback((item: SensitiveItem) => {
    const store = useWorkspaceStore.getState();
    const filtered = store.currentSensitiveItems.filter((i) => i.id !== item.id);
    store.setCurrentSensitiveItems(filtered);
    const rawFiltered = store.rawSensitiveItems.filter((i) => i.id !== item.id);
    store.setRawSensitiveItems(rawFiltered);
    reDesensitizeWithFilteredItems();
  }, [reDesensitizeWithFilteredItems]);

  // 加入白名单后：触发重新脱敏以更新右侧对比视图
  const handleAddToWhitelist = useCallback(() => {
    reDesensitizeWithFilteredItems();
  }, [reDesensitizeWithFilteredItems]);

  // 历史记录重新处理
  const handleReprocess = useCallback(async (record: ProcessingRecord) => {
    setReprocessing(true);
    try {
      await reprocessFromRecord(record);
    } finally {
      setReprocessing(false);
    }
  }, [reprocessFromRecord]);

  // 如果是查看历史记录（没有 currentResult 但有 activeRecordId），从记录中获取数据
  const activeRecord: ProcessingRecord | null = useMemo(() => {
    if (activeRecordId && wsData) {
      return wsData.history.find((r) => r.id === activeRecordId) || null;
    }
    return null;
  }, [activeRecordId, wsData]);

  // 判断数据来源
  const hasCurrentResult = !!currentResult && !!currentFileContent;
  const hasRecordResult = !!activeRecord;

  // 构建脱敏后内容的高亮项
  const desensitizedItems = useMemo(() => {
    if (!currentResult || currentSensitiveItems.length === 0) return [];
    return buildDesensitizedItems(currentSensitiveItems, currentResult.mappings);
  }, [currentSensitiveItems, currentResult]);

  // 判断是否为模版替换模式
  const isTemplateMode = wsData?.workspace.mode === "TemplateReplace";

  // 模版替换模式：构建原文 → 替换值映射，用于高亮区分
  const templateReplacements = useMemo(() => {
    if (!wsData || !isTemplateMode) return undefined;
    const map = new Map<string, string>();
    for (const entry of wsData.workspace.dict_entries) {
      if (entry.replacement) {
        map.set(entry.text, entry.replacement);
      }
    }
    return map.size > 0 ? map : undefined;
  }, [wsData, isTemplateMode]);

  // 当前 sheet 的已确认列规则
  const sheetColumnRules = useMemo(() => {
    const result: Record<number, ColumnRule> = {};
    for (const [key, rule] of Object.entries(confirmedColumnRules)) {
      if (key.startsWith(`${activeSheetIndex}:`)) {
        const col = Number(key.split(":")[1]);
        result[col] = rule;
      }
    }
    return result;
  }, [confirmedColumnRules, activeSheetIndex]);

  const ensureColWidths = useCallback(
    (colCount: number) => {
      if (colWidths.length !== colCount) {
        setColWidths(Array.from({ length: colCount }, () => COL_DEFAULT_WIDTH));
      }
    },
    [colWidths.length]
  );

  const handleColResize = useCallback((colIndex: number, width: number) => {
    setColWidths((prev) => {
      const next = [...prev];
      next[colIndex] = width;
      return next;
    });
  }, []);

  const handleLeftScroll = useCallback((scrollLeft: number, scrollTop: number) => {
    if (isSyncing.current) return;
    isSyncing.current = true;
    const rightEl = rightRef.current?.getScrollElement();
    if (rightEl) {
      rightEl.scrollLeft = scrollLeft;
      rightEl.scrollTop = scrollTop;
    }
    requestAnimationFrame(() => { isSyncing.current = false; });
  }, []);

  const handleRightScroll = useCallback((scrollLeft: number, scrollTop: number) => {
    if (isSyncing.current) return;
    isSyncing.current = true;
    const leftEl = leftRef.current?.getScrollElement();
    if (leftEl) {
      leftEl.scrollLeft = scrollLeft;
      leftEl.scrollTop = scrollTop;
    }
    requestAnimationFrame(() => { isSyncing.current = false; });
  }, []);

  // 提前计算 isPdf，供 useEffect 使用
  const isPdf = currentFileContent?.type === "Document" && currentFileContent?.file_type === "Pdf";

  // 加载 PDF 页面（共享给左右两侧）
  useEffect(() => {
    if (!isPdf || !currentFilePath) return;
    setPdfLoading(true);
    invoke<PdfPageRender[]>("render_pdf_pages", { filePath: currentFilePath })
      .then(setPdfPages)
      .catch((err) => console.error("渲染 PDF 失败:", err))
      .finally(() => setPdfLoading(false));
  }, [isPdf, currentFilePath]);

  // 计算 PDF 涂黑预览矩形（敏感项变化时重新计算）
  useEffect(() => {
    if (!isPdf || !currentFilePath || currentSensitiveItems.length === 0) {
      setPdfOverlayRects([]);
      return;
    }
    invoke<Array<{page_index: number; left: number; top: number; right: number; bottom: number}>>(
      "compute_pdf_redact_preview",
      { filePath: currentFilePath, sensitiveItems: currentSensitiveItems }
    )
      .then(setPdfOverlayRects)
      .catch((err) => console.error("计算涂黑预览失败:", err));
  }, [isPdf, currentFilePath, currentSensitiveItems]);

  // PDF 滚动同步回调
  const handlePdfLeftScroll = useCallback((scrollLeft: number, scrollTop: number) => {
    if (pdfSyncing.current) return;
    pdfSyncing.current = true;
    setPdfRightScroll({ left: scrollLeft, top: scrollTop });
    requestAnimationFrame(() => { pdfSyncing.current = false; });
  }, []);

  const handlePdfRightScroll = useCallback((scrollLeft: number, scrollTop: number) => {
    if (pdfSyncing.current) return;
    pdfSyncing.current = true;
    setPdfLeftScroll({ left: scrollLeft, top: scrollTop });
    requestAnimationFrame(() => { pdfSyncing.current = false; });
  }, []);

  // PDF 缩放时按比例保持滚动位置
  useEffect(() => {
    const el = pdfLeftRef.current;
    if (!el || prevZoom.current === pdfZoom) return;
    const scrollable = el.scrollHeight - el.clientHeight;
    const ratio = scrollable > 0 ? el.scrollTop / scrollable : 0;
    requestAnimationFrame(() => {
      const newScrollable = el.scrollHeight - el.clientHeight;
      el.scrollTop = ratio * newScrollable;
    });
    prevZoom.current = pdfZoom;
  }, [pdfZoom]);

  // 当前结果模式（模版模式下即使没有 currentResult，只要有 currentFileContent 也可渲染）
  const canRender = hasCurrentResult || (isTemplateMode && !!currentFileContent);
  if (canRender && currentFileContent) {
    const desensitizedContent = currentResult?.content ?? currentFileContent;

    if (currentFileContent.type === "Spreadsheet") {
      const currentSheet = currentFileContent.sheets[activeSheetIndex] ?? currentFileContent.sheets[0];
      if (currentSheet) {
        ensureColWidths(currentSheet.headers.length);
      }
    }

    const isSpreadsheet = currentFileContent.type === "Spreadsheet";
    const sheets = isSpreadsheet ? currentFileContent.sheets : [];
    const showSheetTabs = sheets.length > 1;
    const currentFontSize = FONT_SIZE_LEVELS[fontSizeIndex];

    // 过滤当前 sheet 的 items（Document 类型 sheet_index 都为 0）
    const sheetItems = isSpreadsheet
      ? currentSensitiveItems.filter((i) => i.sheet_index === activeSheetIndex)
      : currentSensitiveItems;
    const sheetDesensitizedItems = isSpreadsheet
      ? desensitizedItems.filter((i) => i.sheet_index === activeSheetIndex)
      : desensitizedItems;

    // 当前 sheet 的列推断
    const sheetInferences = isSpreadsheet
      ? columnInferences.filter((i) => i.sheet_index === activeSheetIndex)
      : columnInferences;

    const isEmptyResult = currentSensitiveItems.length === 0 && (currentResult?.mappings.length ?? 0) === 0;

    return (
      <div className="flex-1 flex flex-col min-h-0 relative" data-testid="view-comparison">
        {/* 空结果提示横幅（模版模式不显示） */}
        {isEmptyResult && !isTemplateMode && (
          <div className="bg-amber-50 border-b border-amber-200 px-4 py-2.5 shrink-0 flex items-center gap-2 text-sm text-amber-800">
            <span>ℹ️</span>
            <span>{t("comparison.noSensitiveManualHint")}</span>
          </div>
        )}

        {/* 缩放工具栏（仅 Spreadsheet 显示） */}
        {isSpreadsheet && (
          <div className="bg-white border-b border-slate-200 px-4 py-1.5 shrink-0 flex items-center justify-end gap-2">
            <span className="text-xs text-slate-400 mr-1">{t("comparison.fontSize")}</span>
            <button
              className="w-6 h-6 flex items-center justify-center rounded text-slate-500 hover:bg-slate-100 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
              disabled={fontSizeIndex <= 0}
              onClick={() => setFontSizeIndex((i) => Math.max(0, i - 1))}
              title="缩小字号"
            >
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" className="w-4 h-4"><path fillRule="evenodd" d="M4 10a.75.75 0 0 1 .75-.75h10.5a.75.75 0 0 1 0 1.5H4.75A.75.75 0 0 1 4 10Z" clipRule="evenodd" /></svg>
            </button>
            <span className="text-xs text-slate-600 tabular-nums w-8 text-center">{currentFontSize}px</span>
            <button
              className="w-6 h-6 flex items-center justify-center rounded text-slate-500 hover:bg-slate-100 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
              disabled={fontSizeIndex >= FONT_SIZE_LEVELS.length - 1}
              onClick={() => setFontSizeIndex((i) => Math.min(FONT_SIZE_LEVELS.length - 1, i + 1))}
              title="放大字号"
            >
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" className="w-4 h-4"><path d="M10.75 4.75a.75.75 0 0 0-1.5 0v4.5h-4.5a.75.75 0 0 0 0 1.5h4.5v4.5a.75.75 0 0 0 1.5 0v-4.5h4.5a.75.75 0 0 0 0-1.5h-4.5v-4.5Z" /></svg>
            </button>
          </div>
        )}

        {/* PDF 缩放工具栏 */}
        {isPdf && (
          <div className="bg-white border-b border-slate-200 px-4 py-1.5 shrink-0 flex items-center justify-end gap-2">
            {/* 框选涂黑按钮 */}
            <button
              onClick={() => setRectSelectMode((v) => !v)}
              className={`flex items-center gap-1 text-xs px-2 py-1 rounded-md transition-colors ${
                rectSelectMode
                  ? "bg-blue-100 text-blue-700"
                  : "text-slate-500 hover:bg-slate-100"
              }`}
              title="框选涂黑"
            >
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" className="w-4 h-4">
                <rect x="3" y="3" width="14" height="14" rx="1" strokeWidth="1.5" strokeDasharray="4 2" />
              </svg>
              {t("comparison.rectSelect")}
            </button>
            <div className="w-px h-4 bg-slate-200 mx-1" />
            <span className="text-xs text-slate-400 mr-1">{t("comparison.zoom")}</span>
            <button
              className="w-6 h-6 flex items-center justify-center rounded text-slate-500 hover:bg-slate-100 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
              disabled={pdfZoom <= PDF_ZOOM_LEVELS[0]}
              onClick={() => setPdfZoom((z) => { const idx = PDF_ZOOM_LEVELS.indexOf(z); return idx > 0 ? PDF_ZOOM_LEVELS[idx - 1] : z; })}
              title="缩小"
            >
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" className="w-4 h-4"><path fillRule="evenodd" d="M4 10a.75.75 0 0 1 .75-.75h10.5a.75.75 0 0 1 0 1.5H4.75A.75.75 0 0 1 4 10Z" clipRule="evenodd" /></svg>
            </button>
            <span className="text-xs text-slate-600 tabular-nums w-8 text-center">{pdfZoom}%</span>
            <button
              className="w-6 h-6 flex items-center justify-center rounded text-slate-500 hover:bg-slate-100 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
              disabled={pdfZoom >= PDF_ZOOM_LEVELS[PDF_ZOOM_LEVELS.length - 1]}
              onClick={() => setPdfZoom((z) => { const idx = PDF_ZOOM_LEVELS.indexOf(z); return idx < PDF_ZOOM_LEVELS.length - 1 ? PDF_ZOOM_LEVELS[idx + 1] : z; })}
              title="放大"
            >
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" className="w-4 h-4"><path d="M10.75 4.75a.75.75 0 0 0-1.5 0v4.5h-4.5a.75.75 0 0 0 0 1.5h4.5v4.5a.75.75 0 0 0 1.5 0v-4.5h4.5a.75.75 0 0 0 0-1.5h-4.5v-4.5Z" /></svg>
            </button>
          </div>
        )}

        {/* DiffPanel */}
        <div className="flex-1 flex min-h-0">
          {/* 左侧：原始内容 */}
          <div ref={leftPanelRef} className="flex-1 flex flex-col min-w-0 border-r border-slate-200">
            <div className="bg-slate-50 px-4 py-2 text-xs font-bold uppercase tracking-wider text-slate-500 border-b border-slate-200 shrink-0 flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-slate-400" />
              {t("comparison.originalContent")}
            </div>
            {isPdf ? (
              <PdfPreviewView
                ref={pdfLeftRef}
                pages={pdfPages ?? undefined}
                items={sheetItems}
                zoom={pdfZoom}
                onScroll={handlePdfLeftScroll}
                scrollTop={pdfLeftScroll.top}
                scrollLeft={pdfLeftScroll.left}
                rectSelectMode={rectSelectMode}
                onAddItem={handleManualAddItem}
              />
            ) : (
              <ContentRenderer
                ref={leftRef}
                content={currentFileContent}
                items={sheetItems}
                onClickItem={handleLeftClickItem}
                colWidths={colWidths.length > 0 ? colWidths : undefined}
                onColResize={handleColResize}
                onScroll={handleLeftScroll}
                columnInferences={isColumnMode ? sheetInferences : undefined}
                confirmedColumnRules={isColumnMode ? sheetColumnRules : undefined}
                onHeaderClick={isColumnMode ? handleHeaderClick : undefined}
                diffMode={templateReplacements ? undefined : "removed"}
                fontSize={isSpreadsheet ? currentFontSize : undefined}
                sheetIndex={activeSheetIndex}
                activeItemId={selectedOriginalId ?? undefined}
                templateReplacements={templateReplacements}
              />
            )}
          </div>

          {/* 右侧：脱敏后/替换后内容 */}
          <div className="flex-1 flex flex-col min-w-0">
            <div className="bg-primary-50 px-4 py-2 text-xs font-bold uppercase tracking-wider text-primary-700 border-b border-primary-200 shrink-0 flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-primary-500" />
              {isTemplateMode ? t("comparison.replacePreview") : (isPdf ? t("comparison.redactPreview") : t("comparison.desensitizedContent"))}
            </div>
            {isPdf ? (
              <PdfPreviewView
                pages={pdfPages ?? undefined}
                items={sheetItems}
                showRedacted
                zoom={pdfZoom}
                onScroll={handlePdfRightScroll}
                scrollTop={pdfRightScroll.top}
                scrollLeft={pdfRightScroll.left}
                overlayRects={pdfOverlayRects}
              />
            ) : (
              <ContentRenderer
                ref={rightRef}
                content={isTemplateMode ? currentFileContent : desensitizedContent}
                items={isTemplateMode ? sheetItems : sheetDesensitizedItems}
                onClickItem={isTemplateMode ? undefined : handleRightClickItem}
                colWidths={colWidths.length > 0 ? colWidths : undefined}
                onColResize={handleColResize}
                onScroll={handleRightScroll}
                templateReplacements={isTemplateMode ? templateReplacements : undefined}
                showReplacedText={isTemplateMode}
                diffMode={isTemplateMode ? undefined : "added"}
                fontSize={isSpreadsheet ? currentFontSize : undefined}
                sheetIndex={activeSheetIndex}
                activeItemId={isTemplateMode ? undefined : (selectedOriginalId ? `desen_${selectedOriginalId}` : undefined)}
              />
            )}
          </div>
        </div>

        {/* Sheet Tab 栏（仅多 Sheet 时显示） */}
        {showSheetTabs && (
          <div className="bg-white border-t border-slate-200 px-2 py-1 shrink-0 flex items-center gap-0.5 overflow-x-auto">
            {sheets.map((sheet, idx) => (
              <button
                key={idx}
                onClick={() => setActiveSheetIndex(idx)}
                className={`px-3 py-1.5 text-xs rounded-t transition-colors whitespace-nowrap ${
                  idx === activeSheetIndex
                    ? "bg-primary-50 text-primary-700 font-medium border border-b-0 border-primary-200"
                    : "text-slate-500 hover:text-slate-700 hover:bg-slate-50"
                }`}
              >
                {sheet.name || `Sheet ${idx + 1}`}
              </button>
            ))}
          </div>
        )}

        {/* 手动标记工具栏（左侧面板） */}
        <TextSelectionToolbar
          containerRef={leftPanelRef}
          onAddItem={handleManualAddItem}
          isTemplateMode={isTemplateMode}
          sheetIndex={activeSheetIndex}
          onAddTemplateMapping={async (text, type, replacement) => {
            await useWorkspaceStore.getState().addDictEntryFromPopover(text, type, replacement);
            await reDetectDict();
          }}
        />

        {/* 敏感项编辑浮层 */}
        <SensitivePopover
          item={sensitivePopoverItem}
          anchorRect={sensitivePopoverAnchor}
          onClose={handleSensitivePopoverClose}
          onRemoveItem={handleRemoveItem}
          onAddToWhitelist={handleAddToWhitelist}
        />

        {/* 列级微调 Popover */}
        {isColumnMode && popoverCol !== null && anchorRect && (
          <ColumnRulePopover
            col={popoverCol}
            inference={sheetInferences.find((i) => i.col === popoverCol) ?? null}
            currentRule={sheetColumnRules[popoverCol] ?? null}
            onConfirm={handlePopoverConfirm}
            onSkip={handlePopoverSkip}
            onClose={handlePopoverClose}
            anchorRect={anchorRect}
          />
        )}
      </div>
    );
  }

  // 查看历史记录模式（仅展示映射表摘要，无原始内容可对比）
  if (hasRecordResult && activeRecord) {
    const record = activeRecord;
    return (
      <div className="flex-1 flex flex-col min-h-0">
        <div className="flex-1 overflow-auto p-6">
          <h2 className="text-lg font-semibold text-slate-800 mb-4">
            {t("comparison.recordTitle", { name: record.file_name })}
          </h2>
          <div className="text-sm text-slate-500 mb-6">
            {t("comparison.processedAt")}{new Date(record.processed_at).toLocaleString()} ·
            {t("comparison.sensitiveItems", { count: record.sensitive_count })}
          </div>

          {/* 映射表 */}
          <h3 className="text-xs font-bold uppercase tracking-wider text-slate-500 mb-3">{t("comparison.mappingTable")}</h3>
          <div className="border border-slate-200 rounded-lg overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-slate-50">
                  <th className="text-left px-4 py-2 text-xs font-bold uppercase tracking-wider text-slate-500">{t("comparison.originalText")}</th>
                  <th className="text-left px-4 py-2 text-xs font-bold uppercase tracking-wider text-slate-500">{t("comparison.desensitizedText")}</th>
                  <th className="text-left px-4 py-2 text-xs font-bold uppercase tracking-wider text-slate-500">{t("comparison.type")}</th>
                  <th className="text-left px-4 py-2 text-xs font-bold uppercase tracking-wider text-slate-500">{t("comparison.strategy")}</th>
                  <th className="text-right px-4 py-2 text-xs font-bold uppercase tracking-wider text-slate-500">{t("comparison.occurrences")}</th>
                </tr>
              </thead>
              <tbody>
                {record.mappings.map((m, i) => {
                  const typeKey = typeof m.sensitive_type === "string" ? m.sensitive_type : "Custom";
                  const info = SENSITIVE_TYPE_CONFIG[typeKey];
                  return (
                    <tr key={i} className="border-t border-slate-100">
                      <td className="px-4 py-2 text-slate-800 font-mono">{m.original_text}</td>
                      <td className="px-4 py-2 text-primary-700 font-mono">{m.replaced_text}</td>
                      <td className="px-4 py-2">
                        <span className={`px-1.5 py-0.5 rounded text-xs ${info?.bgClass || ""} ${info?.textClass || ""}`}>
                          {info?.label || typeKey}
                        </span>
                      </td>
                      <td className="px-4 py-2 text-slate-500">{m.strategy}</td>
                      <td className="px-4 py-2 text-right text-slate-500">{m.occurrences}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>

        <div className="bg-white border-t border-slate-200 px-6 py-3 shrink-0 flex justify-between">
          <button
            onClick={() => setCenterView("dropzone")}
            className="px-4 py-2 text-sm text-slate-600 hover:text-slate-800 hover:bg-slate-100 rounded-lg transition-colors"
          >
            {t("common.back")}
          </button>
          <button
            onClick={() => handleReprocess(record)}
            disabled={reprocessing}
            className="inline-flex items-center gap-1.5 px-4 py-2 text-sm font-medium text-primary-600 hover:text-primary-700 hover:bg-primary-50 rounded-lg transition-colors disabled:opacity-50"
          >
            <RefreshCw className={`w-4 h-4 ${reprocessing ? "animate-spin" : ""}`} />
            {reprocessing ? t("comparison.reprocessing") : t("comparison.reprocess")}
          </button>
        </div>
      </div>
    );
  }

  // 无数据
  return (
    <div className="flex-1 flex items-center justify-center text-slate-400">
      {t("comparison.noData")}
    </div>
  );
}
