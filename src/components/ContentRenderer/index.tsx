import { forwardRef, useRef, useCallback, useImperativeHandle, useMemo } from "react";
import { SpreadsheetView } from "../SpreadsheetView";
import type { SpreadsheetViewHandle } from "../SpreadsheetView";
import { HighlightedText } from "../HighlightedText";
import type { FileContent, SensitiveItem, Paragraph, ColumnInference, ColumnRule } from "../../types";

interface ContentRendererProps {
  content: FileContent;
  items: SensitiveItem[];
  onClickItem?: (item: SensitiveItem, event: React.MouseEvent) => void;
  /** 受控列宽 */
  colWidths?: number[];
  /** 列宽变化回调 */
  onColResize?: (colIndex: number, width: number) => void;
  /** 滚动事件回调 */
  onScroll?: (scrollLeft: number, scrollTop: number) => void;
  /** 列级推断结果 */
  columnInferences?: ColumnInference[];
  /** 已确认的列规则 */
  confirmedColumnRules?: Record<number, ColumnRule>;
  /** 列头点击回调 */
  onHeaderClick?: (col: number, rect: DOMRect) => void;
  /** Diff 模式，透传给高亮组件 */
  diffMode?: "removed" | "added";
  /** 单元格字号（px），仅对 Spreadsheet 生效 */
  fontSize?: number;
  /** 当前 Sheet 索引 */
  sheetIndex?: number;
  /** 当前选中高亮的项 ID */
  activeItemId?: string;
  /** 模版替换模式：原文 → 替换值 映射 */
  templateReplacements?: Map<string, string>;
  /** 右侧预览时为 true，显示替换后的文本而非原文 */
  showReplacedText?: boolean;
}

export const ContentRenderer = forwardRef<SpreadsheetViewHandle, ContentRendererProps>(
  function ContentRenderer(
    { content, items, onClickItem, colWidths, onColResize, onScroll, columnInferences, confirmedColumnRules, onHeaderClick, diffMode, fontSize, sheetIndex = 0, activeItemId, templateReplacements, showReplacedText },
    ref
  ) {
    if (content.type === "Spreadsheet") {
      const sheet = content.sheets[sheetIndex] ?? content.sheets[0];
      if (!sheet) return null;
      return (
        <SpreadsheetView
          ref={ref}
          headers={sheet.headers}
          rows={sheet.rows}
          items={items}
          onClickItem={onClickItem}
          colWidths={colWidths}
          onColResize={onColResize}
          onScroll={onScroll}
          columnInferences={columnInferences}
          confirmedColumnRules={confirmedColumnRules}
          onHeaderClick={onHeaderClick}
          diffMode={diffMode}
          fontSize={fontSize}
          activeItemId={activeItemId}
          templateReplacements={templateReplacements}
          showReplacedText={showReplacedText}
        />
      );
    }

    if (content.type === "Document") {
      return (
        <DocumentView
          ref={ref}
          paragraphs={content.paragraphs}
          items={items}
          onClickItem={onClickItem}
          onScroll={onScroll}
          diffMode={diffMode}
          activeItemId={activeItemId}
          templateReplacements={templateReplacements}
          showReplacedText={showReplacedText}
        />
      );
    }

    return null;
  }
);

/** 渲染块类型：普通段落或表格块 */
type RenderBlock =
  | { type: "paragraph"; paragraph: Paragraph }
  | { type: "table"; tableIndex: number; rows: Map<number, Paragraph[]> };

