import { useEffect, useState, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useDetectStore } from "../../stores/detectStore";
import { SENSITIVE_TYPE_CONFIG } from "../../types";
import type { SensitiveItem, SensitiveType, PdfBbox } from "../../types";

interface SelectionInfo {
  text: string;
  row: number;
  col: number;
  start: number;
  end: number;
  rect: DOMRect;
  /** 多行选中按行拆 N 个 bbox；矩形框选长度为 1；跨页时仅保留 startSpan 所在页 */
  pdf_bboxes?: PdfBbox[];
}

/** 从 DOM 选区中提取坐标信息 */
function getSelectionInfo(): SelectionInfo | null {
  const sel = window.getSelection();
  if (!sel || sel.isCollapsed || !sel.rangeCount) return null;

  const text = sel.toString().trim();
  if (!text) return null;

  const range = sel.getRangeAt(0);

  // 找到包含 data-row 的祖先容器
  const container = findDataContainer(range.startContainer);
  if (!container) return null;

  const row = parseInt(container.getAttribute("data-row") || "-1", 10);
  const col = parseInt(container.getAttribute("data-col") || "0", 10);
  if (row < 0) return null;

  // PDF 文本层模式：使用 data-char-offset 计算偏移，并按行拆 pdf_bboxes
  // （pdf_bboxes 是关键路径——后端按 bbox 涂黑，绕开 row/start-end 与
  // paragraph 索引/段内偏移不匹配的问题。多行选中用 getClientRects 拆每行
  // 一个 bbox，避免 getBoundingClientRect 的并集矩形横盖行间无关文字。
  // 跨页时仅保留 startSpan 所在页的 client rect。）
  const startSpan = findCharOffsetSpan(range.startContainer);
  if (startSpan) {
    const baseOffset = parseInt(startSpan.getAttribute("data-char-offset") || "0", 10);
    const start = baseOffset + range.startOffset;
    const end = start + text.length;
    const rect = range.getBoundingClientRect();
    const pdfPageEl = startSpan.closest("[data-pdf-page]") as HTMLElement | null;
    let pdf_bboxes: PdfBbox[] | undefined;
    if (pdfPageEl) {
      const pageRect = pdfPageEl.getBoundingClientRect();
      // 防御：容器尚未布局或被 display:none 时 width/height=0，避免算出 Infinity
      // 让 bbox 失败后端走文字匹配路径，至少不会破坏 IPC 序列化
      if (pageRect.width > 0 && pageRect.height > 0) {
        const pageIdx = parseInt(pdfPageEl.getAttribute("data-pdf-page") || "0", 10);
        const clientRects = Array.from(range.getClientRects());
        // 跨页过滤：只保留中心点落在 startPage 矩形内的 rect
        const inPage = clientRects.filter((r) => {
          if (r.width <= 0 || r.height <= 0) return false;
          const cx = r.left + r.width / 2;
          const cy = r.top + r.height / 2;
          return (
            cx >= pageRect.left && cx <= pageRect.right &&
            cy >= pageRect.top && cy <= pageRect.bottom
          );
        });
        if (inPage.length > 0) {
          pdf_bboxes = inPage.map((r) => ({
            page_index: pageIdx,
            left: (r.left - pageRect.left) / pageRect.width,
            top: (r.top - pageRect.top) / pageRect.height,
            right: (r.right - pageRect.left) / pageRect.width,
            bottom: (r.bottom - pageRect.top) / pageRect.height,
          }));
        }
      }
    }
    return { text, row, col, start, end, rect, pdf_bboxes };
  }

  // 计算文本偏移：遍历容器内的文本节点
  const start = getTextOffset(container, range.startContainer, range.startOffset);
  const end = getTextOffset(container, range.endContainer, range.endOffset);

  if (start < 0 || end <= start) return null;

  const rect = range.getBoundingClientRect();
  return { text, row, col, start, end, rect };
}

/** 向上查找带 data-row 属性的容器 */
function findDataContainer(node: Node): HTMLElement | null {
  let current: Node | null = node;
  while (current) {
    if (current instanceof HTMLElement && current.hasAttribute("data-row")) {
      return current;
    }
    current = current.parentNode;
  }
  return null;
}

