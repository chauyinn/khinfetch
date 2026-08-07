use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use futures_util::{stream::FuturesUnordered, StreamExt};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, watch, Mutex, OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{
    domain::{AlbumJob, AppError, AppResult, ErrorCode, JobState, TrackProgressEvent},
    khinsider::KhinsiderClient,
    storage::{timestamp, ArtworkJobRecord, Storage, TrackJobRecord},
};

use super::{
    downloader::{DownloadClient, DownloadProgress, ProgressCallback},
    naming::{artwork_filename, track_filename, unique_target_path},
    state::can_transition,
    variant::select_variant,
};

const EVENT_INTERVAL: Duration = Duration::from_millis(250);
const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(2);
const CHECKPOINT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_CONCURRENCY: u8 = 8;

#[derive(Clone)]
pub struct DownloadManager {
    inner: Arc<DownloadManagerInner>,
}

struct DownloadManagerInner {
    storage: Storage,
    khinsider: KhinsiderClient,
    downloader: DownloadClient,
    limiter: ConcurrencyLimiter,
    active: Mutex<HashMap<String, ActiveControl>>,
}

#[derive(Clone)]
struct ActiveControl {
    cancellation: CancellationToken,
    stop_reason: Arc<AtomicU8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopReason {
    None = 0,
    Pause = 1,
    Cancel = 2,
}

impl StopReason {
    fn load(value: &AtomicU8) -> Self {
        match value.load(Ordering::SeqCst) {
            1 => Self::Pause,
            2 => Self::Cancel,
            _ => Self::None,
        }
    }
}

impl DownloadManager {
    pub fn new(storage: Storage, khinsider: KhinsiderClient, concurrency: u8) -> AppResult<Self> {
        Ok(Self {
            inner: Arc::new(DownloadManagerInner {
                storage,
                khinsider,
                downloader: DownloadClient::new(env!("CARGO_PKG_VERSION"))?,
                limiter: ConcurrencyLimiter::new(concurrency)?,
                active: Mutex::new(HashMap::new()),
            }),
        })
    }

