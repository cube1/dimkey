import { useCallback } from "react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useAutoDesensitize } from "../../hooks/useAutoDesensitize";
import { EmptyDropzoneView } from "./EmptyDropzoneView";
import { DropzoneView } from "./DropzoneView";
import { ProcessingView } from "./ProcessingView";
import { ComparisonView } from "./ComparisonView";
import { RestoreView } from "./RestoreView";
import { FileQueueTabs } from "./FileQueueTabs";
import { PasswordModal } from "../PasswordModal";
import { AliasLinkBar } from "../AliasLinkMode";

export function CenterPanel() {
  const centerView = useWorkspaceStore((s) => s.centerView);
  const passwordModal = useWorkspaceStore((s) => s.passwordModal);
  const setPasswordModal = useWorkspaceStore((s) => s.setPasswordModal);
  const { processFile } = useAutoDesensitize();

  const handlePasswordSubmit = useCallback(async (password: string) => {
    const filePath = passwordModal.filePath;
    if (!filePath) return;
    await processFile(filePath, password);
  }, [passwordModal.filePath, processFile]);

  const handlePasswordCancel = useCallback(() => {
    setPasswordModal(null);
    const store = useWorkspaceStore.getState();
    if (store.centerView === "processing") {
      store.setCenterView(store.activeWorkspaceId ? "dropzone" : "empty");
    }
  }, [setPasswordModal]);

  return (
    <>
      {centerView === "empty" ? (
        <EmptyDropzoneView />
      ) : (
        <>
          <FileQueueTabs />
          <AliasLinkBar />
          {centerView === "dropzone" && <DropzoneView />}
          {centerView === "processing" && <ProcessingView />}
          {centerView === "comparison" && <ComparisonView />}
          {centerView === "restore" && <RestoreView />}
        </>
      )}
      <PasswordModal
        visible={passwordModal.visible}
        fileType={passwordModal.fileType}
        attemptsLeft={passwordModal.attemptsLeft}
        errorMessage={passwordModal.errorMessage}
        onSubmit={handlePasswordSubmit}
        onCancel={handlePasswordCancel}
      />
    </>
  );
}
