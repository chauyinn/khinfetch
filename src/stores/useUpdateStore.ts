import { create } from "zustand";
import { backendClient, type AppUpdateInfo } from "../api/client";
import { useJobsStore } from "./useJobsStore";

export type UpdateStatus =
  | "idle"
  | "checking"
  | "current"
  | "available"
  | "downloading"
  | "downloaded"
  | "installing"
  | "error";

interface UpdateState {
  status: UpdateStatus;
  update: AppUpdateInfo | null;
  downloadedBytes: number;
  totalBytes: number | null;
  error: unknown;
  checkForUpdate: () => Promise<AppUpdateInfo | null>;
  downloadUpdate: () => Promise<void>;
  installUpdate: () => Promise<boolean>;
  openReleasePage: () => Promise<void>;
}

let activeCheck: Promise<AppUpdateInfo | null> | null = null;

export const useUpdateStore = create<UpdateState>((set, get) => ({
  status: "idle",
  update: null,
  downloadedBytes: 0,
  totalBytes: null,
  error: null,

  checkForUpdate: async () => {
    let check = activeCheck;
    if (!check) {
      check = backendClient.checkForUpdate();
      activeCheck = check;
    }

    set({ status: "checking", update: null, error: null, downloadedBytes: 0, totalBytes: null });
    try {
      const update = await check;
      if (activeCheck === check) {
        set({ status: update ? "available" : "current", update });
      }
      return update;
    } catch (error) {
      if (activeCheck === check) {
        set({ status: "error", error });
      }
      return null;
    } finally {
      if (activeCheck === check) {
        activeCheck = null;
      }
    }
  },

  downloadUpdate: async () => {
    const update = get().update;
    if (!update || update.environment.installStrategy !== "integrated") return;

    set({ status: "downloading", error: null, downloadedBytes: 0, totalBytes: null });
    try {
      await backendClient.downloadUpdate(({ downloadedBytes, totalBytes }) => {
        set({ downloadedBytes, totalBytes });
      });
      set({ status: "downloaded" });
    } catch (error) {
      set({ status: "available", error });
    }
  },

  installUpdate: async () => {
    if (useJobsStore.getState().activeDownloadsCount > 0) return false;

    set({ status: "installing", error: null });
    try {
      await backendClient.installUpdate();
      return true;
    } catch (error) {
      set({ status: "downloaded", error });
      return false;
    }
  },

  openReleasePage: async () => {
    const releaseUrl = get().update?.environment.releaseUrl;
    if (!releaseUrl) return;
    await backendClient.openExternalUrl(releaseUrl);
  },
}));