    pub async fn start(&self, app: AppHandle, job_id: &str) -> AppResult<()> {
        let job = self
            .inner
            .storage
            .get_job(job_id)
            .await?
            .ok_or_else(|| storage_error("下载任务不存在"))?;
        if job.state != JobState::Queued {
            return Err(invalid_state("只有排队中的任务可以启动"));
        }

        let control = ActiveControl {
            cancellation: CancellationToken::new(),
            stop_reason: Arc::new(AtomicU8::new(StopReason::None as u8)),
        };
        {
            let mut active = self.inner.active.lock().await;
            if active.contains_key(job_id) {
                return Ok(());
            }
            active.insert(job_id.to_owned(), control.clone());
        }

        let manager = self.clone();
        let job_id = job_id.to_owned();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = manager.run_job(&app, &job_id, &control).await {
                tracing::error!(
                    job_id = %job_id,
                    error = %crate::logging::redact(&error.to_string()),
                    "download job failed"
                );
                if StopReason::load(&control.stop_reason) == StopReason::None {
                    if let Ok(job) = manager
                        .inner
                        .storage
                        .set_job_state(&job_id, JobState::Failed, Some(&error), None)
                        .await
                    {
                        emit_job(&app, &job);
                    }
                }
            }
            manager.inner.active.lock().await.remove(&job_id);
        });
        Ok(())
    }

    pub async fn pause(&self, app: &AppHandle, job_id: &str) -> AppResult<AlbumJob> {
        self.stop(app, job_id, StopReason::Pause).await
    }

    pub async fn cancel(&self, app: &AppHandle, job_id: &str) -> AppResult<AlbumJob> {
        self.stop(app, job_id, StopReason::Cancel).await
    }

    pub async fn resume(&self, app: AppHandle, job_id: &str) -> AppResult<AlbumJob> {
        let job = self
            .inner
            .storage
            .get_job(job_id)
            .await?
            .ok_or_else(|| storage_error("下载任务不存在"))?;
        if !matches!(
            job.state,
            JobState::Paused | JobState::Interrupted | JobState::Failed
        ) {
            return Err(invalid_state("只有暂停、中断或失败的任务可以恢复"));
        }
        let mut tracks = self.inner.storage.list_track_jobs(job_id).await?;
        for track in &mut tracks {
            if track.state != JobState::Completed {
                track.state = JobState::Queued;
                track.error = None;
                self.inner.storage.save_track_job(track).await?;
            }
        }
        let mut artworks = self.inner.storage.list_artwork_jobs(job_id).await?;
        for artwork in &mut artworks {
            if artwork.state != JobState::Completed {
                artwork.state = JobState::Queued;
                artwork.error = None;
                self.inner.storage.save_artwork_job(artwork).await?;
            }
        }
        let job = self
            .inner
            .storage
            .set_job_state(job_id, JobState::Queued, None, None)
            .await?;
        emit_job(&app, &job);
        self.start(app, job_id).await?;
        Ok(job)
    }

    pub async fn remove(&self, job_id: &str) -> AppResult<()> {
        if self.inner.active.lock().await.contains_key(job_id) {
            return Err(invalid_state("请先暂停或取消正在运行的任务"));
        }
        self.inner.storage.remove_job(job_id).await
    }

    pub fn set_concurrency(&self, concurrency: u8) -> AppResult<()> {
        self.inner.limiter.set_limit(concurrency)
    }

    async fn stop(&self, app: &AppHandle, job_id: &str, reason: StopReason) -> AppResult<AlbumJob> {
        let job = self
            .inner
            .storage
            .get_job(job_id)
            .await?
            .ok_or_else(|| storage_error("下载任务不存在"))?;
        let target_state = match reason {
            StopReason::Pause => JobState::Paused,
            StopReason::Cancel => JobState::Canceled,
            StopReason::None => return Err(AppError::internal("无效停止原因")),
        };
        if !can_transition(job.state, target_state) {
            return Err(invalid_state("当前任务状态不支持此操作"));
        }

        if let Some(control) = self.inner.active.lock().await.get(job_id).cloned() {
            control.stop_reason.store(reason as u8, Ordering::SeqCst);
            control.cancellation.cancel();
            return Ok(job);
        }

        self.persist_stop(app, job_id, reason, target_state).await
    }

    async fn run_job(
        &self,
        app: &AppHandle,
        job_id: &str,
        control: &ActiveControl,
    ) -> AppResult<()> {
        let job = self
            .inner
            .storage
            .set_job_state(job_id, JobState::Resolving, None, None)
            .await?;
        emit_job(app, &job);
        let tracks = self.inner.storage.list_track_jobs(job_id).await?;
        let settings = self.inner.storage.get_settings().await?;
        let skip_verified_files = settings.skip_verified_files;
        let mut tasks = FuturesUnordered::new();
        for track in tracks
            .into_iter()
            .filter(|track| track.state != JobState::Completed)
        {
            let manager = self.clone();
            let app = app.clone();
            let cancellation = control.cancellation.clone();
            let quality = job.quality;
            tasks.push(async move {
                manager
                    .process_track(&app, track, quality, skip_verified_files, &cancellation)
                    .await
            });
        }

        let mut first_error = None;
        while let Some(result) = tasks.next().await {
            if let Err(error) = result {
                if error.code != ErrorCode::Canceled && first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }

        let reason = StopReason::load(&control.stop_reason);
        if reason != StopReason::None {
            let target_state = stop_state(reason)?;
            self.persist_stop(app, job_id, reason, target_state).await?;
            return Ok(());
        }

        if let Some(error) = first_error {
            self.inner
                .storage
                .refresh_job_progress(job_id, None)
                .await?;
            return Err(error);
        }

        let artworks = self.inner.storage.list_artwork_jobs(job_id).await?;
        let mut artwork_tasks = FuturesUnordered::new();
        for (index, artwork) in artworks
            .into_iter()
            .enumerate()
            .filter(|(_, artwork)| artwork.state != JobState::Completed)
        {
            let manager = self.clone();
            let app = app.clone();
            let cancellation = control.cancellation.clone();
            artwork_tasks.push(async move {
                manager
                    .process_artwork(&app, index, artwork, skip_verified_files, &cancellation)
                    .await
            });
        }
        while let Some(result) = artwork_tasks.next().await {
            if let Err(error) = result {
                if error.code != ErrorCode::Canceled && first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }

        let reason = StopReason::load(&control.stop_reason);
        if reason != StopReason::None {
            let target_state = stop_state(reason)?;
            self.persist_stop(app, job_id, reason, target_state).await?;
            return Ok(());
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        self.inner
            .storage
            .refresh_job_progress(job_id, None)
            .await?;
        let job = self
            .inner
            .storage
            .set_job_state(job_id, JobState::Completed, None, None)
            .await?;
        emit_job(app, &job);
        Ok(())
    }

    async fn persist_stop(
        &self,
        app: &AppHandle,
        job_id: &str,
        reason: StopReason,
        target_state: JobState,
    ) -> AppResult<AlbumJob> {
        let mut tracks = self.inner.storage.list_track_jobs(job_id).await?;
        for track in &mut tracks {
            if track.state == JobState::Completed {
                continue;
            }
            track.state = target_state;
            track.error = None;
            if reason == StopReason::Cancel {
                remove_part_path(track.part_path.as_deref()).await?;
            }
            self.inner.storage.save_track_job(track).await?;
        }
        let mut artworks = self.inner.storage.list_artwork_jobs(job_id).await?;
        for artwork in &mut artworks {
            if artwork.state == JobState::Completed {
                continue;
            }
            artwork.state = target_state;
            artwork.error = None;
            if reason == StopReason::Cancel {
                remove_part_path(artwork.part_path.as_deref()).await?;
            }
            self.inner.storage.save_artwork_job(artwork).await?;
        }
        let job = self
            .inner
            .storage
            .set_job_state(job_id, target_state, None, None)
            .await?;
        emit_job(app, &job);
        Ok(job)
    }

    async fn process_track(
        &self,
        app: &AppHandle,
        mut track: TrackJobRecord,
        quality: crate::domain::QualityPreference,
        skip_verified_files: bool,
        cancellation: &CancellationToken,
    ) -> AppResult<()> {
        let _permit = self.inner.limiter.acquire(cancellation).await?;
        track.state = JobState::Resolving;
        track.error = None;
        self.inner.storage.save_track_job(&track).await?;
        let job = self
            .inner
            .storage
            .set_job_state(
                &track.job_id,
                JobState::Downloading,
                None,
                Some(&track.track.title),
            )
            .await?;
        emit_job(app, &job);

        let variants = match self
            .inner
            .khinsider
            .resolve_track_variants(&track.track.track_page_url)
            .await
        {
            Ok(variants) => variants,
            Err(error) => {
                track.state = JobState::Failed;
                track.error = Some(error.clone());
                track.retry_count = track.retry_count.saturating_add(1);
                self.inner.storage.save_track_job(&track).await?;
                return Err(error);
            }
        };
        let variant = match select_variant(&variants, quality) {
            Ok(variant) => variant,
            Err(error) => {
                track.state = JobState::Failed;
                track.error = Some(error.clone());
                self.inner.storage.save_track_job(&track).await?;
                return Err(error);
            }
        };
        track.resolved_media_url = Some(variant.media_url.to_string());
        let (target, part, already_complete) = prepare_paths(
            &track,
            &variant.media_url,
            skip_verified_files,
            &self.inner.downloader,
        )
        .await?;
        track.target_path = Some(target.to_string_lossy().into_owned());
        track.part_path = Some(part.to_string_lossy().into_owned());
        if already_complete {
            track.bytes_received = tokio::fs::metadata(&target)
                .await
                .map_err(file_error)?
                .len();
            track.bytes_total = Some(track.bytes_received);
            track.state = JobState::Completed;
            self.inner.storage.save_track_job(&track).await?;
            let job = self
                .inner
                .storage
                .refresh_job_progress(&track.job_id, None)
                .await?;
            emit_job(app, &job);
            return Ok(());
        }
        track.state = JobState::Downloading;
        track.updated_at = timestamp();
        self.inner.storage.save_track_job(&track).await?;

        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<DownloadProgress>();
        let callback: ProgressCallback = Arc::new(move |progress| {
            let _ = progress_tx.send(progress);
        });
        let sequence = AtomicU64::new(0);
        let media_url = variant.media_url;
        let download =
            self.inner
                .downloader
                .download(&media_url, &target, &part, cancellation, callback);
        tokio::pin!(download);
        let mut last_event = Instant::now() - EVENT_INTERVAL;
        let mut last_checkpoint = Instant::now();
        let mut checkpoint_bytes = track.bytes_received;
        let mut speed_at = Instant::now();
        let mut speed_bytes = track.bytes_received;

        let outcome = loop {
            tokio::select! {
                result = &mut download => break result,
                Some(progress) = progress_rx.recv() => {
                    track.bytes_received = progress.bytes_received;
                    track.bytes_total = progress.bytes_total;
                    let now = Instant::now();
                    if now.duration_since(last_event) >= EVENT_INTERVAL
                        || progress.bytes_total == Some(progress.bytes_received)
                    {
                        let elapsed = now.duration_since(speed_at).as_secs_f64();
                        let speed = if elapsed > 0.0 {
                            ((progress.bytes_received.saturating_sub(speed_bytes)) as f64 / elapsed) as u64
                        } else {
                            0
                        };
                        emit_progress(app, TrackProgressEvent {
                            job_id: track.job_id.clone(),
                            track_id: track.id.clone(),
                            bytes_received: progress.bytes_received,
                            bytes_total: progress.bytes_total,
                            bytes_per_second: speed,
                            sequence: sequence.fetch_add(1, Ordering::Relaxed) + 1,
                        });
                        last_event = now;
                        speed_at = now;
                        speed_bytes = progress.bytes_received;
                    }
                    if now.duration_since(last_checkpoint) >= CHECKPOINT_INTERVAL
                        || progress.bytes_received.saturating_sub(checkpoint_bytes) >= CHECKPOINT_BYTES
                    {
                        self.inner.storage.save_track_job(&track).await?;
                        let job = self
                            .inner
                            .storage
                            .refresh_job_progress(&track.job_id, Some(&track.track.title))
                            .await?;
                        emit_job(app, &job);
                        last_checkpoint = now;
                        checkpoint_bytes = progress.bytes_received;
                    }
                }
            }
        };

        match outcome {
            Ok(outcome) => {
                track.bytes_received = outcome.bytes_received;
                track.bytes_total = outcome.bytes_total;
                track.state = JobState::Completed;
                track.error = None;
                self.inner.storage.save_track_job(&track).await?;
                let job = self
                    .inner
                    .storage
                    .refresh_job_progress(&track.job_id, None)
                    .await?;
                emit_job(app, &job);
                Ok(())
            }
            Err(error) if error.code == ErrorCode::Canceled => Err(error),
            Err(error) => {
                track.state = JobState::Failed;
                track.error = Some(error.clone());
                track.retry_count = track.retry_count.saturating_add(1);
                self.inner.storage.save_track_job(&track).await?;
                Err(error)
            }
        }
    }

    async fn process_artwork(
        &self,
        app: &AppHandle,
        index: usize,
        mut artwork: ArtworkJobRecord,
        skip_verified_files: bool,
        cancellation: &CancellationToken,
    ) -> AppResult<()> {
        let _permit = self.inner.limiter.acquire(cancellation).await?;
        let artwork_url =
            Url::parse(&artwork.source_url).map_err(|_| AppError::invalid_url("原图链接无效"))?;
        artwork.state = JobState::Resolving;
        artwork.error = None;
        self.inner.storage.save_artwork_job(&artwork).await?;
        let (target, part) = prepare_artwork_paths(&artwork, index, &artwork_url).await?;
        artwork.target_path = Some(target.to_string_lossy().into_owned());
        artwork.part_path = Some(part.to_string_lossy().into_owned());
        artwork.state = JobState::Downloading;
        artwork.updated_at = timestamp();
        self.inner.storage.save_artwork_job(&artwork).await?;
        let job = self
            .inner
            .storage
            .set_job_state(
                &artwork.job_id,
                JobState::Downloading,
                None,
                Some("专辑原图"),
            )
            .await?;
        emit_job(app, &job);

        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<DownloadProgress>();
        let callback: ProgressCallback = Arc::new(move |progress| {
            let _ = progress_tx.send(progress);
        });
        let download = self.inner.downloader.download_artwork_with_skip(
            &artwork_url,
            &target,
            &part,
            cancellation,
            skip_verified_files,
            callback,
        );
        tokio::pin!(download);
        let mut last_checkpoint = Instant::now();
        let mut checkpoint_bytes = artwork.bytes_received;

        let outcome = loop {
            tokio::select! {
                result = &mut download => break result,
                Some(progress) = progress_rx.recv() => {
                    artwork.bytes_received = progress.bytes_received;
                    artwork.bytes_total = progress.bytes_total;
                    let now = Instant::now();
                    if now.duration_since(last_checkpoint) >= CHECKPOINT_INTERVAL
                        || progress.bytes_received.saturating_sub(checkpoint_bytes) >= CHECKPOINT_BYTES
                    {
                        self.inner.storage.save_artwork_job(&artwork).await?;
                        last_checkpoint = now;
                        checkpoint_bytes = progress.bytes_received;
                    }
                }
            }
        };

        match outcome {
            Ok(outcome) => {
                artwork.bytes_received = outcome.bytes_received;
                artwork.bytes_total = outcome.bytes_total;
                artwork.state = JobState::Completed;
                artwork.error = None;
                self.inner.storage.save_artwork_job(&artwork).await
            }
            Err(error) if error.code == ErrorCode::Canceled => Err(error),
            Err(error) => {
                artwork.state = JobState::Failed;
                artwork.error = Some(error.clone());
                artwork.retry_count = artwork.retry_count.saturating_add(1);
                self.inner.storage.save_artwork_job(&artwork).await?;
                Err(error)
            }
        }
    }
}

async fn prepare_paths(
    track: &TrackJobRecord,
    media_url: &Url,
    skip_verified_files: bool,
    downloader: &DownloadClient,
) -> AppResult<(PathBuf, PathBuf, bool)> {
    if let (Some(target), Some(part)) = (&track.target_path, &track.part_path) {
        return Ok((PathBuf::from(target), PathBuf::from(part), false));
    }
    let extension = Path::new(media_url.path())
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::UnsupportedFormat,
                "媒体链接缺少文件扩展名",
                false,
            )
        })?;
    let directory = PathBuf::from(&track.output_directory);
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(file_error)?;
    let requested = directory.join(track_filename(
        track.track.disc_number,
        track.track.index,
        &track.track.title,
        extension,
    ));
    if skip_verified_files
        && tokio::fs::try_exists(&requested)
            .await
            .map_err(file_error)?
    {
        if let Some(remote_size) = downloader.remote_size(media_url).await? {
            let local_size = tokio::fs::metadata(&requested)
                .await
                .map_err(file_error)?
                .len();
            if local_size == remote_size {
                let filename = requested
                    .file_name()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| AppError::internal("无法生成临时文件名"))?;
                return Ok((
                    requested.clone(),
                    requested.with_file_name(format!("{filename}.part")),
                    true,
                ));
            }
        }
    }
    let target = unique_target_path(&requested).await.map_err(file_error)?;
    let filename = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AppError::internal("无法生成临时文件名"))?;
    let part = target.with_file_name(format!("{filename}.part"));
    Ok((target, part, false))
}

