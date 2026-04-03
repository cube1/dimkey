import { useState, useCallback, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Upload, ClipboardPaste } from "lucide-react";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useAutoDesensitize } from "../../hooks/useAutoDesensitize";

/** 支持的文件扩展名 */
const SUPPORTED_EXTENSIONS = [".xlsx", ".xls", ".csv", ".tsv", ".docx", ".txt", ".pdf"];

function getExtension(path: string): string {
  const dot = path.lastIndexOf(".");
  return dot >= 0 ? path.slice(dot).toLowerCase() : "";
}

function validateFile(filePath: string): string | null {
  const ext = getExtension(filePath);
  if (!SUPPORTED_EXTENSIONS.includes(ext)) {
    return "unsupported";
  }
  return null;
}

/** 从文件路径提取不带扩展名的文件名 */
function extractFileName(filePath: string): string {
  const name = filePath.split(/[/\\]/).pop() || filePath;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

/**
 * 未选中工作区时的拖放视图
 * 拖入文件后自动创建工作区并开始处理
 */
export function EmptyDropzoneView() {
  const { t } = useTranslation();
  const [isDragOver, setIsDragOver] = useState(false);
  const createWorkspace = useWorkspaceStore((s) => s.createWorkspace);
  const createClipboardWorkspace = useWorkspaceStore((s) => s.createClipboardWorkspace);
  const { processFile, processClipboardText } = useAutoDesensitize();

  const handleImportFile = useCallback(
    async (filePath: string) => {
      const error = validateFile(filePath);
      if (error) {
        toast.error(t("home.supportedFormatsLong"));
        return;
      }

      // 以文件名创建工作区并选中
      const wsName = extractFileName(filePath);
      try {
        await createWorkspace(wsName);
      } catch {
        toast.error(t("dict.createWorkspaceFailed"));
        return;
      }

      // 工作区创建后 store 已自动选中，开始处理
      await processFile(filePath);
    },
    [createWorkspace, processFile]
  );

  const handlePasteText = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (!text.trim()) {
        toast.error(t("dict.clipboardEmpty"));
        return;
      }

      // 创建粘贴板工作区
      const now = new Date();
      const wsName = `粘贴板 ${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")} ${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
      try {
        await createClipboardWorkspace(wsName);
      } catch {
        toast.error(t("dict.createWorkspaceFailed"));
        return;
      }

      await processClipboardText(text);
    } catch {
      toast.error(t("dict.clipboardFailed"));
    }
  };

  // 监听 Tauri 拖放事件
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
        if (paths.length > 0) {
          handleImportFile(paths[0]);
        }
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [handleImportFile]);

  // 监听键盘粘贴事件（Ctrl/Cmd+V）
  useEffect(() => {
    const handlePaste = async (e: ClipboardEvent) => {
      if (e.clipboardData?.files?.length) return;
      const text = e.clipboardData?.getData("text/plain");
      if (text?.trim()) {
        e.preventDefault();

        // 创建粘贴板工作区
        const now = new Date();
        const wsName = `粘贴板 ${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")} ${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
        try {
          await createClipboardWorkspace(wsName);
        } catch {
          toast.error(t("dict.createWorkspaceFailed"));
          return;
        }

        processClipboardText(text);
      }
    };
    window.addEventListener("paste", handlePaste);
    return () => window.removeEventListener("paste", handlePaste);
  }, [createClipboardWorkspace, processClipboardText]);

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

  return (
    <div className="flex-1 flex items-center justify-center animate-fade-in p-6" data-testid="view-empty">
      <div
        onClick={handleClickSelect}
        className={`
          flex flex-col items-center justify-center
          w-full max-w-lg h-64 border-2 border-dashed rounded-2xl
          transition-all cursor-pointer
          ${
            isDragOver
              ? "border-primary-500 bg-primary-50 ring-4 ring-primary-500/10 shadow-elevated scale-[1.01]"
              : "border-slate-300 bg-gradient-to-b from-white to-slate-50/50 hover:border-slate-400 hover:bg-white"
          }
        `}
      >
        <div className={`w-12 h-12 rounded-xl flex items-center justify-center mb-3 ${
          isDragOver
            ? "bg-primary-100"
            : "bg-gradient-to-br from-primary-50 to-slate-100"
        }`}>
          <Upload
            className={`w-6 h-6 transition-colors ${
              isDragOver ? "text-primary-500" : "text-slate-400"
            }`}
          />
        </div>
        <p className="text-base text-slate-600 font-medium">
          {isDragOver ? t("home.dropRelease") : t("home.dropHint")}
        </p>
        <p className="text-sm text-slate-400 mt-1">
          {t("home.selectFile")}
        </p>
        <p className="text-xs text-slate-400 mt-2">
          {t("home.supportedFormats")}
        </p>
        <button
          onClick={(e) => { e.stopPropagation(); handlePasteText(); }}
          className="mt-3 inline-flex items-center gap-1.5 px-3 py-1.5
                     bg-white border border-slate-200 rounded-lg
                     text-xs text-slate-500 hover:text-primary-600 hover:border-primary-300
                     transition-colors"
        >
          <ClipboardPaste className="w-3.5 h-3.5" />
          {t("home.pasteText")}
        </button>
      </div>
    </div>
  );
}
