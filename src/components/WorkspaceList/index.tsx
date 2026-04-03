import { useState, useRef, useEffect } from "react";
import { Plus, Trash2, Loader2, Info, FolderOpen, ClipboardList } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useUpdateStore } from "../../stores/updateStore";
import { AboutModal } from "../AboutModal";

/** 格式化时间为简短格式 */
function formatTime(iso: string, lang: string): string {
  try {
    const locale = lang.startsWith("en") ? "en-US" : "zh-CN";
    const d = new Date(iso);
    const now = new Date();
    const isToday =
      d.getFullYear() === now.getFullYear() &&
      d.getMonth() === now.getMonth() &&
      d.getDate() === now.getDate();
    if (isToday) {
      return d.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
    }
    return d.toLocaleDateString(locale, { month: "short", day: "numeric" });
  } catch {
    return "";
  }
}

export function WorkspaceList() {
  const { t, i18n } = useTranslation();
  const workspaces = useWorkspaceStore((s) => s.workspaces);
  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId);
  const selectWorkspace = useWorkspaceStore((s) => s.selectWorkspace);
  const createWorkspace = useWorkspaceStore((s) => s.createWorkspace);
  const deleteWorkspace = useWorkspaceStore((s) => s.deleteWorkspace);
  const renameWorkspace = useWorkspaceStore((s) => s.renameWorkspace);

  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState("");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const confirmTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (editingId && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [editingId]);

  // 清理确认删除定时器
  useEffect(() => {
    return () => {
      if (confirmTimerRef.current) clearTimeout(confirmTimerRef.current);
    };
  }, []);

  const handleCreate = async () => {
    try {
      await createWorkspace(t("workspace.newWorkspace"));
    } catch {
      toast.error(t("workspace.createFailed"));
    }
  };

  const handleDoubleClick = (id: string, name: string) => {
    setEditingId(id);
    setEditName(name);
  };

  const handleRenameConfirm = async () => {
    if (!editingId || !editName.trim()) {
      setEditingId(null);
      return;
    }
    try {
      await renameWorkspace(editingId, editName.trim());
    } catch {
      toast.error(t("workspace.renameFailed"));
    }
    setEditingId(null);
  };

  const handleDelete = async (id: string) => {
    // 队列保护：如果有未完成的批量任务，先确认
    const store = useWorkspaceStore.getState();
    if (store.isBatchMode() && store.hasUnfinishedFiles()) {
      const unfinished = store.fileQueue.filter(f => f.status === "pending" || f.status === "processing").length;
      const confirmed = window.confirm(t("workspace.confirmDeleteQueue", { count: unfinished }));
      if (!confirmed) return;
      store.clearFileQueue();
    }
    try {
      await deleteWorkspace(id);
      setConfirmDeleteId(null);
      toast.success(t("workspace.deleted"));
    } catch {
      toast.error(t("workspace.deleteFailed"));
    }
  };

  // 版本号
  const [version, setVersion] = useState("");
  const updateState = useUpdateStore((s) => s.state);
  const checkForUpdate = useUpdateStore((s) => s.checkForUpdate);

  useEffect(() => {
    getVersion().then((v) => setVersion(v)).catch(() => {});
  }, []);

  const handleCheckUpdate = async () => {
    const result = await checkForUpdate();
    if (result === "latest") {
      toast.success(t("workspace.latestVersion"));
    } else if (result === "error") {
      toast.error(t("workspace.updateCheckFailed"));
    }
    // "available" 时 UpdateChecker 浮窗会自动显示
  };

  // 关于弹窗
  const [aboutVisible, setAboutVisible] = useState(false);

  // 统计开关
  const [analyticsEnabled, setAnalyticsEnabled] = useState(true);
  useEffect(() => {
    invoke<boolean>("get_analytics_enabled").then(setAnalyticsEnabled).catch(() => {});
  }, []);

  const toggleAnalytics = async () => {
    const next = !analyticsEnabled;
    setAnalyticsEnabled(next);
    try {
      await invoke("set_analytics_enabled", { enabled: next });
    } catch {
      setAnalyticsEnabled(!next); // 回滚
    }
  };

  return (
    <>
      {/* 标题 + 新建按钮 */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-slate-200">
        <h1 className="text-xs font-bold uppercase tracking-wider text-slate-500">{t("workspace.title")}</h1>
        <button
          onClick={handleCreate}
          className="p-1 text-slate-400 hover:text-primary-600 hover:bg-primary-50 rounded transition-colors"
          title={t("workspace.newWorkspaceShortcut")}
        >
          <Plus className="w-4 h-4" />
        </button>
      </div>

      {/* 工作区列表 */}
      <div className="flex-1 overflow-auto" data-testid="workspace-list">
        {workspaces.length === 0 ? (
          <div className="px-4 py-8 text-center">
            <p className="text-sm text-slate-400 mb-3">{t("workspace.empty")}</p>
            <button
              onClick={handleCreate}
              className="text-sm text-primary-500 hover:text-primary-600"
            >
              {t("workspace.createFirst")}
            </button>
          </div>
        ) : (
          <div className="py-1">
            {workspaces.map((ws) => (
              <div
                key={ws.id}
                onClick={() => {
                  const store = useWorkspaceStore.getState();
                  if (store.isBatchMode() && store.hasUnfinishedFiles()) {
                    const unfinished = store.fileQueue.filter(f => f.status === "pending" || f.status === "processing").length;
                    const confirmed = window.confirm(t("workspace.confirmSwitchQueue", { count: unfinished }));
                    if (!confirmed) return;
                    store.clearFileQueue();
                  }
                  selectWorkspace(ws.id).catch(() => toast.error(t("workspace.loadFailed")));
                }}
                onDoubleClick={() => handleDoubleClick(ws.id, ws.name)}
                className={`group flex items-center gap-2 px-4 py-2.5 cursor-pointer transition-colors ${
                  activeWorkspaceId === ws.id
                    ? "bg-primary-100 border-r-2 border-primary-500"
                    : "hover:bg-slate-50"
                }`}
              >
                {editingId === ws.id ? (
                  <input
                    ref={inputRef}
                    value={editName}
                    onChange={(e) => setEditName(e.target.value)}
                    onBlur={handleRenameConfirm}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleRenameConfirm();
                      if (e.key === "Escape") setEditingId(null);
                    }}
                    className="flex-1 min-w-0 text-sm px-1 py-0 border border-primary-300 rounded focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-400"
                    onClick={(e) => e.stopPropagation()}
                  />
                ) : (
                  <div className="flex-1 min-w-0">
                    <div className={`text-sm truncate flex items-center gap-1.5 ${
                      activeWorkspaceId === ws.id ? "text-primary-600 font-medium" : "text-slate-800"
                    }`}>
                      {ws.source === "Clipboard" ? (
                        <ClipboardList className="w-3.5 h-3.5 shrink-0 text-blue-400" />
                      ) : (
                        <FolderOpen className="w-3.5 h-3.5 shrink-0 text-slate-400" />
                      )}
                      <span className="truncate">{ws.name}</span>
                    </div>
                    <div className={`text-xs mt-0.5 ${
                      activeWorkspaceId === ws.id ? "text-primary-500/70" : "text-slate-400"
                    }`}>
                      {formatTime(ws.updated_at, i18n.language)}
                      {ws.history_count > 0 && ` · ${t("workspace.records", { count: ws.history_count })}`}
                    </div>
                  </div>
                )}

                {/* 删除按钮 */}
                {editingId !== ws.id && (
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      if (confirmDeleteId === ws.id) {
                        handleDelete(ws.id);
                      } else {
                        setConfirmDeleteId(ws.id);
                        if (confirmTimerRef.current) clearTimeout(confirmTimerRef.current);
                        confirmTimerRef.current = setTimeout(() => setConfirmDeleteId(null), 3000);
                      }
                    }}
                    className={`shrink-0 p-1 rounded transition-all ${
                      confirmDeleteId === ws.id
                        ? "text-rose-500 bg-rose-50"
                        : "text-slate-300 hover:text-rose-500 opacity-0 group-hover:opacity-100"
                    }`}
                    title={confirmDeleteId === ws.id ? t("workspace.confirmDeleteAgain") : t("workspace.deleteWorkspace")}
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* 底部：版本号 + 统计开关（单行紧凑布局） */}
      <div className="px-4 py-2 border-t border-slate-200 shrink-0">
        <div className="flex items-center justify-between">
          {/* 左侧：版本号 + 关于 */}
          <div className="flex items-center gap-1.5">
            {version && (
              <button
                onClick={handleCheckUpdate}
                disabled={updateState.status === "checking"}
                className="flex items-center gap-1 text-[11px] text-slate-400 hover:text-primary-500 transition-colors disabled:opacity-50 tabular-nums"
                title={t("workspace.checkUpdate")}
              >
                {updateState.status === "checking" ? (
                  <Loader2 className="w-3 h-3 animate-spin" />
                ) : (
                  <span className="w-1.5 h-1.5 rounded-full bg-slate-300 shrink-0" />
                )}
                <span>v{version}</span>
              </button>
            )}
            <button
              onClick={() => setAboutVisible(true)}
              className="p-0.5 text-slate-300 hover:text-primary-500 transition-colors"
              title={t("workspace.aboutApp")}
            >
              <Info className="w-3.5 h-3.5" />
            </button>
          </div>

          {/* 右侧：匿名统计 */}
          <button
            onClick={toggleAnalytics}
            className="flex items-center gap-1.5 group"
            title={analyticsEnabled ? t("workspace.analyticsEnabled") : t("workspace.analyticsDisabled")}
          >
            <span className="text-[11px] text-slate-400 group-hover:text-slate-500 transition-colors">
              {t("workspace.analytics")}
            </span>
            <div
              className={`relative w-6 h-3.5 rounded-full transition-colors ${
                analyticsEnabled ? "bg-primary-500" : "bg-slate-300"
              }`}
            >
              <div
                className={`absolute top-0.5 w-2.5 h-2.5 rounded-full bg-white shadow-sm transition-transform ${
                  analyticsEnabled ? "translate-x-3" : "translate-x-0.5"
                }`}
              />
            </div>
          </button>
        </div>
      </div>

      <AboutModal visible={aboutVisible} onClose={() => setAboutVisible(false)} />
    </>
  );
}
