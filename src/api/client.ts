import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  check as checkForTauriUpdate,
  type DownloadEvent,
  type Update,
} from "@tauri-apps/plugin-updater";

import type {
  AlbumDetails,
  AlbumJob,
  AlbumSummary,
  AppSettings,
  AppSettingsPatch,
  BackendNotice,
  BatchEnqueueRequest,
  BatchEnqueueResult,
  EnqueueAlbumRequest,
  TrackProgressEvent,
  TrackPreview,
  UpdateEnvironment,
} from "./generated";

export interface AppUpdateInfo {
  currentVersion: string;
  version: string;
  date: string | null;
  body: string | null;
  environment: UpdateEnvironment;
}

export interface UpdateDownloadProgress {
  downloadedBytes: number;
  totalBytes: number | null;
}

export interface BackendClient {
  searchAlbums(query: string): Promise<AlbumSummary[]>;
  getAlbumDetails(sourceUrl: string): Promise<AlbumDetails>;
  getTrackPreview(trackPageUrl: string): Promise<TrackPreview>;
  enqueueAlbum(request: EnqueueAlbumRequest): Promise<AlbumJob>;
  enqueueUrls(request: BatchEnqueueRequest): Promise<BatchEnqueueResult>;
  listJobs(): Promise<AlbumJob[]>;
  pauseJob(jobId: string): Promise<void>;
  resumeJob(jobId: string): Promise<void>;
  cancelJob(jobId: string): Promise<void>;
  retryJob(jobId: string): Promise<void>;
  removeJob(jobId: string): Promise<void>;
  getSettings(): Promise<AppSettings>;
  getAppVersion(): Promise<string>;
  checkForUpdate(): Promise<AppUpdateInfo | null>;
  downloadUpdate(onProgress: (progress: UpdateDownloadProgress) => void): Promise<void>;
  installUpdate(): Promise<void>;
  updateSettings(patch: AppSettingsPatch): Promise<AppSettings>;
  chooseDownloadDirectory(): Promise<string | null>;
  openDownloadDirectory(): Promise<void>;
  openLogDirectory(): Promise<void>;
  openExternalUrl(url: string): Promise<void>;
  onJobUpdated(handler: (job: AlbumJob) => void): Promise<UnlistenFn>;
  onTrackProgress(handler: (event: TrackProgressEvent) => void): Promise<UnlistenFn>;
  onNotice(handler: (notice: BackendNotice) => void): Promise<UnlistenFn>;
}

class TauriBackendClient implements BackendClient {
  private pendingUpdate: Update | null = null;

  searchAlbums(query: string) {
    return invoke<AlbumSummary[]>("search_albums", { query });
  }

  getAlbumDetails(sourceUrl: string) {
    return invoke<AlbumDetails>("get_album_details", { sourceUrl });
  }

  getTrackPreview(trackPageUrl: string) {
    return invoke<TrackPreview>("get_track_preview", { trackPageUrl });
  }

  enqueueAlbum(request: EnqueueAlbumRequest) {
    return invoke<AlbumJob>("enqueue_album", { request });
  }

  enqueueUrls(request: BatchEnqueueRequest) {
    return invoke<BatchEnqueueResult>("enqueue_urls", { request });
  }

  listJobs() { return invoke<AlbumJob[]>("list_jobs"); }
  pauseJob(jobId: string) { return invoke<void>("pause_job", { jobId }); }
  resumeJob(jobId: string) { return invoke<void>("resume_job", { jobId }); }
  cancelJob(jobId: string) { return invoke<void>("cancel_job", { jobId }); }
  retryJob(jobId: string) { return invoke<void>("retry_job", { jobId }); }
  removeJob(jobId: string) { return invoke<void>("remove_job", { jobId }); }
  getSettings() { return invoke<AppSettings>("get_settings"); }
  getAppVersion() { return getVersion(); }
  async checkForUpdate(): Promise<AppUpdateInfo | null> {
    await this.pendingUpdate?.close();
    this.pendingUpdate = null;

    const environment = await invoke<UpdateEnvironment>("get_update_environment");
    const update = await checkForTauriUpdate({ timeout: 30_000 });
    if (!update) return null;

    const info: AppUpdateInfo = {
      currentVersion: update.currentVersion,
      version: update.version,
      date: update.date ?? null,
      body: update.body ?? null,
      environment,
    };

    if (environment.installStrategy === "integrated") {
      this.pendingUpdate = update;
    } else {
      await update.close();
    }

    return info;
  }
  async downloadUpdate(onProgress: (progress: UpdateDownloadProgress) => void) {
    if (!this.pendingUpdate) {
      throw new Error("No integrated update is ready to download");
    }

    let downloadedBytes = 0;
    let totalBytes: number | null = null;
    const handleEvent = (event: DownloadEvent) => {
      if (event.event === "Started") {
        totalBytes = event.data.contentLength ?? null;
        onProgress({ downloadedBytes, totalBytes });
      } else if (event.event === "Progress") {
        downloadedBytes += event.data.chunkLength;
        onProgress({ downloadedBytes, totalBytes });
      } else {
        onProgress({ downloadedBytes: totalBytes ?? downloadedBytes, totalBytes });
      }
    };

    await this.pendingUpdate.download(handleEvent, { timeout: 10 * 60_000 });
  }
  async installUpdate() {
    if (!this.pendingUpdate) {
      throw new Error("No downloaded update is ready to install");
    }
    await this.pendingUpdate.install();
    this.pendingUpdate = null;
    await relaunch();
  }
  updateSettings(patch: AppSettingsPatch) { return invoke<AppSettings>("update_settings", { patch }); }
  chooseDownloadDirectory() { return invoke<string | null>("choose_download_directory"); }
  openDownloadDirectory() { return invoke<void>("open_download_directory"); }
  openLogDirectory() { return invoke<void>("open_log_directory"); }
  openExternalUrl(url: string) { return invoke<void>("open_external_url", { url }); }
  onJobUpdated(handler: (job: AlbumJob) => void) { return listen<AlbumJob>("khinfetch:job-updated", (event) => handler(event.payload)); }
  onTrackProgress(handler: (event: TrackProgressEvent) => void) { return listen<TrackProgressEvent>("khinfetch:track-progress", (event) => handler(event.payload)); }
  onNotice(handler: (notice: BackendNotice) => void) { return listen<BackendNotice>("khinfetch:notice", (event) => handler(event.payload)); }
}

export const backendClient: BackendClient = new TauriBackendClient();