async fn prepare_artwork_paths(
    artwork: &ArtworkJobRecord,
    index: usize,
    artwork_url: &Url,
) -> AppResult<(PathBuf, PathBuf)> {
    if let (Some(target), Some(part)) = (&artwork.target_path, &artwork.part_path) {
        return Ok((PathBuf::from(target), PathBuf::from(part)));
    }
    let directory = PathBuf::from(&artwork.output_directory);
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(file_error)?;
    let requested = directory.join(artwork_filename(index, artwork_url));
    let filename = requested
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AppError::internal("无法生成原图临时文件名"))?;
    let part = requested.with_file_name(format!("{filename}.{}.part", artwork.id));
    Ok((requested, part))
}

async fn remove_part_path(path: Option<&str>) -> AppResult<()> {
    let Some(path) = path else {
        return Ok(());
    };
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(file_error(error)),
    }
}

fn stop_state(reason: StopReason) -> AppResult<JobState> {
    match reason {
        StopReason::Pause => Ok(JobState::Paused),
        StopReason::Cancel => Ok(JobState::Canceled),
        StopReason::None => Err(AppError::internal("无效停止原因")),
    }
}

fn emit_job(app: &AppHandle, job: &AlbumJob) {
    if let Err(error) = app.emit("khinfetch:job-updated", job) {
        tracing::warn!(
            error = %crate::logging::redact(&error.to_string()),
            "failed to emit job update"
        );
    }
}

