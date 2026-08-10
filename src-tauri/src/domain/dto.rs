use serde::{Deserialize, Serialize};

use super::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    Album,
    Playlist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QualityPreference {
    BestAvailable,
    Mp3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobState {
    Queued,
    Resolving,
    Waiting,
    Downloading,
    Paused,
    Interrupted,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LanguagePreference {
    ZhCn,
    En,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSummary {
    pub source_url: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub year: Option<String>,
    pub platforms: Vec<String>,
    pub album_type: Option<String>,
    pub source_kind: SourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSummary {
    pub id: String,
    pub disc_number: Option<u32>,
    pub index: Option<u32>,
    pub title: String,
    pub duration_seconds: Option<u32>,
    pub track_page_url: String,
    pub available_formats: Vec<String>,
    pub available_variants: Vec<VariantSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumDetails {
    pub summary: AlbumSummary,
    pub publisher: Option<String>,
    pub developer: Option<String>,
    pub artwork_urls: Vec<String>,
    pub tracks: Vec<TrackSummary>,
    pub available_formats: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantSummary {
    pub format: String,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueAlbumRequest {
    pub source_url: String,
    pub quality: QualityPreference,
    pub download_artwork: bool,
    pub selected_track_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchEnqueueRequest {
    pub source_urls: Vec<String>,
    pub quality: QualityPreference,
    pub download_artwork: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumJob {
    pub id: String,
    pub album: AlbumSummary,
    pub quality: QualityPreference,
    pub selected_formats: Vec<String>,
    pub state: JobState,
    pub selected_track_ids: Vec<String>,
    pub selected_track_titles: Vec<String>,
    pub total_tracks: u32,
    pub completed_tracks: u32,
    pub bytes_received: u64,
    pub bytes_total: Option<u64>,
    pub current_track_title: Option<String>,
    pub error: Option<AppError>,
    pub sequence: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackPreview {
    pub media_url: String,
    pub format: String,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueFailure {
    pub source_url: String,
    pub error: AppError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchEnqueueResult {
    pub accepted: Vec<AlbumJob>,
    pub rejected: Vec<EnqueueFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub download_directory: String,
    pub concurrency: u8,
    pub preferred_quality: QualityPreference,
    pub download_artwork: bool,
    pub skip_verified_files: bool,
    pub logging_enabled: bool,
    pub theme: ThemePreference,
    pub language: LanguagePreference,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            download_directory: String::new(),
            concurrency: 4,
            preferred_quality: QualityPreference::BestAvailable,
            download_artwork: false,
            skip_verified_files: true,
            logging_enabled: true,
            theme: ThemePreference::System,
            language: LanguagePreference::ZhCn,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsPatch {
    pub download_directory: Option<String>,
    pub concurrency: Option<u8>,
    pub preferred_quality: Option<QualityPreference>,
    pub download_artwork: Option<bool>,
    pub skip_verified_files: Option<bool>,
    pub logging_enabled: Option<bool>,
    pub theme: Option<ThemePreference>,
    pub language: Option<LanguagePreference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackProgressEvent {
    pub job_id: String,
    pub track_id: String,
    pub bytes_received: u64,
    pub bytes_total: Option<u64>,
    pub bytes_per_second: u64,
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendNotice {
    pub code: String,
    pub message: String,
    pub job_id: Option<String>,
}
