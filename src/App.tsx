import { useEffect } from "react";
import { Toaster } from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "./stores/workspaceStore";
import { useUpdateStore } from "./stores/updateStore";
import { WorkspaceLayout } from "./layouts/WorkspaceLayout";
import { UpdateChecker } from "./components/UpdateChecker";
import { AnalyticsConsent } from "./components/AnalyticsConsent";

function App() {
  const { t } = useTranslation();

  // 应用启动时加载工作区列表
  useEffect(() => {
    useWorkspaceStore.getState().loadWorkspaces();
  }, []);

  // 启动后延迟 3 秒检查更新
  useEffect(() => {
    const timer = setTimeout(() => {
      useUpdateStore.getState().checkForUpdate();
    }, 3000);
    return () => clearTimeout(timer);
  }, []);

  // 全局快捷键
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;

      // Cmd+N：新建工作区
      if (mod && e.key === "n") {
        e.preventDefault();
        useWorkspaceStore.getState().createWorkspace(t("workspace.newWorkspace")).catch(() => {});
        return;
      }

      // Cmd+Shift+L：切换左侧栏
      if (mod && e.shiftKey && e.key === "L") {
        e.preventDefault();
        useWorkspaceStore.getState().toggleLeftSidebar();
        return;
      }

      // Cmd+Shift+R：切换右侧栏
      if (mod && e.shiftKey && e.key === "R") {
        e.preventDefault();
        useWorkspaceStore.getState().toggleRightSidebar();
        return;
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  return (
    <>
      <WorkspaceLayout />
      <UpdateChecker />
      <AnalyticsConsent />
      <Toaster
        position="top-right"
        toastOptions={{
          success: { duration: 3000 },
          error: { duration: 5000 },
          style: {
            fontSize: "13px",
            fontFamily: "Inter, 'PingFang SC', 'Microsoft YaHei', system-ui, sans-serif",
            borderRadius: "10px",
            boxShadow: "0 4px 12px -2px rgb(0 0 0 / 0.06), 0 2px 6px -2px rgb(0 0 0 / 0.04)",
          },
        }}
      />
    </>
  );
}

export default App;
