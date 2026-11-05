import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AlbumJob } from "../api/generated";
import { deferred } from "../test/deferred";
import type { useJobsStore } from "./useJobsStore";

const backend = vi.hoisted(() => ({
  listJobs: vi.fn(), removeJob: vi.fn(), cancelJob: vi.fn(),
  onJobUpdated: vi.fn(), onNotice: vi.fn(),
}));
vi.mock("../api/client", () => ({ backendClient: backend }));
vi.mock("./useAppStore", () => ({
  useAppStore: { getState: () => ({ languageSetting: "zh-cn" }) },
}));
vi.mock("./useToastStore", () => ({
  useToastStore: { getState: () => ({ showToast: vi.fn() }) },
}));

function job(id = "A", sequence = 1, state: AlbumJob["state"] = "paused"): AlbumJob {
  return {
    id, sequence, state,
    album: {
      sourceUrl: `https://downloads.khinsider.com/game-soundtracks/album/${id}`,
      title: id, coverUrl: null, year: null, platforms: [], albumType: null, sourceKind: "album",
    },
    quality: "mp3", selectedFormats: ["MP3"], selectedTrackIds: [], selectedTrackTitles: [],
    totalTracks: 1, completedTracks: state === "completed" ? 1 : 0,
    bytesReceived: 0, bytesTotal: null, currentTrackTitle: null, error: null,
    createdAt: "0", updatedAt: "0",
  };
}

let store: typeof useJobsStore;
let emitJob: (job: AlbumJob) => void;
beforeEach(async () => {
  vi.resetModules();
  vi.resetAllMocks();
  backend.listJobs.mockResolvedValue([]);
  backend.removeJob.mockResolvedValue(undefined);
  backend.cancelJob.mockResolvedValue(undefined);
  backend.onJobUpdated.mockImplementation(async (handler: typeof emitJob) => {
    emitJob = handler;
    return () => {};
  });
  backend.onNotice.mockResolvedValue(() => {});
  store = (await import("./useJobsStore")).useJobsStore;
});

describe("queue snapshot ordering", () => {
  it("ignores an older refresh that finishes after a newer refresh", async () => {
    const oldSnapshot = deferred<AlbumJob[]>();
    backend.listJobs.mockReturnValueOnce(oldSnapshot.promise).mockResolvedValueOnce([job("A", 3, "completed")]);
    const oldRefresh = store.getState().refreshJobs();
    await store.getState().refreshJobs();
    oldSnapshot.resolve([job("A", 2, "downloading")]);
    await oldRefresh;
    expect(store.getState().jobs).toEqual([job("A", 3, "completed")]);
    expect(store.getState().activeDownloadsCount).toBe(0);
  });

  it("ignores obsolete refresh errors", async () => {
    const oldSnapshot = deferred<AlbumJob[]>();
    backend.listJobs.mockReturnValueOnce(oldSnapshot.promise).mockResolvedValueOnce([job()]);
    const oldRefresh = store.getState().refreshJobs();
    await store.getState().refreshJobs();
    oldSnapshot.reject(new Error("Obsolete error"));
    await oldRefresh;
    expect(store.getState().error).toBeNull();
  });

  it("does not let initialization replace a newer refresh", async () => {
    const oldSnapshot = deferred<AlbumJob[]>();
    backend.listJobs.mockReturnValueOnce(oldSnapshot.promise);
    const initializing = store.getState().initialize();
    await vi.waitFor(() => expect(backend.listJobs).toHaveBeenCalledTimes(1));
    backend.listJobs.mockResolvedValueOnce([job("A", 3, "completed")]);
    await store.getState().refreshJobs();
    expect(store.getState().isLoading).toBe(false);
    oldSnapshot.resolve([job("A", 2, "downloading")]);
    const cleanup = await initializing;
    expect(store.getState().jobs).toEqual([job("A", 3, "completed")]);
    cleanup();
  });

  it("retains newer events and jobs added during a snapshot", async () => {
    const cleanup = await store.getState().initialize();
    const pending = deferred<AlbumJob[]>();
    backend.listJobs.mockReturnValueOnce(pending.promise);
    const refreshing = store.getState().refreshJobs();
    emitJob(job("A", 4, "completed"));
    emitJob(job("B", 1, "queued"));
    pending.resolve([job("A", 2, "downloading")]);
    await refreshing;
    expect(store.getState().jobs).toEqual([job("B", 1, "queued"), job("A", 4, "completed")]);
    expect(store.getState().activeDownloadsCount).toBe(1);
    cleanup();
  });

  it("honors a newer job sequence even without events during the query", async () => {
    const cleanup = await store.getState().initialize();
    emitJob(job("A", 4, "completed"));
    backend.listJobs.mockResolvedValueOnce([job("A", 3, "downloading")]);
    await store.getState().refreshJobs();
    expect(store.getState().jobs).toEqual([job("A", 4, "completed")]);
    cleanup();
  });

  it("removes obsolete local entries when the authoritative snapshot is empty", async () => {
    backend.listJobs.mockResolvedValueOnce([job()]);
    await store.getState().refreshJobs();
    await store.getState().refreshJobs();
    expect(store.getState().jobs).toEqual([]);
  });
});

describe("queue deletion", () => {
  it.each([false, true])("does not resurrect a deleted job (other event: %s)", async (otherEvent) => {
    backend.listJobs.mockResolvedValueOnce([job()]);
    const cleanup = await store.getState().initialize();
    const pending = deferred<AlbumJob[]>();
    backend.listJobs.mockReturnValueOnce(pending.promise);
    const refreshing = store.getState().refreshJobs();
    await store.getState().removeJob("A");
    if (otherEvent) emitJob(job("B", 1, "queued"));
    pending.resolve([job()]);
    await refreshing;
    emitJob(job("A", 2, "paused"));
    expect(store.getState().jobs.map((entry) => entry.id)).toEqual(otherEvent ? ["B"] : []);
    cleanup();
  });

  it("keeps a job when deletion fails", async () => {
    backend.listJobs.mockResolvedValueOnce([job()]);
    const cleanup = await store.getState().initialize();
    backend.removeJob.mockRejectedValueOnce(new Error("Delete failed"));
    await store.getState().removeJob("A");
    emitJob(job("A", 2));
    expect(store.getState().jobs).toEqual([job("A", 2)]);
    cleanup();
  });

  it("records successful bulk deletions even when the final refresh fails", async () => {
    backend.listJobs.mockResolvedValueOnce([job("A"), job("B")]);
    const cleanup = await store.getState().initialize();
    backend.listJobs.mockResolvedValueOnce([job("A"), job("B")]).mockRejectedValueOnce(new Error("Refresh failed"));
    backend.removeJob.mockResolvedValueOnce(undefined).mockRejectedValueOnce(new Error("B cannot be deleted"));
    await store.getState().removeAllJobs();
    emitJob(job("A", 2));
    expect(store.getState().jobs).toEqual([job("B")]);
    expect(backend.removeJob.mock.calls.map(([id]) => id)).toEqual(["A", "B"]);
    cleanup();
  });
});
