import { useEffect, useRef, useState } from "react";
import toast, { Toaster } from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "./stores/workspaceStore";
import { useUpdateStore } from "./stores/updateStore";
import { useLicenseStore } from "./stores/licenseStore";
import { WorkspaceLayout } from "./layouts/WorkspaceLayout";
import { UpdateChecker } from "./components/UpdateChecker";
import { AnalyticsConsent } from "./components/AnalyticsConsent";
import { TrialExpiredBanner } from "./components/license/TrialExpiredBanner";
import { ActivationDialog } from "./components/license/ActivationDialog";

function App() {
  const { t } = useTranslation();

  const initLicense = useLicenseStore((s) => s.init);
  const initialized = useLicenseStore((s) => s.initialized);
  const licenseState = useLicenseStore((s) => s.state);
  const welcomeShownRef = useRef(false);
  const [activationOpen, setActivationOpen] = useState(false);

  // 应用启动时加载工作区列表
  useEffect(() => {
    useWorkspaceStore.getState().loadWorkspaces();
  }, []);

  // 初始化 license store —— 拉取状态 + 监听后端 state-changed 事件
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    initLicense().then((un) => {
      unlisten = un;
    });
    return () => {
      if (unlisten) unlisten();
    };
  }, [initLicense]);

  // 首启 welcome toast —— 仅在初始化后是 Trial 30 天（>=29）时弹一次
  useEffect(() => {
    if (
      initialized &&
      !welcomeShownRef.current &&
      licenseState.kind === "Trial" &&
      licenseState.days_remaining >= 29
    ) {
      toast(t("license.trial.welcome_toast"), { duration: 6000 });
      welcomeShownRef.current = true;
    }
  }, [initialized, licenseState, t]);

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
      <TrialExpiredBanner onActivate={() => setActivationOpen(true)} />
      <WorkspaceLayout />
      <UpdateChecker />
      <AnalyticsConsent />
      <ActivationDialog
        visible={activationOpen}
        onClose={() => setActivationOpen(false)}
      />
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