fn emit_progress(app: &AppHandle, progress: TrackProgressEvent) {
    if let Err(error) = app.emit("khinfetch:track-progress", progress) {
        tracing::warn!(
            error = %crate::logging::redact(&error.to_string()),
            "failed to emit track progress"
        );
    }
}

fn invalid_state(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::Internal, message, false)
}

fn storage_error(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::Storage, message, false)
}

fn file_error(error: std::io::Error) -> AppError {
    let code = match error.raw_os_error() {
        Some(28 | 39 | 112) => ErrorCode::DiskFull,
        _ if error.kind() == std::io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
        _ => ErrorCode::Internal,
    };
    AppError::new(code, format!("文件操作失败: {error}"), false)
}

#[derive(Clone)]
struct ConcurrencyLimiter {
    semaphore: Arc<Semaphore>,
    desired: watch::Sender<u8>,
}

impl ConcurrencyLimiter {
    fn new(limit: u8) -> AppResult<Self> {
        if !(1..=MAX_CONCURRENCY).contains(&limit) {
            return Err(AppError::invalid_url("并发数必须在 1 到 8 之间"));
        }
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENCY as usize));
        let mut held = Vec::new();
        for _ in limit..MAX_CONCURRENCY {
            held.push(
                semaphore
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| AppError::internal("无法初始化并发限制器"))?,
            );
        }
        let (desired, receiver) = watch::channel(limit);
        let task_semaphore = semaphore.clone();
        tauri::async_runtime::spawn(adjust_permits(task_semaphore, receiver, held));
        Ok(Self { semaphore, desired })
    }

    fn set_limit(&self, limit: u8) -> AppResult<()> {
        if !(1..=MAX_CONCURRENCY).contains(&limit) {
            return Err(AppError::invalid_url("并发数必须在 1 到 8 之间"));
        }
        self.desired
            .send(limit)
            .map_err(|_| AppError::internal("并发限制器已停止"))
    }

    async fn acquire(&self, cancellation: &CancellationToken) -> AppResult<OwnedSemaphorePermit> {
        tokio::select! {
            _ = cancellation.cancelled() => Err(AppError::new(ErrorCode::Canceled, "下载已取消", false)),
            permit = self.semaphore.clone().acquire_owned() => {
                permit.map_err(|_| AppError::internal("并发限制器已关闭"))
            }
        }
    }
}

