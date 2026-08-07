import type { SourceKind } from "./SourceKind";

export type AlbumSummary = { sourceUrl: string, title: string, coverUrl: string | null, year: string | null, platforms: Array<string>, albumType: string | null, sourceKind: SourceKind, };
