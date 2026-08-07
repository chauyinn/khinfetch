import type { QualityPreference } from "./QualityPreference";

export type EnqueueAlbumRequest = { sourceUrl: string, quality: QualityPreference, downloadArtwork: boolean, selectedTrackIds: Array<string> | null, };
