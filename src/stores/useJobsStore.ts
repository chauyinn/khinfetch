import { create } from "zustand";
import type { AlbumJob, BackendNotice } from "../api/generated";
import { backendClient } from "../api/client";
import { errorMessage } from "../api/errors";
import { getStrings } from "../i18n/catalog";
import { useAppStore } from "./useAppStore";
import { useToastStore } from "./useToastStore";

interface JobsState {
  jobs: AlbumJob[];
  isLoading: boolean;
  error: string | null;
  activeDownloadsCount: number;
  
  initialize: () => Promise<() => void>;
  refreshJobs: () => Promise<void>;
  
  pauseJob: (jobId: string) => Promise<void>;
  resumeJob: (jobId: string) => Promise<void>;
  cancelJob: (jobId: string) => Promise<void>;
  retryJob: (jobId: string) => Promise<void>;
  removeJob: (jobId: string) => Promise<void>;

  pauseAllJobs: () => Promise<void>;
  resumeAllJobs: () => Promise<void>;
}

export const useJobsStore = create<JobsState>((set, get) => ({
  jobs: [],
  isLoading: false,
  error: null,
  activeDownloadsCount: 0,

  initialize: async () => {
    set({ isLoading: true });

    // Register listeners first before listJobs() to prevent race conditions
    const unlistenJobUpdated = await backendClient.onJobUpdated((updatedJob) => {
      set((state) => {
        const existingIndex = state.jobs.findIndex((j) => j.id === updatedJob.id);
        let updatedList: AlbumJob[];

        if (existingIndex >= 0) {
          const existing = state.jobs[existingIndex];
          // Ignore stale sequence events
          if (updatedJob.sequence <= existing.sequence) {
            return state;
          }
          updatedList = [...state.jobs];
          updatedList[existingIndex] = updatedJob;
        } else {
          updatedList = [updatedJob, ...state.jobs];
        }

        const activeCount = updatedList.filter(
          (j) => j.state === "downloading" || j.state === "resolving" || j.state === "queued"
        ).length;

        return { jobs: updatedList, activeDownloadsCount: activeCount };
      });
    });

    const unlistenNotice = await backendClient.onNotice((notice: BackendNotice) => {
      useToastStore.getState().showToast(notice.message, "warning");
    });

    // Load authoritative list from SQLite / backend
    try {
      const initialJobs = await backendClient.listJobs();
      const activeCount = initialJobs.filter(
        (j) => j.state === "downloading" || j.state === "resolving" || j.state === "queued"
      ).length;

      set({ jobs: initialJobs, activeDownloadsCount: activeCount, isLoading: false, error: null });
    } catch (error) {
      const strings = getStrings(useAppStore.getState().languageSetting);
      set({ isLoading: false, error: errorMessage(error, strings.errors.jobsLoadFallback) });
    }

    return () => {
      unlistenJobUpdated();
      unlistenNotice();
    };
  },

  refreshJobs: async () => {
    try {
      const jobs = await backendClient.listJobs();
      const activeCount = jobs.filter(
        (j) => j.state === "downloading" || j.state === "resolving" || j.state === "queued"
      ).length;
      set({ jobs, activeDownloadsCount: activeCount });
    } catch (error) {
      const strings = getStrings(useAppStore.getState().languageSetting);
      set({ error: errorMessage(error, strings.errors.jobsRefreshFallback) });
    }
  },

  pauseJob: async (jobId: string) => {
    await backendClient.pauseJob(jobId);
    await get().refreshJobs();
  },

  resumeJob: async (jobId: string) => {
    await backendClient.resumeJob(jobId);
    await get().refreshJobs();
  },

  cancelJob: async (jobId: string) => {
    await backendClient.cancelJob(jobId);
    await get().refreshJobs();
  },

  retryJob: async (jobId: string) => {
    await backendClient.retryJob(jobId);
    await get().refreshJobs();
  },

  removeJob: async (jobId: string) => {
    await backendClient.removeJob(jobId);
    await get().refreshJobs();
  },

  pauseAllJobs: async () => {
    const activeJobs = get().jobs.filter(
      (j) => j.state === "downloading" || j.state === "resolving" || j.state === "queued"
    );
    const results = await Promise.allSettled(activeJobs.map((j) => backendClient.pauseJob(j.id)));
    const rejected = results.filter((r) => r.status === "rejected");
    if (rejected.length > 0) {
      const strings = getStrings(useAppStore.getState().languageSetting);
      useToastStore.getState().showToast(strings.toast.pauseAllError(rejected.length), "error");
    }
    await get().refreshJobs();
  },

  resumeAllJobs: async () => {
    const pausedJobs = get().jobs.filter((j) => j.state === "paused" || j.state === "interrupted");
    const results = await Promise.allSettled(pausedJobs.map((j) => backendClient.resumeJob(j.id)));
    const rejected = results.filter((r) => r.status === "rejected");
    if (rejected.length > 0) {
      const strings = getStrings(useAppStore.getState().languageSetting);
      useToastStore.getState().showToast(strings.toast.resumeAllError(rejected.length), "error");
    }
    await get().refreshJobs();
  },
}));
