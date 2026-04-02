import { Download, RefreshCw, X } from "lucide-react";
import { relaunch } from "@tauri-apps/plugin-process";
import { useTranslation } from "react-i18next";
import { useUpdateStore } from "../../stores/updateStore";

export function UpdateChecker() {
  const { t } = useTranslation();
  const state = useUpdateStore((s) => s.state);
  const dismissed = useUpdateStore((s) => s.dismissed);
  const downloadAndInstall = useUpdateStore((s) => s.downloadAndInstall);
  const dismiss = useUpdateStore((s) => s.dismiss);

  const handleRelaunch = async () => {
    await relaunch();
  };

  // 无更新、检查中、或已关闭
  if (state.status === "idle" || state.status === "checking" || dismissed) return null;

  return (
    <div className="fixed bottom-4 right-4 z-50 w-80 rounded-xl bg-white shadow-lg ring-1 ring-gray-200 overflow-hidden">
      {/* 标题栏 */}
      <div className="flex items-center justify-between px-4 py-3 bg-gray-50 border-b border-gray-100">
        <span className="text-sm font-medium text-gray-700">{t("update.title")}</span>
        {state.status !== "downloading" && state.status !== "ready" && (
          <button
            onClick={dismiss}
            className="p-0.5 rounded hover:bg-gray-200 text-gray-400 hover:text-gray-600 transition-colors"
          >
            <X size={14} />
          </button>
        )}
      </div>

      <div className="px-4 py-3 space-y-3">
        {/* 发现新版本 */}
        {state.status === "available" && (
          <>
            <p className="text-sm text-gray-600">
              {t("update.newVersion", { version: state.version })}
            </p>
            {state.body && (
              <p className="text-xs text-gray-500 line-clamp-3">{state.body}</p>
            )}
            <button
              onClick={downloadAndInstall}
              className="w-full flex items-center justify-center gap-2 px-3 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-lg transition-colors"
            >
              <Download size={14} />
              {t("update.updateNow")}
            </button>
          </>
        )}

        {/* 下载中 */}
        {state.status === "downloading" && (
          <>
            <p className="text-sm text-gray-600">{t("update.downloading")}</p>
            <div className="w-full h-2 bg-gray-100 rounded-full overflow-hidden">
              <div
                className="h-full bg-blue-500 rounded-full transition-all duration-300"
                style={{ width: `${state.progress}%` }}
              />
            </div>
            <p className="text-xs text-gray-400 text-right">{state.progress}%</p>
          </>
        )}

        {/* 下载完成，等待重启 */}
        {state.status === "ready" && (
          <>
            <p className="text-sm text-gray-600">{t("update.downloaded")}</p>
            <button
              onClick={handleRelaunch}
              className="w-full flex items-center justify-center gap-2 px-3 py-2 text-sm font-medium text-white bg-green-600 hover:bg-green-700 rounded-lg transition-colors"
            >
              <RefreshCw size={14} />
              {t("update.restart")}
            </button>
          </>
        )}

        {/* 错误 */}
        {state.status === "error" && (
          <>
            <p className="text-sm text-red-600">{t("update.failed", { message: state.message })}</p>
            <button
              onClick={dismiss}
              className="w-full px-3 py-2 text-sm text-gray-600 hover:bg-gray-100 rounded-lg transition-colors"
            >
              {t("common.close")}
            </button>
          </>
        )}
      </div>
    </div>
  );
}