/** 向上查找带 data-char-offset 属性的 span（PDF 文本层） */
function findCharOffsetSpan(node: Node): HTMLElement | null {
  let current: Node | null = node;
  while (current) {
    if (current instanceof HTMLElement && current.hasAttribute("data-char-offset")) {
      return current;
    }
    current = current.parentNode;
  }
  return null;
}

/** 计算 node+offset 在 container 内的文本偏移量 */
function getTextOffset(container: Node, targetNode: Node, targetOffset: number): number {
  let offset = 0;
  const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT);

  let node: Node | null;
  while ((node = walker.nextNode())) {
    if (node === targetNode) {
      return offset + targetOffset;
    }
    offset += (node.textContent || "").length;
  }

  // targetNode 可能不是文本节点（例如元素节点）
  // 此时 targetOffset 表示子节点索引
  if (targetNode === container || container.contains(targetNode)) {
    return offset + targetOffset;
  }

  return -1;
}

/** 常用敏感类型（快速选择） */
const COMMON_TYPES = ["Phone", "IdCard", "PersonName", "Email", "Address", "BankCard"];

interface TextSelectionToolbarProps {
  /** 工具条所在的容器元素（用于限定选区范围） */
  containerRef: React.RefObject<HTMLElement | null>;
  /** 自定义添加回调（优先于 detectStore.addItem） */
  onAddItem?: (item: SensitiveItem) => void;
  /** 是否为模版替换模式 */
  isTemplateMode?: boolean;
  /** 模版模式下添加词典映射的回调 */
  onAddTemplateMapping?: (text: string, sensitiveType: SensitiveType, replacement: string) => void;
  /** 当前活动的 sheet 索引（多 sheet Excel） */
  sheetIndex?: number;
}