async fn adjust_permits(
    semaphore: Arc<Semaphore>,
    mut desired: watch::Receiver<u8>,
    mut held: Vec<OwnedSemaphorePermit>,
) {
    loop {
        let target_held = (MAX_CONCURRENCY - *desired.borrow_and_update()) as usize;
        while held.len() > target_held {
            held.pop();
        }
        while held.len() < target_held {
            tokio::select! {
                changed = desired.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    break;
                }
                permit = semaphore.clone().acquire_owned() => {
                    match permit {
                        Ok(permit) => held.push(permit),
                        Err(_) => return,
                    }
                }
            }
        }
        if desired.changed().await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artwork_record(id: &str, output_directory: &Path) -> ArtworkJobRecord {
        ArtworkJobRecord {
            id: id.to_owned(),
            job_id: id.to_owned(),
            source_url: "https://images.khinsider.com/example/cover.jpg".to_owned(),
            output_directory: output_directory.to_string_lossy().into_owned(),
            target_path: None,
            part_path: None,
            bytes_received: 0,
            bytes_total: None,
            state: JobState::Queued,
            error: None,
            retry_count: 0,
            created_at: "0".to_owned(),
            updated_at: "0".to_owned(),
        }
    }

    #[tokio::test]
    async fn artwork_jobs_share_the_target_but_not_the_part_file() {
        let directory = std::env::temp_dir().join(format!(
            "khinfetch-artwork-path-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .expect("system time after unix epoch")
                .as_nanos()
        ));
        let artwork_url = Url::parse("https://images.khinsider.com/example/cover.jpg")
            .expect("valid artwork URL");

        let (first_target, first_part) =
            prepare_artwork_paths(&artwork_record("first-job", &directory), 0, &artwork_url)
                .await
                .expect("prepare first artwork path");
        let (second_target, second_part) =
            prepare_artwork_paths(&artwork_record("second-job", &directory), 0, &artwork_url)
                .await
                .expect("prepare second artwork path");

        assert_eq!(first_target, second_target);
        assert_ne!(first_part, second_part);

        tokio::fs::remove_dir_all(directory)
            .await
            .expect("remove test artwork directory");
    }
}
