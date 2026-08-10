
import type { VariantSummary } from "./VariantSummary";

export type TrackSummary = { id: string, discNumber: number | null, index: number | null, title: string, durationSeconds: number | null, trackPageUrl: string, availableFormats: Array<string>, availableVariants: Array<VariantSummary>, };
