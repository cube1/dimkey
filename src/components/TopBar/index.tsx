import { useTranslation } from "react-i18next";
import { useAppStore } from "../../stores/appStore";
import type { ViewType } from "../../types";

/** 顶部栏按钮配置 */
interface TopBarAction {
  label: string;
  onClick: () => void;
  visible?: (view: ViewType) => boolean;
}

export function TopBar() {
  const { t } = useTranslation();
  const view = useAppStore((s) => s.view);
  const setView = useAppStore((s) => s.setView);
  const goBack = useAppStore((s) => s.goBack);
  const setDictDrawerOpen = useAppStore((s) => s.setDictDrawerOpen);
  const setStrategyPanelOpen = useAppStore((s) => s.setStrategyPanelOpen);

  const showBack = view !== "home";

  const actions: TopBarAction[] = [
    {
      label: t("topBar.dictManager"),
      onClick: () => setDictDrawerOpen(true),
    },
    {
      label: t("topBar.strategyConfig"),
      onClick: () => setStrategyPanelOpen(true),
      visible: (v) => v === "preview",
    },
    {
      label: t("topBar.historyTasks"),
      onClick: () => setView("history"),
      visible: (v) => v !== "history",
    },
  ];

  return (
    <header className="bg-white border-b border-gray-200 sticky top-0 z-30">
      <div className="max-w-7xl mx-auto px-6 h-14 flex items-center justify-between">
        {/* 左侧：返回 + 标题 */}
        <div className="flex items-center gap-3">
          {showBack && (
            <button
              onClick={goBack}
              className="text-gray-500 hover:text-gray-700 transition-colors text-sm flex items-center gap-1"
            >
              <svg
                className="w-4 h-4"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M15 19l-7-7 7-7"
                />
              </svg>
              {t("common.back")}
            </button>
          )}
          <h1 className="text-lg font-semibold text-gray-800">{t("topBar.desensitizeTool")}</h1>
        </div>

        {/* 右侧：操作按钮 */}
        <div className="flex items-center gap-2">
          {actions
            .filter((a) => !a.visible || a.visible(view))
            .map((a) => (
              <button
                key={a.label}
                onClick={a.onClick}
                className="px-3 py-1.5 text-sm text-gray-600 hover:text-gray-800 hover:bg-gray-100 rounded-md transition-colors"
              >
                {a.label}
              </button>
            ))}
        </div>
      </div>
    </header>
  );
}
