import type { LanguagePreference } from "./LanguagePreference";
import type { QualityPreference } from "./QualityPreference";
import type { ThemePreference } from "./ThemePreference";

export type AppSettingsPatch = { downloadDirectory: string | null, concurrency: number | null, preferredQuality: QualityPreference | null, downloadArtwork: boolean | null, skipVerifiedFiles: boolean | null, loggingEnabled: boolean | null, theme: ThemePreference | null, language: LanguagePreference | null, };
