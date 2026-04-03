import { useState, useRef, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { ContentRenderer } from "../ContentRenderer";
import type { SpreadsheetViewHandle } from "../SpreadsheetView";
import type { RestoreItem, SensitiveItem } from "../../types";

const COL_DEFAULT_WIDTH = 150;

/** 将 RestoreItem[] 转换为 SensitiveItem[]，以复用 ContentRenderer 高亮 */
function restoreItemsToSensitiveItems(items: RestoreItem[]): SensitiveItem[] {
  return items.map((item, index) => ({
    id: `restore-${index}`,
    text: item.text,
    sensitive_type: item.sensitive_type,
    source: "Regex" as const,
    confidence: 1.0,
    start: item.start,
    end: item.end,
    row: item.row,
    col: item.col,
    sheet_index: item.sheet_index,
  }));
}

export function RestoreView() {
  const { t } = useTranslation();
  const restoreResult = useWorkspaceStore((s) => s.restoreResult);

  const [colWidths, setColWidths] = useState<number[]>([]);

  const leftRef = useRef<SpreadsheetViewHandle>(null);
  const rightRef = useRef<SpreadsheetViewHandle>(null);
  const isSyncing = useRef(false);

  // 转换高亮项
  const leftItems = useMemo(
    () => (restoreResult ? restoreItemsToSensitiveItems(restoreResult.original_items) : []),
    [restoreResult]
  );
  const rightItems = useMemo(
    () => (restoreResult ? restoreItemsToSensitiveItems(restoreResult.restore_items) : []),
    [restoreResult]
  );

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

  if (!restoreResult) {
    return (
      <div className="flex-1 flex items-center justify-center text-slate-400">
        {t("restorePage.noResult")}
      </div>
    );
  }

  const { original_content, restored_content } = restoreResult;

  if (original_content.type === "Spreadsheet") {
    const firstSheet = original_content.sheets[0];
    if (firstSheet) ensureColWidths(firstSheet.headers.length);
  }

  return (
    <div className="flex-1 flex flex-col min-h-0" data-testid="view-restore">
      {/* DiffPanel */}
      <div className="flex-1 flex min-h-0">
        {/* 左：脱敏后文件 */}
        <div className="flex-1 flex flex-col min-w-0 border-r border-slate-200">
          <div className="bg-slate-50 px-4 py-2 text-xs font-bold uppercase tracking-wider text-slate-500 border-b border-slate-200 shrink-0 flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-slate-400" />
            {t("restorePage.desensitizedContent")}
          </div>
          <ContentRenderer
            ref={leftRef}
            content={original_content}
            items={leftItems}
            colWidths={colWidths.length > 0 ? colWidths : undefined}
            onColResize={handleColResize}
            onScroll={handleLeftScroll}
          />
        </div>

        {/* 右：还原后文件 */}
        <div className="flex-1 flex flex-col min-w-0">
          <div className="bg-emerald-50 px-4 py-2 text-xs font-bold uppercase tracking-wider text-emerald-700 border-b border-emerald-200 shrink-0 flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-emerald-500" />
            {t("restorePage.restoredContent")}
          </div>
          <ContentRenderer
            ref={rightRef}
            content={restored_content}
            items={rightItems}
            colWidths={colWidths.length > 0 ? colWidths : undefined}
            onColResize={handleColResize}
            onScroll={handleRightScroll}
          />
        </div>
      </div>
    </div>
  );
}
