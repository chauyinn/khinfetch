use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

use crate::domain::{
    AlbumDetails, AlbumJob, AlbumSummary, AppError, AppResult, AppSettings, AppSettingsPatch,
    BackendNotice, BatchEnqueueRequest, BatchEnqueueResult, EnqueueAlbumRequest, EnqueueFailure,
    ErrorCode, JobState, QualityPreference, TrackPreview, TrackSummary, UpdateEnvironment,
    UpdateInstallStrategy,
};
use crate::downloads::{
    manager::DownloadManager,
    naming::sanitize_component,
    variant::{format_rank, select_format, select_variant},
};
use crate::khinsider::{validate_source_url, KhinsiderClient};
use crate::logging::{redact, LoggingHandle};
use crate::storage::{timestamp, ArtworkJobRecord, Storage, TrackJobRecord};

const RELEASE_URL: &str = "https://github.com/chauyinn/khinfetch/releases/latest";
#[cfg(target_os = "windows")]
const PORTABLE_MARKER_FILE: &str = "khinfetch-portable.txt";

const ALLOWED_EXTERNAL_URLS: [&str; 3] = [
    "https://downloads.khinsider.com",
    "https://github.com/weespin/KhinsiderDownloader",
    RELEASE_URL,
];

pub struct AppState {
    khinsider: KhinsiderClient,
    pub storage: Storage,
    downloads: DownloadManager,
    logging: Option<LoggingHandle>,
}

impl AppState {
    pub async fn new(database_path: impl AsRef<Path>) -> AppResult<Self> {
        let khinsider = KhinsiderClient::new(env!("CARGO_PKG_VERSION"))?;
        let storage = Storage::open(database_path.as_ref()).await?;
        let settings = storage.get_settings().await?;
        let downloads =
            DownloadManager::new(storage.clone(), khinsider.clone(), settings.concurrency)?;
        Ok(Self {
            khinsider,
            storage,
            downloads,
            logging: None,
        })
    }

    pub fn install_logging(&mut self, logging: LoggingHandle) {
        self.logging = Some(logging);
    }
}

