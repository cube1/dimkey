import { create } from "zustand";
import { check } from "@tauri-apps/plugin-updater";

export type UpdateState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "available"; version: string; body: string }
  | { status: "downloading"; progress: number }
  | { status: "ready" }
  | { status: "error"; message: string };

interface UpdateStore {
  state: UpdateState;
  dismissed: boolean;
  /** 检查更新，返回 "available" | "latest" | "error" */
  checkForUpdate: () => Promise<"available" | "latest" | "error">;
  downloadAndInstall: () => Promise<void>;
  dismiss: () => void;
}

export const useUpdateStore = create<UpdateStore>((set) => ({
  state: { status: "idle" },
  dismissed: false,

  checkForUpdate: async () => {
    set({ state: { status: "checking" }, dismissed: false });
    try {
      const update = await check();
      if (update) {
        set({
          state: {
            status: "available",
            version: update.version,
            body: update.body ?? "",
          },
        });
        return "available";
      } else {
        set({ state: { status: "idle" } });
        return "latest";
      }
    } catch (e) {
      console.warn("更新检查失败:", e);
      set({
        state: {
          status: "error",
          message: e instanceof Error ? e.message : "检查更新失败",
        },
      });
      return "error";
    }
  },

  downloadAndInstall: async () => {
    try {
      set({ state: { status: "downloading", progress: 0 } });
      const update = await check();
      if (!update) return;

      let totalBytes = 0;
      let downloadedBytes = 0;

      await update.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data.contentLength) {
          totalBytes = event.data.contentLength;
        } else if (event.event === "Progress") {
          downloadedBytes += event.data.chunkLength;
          if (totalBytes > 0) {
            set({
              state: {
                status: "downloading",
                progress: Math.round((downloadedBytes / totalBytes) * 100),
              },
            });
          }
        } else if (event.event === "Finished") {
          set({ state: { status: "ready" } });
        }
      });

      set({ state: { status: "ready" } });
    } catch (e) {
      set({
        state: {
          status: "error",
          message: e instanceof Error ? e.message : "更新失败",
        },
      });
    }
  },

  dismiss: () => {
    set({ dismissed: true });
  },
}));
