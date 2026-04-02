import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useWorkspaceStore } from "../stores/workspaceStore";
import { useDetectStore } from "../stores/detectStore";
import type { SensitiveItem } from "../types";

/**
 * 共享 hook：重新执行词典检测并更新 detectStore + workspaceStore
 * 用于词典条目变更后刷新高亮
 */
export function useReDetectDict() {
  const replaceDictItems = useDetectStore((s) => s.replaceDictItems);

  const reDetectDict = useCallback(async () => {
    const store = useWorkspaceStore.getState();
    const fileContent = store.currentFileContent;
    const wsData = store.activeWorkspaceData;
    const dictEntries = wsData?.workspace.dict_entries || [];
    if (!fileContent) return;

    try {
      const dictItems = dictEntries.length > 0
        ? await invoke<SensitiveItem[]>("detect_by_dict", {
            content: fileContent,
            dictEntries,
          })
        : [];

      // 更新 detectStore（保持现有行为）
      replaceDictItems(dictItems);

      // 同步更新 workspaceStore.currentSensitiveItems
      const currentItems = store.currentSensitiveItems;
      const nonDictItems = currentItems.filter((i) => i.source !== "Dict");

      // enabledTypes 过滤
      const enabledTypes = wsData?.workspace.enabled_types || [];
      const enabledDictItems = dictItems.filter((item) => {
        const key = typeof item.sensitive_type === "string"
          ? item.sensitive_type
          : "Custom";
        return enabledTypes.includes(key);
      });

      store.setCurrentSensitiveItems([...nonDictItems, ...enabledDictItems]);

      // 同时更新 rawSensitiveItems
      const rawItems = store.rawSensitiveItems;
      const rawNonDict = rawItems.filter((i) => i.source !== "Dict");
      store.setRawSensitiveItems([...rawNonDict, ...dictItems]);
    } catch {
      // 静默处理
    }
  }, [replaceDictItems]);

  return reDetectDict;
}