#[tauri::command]
pub async fn search_albums(
    state: State<'_, AppState>,
    query: String,
) -> AppResult<Vec<AlbumSummary>> {
    let query = query.trim();
    let query_length = query.chars().count();
    if !(1..=120).contains(&query_length) {
        return Err(AppError::invalid_url(
            "搜索内容长度必须在 1 到 120 个字符之间",
        ));
    }

    state.khinsider.search(query).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_album_details(
    state: State<'_, AppState>,
    source_url: String,
) -> AppResult<AlbumDetails> {
    let source = validate_source_url(&source_url)?;
    state.khinsider.album_details(&source).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_track_preview(
    state: State<'_, AppState>,
    track_page_url: String,
) -> AppResult<TrackPreview> {
    let variants = state
        .khinsider
        .resolve_track_variants(&track_page_url)
        .await?;
    let selected = select_variant(&variants, QualityPreference::Mp3)?;
    Ok(TrackPreview {
        media_url: selected.media_url.to_string(),
        format: selected.format,
        size_bytes: selected.size_bytes,
    })
}

#[tauri::command]
pub async fn list_jobs(state: State<'_, AppState>) -> AppResult<Vec<AlbumJob>> {
    state.storage.list_jobs().await
}

#[tauri::command]
pub async fn get_settings(app: AppHandle, state: State<'_, AppState>) -> AppResult<AppSettings> {
    effective_settings(&app, state.inner()).await
}

#[tauri::command]
pub fn get_update_environment() -> UpdateEnvironment {
    UpdateEnvironment {
        install_strategy: update_install_strategy(),
        release_url: RELEASE_URL.to_owned(),
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn update_settings(
    state: State<'_, AppState>,
    patch: AppSettingsPatch,
) -> AppResult<AppSettings> {
    let settings = state.storage.update_settings(&patch).await?;
    if patch.concurrency.is_some() {
        state.downloads.set_concurrency(settings.concurrency)?;
    }
    Ok(settings)
}

#[tauri::command]
pub async fn enqueue_album(
    app: AppHandle,
    state: State<'_, AppState>,
    request: EnqueueAlbumRequest,
) -> AppResult<AlbumJob> {
    enqueue_one(&app, state.inner(), request).await
}

#[tauri::command]
pub async fn enqueue_urls(
    app: AppHandle,
    state: State<'_, AppState>,
    request: BatchEnqueueRequest,
) -> AppResult<BatchEnqueueResult> {
    if request.source_urls.is_empty() || request.source_urls.len() > 100 {
        return Err(AppError::invalid_url("每批必须包含 1 到 100 个链接"));
    }
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for source_url in request.source_urls {
        let album_request = EnqueueAlbumRequest {
            source_url: source_url.clone(),
            quality: request.quality,
            download_artwork: request.download_artwork,
            selected_track_ids: None,
        };
        match enqueue_one(&app, state.inner(), album_request).await {
            Ok(job) => accepted.push(job),
            Err(error) => rejected.push(EnqueueFailure { source_url, error }),
        }
    }
    Ok(BatchEnqueueResult { accepted, rejected })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn pause_job(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> AppResult<()> {
    state.downloads.pause(&app, &job_id).await?;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn resume_job(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> AppResult<()> {
    state.downloads.resume(app, &job_id).await?;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cancel_job(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> AppResult<()> {
    state.downloads.cancel(&app, &job_id).await?;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn retry_job(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> AppResult<()> {
    state.downloads.resume(app, &job_id).await?;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn remove_job(state: State<'_, AppState>, job_id: String) -> AppResult<()> {
    state.downloads.remove(&job_id).await
}

#[tauri::command]
pub async fn choose_download_directory(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<String>> {
    let selected = app.dialog().file().blocking_pick_folder();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected.into_path().map_err(|error| {
        AppError::new(
            crate::domain::ErrorCode::PermissionDenied,
            format!("无法读取所选目录: {error}"),
            false,
        )
    })?;
    let path = normalize_directory(path)?;
    let settings = state
        .storage
        .update_settings(&AppSettingsPatch {
            download_directory: Some(path.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    Ok(Some(settings.download_directory))
}

#[tauri::command]
pub async fn open_download_directory(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let settings = effective_settings(&app, state.inner()).await?;
    app.opener()
        .open_path(settings.download_directory, None::<String>)
        .map_err(|error| {
            AppError::new(
                crate::domain::ErrorCode::PermissionDenied,
                format!("无法打开下载目录: {error}"),
                false,
            )
        })
}

#[tauri::command]
pub async fn open_log_directory(app: AppHandle) -> AppResult<()> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| AppError::internal(format!("无法定位应用数据目录: {error}")))?;
    let log_dir = data_dir.join("logs");
    std::fs::create_dir_all(&log_dir).map_err(|error| {
        AppError::new(
            crate::domain::ErrorCode::PermissionDenied,
            format!("无法创建日志目录: {error}"),
            false,
        )
    })?;
    app.opener()
        .open_path(log_dir.to_string_lossy().into_owned(), None::<String>)
        .map_err(|error| {
            AppError::new(
                crate::domain::ErrorCode::PermissionDenied,
                format!("无法打开日志目录: {error}"),
                false,
            )
        })
}

#[tauri::command(rename_all = "camelCase")]
pub fn open_external_url(app: AppHandle, url: String) -> AppResult<()> {
    let url = validate_external_url(&url)?;
    app.opener().open_url(url, None::<String>).map_err(|error| {
        AppError::new(
            ErrorCode::PermissionDenied,
            format!("无法打开外部链接: {error}"),
            false,
        )
    })
}

fn validate_external_url(url: &str) -> AppResult<String> {
    let url = url.trim();
    if ALLOWED_EXTERNAL_URLS.contains(&url) {
        return Ok(url.to_owned());
    }

    validate_source_url(url)
        .map(|source| source.url.to_string())
        .map_err(|_| AppError::invalid_url("不允许打开此外部链接"))
}

#[cfg(target_os = "windows")]
fn update_install_strategy() -> UpdateInstallStrategy {
    let is_portable = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|parent| parent.join(PORTABLE_MARKER_FILE))
        })
        .is_some_and(|marker| marker.is_file());
    if is_portable {
        UpdateInstallStrategy::External
    } else {
        UpdateInstallStrategy::Integrated
    }
}

#[cfg(target_os = "linux")]
fn update_install_strategy() -> UpdateInstallStrategy {
    if std::env::var_os("APPIMAGE").is_some() {
        UpdateInstallStrategy::Integrated
    } else {
        UpdateInstallStrategy::External
    }
}

#[cfg(target_os = "macos")]
fn update_install_strategy() -> UpdateInstallStrategy {
    UpdateInstallStrategy::Integrated
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn update_install_strategy() -> UpdateInstallStrategy {
    UpdateInstallStrategy::External
}

fn normalize_directory(path: PathBuf) -> AppResult<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(AppError::new(
            crate::domain::ErrorCode::PermissionDenied,
            "下载目录不能为空",
            false,
        ));
    }
    if path.to_string_lossy().contains('\0') {
        return Err(AppError::new(
            crate::domain::ErrorCode::PermissionDenied,
            "下载目录包含无效字符",
            false,
        ));
    }
    Ok(path)
}

async fn enqueue_one(
    app: &AppHandle,
    state: &AppState,
    request: EnqueueAlbumRequest,
) -> AppResult<AlbumJob> {
    let source = validate_source_url(&request.source_url)?;
    let settings = effective_settings(app, state).await?;
    let details = state.khinsider.album_details(&source).await?;
    let selected_tracks = select_tracks(&details, request.selected_track_ids.as_deref())?;
    let download_artwork = request.download_artwork || settings.download_artwork;
    let directory_name = album_directory_name(&details.summary.title);
    let output_directory = PathBuf::from(&settings.download_directory).join(directory_name);
    tokio::fs::create_dir_all(&output_directory)
        .await
        .map_err(file_error)?;

    let job_id = Uuid::new_v4().to_string();
    let now = timestamp();
    let selected_track_ids = selected_tracks
        .iter()
        .map(|track| track.id.clone())
        .collect::<Vec<_>>();
    let selected_track_titles = selected_tracks
        .iter()
        .map(|track| track.title.clone())
        .collect::<Vec<_>>();
    let selected_formats = formats_for_tracks(&selected_tracks, request.quality);
    let job = AlbumJob {
        id: job_id.clone(),
        album: details.summary,
        quality: request.quality,
        selected_formats,
        state: JobState::Queued,
        selected_track_ids: selected_track_ids.clone(),
        selected_track_titles,
        total_tracks: selected_tracks.len() as u32,
        completed_tracks: 0,
        bytes_received: 0,
        bytes_total: None,
        current_track_title: None,
        error: None,
        sequence: 1,
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    let tracks = selected_tracks
        .into_iter()
        .map(|track| TrackJobRecord {
            id: track_job_id(&job_id, &track.id),
            job_id: job_id.clone(),
            track,
            resolved_media_url: None,
            output_directory: output_directory.to_string_lossy().into_owned(),
            target_path: None,
            part_path: None,
            bytes_received: 0,
            bytes_total: None,
            state: JobState::Queued,
            error: None,
            retry_count: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
        })
        .collect::<Vec<_>>();
    let artworks = if download_artwork {
        details
            .artwork_urls
            .iter()
            .map(|source_url| ArtworkJobRecord {
                id: Uuid::new_v5(
                    &Uuid::NAMESPACE_URL,
                    format!("{job_id}:{source_url}").as_bytes(),
                )
                .to_string(),
                job_id: job_id.clone(),
                source_url: source_url.clone(),
                output_directory: output_directory.to_string_lossy().into_owned(),
                target_path: None,
                part_path: None,
                bytes_received: 0,
                bytes_total: None,
                state: JobState::Queued,
                error: None,
                retry_count: 0,
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    state
        .storage
        .insert_job_with_tracks(&job, &tracks, download_artwork, &artworks)
        .await?;
    if download_artwork && artworks.is_empty() {
        let _ = app.emit(
            "khinfetch:notice",
            BackendNotice {
                code: "artwork_unavailable".to_owned(),
                message: "专辑页面没有提供可下载原图".to_owned(),
                job_id: Some(job_id.clone()),
            },
        );
    }
    if let Err(error) = app.emit("khinfetch:job-updated", &job) {
        tracing::warn!(error = %redact(&error.to_string()), "failed to emit enqueued job");
    }
    state.downloads.start(app.clone(), &job.id).await?;
    Ok(job)
}

fn formats_for_tracks(tracks: &[TrackSummary], quality: QualityPreference) -> Vec<String> {
    let mut selected = Vec::new();
    for track in tracks {
        let format = if track.available_variants.is_empty() {
            select_format(track.available_formats.iter().map(String::as_str), quality)
        } else {
            select_format(
                track
                    .available_variants
                    .iter()
                    .map(|variant| variant.format.as_str()),
                quality,
            )
        };
        let Some(format) = format else {
            continue;
        };
        let format = format.to_ascii_uppercase();
        if !selected.contains(&format) {
            selected.push(format);
        }
    }
    selected.sort_by_key(|format| format_rank(format).unwrap_or(u8::MAX));
    selected
}

fn track_job_id(job_id: &str, source_track_id: &str) -> String {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("{job_id}:{source_track_id}").as_bytes(),
    )
    .to_string()
}

fn album_directory_name(title: &str) -> String {
    sanitize_component(title)
}

async fn effective_settings(app: &AppHandle, state: &AppState) -> AppResult<AppSettings> {
    let settings = state.storage.get_settings().await?;
    if !settings.download_directory.trim().is_empty() {
        return Ok(settings);
    }

    let default_directory = normalize_directory(app.path().download_dir().map_err(|error| {
        AppError::new(
            ErrorCode::PermissionDenied,
            format!("无法定位系统下载目录: {error}"),
            false,
        )
    })?)?;
    tokio::fs::create_dir_all(&default_directory)
        .await
        .map_err(file_error)?;
    state
        .storage
        .update_settings(&AppSettingsPatch {
            download_directory: Some(default_directory.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await
}

fn select_tracks(
    details: &AlbumDetails,
    selected_track_ids: Option<&[String]>,
) -> AppResult<Vec<crate::domain::TrackSummary>> {
    let Some(selected_track_ids) = selected_track_ids else {
        if details.tracks.is_empty() {
            return Err(AppError::new(
                ErrorCode::ParseChanged,
                "专辑没有可下载曲目",
                false,
            ));
        }
        return Ok(details.tracks.clone());
    };
    let requested = selected_track_ids.iter().collect::<HashSet<_>>();
    if requested.is_empty() {
        return Err(AppError::invalid_url("请至少选择一首曲目"));
    }
    let tracks = details
        .tracks
        .iter()
        .filter(|track| requested.contains(&track.id))
        .cloned()
        .collect::<Vec<_>>();
    if tracks.len() != requested.len() {
        return Err(AppError::invalid_url("选中的曲目不存在或已经失效"));
    }
    Ok(tracks)
}

fn file_error(error: std::io::Error) -> AppError {
    let code = match error.raw_os_error() {
        Some(28 | 39 | 112) => ErrorCode::DiskFull,
        _ if error.kind() == std::io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
        _ => ErrorCode::Internal,
    };
    AppError::new(code, format!("文件操作失败: {error}"), false)
}

#[cfg(test)]
mod tests {
    use super::{formats_for_tracks, validate_external_url, RELEASE_URL};
    use crate::domain::{QualityPreference, TrackSummary, VariantSummary};

    fn track_with_formats(formats: &[&str]) -> TrackSummary {
        TrackSummary {
            id: "track".to_owned(),
            disc_number: None,
            index: None,
            title: "Track".to_owned(),
            duration_seconds: None,
            track_page_url: "https://downloads.khinsider.com/game-soundtracks/album/example/track"
                .to_owned(),
            available_formats: formats.iter().map(|format| (*format).to_owned()).collect(),
            available_variants: formats
                .iter()
                .map(|format| VariantSummary {
                    format: (*format).to_owned(),
                    size_bytes: None,
                })
                .collect(),
        }
    }

    #[test]
    fn job_formats_are_unique_and_sorted_by_quality() {
        let tracks = [
            track_with_formats(&["MP3"]),
            track_with_formats(&["MP3", "OGG"]),
            track_with_formats(&["MP3", "FLAC"]),
        ];

        assert_eq!(
            formats_for_tracks(&tracks, QualityPreference::BestAvailable),
            ["FLAC", "OGG", "MP3"]
        );
        assert_eq!(formats_for_tracks(&tracks, QualityPreference::Mp3), ["MP3"]);
    }

    #[test]
    fn external_urls_are_restricted_to_declared_project_links() {
        assert!(validate_external_url("https://downloads.khinsider.com").is_ok());
        assert!(validate_external_url("https://github.com/weespin/KhinsiderDownloader").is_ok());
        assert!(validate_external_url(RELEASE_URL).is_ok());
        assert!(validate_external_url(
            "https://downloads.khinsider.com/game-soundtracks/album/example"
        )
        .is_ok());
        assert!(validate_external_url("https://downloads.khinsider.com/playlist/example").is_ok());
        assert!(validate_external_url(
            "https://downloads.khinsider.com.evil.example/game-soundtracks/album/example"
        )
        .is_err());
        assert!(
            validate_external_url("https://downloads.khinsider.com/search?search=example").is_err()
        );
        assert!(validate_external_url("https://example.com").is_err());
    }
}
