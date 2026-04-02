import { useState, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useAppStore } from "../../stores/appStore";
import { ContentRenderer } from "../../components/ContentRenderer";

export function RestorePage() {
  const { t } = useTranslation();
  const restoreResult = useAppStore((s) => s.restoreResult);
  const setRestoreResult = useAppStore((s) => s.setRestoreResult);
  const setView = useAppStore((s) => s.setView);

  const [exporting, setExporting] = useState(false);

  // 同步滚动
  const leftRef = useRef<HTMLDivElement>(null);
  const rightRef = useRef<HTMLDivElement>(null);
  const isSyncing = useRef(false);

  const handleScroll = useCallback((source: "left" | "right") => {
    if (isSyncing.current) return;
    isSyncing.current = true;

    const sourceRef = source === "left" ? leftRef : rightRef;
    const targetRef = source === "left" ? rightRef : leftRef;

    if (sourceRef.current && targetRef.current) {
      targetRef.current.scrollTop = sourceRef.current.scrollTop;
      targetRef.current.scrollLeft = sourceRef.current.scrollLeft;
    }

    requestAnimationFrame(() => {
      isSyncing.current = false;
    });
  }, []);

  if (!restoreResult) {
    return (
      <div className="flex-1 flex items-center justify-center text-gray-400">
        {t("restorePage.noResult")}
      </div>
    );
  }

  const { original_content, restored_content, matched_count } = restoreResult;

  const handleExport = async () => {
    setExporting(true);
    try {
      const fileName = restored_content.file_name;
      const rawExt = fileName.split(".").pop()?.toLowerCase() || "csv";
      // rust_xlsxwriter 只能写 xlsx 格式，xls 自动转为 xlsx
      const ext = rawExt === "xls" ? "xlsx" : rawExt;
      const defaultName = fileName.replace(/\.[^.]+$/, `${t("fileSuffix.restored")}.${ext}`);

      const outputPath = await save({
        defaultPath: defaultName,
        filters: [{ name: ext, extensions: [ext] }],
      });

      if (!outputPath) {
        setExporting(false);
        return;
      }

      await invoke("export_file", {
        content: restored_content,
        outputPath,
      });

      toast.success(t("restorePage.restoreSuccess"));
    } catch (e) {
      toast.error(typeof e === "string" ? e : t("restorePage.exportFailed"));
    } finally {
      setExporting(false);
    }
  };

  const handleBack = () => {
    setRestoreResult(null);
    setView("history");
  };

  return (
    <div className="flex-1 flex flex-col min-h-0">
      {/* DiffPanel */}
      <div className="flex-1 flex min-h-0">
        {/* 左：脱敏后文件（AI 处理结果） */}
        <div className="flex-1 flex flex-col min-w-0 border-r border-gray-200">
          <div className="bg-gray-50 px-4 py-2 text-sm font-medium text-gray-600 border-b border-gray-200 shrink-0">
            {t("restorePage.desensitizedContent")}
          </div>
          <div
            ref={leftRef}
            className="flex-1 overflow-auto"
            onScroll={() => handleScroll("left")}
          >
            <ContentRenderer content={original_content} items={[]} />
          </div>
        </div>

        {/* 右：还原后文件 */}
        <div className="flex-1 flex flex-col min-w-0">
          <div className="bg-green-50 px-4 py-2 text-sm font-medium text-green-700 border-b border-green-200 shrink-0">
            {t("restorePage.restoredContent")}
          </div>
          <div
            ref={rightRef}
            className="flex-1 overflow-auto"
            onScroll={() => handleScroll("right")}
          >
            <ContentRenderer content={restored_content} items={[]} />
          </div>
        </div>
      </div>

      {/* RestoreFooter */}
      <div className="bg-white border-t border-gray-200 px-6 py-4 flex items-center justify-between shrink-0">
        <div className="text-sm text-gray-600">
          {t("restorePage.totalRestored", { count: matched_count })}
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={handleBack}
            className="px-4 py-2 text-sm text-gray-600 hover:text-gray-800 hover:bg-gray-100 rounded-lg transition-colors"
          >
            {t("common.back")}
          </button>
          <button
            onClick={handleExport}
            disabled={exporting}
            className="px-6 py-2 bg-green-600 text-white text-sm font-medium rounded-lg hover:bg-green-700 transition-colors disabled:opacity-50"
          >
            {exporting ? t("common.loading") : t("common.export")}
          </button>
        </div>
      </div>
    </div>
  );
}