/** 将段落列表按表格结构分组为渲染块 */
function groupParagraphsToBlocks(paragraphs: Paragraph[]): RenderBlock[] {
  const result: RenderBlock[] = [];
  let currentTable: RenderBlock | null = null;

  for (const p of paragraphs) {
    if (p.table_position) {
      const ti = p.table_position.table_index;
      if (currentTable && currentTable.type === "table" && currentTable.tableIndex === ti) {
        const rowKey = p.table_position.row;
        if (!currentTable.rows.has(rowKey)) {
          currentTable.rows.set(rowKey, []);
        }
        currentTable.rows.get(rowKey)!.push(p);
      } else {
        if (currentTable) result.push(currentTable);
        currentTable = {
          type: "table",
          tableIndex: ti,
          rows: new Map([[p.table_position.row, [p]]]),
        };
      }
    } else {
      if (currentTable) {
        result.push(currentTable);
        currentTable = null;
      }
      result.push({ type: "paragraph", paragraph: p });
    }
  }
  if (currentTable) result.push(currentTable);

  return result;
}

/** Word 段落渲染（支持表格） */
const DocumentView = forwardRef<SpreadsheetViewHandle, {
  paragraphs: Paragraph[];
  items: SensitiveItem[];
  onClickItem?: (item: SensitiveItem, event: React.MouseEvent) => void;
  onScroll?: (scrollLeft: number, scrollTop: number) => void;
  diffMode?: "removed" | "added";
  activeItemId?: string;
  templateReplacements?: Map<string, string>;
  showReplacedText?: boolean;
}>(function DocumentView({ paragraphs, items, onClickItem, onScroll, diffMode, activeItemId, templateReplacements, showReplacedText }, ref) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useImperativeHandle(ref, () => ({
    getScrollElement: () => scrollRef.current,
  }));

  const handleScroll = useCallback(() => {
    if (scrollRef.current && onScroll) {
      onScroll(scrollRef.current.scrollLeft, scrollRef.current.scrollTop);
    }
  }, [onScroll]);

  const styleMap: Record<string, string> = {
    heading1: "text-2xl font-bold mt-6 mb-2",
    heading2: "text-xl font-semibold mt-5 mb-2",
    heading3: "text-lg font-semibold mt-4 mb-1",
    normal: "text-base",
    listParagraph: "text-base pl-6",
  };

  const getItemsForParagraph = (index: number): SensitiveItem[] => {
    return items.filter((item) => item.row === index);
  };

  const blocks = useMemo(() => groupParagraphsToBlocks(paragraphs), [paragraphs]);

  /** 渲染段落内容（高亮或纯文本） */
  const renderParagraphContent = (p: Paragraph) => {
    const pItems = getItemsForParagraph(p.index);
    if (pItems.length > 0) {
      return (
        <HighlightedText
          text={p.text}
          items={pItems}
          onClickItem={onClickItem}
          diffMode={diffMode}
          activeItemId={activeItemId}
          templateReplacements={templateReplacements}
          showReplacedText={showReplacedText}
        />
      );
    }
    return p.text;
  };

  return (
    <div ref={scrollRef} className="flex-1 overflow-auto p-6" onScroll={handleScroll}>
      <div className="max-w-3xl mx-auto space-y-2">
        {blocks.map((block) => {
          if (block.type === "paragraph") {
            const p = block.paragraph;
            const className = styleMap[p.style] ?? styleMap.normal;
            return (
              <p key={p.index} data-row={p.index} data-col={0} className={`${className} text-slate-700 leading-relaxed`}>
                {renderParagraphContent(p)}
              </p>
            );
          }

          // 表格块
          return (
            <table
              key={`table-${block.tableIndex}`}
              className="w-full border-collapse border border-slate-300 my-4 text-sm"
            >
              <tbody>
                {Array.from(block.rows.entries())
                  .sort(([a], [b]) => a - b)
                  .map(([rowIdx, cells]) => (
                    <tr key={rowIdx} className="border border-slate-200">
                      {cells
                        .sort((a, b) => a.table_position!.col - b.table_position!.col)
                        .map((cell) => (
                          <td
                            key={cell.index}
                            data-row={cell.index}
                            data-col={0}
                            className="border border-slate-200 px-3 py-2 text-slate-700 align-top"
                          >
                            {renderParagraphContent(cell)}
                          </td>
                        ))}
                    </tr>
                  ))}
              </tbody>
            </table>
          );
        })}
      </div>
    </div>
  );
});
