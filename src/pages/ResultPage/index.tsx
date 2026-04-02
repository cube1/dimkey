import { useState, useRef, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useAppStore } from "../../stores/appStore";
import { useActiveItems } from "../../stores/detectStore";
import { ContentRenderer } from "../../components/ContentRenderer";
import type { SpreadsheetViewHandle } from "../../components/SpreadsheetView";
import type { TaskRecord, SensitiveItem, MappingEntry } from "../../types";
import { SENSITIVE_TYPE_CONFIG } from "../../types";

/** 列默认宽度 */
const COL_DEFAULT_WIDTH = 150;

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

  // 按 (row, col) 分组，组内按 start 排序
  const grouped = new Map<string, SensitiveItem[]>();
  for (const item of originalItems) {
    const key = `${item.row}:${item.col}`;
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

export function ResultPage() {
  const { t } = useTranslation();
  const fileContent = useAppStore((s) => s.fileContent);
  const filePath = useAppStore((s) => s.filePath);
  const desensitizeResult = useAppStore((s) => s.desensitizeResult);
  const setView = useAppStore((s) => s.setView);
  const setDesensitizeResult = useAppStore((s) => s.setDesensitizeResult);
  const activeItems = useActiveItems();

  const [exporting, setExporting] = useState(false);

  // 共享列宽状态
  const [colWidths, setColWidths] = useState<number[]>([]);

  // 左右 SpreadsheetView 的 handle ref
  const leftRef = useRef<SpreadsheetViewHandle>(null);
  const rightRef = useRef<SpreadsheetViewHandle>(null);
  const isSyncing = useRef(false);

  // 初始化列宽（当内容变化时）
  const ensureColWidths = useCallback(
    (colCount: number) => {
      if (colWidths.length !== colCount) {
        setColWidths(Array.from({ length: colCount }, () => COL_DEFAULT_WIDTH));
      }
    },
    [colWidths.length]
  );

  // 列宽调整（两边共享）
  const handleColResize = useCallback((colIndex: number, width: number) => {
    setColWidths((prev) => {
      const next = [...prev];
      next[colIndex] = width;
      return next;
    });
  }, []);

  // 左侧滚动 → 同步右侧
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

  // 右侧滚动 → 同步左侧
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

  // 计算右侧高亮项
  const desensitizedItems = useMemo(() => {
    if (!desensitizeResult) return [];
    return buildDesensitizedItems(activeItems, desensitizeResult.mappings);
  }, [activeItems, desensitizeResult]);

  if (!fileContent || !desensitizeResult) {
    return (
      <div className="flex-1 flex items-center justify-center text-gray-400">
        {t("resultPage.noResult")}
      </div>
    );
  }

  const { content: desensitizedContent, summary, mappings } = desensitizeResult;

  // 确保列宽已初始化
  if (fileContent.type === "Spreadsheet") {
    const firstSheet = fileContent.sheets[0];
    if (firstSheet) ensureColWidths(firstSheet.headers.length);
  }

  const handleGoBack = () => {
    setDesensitizeResult(null);
    setView("preview");
  };

  const handleExport = async () => {
    setExporting(true);
    try {
      const fileName = fileContent.file_name;
      const rawExt = fileName.split(".").pop()?.toLowerCase() || "csv";
      // rust_xlsxwriter 只能写 xlsx 格式，xls 自动转为 xlsx
      const ext = rawExt === "xls" ? "xlsx" : rawExt;
      const defaultName = fileName.replace(/\.[^.]+$/, `${t("resultPage.desensitizedSuffix")}.${ext}`);

      const outputPath = await save({
        defaultPath: defaultName,
        filters: [{ name: t("resultPage.desensitizedFile"), extensions: [ext] }],
      });

      if (!outputPath) {
        setExporting(false);
        return;
      }

      const isPdf = fileContent.file_type === "Pdf";

      if (isPdf) {
        // PDF 使用专用涂黑导出命令，后端内部重新解析坐标
        await invoke("export_pdf_redacted_cmd", {
          originalPath: filePath,
          sensitiveItems: activeItems,
          outputPath,
        });
      } else {
        await invoke("export_file", {
          content: desensitizedContent,
          outputPath,
          originalPath: filePath,
        });
      }

      const task: TaskRecord = {
        id: generateTaskId(),
        original_file_name: fileName,
        file_type: fileContent.file_type,
        created_at: new Date().toISOString(),
        sensitive_count: summary.total,
        replaced_count: mappings.length,
        mappings,
      };

      try {
        await invoke("save_task", { task });
      } catch (e) {
        console.error("保存任务记录失败:", e);
      }

      // 自定义 toast：包含"在 Finder 中显示"按钮
      toast.success(
        (toastInstance) => (
          <div className="flex items-center gap-3">
            <span>{t("resultPage.exportSuccess")}</span>
            <button
              onClick={() => {
                revealItemInDir(outputPath).catch(() => {});
                toast.dismiss(toastInstance.id);
              }}
              className="text-blue-600 hover:text-blue-800 text-sm font-medium whitespace-nowrap underline underline-offset-2"
            >
              {t("resultPage.openDir")}
            </button>
          </div>
        ),
        { duration: 8000 }
      );
    } catch (err) {
      const message = typeof err === "string" ? err : t("resultPage.exportFailed");
      toast.error(message);
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="flex-1 flex flex-col min-h-0">
      {/* DiffPanel: 左右对比 */}
      <div className="flex-1 flex min-h-0">
        {/* 左侧：原始内容 */}
        <div className="flex-1 flex flex-col min-w-0 border-r border-gray-200">
          <div className="bg-gray-50 px-4 py-2 text-sm font-medium text-gray-600 border-b border-gray-200 shrink-0">
            {t("resultPage.originalContent")}
          </div>
          <ContentRenderer
            ref={leftRef}
            content={fileContent}
            items={activeItems}
            colWidths={colWidths.length > 0 ? colWidths : undefined}
            onColResize={handleColResize}
            onScroll={handleLeftScroll}
          />
        </div>

        {/* 右侧：脱敏后内容（带高亮） */}
        <div className="flex-1 flex flex-col min-w-0">
          <div className="bg-blue-50 px-4 py-2 text-sm font-medium text-blue-700 border-b border-blue-200 shrink-0">
            {t("resultPage.desensitizedContent")}
          </div>
          <ContentRenderer
            ref={rightRef}
            content={desensitizedContent}
            items={desensitizedItems}
            colWidths={colWidths.length > 0 ? colWidths : undefined}
            onColResize={handleColResize}
            onScroll={handleRightScroll}
          />
        </div>
      </div>

      {/* ResultFooter */}
      <div className="bg-white border-t border-gray-200 px-6 py-3 shrink-0">
        <div className="flex items-center gap-4 mb-3">
          <span className="text-sm text-gray-600">
            {t("resultPage.totalDesensitized", { count: summary.total })}
          </span>
          <div className="flex gap-2 flex-wrap">
            {Object.entries(summary.by_type).map(([typeKey, count]) => {
              const info = SENSITIVE_TYPE_CONFIG[typeKey];
              if (!info) return null;
              return (
                <span
                  key={typeKey}
                  className={`inline-flex items-center px-2 py-0.5 rounded text-xs ${info.bgClass} ${info.textClass}`}
                >
                  {info.label}: {count}
                </span>
              );
            })}
          </div>
        </div>

        <div className="flex items-center justify-between">
          <button
            onClick={handleGoBack}
            className="px-4 py-2 text-sm text-gray-600 hover:text-gray-800 hover:bg-gray-100 rounded-lg transition-colors"
          >
            {t("resultPage.goBack")}
          </button>
          <button
            onClick={handleExport}
            disabled={exporting}
            className="px-6 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50"
          >
            {exporting ? t("resultPage.exporting") : t("resultPage.exportFile")}
          </button>
        </div>
      </div>
    </div>
  );
}

function generateTaskId(): string {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const date = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}`;
  const time = `${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  const rand = Math.random().toString(36).slice(2, 6);
  return `task_${date}_${time}_${rand}`;
}
