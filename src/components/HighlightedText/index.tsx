import type { SensitiveItem } from "../../types";
import { getSensitiveTypeInfo } from "../../types";

interface HighlightedTextProps {
  text: string;
  items: SensitiveItem[];
  onClickItem?: (item: SensitiveItem, event: React.MouseEvent) => void;
  /** Diff 模式：removed 红色删除线，added 绿色背景，不传使用类型配色 */
  diffMode?: "removed" | "added";
  /** 当前选中高亮的项 ID，匹配时显示选中态 */
  activeItemId?: string;
  /** 模版替换模式：原文 → 替换值 映射，用于区分高亮颜色 */
  templateReplacements?: Map<string, string>;
  /** 右侧预览时为 true，显示替换后的文本而非原文 */
  showReplacedText?: boolean;
}

interface TextSegment {
  text: string;
  item?: SensitiveItem;
}

/** 将文本根据敏感项位置切割为普通/高亮片段 */
function splitText(text: string, items: SensitiveItem[]): TextSegment[] {
  if (items.length === 0) return [{ text }];

  // 按 start 排序；start 相同时长的优先，确保手动选择的长文本不被短子项跳过
  const sorted = [...items].sort((a, b) => {
    if (a.start !== b.start) return a.start - b.start;
    return (b.end - b.start) - (a.end - a.start);
  });
  const segments: TextSegment[] = [];
  let cursor = 0;

  for (const item of sorted) {
    // 跳过重叠的项
    if (item.start < cursor) continue;

    // 前面的普通文本
    if (item.start > cursor) {
      segments.push({ text: text.slice(cursor, item.start) });
    }

    // 高亮片段
    segments.push({
      text: text.slice(item.start, item.end),
      item,
    });
    cursor = item.end;
  }

  // 尾部普通文本
  if (cursor < text.length) {
    segments.push({ text: text.slice(cursor) });
  }

  return segments;
}

/** diffMode 对应的样式 */
const DIFF_STYLES = {
  removed: "bg-red-100/80 text-red-900 line-through ring-1 ring-red-300/50",
  added: "bg-green-100/80 text-green-900 ring-1 ring-green-300/50",
} as const;

export function HighlightedText({
  text,
  items,
  onClickItem,
  diffMode,
  activeItemId,
  templateReplacements,
  showReplacedText,
}: HighlightedTextProps) {
  const segments = splitText(text, items);

  return (
    <span>
      {segments.map((seg, i) => {
        if (!seg.item) {
          return <span key={i}>{seg.text}</span>;
        }

        const info = getSensitiveTypeInfo(seg.item.sensitive_type);
        const isActive = activeItemId != null && seg.item.id === activeItemId;
        const activeRing = isActive ? " ring-2 ring-primary-500 shadow-sm" : "";

        let highlightClass: string;
        let titleText: string = "";
        // displayText 默认为原文，showReplacedText 时可能替换为替换值
        let displayText: string = seg.text;
        if (templateReplacements) {
          // 模版替换模式
          const replacement = templateReplacements.get(seg.item.text);
          if (replacement !== undefined) {
            if (showReplacedText) {
              // 右侧预览：显示替换后的文本，绿色标记
              displayText = replacement;
              highlightClass = `bg-green-100 text-green-800 rounded px-0.5 py-px ring-1 ring-inset ring-green-300/30`;
              titleText = `${seg.item.text} → ${replacement}`;
            } else {
              // 左侧原文：teal 高亮，保持原文
              highlightClass = `bg-teal-100 text-teal-800 rounded px-0.5 py-px ring-1 ring-inset ring-teal-300/30 cursor-pointer hover:ring-2 hover:ring-teal-400/40 transition-all${activeRing}`;
              titleText = `${seg.item.text} → ${replacement}`;
            }
          } else {
            if (showReplacedText) {
              // 右侧预览：无替换值的项保持原文，不高亮
              highlightClass = "";
            } else {
              // 左侧：灰色淡化
              highlightClass = `bg-slate-100/60 text-slate-400 rounded px-0.5 py-px cursor-pointer hover:ring-2 hover:ring-slate-300/40 transition-all${activeRing}`;
              titleText = `${info.label}（无替换值）`;
            }
          }
        } else if (diffMode) {
          highlightClass = `${DIFF_STYLES[diffMode]} rounded px-0.5 py-px cursor-pointer hover:ring-2 hover:ring-current/20 transition-all${activeRing}`;
          titleText = `${info.label} (${seg.item.source})`;
        } else {
          highlightClass = `${info.bgClass} ${info.textClass} rounded px-0.5 py-px ring-1 ring-inset ring-current/5 cursor-pointer hover:ring-2 hover:ring-current/20 transition-all${activeRing}`;
          titleText = `点击编辑 · ${info.label}`;
        }

        return (
          <span
            key={i}
            className={highlightClass || undefined}
            title={titleText}
            onClick={(e) => {
              e.stopPropagation();
              onClickItem?.(seg.item!, e);
            }}
          >
            {displayText}
          </span>
        );
      })}
    </span>
  );
}
