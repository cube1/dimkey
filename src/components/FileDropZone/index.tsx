import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Upload, Loader2 } from "lucide-react";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useAppStore } from "../../stores/appStore";
import { useDetectStore } from "../../stores/detectStore";
import type { FileContent } from "../../types";

/** 支持的文件扩展名 */
const SUPPORTED_EXTENSIONS = [".xlsx", ".xls", ".csv", ".tsv", ".docx", ".txt", ".pdf"];

/** 从路径提取扩展名 */
function getExtension(path: string): string {
  const dot = path.lastIndexOf(".");
  return dot >= 0 ? path.slice(dot).toLowerCase() : "";
}

/** 校验文件路径 */
function validateFile(filePath: string): string | null {
  const ext = getExtension(filePath);
  if (!SUPPORTED_EXTENSIONS.includes(ext)) {
    return "unsupported";
  }
  return null;
}

export function FileDropZone() {
  const { t } = useTranslation();
  const [isDragOver, setIsDragOver] = useState(false);
  const [isLoading, setIsLoading] = useState(false);

  const setFileContent = useAppStore((s) => s.setFileContent);
  const setView = useAppStore((s) => s.setView);
  const resetDetect = useDetectStore((s) => s.resetDetect);

  /** 处理文件导入 */
  const handleImportFile = useCallback(
    async (filePath: string) => {
      // 校验格式
      const error = validateFile(filePath);
      if (error) {
        toast.error(t("home.supportedFormatsLong"));
        return;
      }

      setIsLoading(true);
      try {
        const content = await invoke<FileContent>("import_file", {
          filePath,
        });
        resetDetect();
        setFileContent(content, filePath);
        setView("preview");
      } catch (err) {
        const message = typeof err === "string" ? err : t("home.fileReadError");
        toast.error(message);
      } finally {
        setIsLoading(false);
      }
    },
    [setFileContent, setView, resetDetect]
  );

  /** 监听 Tauri 拖放事件（获取文件路径） */
  useEffect(() => {
    const webview = getCurrentWebview();
    const unlisten = webview.onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        setIsDragOver(true);
      } else if (event.payload.type === "leave") {
        setIsDragOver(false);
      } else if (event.payload.type === "drop") {
        setIsDragOver(false);
        const paths = event.payload.paths;
        if (paths.length > 1) {
          toast.error(t("dict.singleFileOnly"));
          return;
        }
        if (paths.length === 1) {
          handleImportFile(paths[0]);
        }
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [handleImportFile]);

  /** 点击选择文件 */
  const handleClickSelect = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "支持的文件",
          extensions: ["xlsx", "xls", "csv", "tsv", "docx", "txt", "pdf"],
        },
      ],
    });
    if (selected) {
      handleImportFile(selected);
    }
  };

  if (isLoading) {
    return (
      <div className="flex flex-col items-center justify-center w-full h-64 border-2 border-dashed rounded-xl border-blue-300 bg-blue-50">
        <Loader2 className="w-10 h-10 text-blue-500 animate-spin mb-4" />
        <p className="text-base text-blue-600 font-medium">{t("home.parsingFile")}</p>
      </div>
    );
  }

  return (
    <div
      onClick={handleClickSelect}
      className={`
        flex flex-col items-center justify-center
        w-full h-64 border-2 border-dashed rounded-xl
        transition-all cursor-pointer
        ${
          isDragOver
            ? "border-blue-500 bg-blue-50 scale-[1.01]"
            : "border-gray-300 bg-white hover:border-gray-400 hover:bg-gray-50"
        }
      `}
    >
      <Upload
        className={`w-12 h-12 mb-4 transition-colors ${
          isDragOver ? "text-blue-500" : "text-gray-400"
        }`}
      />
      <p className="text-lg text-gray-600 font-medium">
        {isDragOver ? t("home.dropRelease") : t("home.dropHint")}
      </p>
      <p className="text-sm text-gray-400 mt-1">
        {t("home.selectFile")}
      </p>
      <p className="text-xs text-gray-400 mt-3">
        {t("home.maxFileSize")}
      </p>
    </div>
  );
}
