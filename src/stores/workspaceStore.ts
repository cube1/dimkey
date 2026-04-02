import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type {
  WorkspaceListItem,
  WorkspaceData,
  Workspace,
  WorkspaceMode,
  Strategy,
  DictEntry,
  SensitiveType,
  CenterView,
  FileContent,
  SensitiveItem,
  DesensitizeResult,
  RestoreResult,
  AutoDesensitizeStep,
  ColumnInference,
  ColumnRule,
  PasswordModalState,
  QueueFile,
  AliasGroup,
} from "../types";
import { getSensitiveTypeKey } from "../types";

interface WorkspaceState {
  /** 工作区列表 */
  workspaces: WorkspaceListItem[];
  /** 当前选中的工作区 ID */
  activeWorkspaceId: string | null;
  /** 当前工作区完整数据 */
  activeWorkspaceData: WorkspaceData | null;
  /** 中栏视图 */
  centerView: CenterView;
  /** 当前查看的处理记录 ID */
  activeRecordId: string | null;
  /** 当前拖入文件的原始内容 */
  currentFileContent: FileContent | null;
  /** 当前文件路径 */
  currentFilePath: string | null;
  /** 当前识别到的敏感项（可能被 enabledTypes 过滤后的子集） */
  currentSensitiveItems: SensitiveItem[];
  /** 全量识别结果（不受 enabledTypes 过滤影响，重新脱敏时使用） */
  rawSensitiveItems: SensitiveItem[];
  /** 当前处理记录 ID（用于列级重新脱敏后同步映射） */
  currentRecordId: string | null;
  /** 当前脱敏结果 */
  currentResult: DesensitizeResult | null;
  /** 当前还原结果 */
  restoreResult: RestoreResult | null;
  /** 当前处理步骤 */
  processingStep: AutoDesensitizeStep;
  /** 当前处理的文件名 */
  processingFileName: string;
  /** 左侧栏是否展开 */
  leftSidebarOpen: boolean;
  /** 右侧栏是否展开 */
  rightSidebarOpen: boolean;

  // --- 列级脱敏 ---
  /** 列类型推断结果 */
  columnInferences: ColumnInference[];
  /** 已确认的列规则（key 为 "sheetIndex:col"） */
  confirmedColumnRules: Record<string, ColumnRule>;
  /** 是否处于列级脱敏模式 */
  isColumnMode: boolean;
  /** 当前活跃的 Sheet 索引 */
  activeSheetIndex: number;

  // --- 批量文件队列 ---
  fileQueue: QueueFile[];
  activeQueueIndex: number;

  // --- 密码弹窗 ---
  passwordModal: PasswordModalState;
  setPasswordModal: (state: PasswordModalState | null) => void;

  // --- 侧栏切换 ---
  toggleLeftSidebar: () => void;
  toggleRightSidebar: () => void;

  // --- 工作区操作 ---
  loadWorkspaces: () => Promise<void>;
  selectWorkspace: (id: string) => Promise<void>;
  createWorkspace: (name: string) => Promise<void>;
  createClipboardWorkspace: (name: string) => Promise<void>;
  deleteWorkspace: (id: string) => Promise<void>;
  renameWorkspace: (id: string, name: string) => Promise<void>;

  // --- 策略/配置更新 ---
  updateStrategies: (strategies: Record<string, Strategy>) => Promise<void>;
  updateDictEntries: (entries: DictEntry[]) => Promise<void>;
  /** 按索引更新单个词典条目 */
  updateSingleDictEntry: (index: number, entry: DictEntry) => Promise<void>;
  /** 从 Popover 添加/更新词典条目（已存在同文本则更新 replacement，否则新增） */
  addDictEntryFromPopover: (text: string, sensitiveType: SensitiveType, replacement: string, matchMode?: DictEntry["match_mode"]) => Promise<void>;
  updateColumnRules: (rules: Record<string, string>) => Promise<void>;
  updateOutputDir: (dir: string | null) => Promise<void>;
  updateEnabledTypes: (types: string[]) => Promise<void>;
  updateWorkspaceMode: (mode: WorkspaceMode) => Promise<string | null>;
  /** 添加白名单条目 */
  addWhitelistEntry: (text: string, matchMode?: "Exact" | "Fuzzy") => Promise<void>;
  /** 删除白名单条目 */
  removeWhitelistEntry: (index: number) => Promise<void>;

