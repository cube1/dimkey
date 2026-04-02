import type { PdfTextObjectInfo } from "./index";

interface PdfTextLayerProps {
  textObjects: PdfTextObjectInfo[];
  displayWidth: number;
  displayHeight: number;
  pageIndex: number;
}

/**
 * 透明文本层：在 PDF 页面图片上叠加不可见的真实 DOM 文本，
 * 使用户可以用鼠标框选文本触发 TextSelectionToolbar。
 */
export function PdfTextLayer({ textObjects, displayWidth, displayHeight, pageIndex }: PdfTextLayerProps) {
  return (
    <div
      className="absolute inset-0 select-text"
      style={{ width: displayWidth, height: displayHeight }}
      data-row={pageIndex}
      data-col={0}
    >
      {textObjects.map((obj, idx) => {
        const left = obj.left * displayWidth;
        const top = obj.top * displayHeight;
        const width = (obj.right - obj.left) * displayWidth;
        const height = (obj.bottom - obj.top) * displayHeight;
        return (
          <span
            key={idx}
            data-char-offset={obj.char_offset}
            className="absolute whitespace-pre"
            style={{
              left, top, width, height,
              color: "transparent",
              fontSize: `${height * 0.8}px`,
              lineHeight: `${height}px`,
              overflow: "hidden",
            }}
          >
            {obj.text}
          </span>
        );
      })}
    </div>
  );
}
