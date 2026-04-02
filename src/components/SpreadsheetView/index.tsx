import { useRef, useState, useCallback, useEffect, useMemo, forwardRef, useImperativeHandle } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ChevronDown } from "lucide-react";
import { HighlightedText } from "../HighlightedText";
import type { SensitiveItem, ColumnInference, ColumnRule, CellValue } from "../../types";
import { SENSITIVE_TYPE_CONFIG, STRATEGY_LABELS, getSensitiveTypeKey } from "../../types";

/** 列默认最小宽度（px） */
const COL_MIN_WIDTH = 80;
const COL_DEFAULT_WIDTH = 150;
const ROW_NUM_WIDTH = 48;

export interface SpreadsheetViewHandle {
  /** 获取内部滚动容器 */
  getScrollElement: () => HTMLDivElement | null;
}

interface SpreadsheetViewProps {
  headers: string[];
  rows: CellValue[][];
  items: SensitiveItem[];
  onClickItem?: (item: SensitiveItem, event: React.MouseEvent) => void;
  /** 受控列宽（如不传则使用内部状态） */
  colWidths?: number[];
  /** 列宽变化回调 */
  onColResize?: (colIndex: number, width: number) => void;
  /** 滚动事件回调（用于外部同步） */
  onScroll?: (scrollLeft: number, scrollTop: number) => void;
  /** 列级推断结果 */
  columnInferences?: ColumnInference[];
  /** 已确认的列规则 */
  confirmedColumnRules?: Record<number, ColumnRule>;
  /** 列头点击回调 */
  onHeaderClick?: (col: number, rect: DOMRect) => void;
  /** Diff 模式，透传给 HighlightedText */
  diffMode?: "removed" | "added";
  /** 单元格字号（px） */
  fontSize?: number;
  /** 当前选中高亮的项 ID */
  activeItemId?: string;
  /** 模版替换模式：原文 → 替换值 映射 */
  templateReplacements?: Map<string, string>;
  /** 右侧预览时为 true，显示替换后的文本而非原文 */
  showReplacedText?: boolean;
}

