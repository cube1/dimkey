import { create } from "zustand";
import type { FileContent, ViewType, DesensitizeResult, RestoreResult } from "../types";

interface AppState {
  /** 当前视图 */
  view: ViewType;
  /** 上一个视图（用于返回） */
  previousView: ViewType | null;
  /** 当前文件内容 */
  fileContent: FileContent | null;
  /** 当前文件路径 */
  filePath: string | null;
  /** 脱敏执行结果（P3 使用） */
  desensitizeResult: DesensitizeResult | null;
  /** 还原结果（P5 使用） */
  restoreResult: RestoreResult | null;
  /** 词典管理抽屉是否打开 */
  dictDrawerOpen: boolean;
  /** 策略配置面板是否打开 */
  strategyPanelOpen: boolean;

  setView: (view: ViewType) => void;
  goBack: () => void;
  setFileContent: (content: FileContent, path: string) => void;
  setDesensitizeResult: (result: DesensitizeResult | null) => void;
  setRestoreResult: (result: RestoreResult | null) => void;
  setDictDrawerOpen: (open: boolean) => void;
  setStrategyPanelOpen: (open: boolean) => void;
  reset: () => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  view: "home",
  previousView: null,
  fileContent: null,
  filePath: null,
  desensitizeResult: null,
  restoreResult: null,
  dictDrawerOpen: false,
  strategyPanelOpen: false,

  setView: (view) =>
    set({
      view,
      previousView: get().view,
    }),

  goBack: () => {
    const prev = get().previousView;
    if (prev) {
      set({ view: prev, previousView: null });
    } else {
      set({ view: "home", previousView: null });
    }
  },

  setFileContent: (content, path) =>
    set({ fileContent: content, filePath: path }),

  setDesensitizeResult: (result) =>
    set({ desensitizeResult: result }),

  setRestoreResult: (result) =>
    set({ restoreResult: result }),

  setDictDrawerOpen: (open) => set({ dictDrawerOpen: open }),

  setStrategyPanelOpen: (open) => set({ strategyPanelOpen: open }),

  reset: () =>
    set({
      view: "home",
      previousView: null,
      fileContent: null,
      filePath: null,
      desensitizeResult: null,
      restoreResult: null,
    }),
}));
