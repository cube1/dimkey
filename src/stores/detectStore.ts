import { create } from "zustand";
import type { SensitiveItem, Strategy } from "../types";
import { getSensitiveTypeKey } from "../types";

/** 编辑操作（用于撤销栈） */
export type EditAction =
  | { type: "remove"; item: SensitiveItem }
  | { type: "add"; item: SensitiveItem }
  | { type: "override"; id: string; previous: Strategy | null };

/** 撤销栈最大深度 */
const MAX_UNDO_STACK = 50;

interface DetectState {
  /** 引擎识别到的全部敏感项（规则+NER+词典） */
  items: SensitiveItem[];
  /** NER 异步状态 */
  nerStatus: "idle" | "running" | "done";
  /** 用户取消标记的项 ID */
  removedIds: Set<string>;
  /** 用户手动标记的项 */
  addedItems: SensitiveItem[];
  /** 单项策略覆盖（M3 浮层中修改的） */
  itemOverrides: Map<string, Strategy>;
  /** 撤销栈 */
  undoStack: EditAction[];
  /** 按类型隐藏的敏感项类型 */
  hiddenTypes: Set<string>;

  // --- 操作 ---
  setItems: (items: SensitiveItem[]) => void;
  appendItems: (items: SensitiveItem[]) => void;
  setNerStatus: (status: "idle" | "running" | "done") => void;
  removeItem: (id: string) => void;
  addItem: (item: SensitiveItem) => void;
  overrideStrategy: (id: string, strategy: Strategy) => void;
  replaceDictItems: (newDictItems: SensitiveItem[]) => void;
  toggleType: (typeKey: string) => void;
  showAllTypes: () => void;
  hideAllTypes: () => void;
  undo: () => void;
  resetDetect: () => void;
}

export const useDetectStore = create<DetectState>((set, get) => ({
  items: [],
  nerStatus: "idle",
  removedIds: new Set(),
  addedItems: [],
  itemOverrides: new Map(),
  undoStack: [],
  hiddenTypes: new Set(),

  setItems: (items) => set({ items }),

  appendItems: (newItems) =>
    set((state) => {
      // 过滤掉与已有项位置重叠的新项（正则/词典优先，NER 只补充空白区域）
      const filtered = newItems.filter((newItem) =>
        !state.items.some(
          (ex) =>
            ex.row === newItem.row &&
            ex.col === newItem.col &&
            ex.start < newItem.end &&
            newItem.start < ex.end
        )
      );
      return { items: [...state.items, ...filtered] };
    }),

  setNerStatus: (status) => set({ nerStatus: status }),

  removeItem: (id) => {
    const state = get();
    const item = getActiveItems(state).find((i) => i.id === id);
    if (!item) return;

    const newRemoved = new Set(state.removedIds);
    newRemoved.add(id);
    const newStack = [...state.undoStack, { type: "remove" as const, item }];
    if (newStack.length > MAX_UNDO_STACK) newStack.shift();

    set({ removedIds: newRemoved, undoStack: newStack });
  },

  addItem: (item) => {
    const state = get();
    const newStack = [...state.undoStack, { type: "add" as const, item }];
    if (newStack.length > MAX_UNDO_STACK) newStack.shift();

    set({
      addedItems: [...state.addedItems, item],
      undoStack: newStack,
    });
  },

  overrideStrategy: (id, strategy) => {
    const state = get();
    const previous = state.itemOverrides.get(id) ?? null;
    const newOverrides = new Map(state.itemOverrides);
    newOverrides.set(id, strategy);

    const newStack = [
      ...state.undoStack,
      { type: "override" as const, id, previous },
    ];
    if (newStack.length > MAX_UNDO_STACK) newStack.shift();

    set({ itemOverrides: newOverrides, undoStack: newStack });
  },

  replaceDictItems: (newDictItems) =>
    set((state) => ({
      items: [
        ...state.items.filter((i) => i.source !== "Dict"),
        ...newDictItems,
      ],
    })),

  toggleType: (typeKey) =>
    set((state) => {
      const newHidden = new Set(state.hiddenTypes);
      if (newHidden.has(typeKey)) {
        newHidden.delete(typeKey);
      } else {
        newHidden.add(typeKey);
      }
      return { hiddenTypes: newHidden };
    }),

  showAllTypes: () => set({ hiddenTypes: new Set() }),

  hideAllTypes: () =>
    set((state) => {
      const allItems = getActiveItems(state);
      const allTypes = new Set(allItems.map((i) => getSensitiveTypeKey(i.sensitive_type)));
      return { hiddenTypes: allTypes };
    }),

  undo: () => {
    const state = get();
    if (state.undoStack.length === 0) return;

    const action = state.undoStack[state.undoStack.length - 1];
    const newStack = state.undoStack.slice(0, -1);

    switch (action.type) {
      case "remove": {
        const newRemoved = new Set(state.removedIds);
        newRemoved.delete(action.item.id);
        set({ removedIds: newRemoved, undoStack: newStack });
        break;
      }
      case "add": {
        set({
          addedItems: state.addedItems.filter((i) => i.id !== action.item.id),
          undoStack: newStack,
        });
        break;
      }
      case "override": {
        const newOverrides = new Map(state.itemOverrides);
        if (action.previous === null) {
          newOverrides.delete(action.id);
        } else {
          newOverrides.set(action.id, action.previous);
        }
        set({ itemOverrides: newOverrides, undoStack: newStack });
        break;
      }
    }
  },

  resetDetect: () =>
    set({
      items: [],
      nerStatus: "idle",
      removedIds: new Set(),
      addedItems: [],
      itemOverrides: new Map(),
      undoStack: [],
      hiddenTypes: new Set(),
    }),
}));

/** 计算当前有效的敏感项列表 */
function getActiveItems(state: DetectState): SensitiveItem[] {
  const fromEngine = state.items.filter((i) => !state.removedIds.has(i.id));
  return [...fromEngine, ...state.addedItems];
}

/** Hook：获取有效敏感项（受 hiddenTypes 过滤） */
export function useActiveItems(): SensitiveItem[] {
  return useDetectStore(
    (state) => {
      const items = getActiveItems(state);
      if (state.hiddenTypes.size === 0) return items;
      return items.filter((i) => !state.hiddenTypes.has(getSensitiveTypeKey(i.sensitive_type)));
    },
    (a, b) => a.length === b.length && a.every((item, i) => item.id === b[i]?.id)
  );
}

/** Hook：获取全部有效敏感项（不受 hiddenTypes 影响，用于统计/脱敏） */
export function useAllActiveItems(): SensitiveItem[] {
  return useDetectStore(
    (state) => getActiveItems(state),
    (a, b) => a.length === b.length && a.every((item, i) => item.id === b[i]?.id)
  );
}

/** Hook：按类型统计数量（返回普通对象以便 shallow 比较） */
export function useSummaryByType(): Record<string, number> {
  return useDetectStore(
    (state) => {
      const items = getActiveItems(state);
      const result: Record<string, number> = {};
      for (const item of items) {
        const key = getSensitiveTypeKey(item.sensitive_type);
        result[key] = (result[key] ?? 0) + 1;
      }
      return result;
    },
    (a, b) => {
      const keysA = Object.keys(a);
      const keysB = Object.keys(b);
      if (keysA.length !== keysB.length) return false;
      return keysA.every((k) => a[k] === b[k]);
    }
  );
}
