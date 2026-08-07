import type { AlbumSummary } from "./AlbumSummary";
import type { TrackSummary } from "./TrackSummary";

export type AlbumDetails = { summary: AlbumSummary, publisher: string | null, developer: string | null, artworkUrls: Array<string>, tracks: Array<TrackSummary>, availableFormats: Array<string>, };