  // --- 中栏视图 ---
  setCenterView: (view: CenterView) => void;
  setCurrentFileContent: (content: FileContent, path: string) => void;
  setCurrentSensitiveItems: (items: SensitiveItem[]) => void;
  setRawSensitiveItems: (items: SensitiveItem[]) => void;
  setCurrentRecordId: (id: string | null) => void;
  setCurrentResult: (result: DesensitizeResult | null) => void;
  setRestoreResult: (result: RestoreResult | null) => void;
  setProcessingStep: (step: AutoDesensitizeStep, fileName?: string) => void;
  viewRecord: (recordId: string) => void;

  // --- 一致性映射 ---
  clearConsistencyMappings: () => Promise<void>;
  clearTypeConsistencyMappings: (typeKey: string) => Promise<void>;

  // --- 别名组 ---
  /** 当前工作区的别名组 */
  aliasGroups: AliasGroup[];
  /** 是否处于关联模式 */
  aliasLinkMode: boolean;
  /** 关联模式中已选的成员 */
  aliasLinkMembers: SensitiveItem[];
  enterAliasLinkMode: (initialItem: SensitiveItem) => void;
  exitAliasLinkMode: () => void;
  addAliasLinkMember: (item: SensitiveItem) => void;
  removeAliasLinkMember: (itemId: string) => void;
  confirmAliasGroup: () => Promise<void>;
  fetchAliasGroups: () => Promise<void>;
  addMemberToGroup: (groupId: string, member: string) => Promise<void>;
  removeMemberFromGroup: (groupId: string, member: string) => Promise<void>;
  deleteAliasGroup: (groupId: string) => Promise<void>;

  // --- 列级脱敏操作 ---
  setColumnInferences: (inferences: ColumnInference[]) => void;
  setColumnRule: (key: string, rule: ColumnRule | null) => void;
  setIsColumnMode: (mode: boolean) => void;
  resetColumnState: () => void;
  setActiveSheetIndex: (index: number) => void;

  // --- 批量队列操作 ---
  initFileQueue: (files: QueueFile[]) => void;
  updateQueueFileStatus: (id: string, status: QueueFile["status"], errorMessage?: string) => void;
  advanceQueue: () => QueueFile | null;
  clearFileQueue: () => void;
  isBatchMode: () => boolean;
  hasUnfinishedFiles: () => boolean;
  /** 推进到下一个文件并标记为 processing，返回该文件；无更多文件时返回 null */
  advanceToNextFile: () => QueueFile | null;

