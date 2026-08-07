import type { LanguagePreference } from "./LanguagePreference";
import type { QualityPreference } from "./QualityPreference";
import type { ThemePreference } from "./ThemePreference";

export type AppSettings = { downloadDirectory: string, concurrency: number, preferredQuality: QualityPreference, downloadArtwork: boolean, skipVerifiedFiles: boolean, loggingEnabled: boolean, theme: ThemePreference, language: LanguagePreference, };