export function TextSelectionToolbar({
  containerRef,
  onAddItem,
  isTemplateMode,
  onAddTemplateMapping,
  sheetIndex = 0,
}: TextSelectionToolbarProps) {
  const { t } = useTranslation();
  const [selectionInfo, setSelectionInfo] = useState<SelectionInfo | null>(null);
  const [showTypeList, setShowTypeList] = useState(false);
  const toolbarRef = useRef<HTMLDivElement>(null);
  const addItem = useDetectStore((s) => s.addItem);

  // 模版模式状态：已选类型和替换值
  const [selectedType, setSelectedType] = useState<string | null>(null);
  const [templateReplacement, setTemplateReplacement] = useState("");

  const handleMouseUp = useCallback(() => {
    // 延迟一帧以确保选区已更新
    requestAnimationFrame(() => {
      const info = getSelectionInfo();
      if (info && containerRef.current) {
        // 确保选区在容器内
        const containerRect = containerRef.current.getBoundingClientRect();
        if (
          info.rect.top >= containerRect.top &&
          info.rect.bottom <= containerRect.bottom + 50
        ) {
          setSelectionInfo(info);
          setShowTypeList(false);
        } else {
          setSelectionInfo(null);
          setSelectedType(null);
          setTemplateReplacement("");
        }
      } else {
        setSelectionInfo(null);
        setSelectedType(null);
        setTemplateReplacement("");
      }
    });
  }, [containerRef]);

  const handleMouseDown = useCallback((e: MouseEvent) => {
    // 如果点击在工具条内，不关闭
    if (toolbarRef.current?.contains(e.target as Node)) return;
    setSelectionInfo(null);
    setSelectedType(null);
    setTemplateReplacement("");
  }, []);

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === "Escape") {
      setSelectionInfo(null);
      setSelectedType(null);
      setTemplateReplacement("");
      window.getSelection()?.removeAllRanges();
    }
  }, []);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    el.addEventListener("mouseup", handleMouseUp);
    document.addEventListener("mousedown", handleMouseDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      el.removeEventListener("mouseup", handleMouseUp);
      document.removeEventListener("mousedown", handleMouseDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [containerRef, handleMouseUp, handleMouseDown, handleKeyDown]);

  /** 非模版模式：选择类型后直接添加敏感项 */
  const handleSelectType = useCallback(
    (typeKey: string) => {
      if (!selectionInfo) return;

      const sensitiveType: SensitiveType =
        typeKey === "Custom" ? { Custom: "Custom" } : (typeKey as SensitiveType);

      const item: SensitiveItem = {
        id: `manual_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
        text: selectionInfo.text,
        sensitive_type: sensitiveType,
        source: "Manual",
        confidence: 1.0,
        start: selectionInfo.start,
        end: selectionInfo.end,
        row: selectionInfo.row,
        col: selectionInfo.col,
        sheet_index: sheetIndex,
        pdf_bboxes: selectionInfo.pdf_bboxes,
      };

      if (onAddItem) {
        onAddItem(item);
      } else {
        addItem(item);
      }
      setSelectionInfo(null);
      window.getSelection()?.removeAllRanges();
    },
    [selectionInfo, addItem, onAddItem, sheetIndex]
  );

  /** 模版模式：选择类型 + 输入替换值后添加词典映射和敏感项 */
  const handleTemplateAdd = useCallback(() => {
    if (!selectionInfo || !selectedType || !templateReplacement.trim()) return;

    const sensitiveType: SensitiveType =
      selectedType === "Custom"
        ? { Custom: "Custom" }
        : (selectedType as SensitiveType);

    // 调用回调添加词典映射
    onAddTemplateMapping?.(
      selectionInfo.text,
      sensitiveType,
      templateReplacement.trim()
    );

    // 同时也添加为 SensitiveItem（用于高亮显示）
    const item: SensitiveItem = {
      id: `manual_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
      text: selectionInfo.text,
      sensitive_type: sensitiveType,
      confidence: 1.0,
      source: "Manual",
      start: selectionInfo.start,
      end: selectionInfo.end,
      row: selectionInfo.row,
      col: selectionInfo.col,
      sheet_index: sheetIndex,
      pdf_bboxes: selectionInfo.pdf_bboxes,
    };
    onAddItem?.(item);

    // 清理状态
    setSelectionInfo(null);
    setSelectedType(null);
    setTemplateReplacement("");
    window.getSelection()?.removeAllRanges();
  }, [selectionInfo, selectedType, templateReplacement, onAddTemplateMapping, onAddItem, sheetIndex]);

  if (!selectionInfo) return null;

  // 工具条位置：选区上方居中
  const top = selectionInfo.rect.top - (showTypeList ? 8 : 44);
  const left = selectionInfo.rect.left + selectionInfo.rect.width / 2;

  // 常用类型（前排显示）
  const mainTypes = COMMON_TYPES;

  // --- 模版模式 UI ---
  if (isTemplateMode) {
    return (
      <div
        ref={toolbarRef}
        className="fixed z-50 -translate-x-1/2"
        style={{ top: `${top}px`, left: `${left}px` }}
      >
        {/* 第一步：选择类型 */}
        {!selectedType && (
          <div className="bg-white rounded-xl shadow-float border border-slate-200/80 overflow-hidden animate-slide-up">
            <div className="px-3 pt-2.5 pb-2">
              <div className="text-[11px] text-slate-400 mb-1.5 tracking-wider">{t("textToolbar.selectType")}</div>
              <div className="flex flex-wrap gap-1">
                {COMMON_TYPES.map((type) => {
                  const config = SENSITIVE_TYPE_CONFIG[type];
                  return (
                    <button
                      key={type}
                      onClick={() => setSelectedType(type)}
                      className={`text-xs px-2 py-1 rounded-md ${config?.bgClass} ${config?.textClass} hover:opacity-75 transition-opacity`}
                    >
                      {config?.label || type}
                    </button>
                  );
                })}
                <button
                  onClick={() => setSelectedType("Custom")}
                  className="text-xs px-2 py-1 rounded-md bg-slate-100 text-slate-600 hover:opacity-75 transition-opacity"
                >
                  {t("textToolbar.custom")}
                </button>
              </div>
            </div>
          </div>
        )}

        {/* 第二步：输入替换值 */}
        {selectedType && (
          <div className="bg-white rounded-xl shadow-float border border-slate-200/80 overflow-hidden animate-slide-up w-64">
            <div className="px-3.5 pt-3 pb-2.5 space-y-2">
              <div className="text-xs text-slate-500">
                <span className={`inline-block rounded-full px-1.5 py-0.5 text-[11px] font-medium ${SENSITIVE_TYPE_CONFIG[selectedType]?.bgClass || "bg-slate-100"} ${SENSITIVE_TYPE_CONFIG[selectedType]?.textClass || "text-slate-700"} mr-1.5`}>
                  {SENSITIVE_TYPE_CONFIG[selectedType]?.label || t("textToolbar.custom")}
                </span>
                <span className="font-medium text-slate-700">{selectionInfo.text}</span>
              </div>
              <div>
                <label className="text-[11px] text-slate-400 mb-1 block tracking-wider">{t("strategyPanel.replaceTo")}</label>
                <input
                  value={templateReplacement}
                  onChange={(e) => setTemplateReplacement(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      handleTemplateAdd();
                      e.stopPropagation();
                    } else if (e.key === "Escape") {
                      setSelectedType(null);
                      setTemplateReplacement("");
                      e.stopPropagation();
                    }
                  }}
                  className="w-full text-sm px-2.5 py-1.5 border border-slate-200 bg-slate-50/80 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-500 transition-colors"
                  placeholder="输入替换值..."
                  autoFocus
                />
              </div>
            </div>
            <div className="border-t border-slate-100 px-3 py-2 flex gap-2 justify-end bg-slate-50/50">
              <button
                onClick={() => {
                  setSelectedType(null);
                  setTemplateReplacement("");
                }}
                className="text-xs px-3 py-1.5 text-slate-400 hover:text-slate-600 rounded-lg hover:bg-slate-100 transition-colors"
              >
                {t("common.back")}
              </button>
              <button
                onClick={handleTemplateAdd}
                disabled={!templateReplacement.trim()}
                className="text-xs px-3 py-1.5 bg-teal-500 text-white rounded-lg hover:bg-teal-600 disabled:opacity-40 transition-colors"
              >
                {t("common.add")}
              </button>
            </div>
          </div>
        )}
      </div>
    );
  }

  // --- 非模版模式 UI ---
  return (
    <div
      ref={toolbarRef}
      className="fixed z-50 -translate-x-1/2"
      style={{ top: `${top}px`, left: `${left}px` }}
    >
      {!showTypeList ? (
        <div className="bg-white rounded-xl shadow-float border border-slate-200/80 px-2.5 py-2 animate-slide-up">
          <div className="flex items-center gap-1 text-xs">
            {mainTypes.map((key) => {
              const config = SENSITIVE_TYPE_CONFIG[key];
              return (
                <button
                  key={key}
                  onClick={() => handleSelectType(key)}
                  className={`px-2 py-1 rounded-md ${config?.bgClass} ${config?.textClass} hover:opacity-75 transition-opacity whitespace-nowrap`}
                  title={config?.label}
                >
                  {config?.label}
                </button>
              );
            })}
            <div className="w-px h-4 bg-slate-200 mx-0.5" />
            <button
              onClick={() => setShowTypeList(true)}
              className="px-2 py-1 rounded-md text-slate-400 hover:text-slate-600 hover:bg-slate-100 transition-colors whitespace-nowrap"
              title={t("textToolbar.moreTypes")}
            >
              {t("textToolbar.moreTypes")}
            </button>
          </div>
        </div>
      ) : (
        <div className="bg-white rounded-xl shadow-float border border-slate-200/80 overflow-hidden animate-slide-up min-w-[260px]">
          <div className="px-3 pt-2.5 pb-2">
            <div className="flex items-center gap-2 mb-2">
              <button
                onClick={() => setShowTypeList(false)}
                className="text-slate-400 hover:text-slate-600 transition-colors"
                title="返回"
              >
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
                </svg>
              </button>
              <span className="text-[11px] text-slate-400 tracking-wider">{t("textToolbar.selectType")}</span>
            </div>
            <div className="flex flex-wrap gap-1">
              {Object.entries(SENSITIVE_TYPE_CONFIG).map(([key, config]) => (
                <button
                  key={key}
                  onClick={() => handleSelectType(key)}
                  className={`text-xs px-2 py-1 rounded-md ${config.bgClass} ${config.textClass} hover:opacity-75 transition-opacity`}
                >
                  {config.label}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
