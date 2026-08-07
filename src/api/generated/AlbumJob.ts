import type { AlbumSummary } from "./AlbumSummary";
import type { AppError } from "./AppError";
import type { JobState } from "./JobState";
import type { QualityPreference } from "./QualityPreference";

export type AlbumJob = { id: string, album: AlbumSummary, quality: QualityPreference, state: JobState, selectedTrackIds: Array<string>, selectedTrackTitles: Array<string>, totalTracks: number, completedTracks: number, bytesReceived: number, bytesTotal: number | null, currentTrackTitle: string | null, error: AppError | null, sequence: number, createdAt: string, updatedAt: string, };
