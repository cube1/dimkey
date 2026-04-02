import { useState, useCallback, useRef } from "react";
import { TypeSelectPopover } from "./TypeSelectPopover";
import type { SensitiveType, SensitiveItem, PdfBbox } from "../../types";

const MIN_DRAG_DISTANCE = 5;

interface RectSelectionLayerProps {
  displayWidth: number;
  displayHeight: number;
  pageIndex: number;
  onAddItem: (item: SensitiveItem) => void;
}

export function RectSelectionLayer({
  displayWidth,
  displayHeight,
  pageIndex,
  onAddItem,
}: RectSelectionLayerProps) {
  const [dragging, setDragging] = useState(false);
  const [rect, setRect] = useState<{ x1: number; y1: number; x2: number; y2: number } | null>(null);
  const [pendingRect, setPendingRect] = useState<{ x1: number; y1: number; x2: number; y2: number } | null>(null);
  const [popoverPos, setPopoverPos] = useState<{ x: number; y: number } | null>(null);
  const layerRef = useRef<HTMLDivElement>(null);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (pendingRect) return;
    const layerRect = layerRef.current?.getBoundingClientRect();
    if (!layerRect) return;
    const x = e.clientX - layerRect.left;
    const y = e.clientY - layerRect.top;
    setDragging(true);
    setRect({ x1: x, y1: y, x2: x, y2: y });
  }, [pendingRect]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!dragging || !rect) return;
    const layerRect = layerRef.current?.getBoundingClientRect();
    if (!layerRect) return;
    const x = Math.max(0, Math.min(e.clientX - layerRect.left, displayWidth));
    const y = Math.max(0, Math.min(e.clientY - layerRect.top, displayHeight));
    setRect((prev) => prev ? { ...prev, x2: x, y2: y } : null);
  }, [dragging, rect, displayWidth, displayHeight]);

  const handleMouseUp = useCallback((e: React.MouseEvent) => {
    if (!dragging || !rect) return;
    setDragging(false);
    const dx = Math.abs(rect.x2 - rect.x1);
    const dy = Math.abs(rect.y2 - rect.y1);
    if (dx < MIN_DRAG_DISTANCE && dy < MIN_DRAG_DISTANCE) {
      setRect(null);
      return;
    }
    setPendingRect(rect);
    setPopoverPos({ x: e.clientX, y: e.clientY });
    setRect(null);
  }, [dragging, rect]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      setDragging(false);
      setRect(null);
      setPendingRect(null);
      setPopoverPos(null);
    }
  }, []);

  const handleSelectType = useCallback((type: SensitiveType) => {
    if (!pendingRect) return;
    const x1 = Math.min(pendingRect.x1, pendingRect.x2);
    const y1 = Math.min(pendingRect.y1, pendingRect.y2);
    const x2 = Math.max(pendingRect.x1, pendingRect.x2);
    const y2 = Math.max(pendingRect.y1, pendingRect.y2);

    const pdf_bbox: PdfBbox = {
      page_index: pageIndex,
      left: x1 / displayWidth,
      top: y1 / displayHeight,
      right: x2 / displayWidth,
      bottom: y2 / displayHeight,
    };

    const item: SensitiveItem = {
      id: `rect_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
      text: `[框选区域 p${pageIndex + 1}]`,
      sensitive_type: type,
      source: "Manual",
      confidence: 1.0,
      start: 0,
      end: 0,
      row: pageIndex,
      col: 0,
      sheet_index: 0,
      pdf_bbox: pdf_bbox,
    };
    onAddItem(item);
    setPendingRect(null);
    setPopoverPos(null);
  }, [pendingRect, pageIndex, displayWidth, displayHeight, onAddItem]);

  const handleCancel = useCallback(() => {
    setPendingRect(null);
    setPopoverPos(null);
  }, []);

  const drawingStyle = rect ? {
    left: Math.min(rect.x1, rect.x2),
    top: Math.min(rect.y1, rect.y2),
    width: Math.abs(rect.x2 - rect.x1),
    height: Math.abs(rect.y2 - rect.y1),
  } : null;

  const pendingStyle = pendingRect ? {
    left: Math.min(pendingRect.x1, pendingRect.x2),
    top: Math.min(pendingRect.y1, pendingRect.y2),
    width: Math.abs(pendingRect.x2 - pendingRect.x1),
    height: Math.abs(pendingRect.y2 - pendingRect.y1),
  } : null;

  return (
    <>
      <div
        ref={layerRef}
        className="absolute inset-0 cursor-crosshair"
        style={{ width: displayWidth, height: displayHeight }}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onKeyDown={handleKeyDown}
        tabIndex={0}
      >
        {drawingStyle && (
          <div
            className="absolute border-2 border-blue-500 bg-blue-500/10 pointer-events-none"
            style={drawingStyle}
          />
        )}
        {pendingStyle && (
          <div
            className="absolute border-2 border-blue-500 bg-blue-500/20 pointer-events-none animate-pulse"
            style={pendingStyle}
          />
        )}
      </div>
      {popoverPos && pendingRect && (
        <TypeSelectPopover
          position={popoverPos}
          onSelect={handleSelectType}
          onCancel={handleCancel}
        />
      )}
    </>
  );
}
