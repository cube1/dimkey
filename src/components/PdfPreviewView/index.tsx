import { forwardRef, useEffect, useState, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SensitiveItem } from "../../types";
import { PdfTextLayer } from "./PdfTextLayer";
import { RectSelectionLayer } from "./RectSelectionLayer";

export interface PdfTextObjectInfo {
  text: string;
  left: number;
  top: number;
  right: number;
  bottom: number;
  char_offset: number;
}

export interface PdfPageRender {
  page_index: number;
  image_base64: string;
  image_width: number;
  image_height: number;
  page_width: number;
  page_height: number;
  text_objects: PdfTextObjectInfo[];
}

interface PdfPreviewViewProps {
  filePath?: string;
  /** 外部共享的页面数据（避免重复调用后端） */
  pages?: PdfPageRender[];
  items: SensitiveItem[];
  /** 是否在敏感项位置显示黑色涂黑（右侧脱敏后预览） */
  showRedacted?: boolean;
  /** 滚动事件回调 */
  onScroll?: (scrollLeft: number, scrollTop: number) => void;
  /** 外部控制的滚动位置 */
  scrollTop?: number;
  scrollLeft?: number;
  zoom?: number;
  rectSelectMode?: boolean;
  onAddItem?: (item: SensitiveItem) => void;
  /** 后端计算的涂黑预览矩形（归一化屏幕坐标） */
  overlayRects?: Array<{page_index: number; left: number; top: number; right: number; bottom: number}>;
}

export type OverlayRect = {page_index: number; left: number; top: number; right: number; bottom: number};

export const PdfPreviewView = forwardRef<HTMLDivElement, PdfPreviewViewProps>(function PdfPreviewView({
  filePath,
  pages: externalPages,
  items,
  showRedacted = false,
  onScroll,
  scrollTop,
  scrollLeft,
  zoom = 100,
  rectSelectMode,
  onAddItem,
  overlayRects,
}: PdfPreviewViewProps, ref) {
  const [pages, setPages] = useState<PdfPageRender[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const isSyncing = useRef(false);

  // 合并内部 containerRef 和外部转发的 ref
  const setRefs = useCallback((node: HTMLDivElement | null) => {
    (containerRef as React.MutableRefObject<HTMLDivElement | null>).current = node;
    if (typeof ref === "function") ref(node);
    else if (ref) (ref as React.MutableRefObject<HTMLDivElement | null>).current = node;
  }, [ref]);

  // 渲染 PDF 页面
  useEffect(() => {
    if (externalPages) {
      setPages(externalPages);
      setLoading(false);
      return;
    }
    if (!filePath) return;

    let cancelled = false;
    setLoading(true);
    setError(null);

    invoke<PdfPageRender[]>("render_pdf_pages", { filePath })
      .then((result) => {
        if (!cancelled) {
          setPages(result);
          setLoading(false);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(typeof err === "string" ? err : "渲染 PDF 失败");
          setLoading(false);
        }
      });

    return () => { cancelled = true; };
  }, [filePath, externalPages]);

  // 同步滚动
  useEffect(() => {
    if (containerRef.current && !isSyncing.current) {
      if (scrollTop !== undefined) {
        containerRef.current.scrollTop = scrollTop;
      }
      if (scrollLeft !== undefined) {
        containerRef.current.scrollLeft = scrollLeft;
      }
    }
  }, [scrollTop, scrollLeft]);

  const handleScroll = useCallback(() => {
    if (containerRef.current && onScroll) {
      isSyncing.current = true;
      onScroll(containerRef.current.scrollLeft, containerRef.current.scrollTop);
      requestAnimationFrame(() => { isSyncing.current = false; });
    }
  }, [onScroll]);

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center text-slate-400">
        <div className="flex flex-col items-center gap-2">
          <div className="w-6 h-6 border-2 border-slate-300 border-t-primary-500 rounded-full animate-spin" />
          <span className="text-sm">正在渲染 PDF 页面...</span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex-1 flex items-center justify-center text-red-400 text-sm">
        {error}
      </div>
    );
  }

  return (
    <div
      ref={setRefs}
      className="flex-1 overflow-auto bg-slate-100 p-4"
      onScroll={handleScroll}
    >
      <div className="flex flex-col items-center gap-4">
        {pages.map((page) => (
          <PdfPageView
            key={page.page_index}
            page={page}
            items={items}
            showRedacted={showRedacted}
            zoom={zoom}
            rectSelectMode={rectSelectMode}
            onAddItem={onAddItem}
            overlayRects={overlayRects}
          />
        ))}
      </div>
    </div>
  );
});