  // --- 刷新当前工作区 ---
  refreshActiveWorkspace: () => Promise<void>;
}

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  workspaces: [],
  activeWorkspaceId: null,
  activeWorkspaceData: null,
  centerView: "empty",
  activeRecordId: null,
  currentFileContent: null,
  currentFilePath: null,
  currentSensitiveItems: [],
  rawSensitiveItems: [],
  currentRecordId: null,
  currentResult: null,
  restoreResult: null,
  processingStep: "idle",
  processingFileName: "",
  leftSidebarOpen: true,
  rightSidebarOpen: true,
  columnInferences: [],
  confirmedColumnRules: {},
  isColumnMode: false,
  activeSheetIndex: 0,
  fileQueue: [],
  activeQueueIndex: -1,
  aliasGroups: [],
  aliasLinkMode: false,
  aliasLinkMembers: [],
  passwordModal: { visible: false, filePath: "", fileType: "", attemptsLeft: 3, errorMessage: null },

  setPasswordModal: (state) => set({
    passwordModal: state || { visible: false, filePath: "", fileType: "", attemptsLeft: 3, errorMessage: null },
  }),

  toggleLeftSidebar: () => set((s) => ({ leftSidebarOpen: !s.leftSidebarOpen })),
  toggleRightSidebar: () => set((s) => ({ rightSidebarOpen: !s.rightSidebarOpen })),

  loadWorkspaces: async () => {
    try {
      const list = await invoke<WorkspaceListItem[]>("list_workspaces");
      set({ workspaces: list });
    } catch (e) {
      console.error("加载工作区列表失败:", e);
    }
  },

  selectWorkspace: async (id) => {
    try {
      const data = await invoke<WorkspaceData>("get_workspace", { id });
      set({
        activeWorkspaceId: id,
        activeWorkspaceData: data,
        centerView: "dropzone",
        activeRecordId: null,
        currentFileContent: null,
        currentFilePath: null,
        currentSensitiveItems: [],
        rawSensitiveItems: [],
        currentRecordId: null,
        currentResult: null,
        restoreResult: null,
        processingStep: "idle",
        processingFileName: "",
        columnInferences: [],
        confirmedColumnRules: {},
        isColumnMode: false,
        activeSheetIndex: 0,
        fileQueue: [],
        activeQueueIndex: -1,
        aliasGroups: data.workspace.alias_groups ?? [],
      });
    } catch (e) {
      console.error("加载工作区失败:", e);
      throw e;
    }
  },

  createWorkspace: async (name) => {
    try {
      const ws = await invoke<Workspace>("create_workspace", { name });
      await get().loadWorkspaces();
      await get().selectWorkspace(ws.id);
    } catch (e) {
      console.error("创建工作区失败:", e);
      throw e;
    }
  },

  createClipboardWorkspace: async (name) => {
    try {
      const ws = await invoke<Workspace>("create_clipboard_workspace", { name });
      await get().loadWorkspaces();
      await get().selectWorkspace(ws.id);
    } catch (e) {
      console.error("创建粘贴板工作区失败:", e);
      throw e;
    }
  },

  deleteWorkspace: async (id) => {
    try {
      await invoke("delete_workspace", { id });
      const state = get();
      if (state.activeWorkspaceId === id) {
        set({
          activeWorkspaceId: null,
          activeWorkspaceData: null,
          centerView: "empty",
          activeRecordId: null,
          currentFileContent: null,
          currentFilePath: null,
          currentSensitiveItems: [],
          rawSensitiveItems: [],
          currentRecordId: null,
          currentResult: null,
          restoreResult: null,
          fileQueue: [],
          activeQueueIndex: -1,
        });
      }
      await get().loadWorkspaces();
    } catch (e) {
      console.error("删除工作区失败:", e);
      throw e;
    }
  },

  renameWorkspace: async (id, name) => {
    try {
      await invoke("rename_workspace", { id, name });
      await get().loadWorkspaces();
      // 如果正在查看该工作区，刷新数据
      if (get().activeWorkspaceId === id) {
        await get().refreshActiveWorkspace();
      }
    } catch (e) {
      console.error("重命名工作区失败:", e);
      throw e;
    }
  },

  updateStrategies: async (strategies) => {
    await updateWorkspaceField(get, set, { strategies });
  },

  updateDictEntries: async (entries) => {
    await updateWorkspaceField(get, set, { dict_entries: entries });
  },

  updateSingleDictEntry: async (index, entry) => {
    const wsData = get().activeWorkspaceData;
    if (!wsData) return;
    const entries = [...wsData.workspace.dict_entries];
    entries[index] = entry;
    await get().updateDictEntries(entries);
  },

  addDictEntryFromPopover: async (text, sensitiveType, replacement, matchMode = "Exact") => {
    const wsData = get().activeWorkspaceData;
    if (!wsData) return;
    const entries = [...wsData.workspace.dict_entries];
    const existingIndex = entries.findIndex((e) => e.text === text);
    if (existingIndex >= 0) {
      entries[existingIndex] = { ...entries[existingIndex], replacement };
    } else {
      entries.push({ text, sensitive_type: sensitiveType, match_mode: matchMode, replacement });
    }
    await get().updateDictEntries(entries);
  },

  updateColumnRules: async (rules) => {
    await updateWorkspaceField(get, set, { column_rules: rules });
  },

  updateOutputDir: async (dir) => {
    await updateWorkspaceField(get, set, { output_dir: dir });
  },

  updateEnabledTypes: async (types) => {
    await updateWorkspaceField(get, set, { enabled_types: types });
  },

  updateWorkspaceMode: async (mode) => {
    const filePath = get().currentFilePath;
    await updateWorkspaceField(get, set, { mode });
    // 切换模式时清除识别结果和脱敏结果，但保留 filePath 用于重新处理
    set({
      currentSensitiveItems: [],
      rawSensitiveItems: [],
      currentResult: null,
      currentFileContent: null,
      currentFilePath: null,
      processingStep: "idle",
    });
    return filePath;
  },

  addWhitelistEntry: async (text, matchMode = "Exact") => {
    const wsData = get().activeWorkspaceData;
    if (!wsData) return;
    const whitelist = [...(wsData.workspace.whitelist || [])];
    // 查重
    if (whitelist.some((w) => w.text === text && w.match_mode === matchMode)) return;
    whitelist.push({ text, match_mode: matchMode });
    await updateWorkspaceField(get, set, { whitelist });
  },

  removeWhitelistEntry: async (index) => {
    const wsData = get().activeWorkspaceData;
    if (!wsData) return;
    const whitelist = [...(wsData.workspace.whitelist || [])];
    whitelist.splice(index, 1);
    await updateWorkspaceField(get, set, { whitelist });
  },

  setCenterView: (view) => set({ centerView: view }),

  setCurrentFileContent: (content, path) =>
    set({ currentFileContent: content, currentFilePath: path }),

  setCurrentSensitiveItems: (items) => set({ currentSensitiveItems: items }),
  setRawSensitiveItems: (items) => set({ rawSensitiveItems: items }),

  setCurrentRecordId: (id) => set({ currentRecordId: id }),

  setCurrentResult: (result) => set({ currentResult: result }),

  setRestoreResult: (result) => set({ restoreResult: result }),

  setProcessingStep: (step, fileName) =>
    set({ processingStep: step, ...(fileName !== undefined && { processingFileName: fileName }) }),

  viewRecord: (recordId) =>
    set({ activeRecordId: recordId, centerView: "comparison" }),

  clearConsistencyMappings: async () => {
    const id = get().activeWorkspaceId;
    if (!id) return;
    try {
      await invoke("clear_consistency_mappings", { workspaceId: id });
      await get().refreshActiveWorkspace();
    } catch (e) {
      console.error("清空一致性映射失败:", e);
      throw e;
    }
  },

  clearTypeConsistencyMappings: async (typeKey: string) => {
    const id = get().activeWorkspaceId;
    if (!id) return;
    try {
      await invoke("clear_type_consistency_mappings", { workspaceId: id, sensitiveTypeKey: typeKey });
    } catch (e) {
      console.error("清除类型一致性映射失败:", e);
    }
  },

  // --- 别名组 ---
  enterAliasLinkMode: (initialItem) => {
    set({ aliasLinkMode: true, aliasLinkMembers: [initialItem] });
  },

  exitAliasLinkMode: () => {
    set({ aliasLinkMode: false, aliasLinkMembers: [] });
  },

  // 按 text 去重（同一文本不重复入组）；移除时按 id（UI 绑定的是具体实例）
  addAliasLinkMember: (item) => {
    const { aliasLinkMembers } = get();
    if (aliasLinkMembers.some((m) => m.text === item.text)) return;
    // 校验敏感类型与首个成员一致
    if (aliasLinkMembers.length > 0) {
      const firstKey = getSensitiveTypeKey(aliasLinkMembers[0].sensitive_type);
      const itemKey = getSensitiveTypeKey(item.sensitive_type);
      if (firstKey !== itemKey) return;
    }
    set({ aliasLinkMembers: [...aliasLinkMembers, item] });
  },

  removeAliasLinkMember: (itemId) => {
    const { aliasLinkMembers } = get();
    set({ aliasLinkMembers: aliasLinkMembers.filter((m) => m.id !== itemId) });
  },

  confirmAliasGroup: async () => {
    const { activeWorkspaceId, aliasLinkMembers } = get();
    if (!activeWorkspaceId || aliasLinkMembers.length < 2) return;
    const members = aliasLinkMembers.map((m) => m.text);
    const sensitiveTypeKey = getSensitiveTypeKey(aliasLinkMembers[0].sensitive_type);
    await invoke("create_alias_group", {
      workspaceId: activeWorkspaceId,
      members,
      sensitiveTypeKey,
    });
    await get().refreshActiveWorkspace();
    set({
      aliasGroups: get().activeWorkspaceData?.workspace.alias_groups ?? [],
      aliasLinkMode: false,
      aliasLinkMembers: [],
    });
  },

  fetchAliasGroups: async () => {
    const { activeWorkspaceId } = get();
    if (!activeWorkspaceId) return;
    const groups = await invoke<AliasGroup[]>("list_alias_groups", {
      workspaceId: activeWorkspaceId,
    });
    set({ aliasGroups: groups });
  },

  addMemberToGroup: async (groupId, member) => {
    const { activeWorkspaceId } = get();
    if (!activeWorkspaceId) return;
    await invoke("add_alias_member", {
      workspaceId: activeWorkspaceId,
      groupId,
      member,
    });
    await get().fetchAliasGroups();
  },

  removeMemberFromGroup: async (groupId, member) => {
    const { activeWorkspaceId } = get();
    if (!activeWorkspaceId) return;
    await invoke("remove_alias_member", {
      workspaceId: activeWorkspaceId,
      groupId,
      member,
    });
    await get().fetchAliasGroups();
  },

  deleteAliasGroup: async (groupId) => {
    const { activeWorkspaceId } = get();
    if (!activeWorkspaceId) return;
    await invoke("delete_alias_group", {
      workspaceId: activeWorkspaceId,
      groupId,
    });
    await get().fetchAliasGroups();
  },

  setColumnInferences: (inferences) => set({ columnInferences: inferences }),

  setColumnRule: (key, rule) =>
    set((s) => {
      const next = { ...s.confirmedColumnRules };
      if (rule) {
        next[key] = rule;
      } else {
        delete next[key];
      }
      return { confirmedColumnRules: next };
    }),

  setIsColumnMode: (mode) => set({ isColumnMode: mode }),

  resetColumnState: () =>
    set({ columnInferences: [], confirmedColumnRules: {}, isColumnMode: false, activeSheetIndex: 0 }),

  setActiveSheetIndex: (index) => set({ activeSheetIndex: index }),

  initFileQueue: (files) => set({ fileQueue: files, activeQueueIndex: 0 }),

  updateQueueFileStatus: (id, status, errorMessage) =>
    set((s) => ({
      fileQueue: s.fileQueue.map((f) =>
        f.id === id ? { ...f, status, ...(errorMessage !== undefined && { errorMessage }) } : f
      ),
    })),

  advanceQueue: () => {
    const state = get();
    const nextIndex = state.fileQueue.findIndex(
      (f, i) => i > state.activeQueueIndex && f.status === "pending"
    );
    if (nextIndex >= 0) {
      set({ activeQueueIndex: nextIndex });
      return state.fileQueue[nextIndex];
    }
    return null;
  },

  clearFileQueue: () => set({ fileQueue: [], activeQueueIndex: -1 }),

  isBatchMode: () => get().fileQueue.length > 1,

  hasUnfinishedFiles: () =>
    get().fileQueue.some((f) => f.status === "pending" || f.status === "processing"),

  advanceToNextFile: () => {
    const state = get();
    const nextIndex = state.fileQueue.findIndex(
      (f, i) => i > state.activeQueueIndex && f.status === "pending"
    );
    if (nextIndex >= 0) {
      const nextFile = state.fileQueue[nextIndex];
      set({
        activeQueueIndex: nextIndex,
        fileQueue: state.fileQueue.map((f, i) =>
          i === nextIndex ? { ...f, status: "processing" as const } : f
        ),
      });
      return nextFile;
    }
    return null;
  },

  refreshActiveWorkspace: async () => {
    const id = get().activeWorkspaceId;
    if (!id) return;
    try {
      const data = await invoke<WorkspaceData>("get_workspace", { id });
      set({ activeWorkspaceData: data });
      set({ aliasGroups: data.workspace.alias_groups ?? [] });
    } catch (e) {
      console.error("刷新工作区失败:", e);
    }
  },
}));

/** 通用工作区字段更新（invoke 后重新读取最新 store，避免竞态覆盖） */
async function updateWorkspaceField(
  get: () => WorkspaceState,
  set: (partial: Partial<WorkspaceState>) => void,
  fields: Partial<Workspace>,
) {
  const data = get().activeWorkspaceData;
  if (!data) return;
  const updated = { ...data.workspace, ...fields };
  try {
    await invoke("update_workspace", { workspace: updated });
    // invoke 成功后，基于最新 store 状态更新，避免覆盖并发修改
    const latest = get().activeWorkspaceData;
    if (latest && latest.workspace.id === data.workspace.id) {
      set({
        activeWorkspaceData: {
          ...latest,
          workspace: { ...latest.workspace, ...fields },
        },
      });
    }
  } catch (e) {
    console.error("更新工作区配置失败:", e);
    throw e;
  }
}
