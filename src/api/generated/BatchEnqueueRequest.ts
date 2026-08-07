import type { QualityPreference } from "./QualityPreference";

export type BatchEnqueueRequest = { sourceUrls: Array<string>, quality: QualityPreference, downloadArtwork: boolean, };
