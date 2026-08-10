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
  removeAllJobs: () => Promise<void>;
}

let jobsEventRevision = 0;

const isActiveJob = (job: AlbumJob) =>
  job.state === "downloading" || job.state === "resolving" || job.state === "waiting" || job.state === "queued";

const activeDownloadsCount = (jobs: AlbumJob[]) => jobs.filter(isActiveJob).length;

const mergeJobsBySequence = (snapshot: AlbumJob[], liveJobs: AlbumJob[]) => {
  const snapshotIds = new Set(snapshot.map((job) => job.id));
  const liveById = new Map(liveJobs.map((job) => [job.id, job]));
  const liveOnly = liveJobs.filter((job) => !snapshotIds.has(job.id));
  return [
    ...liveOnly,
    ...snapshot.map((job) => {
      const live = liveById.get(job.id);
      return live && live.sequence > job.sequence ? live : job;
    }),
  ];
};

const showActionError = (error: unknown) => {
  const strings = getStrings(useAppStore.getState().languageSetting);
  useToastStore.getState().showToast(errorMessage(error, strings.errors.unknown), "error");
};

export const useJobsStore = create<JobsState>((set, get) => ({
  jobs: [],
  isLoading: false,
  error: null,
  activeDownloadsCount: 0,

  initialize: async () => {
    set({ isLoading: true });

    // Register listeners first before listJobs() to prevent race conditions
    const unlistenJobUpdated = await backendClient.onJobUpdated((updatedJob) => {
      jobsEventRevision += 1;
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

        return { jobs: updatedList, activeDownloadsCount: activeDownloadsCount(updatedList) };
      });
    });

    const unlistenNotice = await backendClient.onNotice((notice: BackendNotice) => {
      useToastStore.getState().showToast(notice.message, "warning");
    });

    // Load authoritative list from SQLite / backend
    try {
      const snapshotRevision = jobsEventRevision;
      const initialJobs = await backendClient.listJobs();
      set((state) => {
        const jobs = jobsEventRevision === snapshotRevision
          ? initialJobs
          : mergeJobsBySequence(initialJobs, state.jobs);
        return {
          jobs,
          activeDownloadsCount: activeDownloadsCount(jobs),
          isLoading: false,
          error: null,
        };
      });
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
      const snapshotRevision = jobsEventRevision;
      const jobs = await backendClient.listJobs();
      set((state) => {
        const merged = jobsEventRevision === snapshotRevision
          ? jobs
          : mergeJobsBySequence(jobs, state.jobs);
        return { jobs: merged, activeDownloadsCount: activeDownloadsCount(merged), error: null };
      });
    } catch (error) {
      const strings = getStrings(useAppStore.getState().languageSetting);
      set({ error: errorMessage(error, strings.errors.jobsRefreshFallback) });
    }
  },

  pauseJob: async (jobId: string) => {
    try {
      await backendClient.pauseJob(jobId);
      await get().refreshJobs();
    } catch (error) {
      showActionError(error);
    }
  },

  resumeJob: async (jobId: string) => {
    try {
      await backendClient.resumeJob(jobId);
      await get().refreshJobs();
    } catch (error) {
      showActionError(error);
    }
  },

  cancelJob: async (jobId: string) => {
    try {
      await backendClient.cancelJob(jobId);
      await get().refreshJobs();
    } catch (error) {
      showActionError(error);
    }
  },

  retryJob: async (jobId: string) => {
    try {
      await backendClient.retryJob(jobId);
      await get().refreshJobs();
    } catch (error) {
      showActionError(error);
    }
  },

  removeJob: async (jobId: string) => {
    try {
      await backendClient.removeJob(jobId);
      set((state) => {
        const jobs = state.jobs.filter((job) => job.id !== jobId);
        return { jobs, activeDownloadsCount: activeDownloadsCount(jobs) };
      });
    } catch (error) {
      showActionError(error);
    }
  },

  pauseAllJobs: async () => {
    const activeJobs = get().jobs.filter(isActiveJob);
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

  removeAllJobs: async () => {
    const initialJobs = get().jobs;
    if (initialJobs.length === 0) return;

    const targetIds = new Set(initialJobs.map((job) => job.id));
    const activeJobs = initialJobs.filter(isActiveJob);
    await Promise.allSettled(activeJobs.map((job) => backendClient.cancelJob(job.id)));
    await get().refreshJobs();

    const removableJobs = get().jobs.filter((job) => targetIds.has(job.id) && !isActiveJob(job));
    await Promise.allSettled(removableJobs.map((job) => backendClient.removeJob(job.id)));
    await get().refreshJobs();

    const remaining = get().jobs.filter((job) => targetIds.has(job.id)).length;
    const strings = getStrings(useAppStore.getState().languageSetting);
    if (remaining > 0) {
      useToastStore.getState().showToast(strings.toast.clearQueueError(remaining), "error");
    } else {
      useToastStore.getState().showToast(strings.toast.clearQueueSuccess(initialJobs.length), "success");
    }
  },
}));