export const SpreadsheetView = forwardRef<SpreadsheetViewHandle, SpreadsheetViewProps>(
  function SpreadsheetView(
    { headers, rows, items, onClickItem, colWidths: controlledWidths, onColResize, onScroll, columnInferences, confirmedColumnRules, onHeaderClick, diffMode, fontSize, activeItemId, templateReplacements, showReplacedText },
    ref
  ) {
    const parentRef = useRef<HTMLDivElement>(null);
    const headerRef = useRef<HTMLDivElement>(null);

    // 内部列宽（非受控时使用）
    const [internalWidths, setInternalWidths] = useState<number[]>(() =>
      headers.map(() => COL_DEFAULT_WIDTH)
    );
    const colWidths = controlledWidths ?? internalWidths;

    // 当 headers 长度变化时重置内部宽度
    useEffect(() => {
      if (!controlledWidths) {
        setInternalWidths(headers.map(() => COL_DEFAULT_WIDTH));
      }
    }, [headers.length, controlledWidths]);

    // 暴露滚动容器给父组件
    useImperativeHandle(ref, () => ({
      getScrollElement: () => parentRef.current,
    }));

    const rowVirtualizer = useVirtualizer({
      count: rows.length,
      getScrollElement: () => parentRef.current,
      estimateSize: () => 40,
      overscan: 10,
    });

    // 预计算 cell → items 映射，O(1) 查找（RI-12 性能优化）
    const cellItemsMap = useMemo(() => {
      const map = new Map<string, SensitiveItem[]>();
      for (const item of items) {
        const key = `${item.row}:${item.col}`;
        const arr = map.get(key);
        if (arr) {
          arr.push(item);
        } else {
          map.set(key, [item]);
        }
      }
      return map;
    }, [items]);

    const getItemsForCell = (row: number, col: number): SensitiveItem[] => {
      return cellItemsMap.get(`${row}:${col}`) ?? [];
    };

    // 同步表头横向滚动 + 通知外部
    const handleScroll = useCallback(() => {
      if (parentRef.current && headerRef.current) {
        headerRef.current.scrollLeft = parentRef.current.scrollLeft;
      }
      if (parentRef.current && onScroll) {
        onScroll(parentRef.current.scrollLeft, parentRef.current.scrollTop);
      }
    }, [onScroll]);

    // --- 列拖拽调整 ---
    const resizingCol = useRef<number | null>(null);
    const resizeStartX = useRef(0);
    const resizeStartWidth = useRef(0);

    const handleResizeStart = useCallback(
      (colIndex: number, e: React.MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
        resizingCol.current = colIndex;
        resizeStartX.current = e.clientX;
        resizeStartWidth.current = colWidths[colIndex] ?? COL_DEFAULT_WIDTH;

        const handleMouseMove = (ev: MouseEvent) => {
          if (resizingCol.current === null) return;
          const delta = ev.clientX - resizeStartX.current;
          const newWidth = Math.max(COL_MIN_WIDTH, resizeStartWidth.current + delta);
          if (onColResize) {
            onColResize(resizingCol.current, newWidth);
          } else {
            setInternalWidths((prev) => {
              const next = [...prev];
              next[resizingCol.current!] = newWidth;
              return next;
            });
          }
        };

        const handleMouseUp = () => {
          resizingCol.current = null;
          document.removeEventListener("mousemove", handleMouseMove);
          document.removeEventListener("mouseup", handleMouseUp);
          document.body.style.cursor = "";
          document.body.style.userSelect = "";
        };

        document.body.style.cursor = "col-resize";
        document.body.style.userSelect = "none";
        document.addEventListener("mousemove", handleMouseMove);
        document.addEventListener("mouseup", handleMouseUp);
      },
      [colWidths, onColResize]
    );

    // 计算表格总宽度
    const tableWidth = ROW_NUM_WIDTH + colWidths.reduce((sum, w) => sum + w, 0);

    return (
      <div className="flex-1 flex flex-col min-h-0">
        {/* 固定表头 */}
        <div
          ref={headerRef}
          className="bg-slate-50/80 border-b border-slate-200 overflow-hidden shrink-0"
        >
          <table className="table-fixed" style={{ width: `${tableWidth}px` }}>
            <colgroup>
              <col style={{ width: `${ROW_NUM_WIDTH}px` }} />
              {colWidths.map((w, i) => (
                <col key={i} style={{ width: `${w}px` }} />
              ))}
            </colgroup>
            <thead>
              <tr>
                <th className="px-3 py-2 text-left text-xs font-semibold text-slate-500 uppercase border-r border-slate-200 bg-slate-50/50 font-mono tabular-nums">
                  #
                </th>
                {headers.map((header, i) => {
                  const inference = columnInferences?.find((inf) => inf.col === i);
                  const rule = confirmedColumnRules?.[i];
                  const isClickable = !!onHeaderClick;

                  // 确定标签
                  let tag: React.ReactNode = null;
                  if (rule) {
                    const info = SENSITIVE_TYPE_CONFIG[rule.sensitive_type];
                    const strategyType = typeof rule.strategy === "string"
                      ? rule.strategy as keyof typeof STRATEGY_LABELS
                      : "Mask" as keyof typeof STRATEGY_LABELS;
                    tag = (
                      <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-emerald-50 text-emerald-600 ring-1 ring-emerald-200 mt-0.5">
                        {info?.label || rule.sensitive_type} · {STRATEGY_LABELS[strategyType]}
                      </span>
                    );
                  } else if (inference?.inferred_type) {
                    const typeKey = getSensitiveTypeKey(inference.inferred_type);
                    const info = SENSITIVE_TYPE_CONFIG[typeKey];
                    tag = (
                      <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-amber-50 text-amber-600 ring-1 ring-amber-200 mt-0.5">
                        {info?.label || typeKey} {(inference.confidence * 100).toFixed(0)}%
                      </span>
                    );
                  }

                  return (
                    <th
                      key={i}
                      className={`relative px-3 py-2 text-left text-xs font-semibold border-r border-slate-200 last:border-r-0 group transition-colors ${
                        isClickable
                          ? "cursor-pointer text-slate-500 hover:bg-primary-50 hover:text-primary-600"
                          : "text-slate-500"
                      }`}
                      onClick={(e) => {
                        if (onHeaderClick) {
                          const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
                          onHeaderClick(i, rect);
                        }
                      }}
                    >
                      <div className="truncate flex items-center gap-1">
                        <span className="truncate">{header}</span>
                        {isClickable && (
                          <ChevronDown className="w-3 h-3 shrink-0 opacity-40 group-hover:opacity-100 transition-opacity" />
                        )}
                      </div>
                      <div className="h-5 flex items-center">{tag}</div>
                      {/* 拖拽调整手柄 */}
                      <div
                        className="absolute top-0 right-0 w-1.5 h-full cursor-col-resize hover:bg-primary-400 active:bg-primary-500 transition-colors"
                        onMouseDown={(e) => {
                          e.stopPropagation();
                          handleResizeStart(i, e);
                        }}
                      />
                    </th>
                  );
                })}
              </tr>
            </thead>
          </table>
        </div>

        {/* 虚拟滚动区域 */}
        <div
          ref={parentRef}
          className="flex-1 overflow-auto"
          onScroll={handleScroll}
        >
          <div
            style={{
              height: `${rowVirtualizer.getTotalSize()}px`,
              width: `${tableWidth}px`,
              position: "relative",
            }}
          >
            {rowVirtualizer.getVirtualItems().map((virtualRow) => {
              const row = rows[virtualRow.index];
              const isOdd = virtualRow.index % 2 === 1;
              return (
                <div
                  key={virtualRow.index}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: `${tableWidth}px`,
                    height: `${virtualRow.size}px`,
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                >
                  <table className="table-fixed" style={{ width: `${tableWidth}px` }}>
                    <colgroup>
                      <col style={{ width: `${ROW_NUM_WIDTH}px` }} />
                      {colWidths.map((w, i) => (
                        <col key={i} style={{ width: `${w}px` }} />
                      ))}
                    </colgroup>
                    <tbody>
                      <tr className={`border-b border-slate-100 hover:bg-primary-50/30 ${isOdd ? "bg-slate-50/40" : ""}`}>
                        <td
                          className="px-3 py-2 text-xs text-slate-400 border-r border-slate-200 bg-slate-50/50 font-mono tabular-nums"
                          style={{ width: `${ROW_NUM_WIDTH}px` }}
                        >
                          {virtualRow.index + 1}
                        </td>
                        {row.map((cellValue, colIdx) => {
                          const cell = cellValue.text;
                          const cellItems = getItemsForCell(
                            virtualRow.index + 1,
                            colIdx
                          );
                          const hasRule = !!confirmedColumnRules?.[colIdx];
                          return (
                            <td
                              key={colIdx}
                              data-row={virtualRow.index + 1}
                              data-col={colIdx}
                              className={`px-3 py-2 text-slate-700 border-r border-slate-200 last:border-r-0 overflow-hidden ${
                                hasRule ? "bg-emerald-50/50" : ""
                              }`}
                            style={fontSize ? { fontSize: `${fontSize}px` } : { fontSize: '14px' }}
                            >
                              <div className="truncate">
                                {cellItems.length > 0 ? (
                                  <HighlightedText
                                    text={cell}
                                    items={cellItems}
                                    onClickItem={onClickItem}
                                    diffMode={diffMode}
                                    activeItemId={activeItemId}
                                    templateReplacements={templateReplacements}
                                    showReplacedText={showReplacedText}
                                  />
                                ) : (
                                  cell
                                )}
                              </div>
                            </td>
                          );
                        })}
                      </tr>
                    </tbody>
                  </table>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    );
  }
);