/** 单个 PDF 页面渲染 */
function PdfPageView({
  page,
  items,
  showRedacted,
  zoom = 100,
  rectSelectMode,
  onAddItem,
  overlayRects,
}: {
  page: PdfPageRender;
  items: SensitiveItem[];
  showRedacted: boolean;
  zoom?: number;
  rectSelectMode?: boolean;
  onAddItem?: (item: SensitiveItem) => void;
  overlayRects?: OverlayRect[];
}) {
  const displayWidth = page.image_width * (zoom / 100);
  const displayHeight = page.image_height * (zoom / 100);

  return (
    <div
      className="relative bg-white shadow-md"
      style={{ width: displayWidth }}
    >
      <img
        src={`data:image/png;base64,${page.image_base64}`}
        alt={`第 ${page.page_index + 1} 页`}
        style={{ width: displayWidth }}
        className="h-auto block"
        draggable={false}
      />
      {/* 透明文本层：非框选模式下允许用户用鼠标选取文本 */}
      {!rectSelectMode && page.text_objects.length > 0 && (
        <PdfTextLayer
          textObjects={page.text_objects}
          displayWidth={displayWidth}
          displayHeight={displayHeight}
          pageIndex={page.page_index}
        />
      )}
      {/* 框选涂黑层：框选模式下允许用户拖拽矩形选区并选择敏感类型 */}
      {rectSelectMode && onAddItem && (
        <RectSelectionLayer
          displayWidth={displayWidth}
          displayHeight={displayHeight}
          pageIndex={page.page_index}
          onAddItem={onAddItem}
        />
      )}
      {/* 涂黑覆盖层（右侧脱敏后预览时显示） */}
      {showRedacted && (
        <RedactionOverlay
          page={page}
          items={items}
          displayWidth={displayWidth}
          displayHeight={displayHeight}
          overlayRects={overlayRects}
        />
      )}
    </div>
  );
}

/** 在 PDF 页面图片上叠加黑色涂黑矩形 */
function RedactionOverlay({
  page,
  items,
  displayWidth,
  displayHeight,
  overlayRects,
}: {
  page: PdfPageRender;
  items: SensitiveItem[];
  displayWidth: number;
  displayHeight: number;
  overlayRects?: OverlayRect[];
}) {
  // 筛选当前页的涂黑矩形（来自后端计算）
  const pageRects = (overlayRects ?? []).filter(
    (r) => r.page_index === page.page_index
  );

  const totalCount = items.length;

  return (
    <div className="absolute inset-0 pointer-events-none" style={{ width: displayWidth, height: displayHeight }}>
      {pageRects.map((rect, idx) => (
        <div
          key={idx}
          className="absolute bg-black/80"
          style={{
            left: rect.left * displayWidth,
            top: rect.top * displayHeight,
            width: (rect.right - rect.left) * displayWidth,
            height: (rect.bottom - rect.top) * displayHeight,
          }}
        />
      ))}
      {totalCount > 0 && (
        <div className="absolute top-2 right-2 bg-black/70 text-white text-xs px-2 py-1 rounded">
          {totalCount} 处敏感信息将被涂黑
        </div>
      )}
    </div>
  );
}
