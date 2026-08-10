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
    naming::{artwork_filename, reserve_unique_target_path, track_filename},
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
    resources: ResourceCoordinator,
    active: Mutex<HashMap<String, ActiveControl>>,
    mutation_lock: Mutex<()>,
}

#[derive(Clone)]
struct ActiveControl {
    cancellation: CancellationToken,
    stop_reason: Arc<AtomicU8>,
    completion: watch::Receiver<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ResourceKey {
    resource_url: String,
    requested_target: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedResource {
    target: PathBuf,
    part: PathBuf,
    owner_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompletedResource {
    target: PathBuf,
    size: u64,
}

#[derive(Default)]
struct ResourceGateState {
    prepared: Option<PreparedResource>,
    completed: Option<CompletedResource>,
}

#[derive(Default)]
struct ResourceGate {
    execution: Mutex<()>,
    state: Mutex<ResourceGateState>,
}

#[derive(Default)]
struct ResourceCoordinator {
    gates: Mutex<HashMap<ResourceKey, Arc<ResourceGate>>>,
}

impl ResourceCoordinator {
    async fn gate_for(&self, key: ResourceKey) -> Arc<ResourceGate> {
        let mut gates = self.gates.lock().await;
        gates
            .entry(key)
            .or_insert_with(|| Arc::new(ResourceGate::default()))
            .clone()
    }

    async fn existing_gate(&self, key: &ResourceKey) -> Option<Arc<ResourceGate>> {
        self.gates.lock().await.get(key).cloned()
    }
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
                resources: ResourceCoordinator::default(),
                active: Mutex::new(HashMap::new()),
                mutation_lock: Mutex::new(()),
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

        let (completion_tx, completion) = watch::channel(false);
        let control = ActiveControl {
            cancellation: CancellationToken::new(),
            stop_reason: Arc::new(AtomicU8::new(StopReason::None as u8)),
            completion,
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
            let _ = completion_tx.send(true);
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
        let _mutation_guard = self.inner.mutation_lock.lock().await;
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
        let _mutation_guard = self.inner.mutation_lock.lock().await;
        if self.inner.active.lock().await.contains_key(job_id) {
            return Err(invalid_state("请先暂停或取消正在运行的任务"));
        }
        let tracks = self.inner.storage.list_track_jobs(job_id).await?;
        let artworks = self.inner.storage.list_artwork_jobs(job_id).await?;
        for track in tracks {
            self.remove_track_part(&track).await?;
        }
        for (index, artwork) in artworks.into_iter().enumerate() {
            self.remove_artwork_part(index, &artwork).await?;
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

        let control = { self.inner.active.lock().await.get(job_id).cloned() };
        if let Some(control) = control {
            match reason {
                StopReason::Cancel => control
                    .stop_reason
                    .store(StopReason::Cancel as u8, Ordering::SeqCst),
                StopReason::Pause => {
                    let _ = control.stop_reason.compare_exchange(
                        StopReason::None as u8,
                        StopReason::Pause as u8,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    );
                }
                StopReason::None => return Err(AppError::internal("无效停止原因")),
            }
            control.cancellation.cancel();
            let mut completion = control.completion.clone();
            if !*completion.borrow() {
                completion
                    .changed()
                    .await
                    .map_err(|_| AppError::internal("下载任务停止前后台执行器意外退出"))?;
            }
            let stopped = self
                .inner
                .storage
                .get_job(job_id)
                .await?
                .ok_or_else(|| storage_error("下载任务不存在"))?;
            if stopped.state != target_state {
                return Err(invalid_state("下载任务未能完成请求的状态变更"));
            }
            return Ok(stopped);
        }

        let current = self
            .inner
            .storage
            .get_job(job_id)
            .await?
            .ok_or_else(|| storage_error("下载任务不存在"))?;
        if !can_transition(current.state, target_state) {
            return Err(invalid_state("当前任务状态不支持此操作"));
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
                self.remove_track_part(track).await?;
                track.part_path = None;
            }
            self.inner.storage.save_track_job(track).await?;
        }
        let mut artworks = self.inner.storage.list_artwork_jobs(job_id).await?;
        for (index, artwork) in artworks.iter_mut().enumerate() {
            if artwork.state == JobState::Completed {
                continue;
            }
            artwork.state = target_state;
            artwork.error = None;
            if reason == StopReason::Cancel {
                self.remove_artwork_part(index, artwork).await?;
                artwork.part_path = None;
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
        let resolution_permit = self.inner.limiter.acquire(cancellation).await?;
        track.state = JobState::Resolving;
        track.error = None;
        self.inner.storage.save_track_job(&track).await?;
        let job = self
            .inner
            .storage
            .refresh_job_runtime_state(&track.job_id, Some(&track.track.title))
            .await?;
        emit_job(app, &job);

        let variants = match tokio::select! {
            _ = cancellation.cancelled() => Err(canceled_error()),
            result = self.inner.khinsider.resolve_track_variants(&track.track.track_page_url) => result,
        } {
            Ok(variants) => variants,
            Err(error) => {
                track.state = JobState::Failed;
                track.error = Some(error.clone());
                track.retry_count = track.retry_count.saturating_add(1);
                self.inner.storage.save_track_job(&track).await?;
                let job = self
                    .inner
                    .storage
                    .refresh_job_runtime_state(&track.job_id, None)
                    .await?;
                emit_job(app, &job);
                return Err(error);
            }
        };
        let variant = match select_variant(&variants, quality) {
            Ok(variant) => variant,
            Err(error) => {
                track.state = JobState::Failed;
                track.error = Some(error.clone());
                self.inner.storage.save_track_job(&track).await?;
                let job = self
                    .inner
                    .storage
                    .refresh_job_runtime_state(&track.job_id, None)
                    .await?;
                emit_job(app, &job);
                return Err(error);
            }
        };
        track.resolved_media_url = Some(variant.media_url.to_string());
        let requested = requested_track_path(&track, &variant.media_url)?;
        let resource_key = ResourceKey {
            resource_url: variant.media_url.to_string(),
            requested_target: requested.clone(),
        };
        drop(resolution_permit);

        let gate = self.inner.resources.gate_for(resource_key).await;
        let _resource_guard = match gate.execution.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                track.state = JobState::Waiting;
                self.inner.storage.save_track_job(&track).await?;
                let job = self
                    .inner
                    .storage
                    .refresh_job_runtime_state(&track.job_id, Some(&track.track.title))
                    .await?;
                emit_job(app, &job);
                tokio::select! {
                    _ = cancellation.cancelled() => return Err(canceled_error()),
                    guard = gate.execution.lock() => guard,
                }
            }
        };

        if let Some(completed) = reusable_completed_resource(&gate).await? {
            return self
                .complete_track(app, &mut track, &completed.target, completed.size)
                .await;
        }

        let _download_permit = self.inner.limiter.acquire(cancellation).await?;
        let prepared = gate.state.lock().await.prepared.clone();
        let (target, part, already_complete) = if let Some(prepared) = prepared {
            (prepared.target, prepared.part, false)
        } else {
            prepare_paths(
                &track,
                &requested,
                &variant.media_url,
                skip_verified_files,
                &self.inner.downloader,
                cancellation,
            )
            .await?
        };
        track.target_path = Some(target.to_string_lossy().into_owned());
        track.part_path = (!already_complete).then(|| part.to_string_lossy().into_owned());
        if already_complete {
            let size = tokio::fs::metadata(&target)
                .await
                .map_err(file_error)?
                .len();
            set_completed_resource(&gate, target.clone(), size).await;
            return self.complete_track(app, &mut track, &target, size).await;
        }
        gate.state.lock().await.prepared = Some(PreparedResource {
            target: target.clone(),
            part: part.clone(),
            owner_id: track.id.clone(),
        });
        track.state = JobState::Downloading;
        track.updated_at = timestamp();
        self.inner.storage.save_track_job(&track).await?;
        let job = self
            .inner
            .storage
            .refresh_job_runtime_state(&track.job_id, Some(&track.track.title))
            .await?;
        emit_job(app, &job);

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
                set_completed_resource(&gate, target.clone(), outcome.bytes_received).await;
                self.inner.storage.save_track_job(&track).await?;
                self.inner
                    .storage
                    .refresh_job_progress(&track.job_id, None)
                    .await?;
                let job = self
                    .inner
                    .storage
                    .refresh_job_runtime_state(&track.job_id, None)
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
                let job = self
                    .inner
                    .storage
                    .refresh_job_runtime_state(&track.job_id, None)
                    .await?;
                emit_job(app, &job);
                Err(error)
            }
        }
    }

    async fn complete_track(
        &self,
        app: &AppHandle,
        track: &mut TrackJobRecord,
        target: &Path,
        size: u64,
    ) -> AppResult<()> {
        track.target_path = Some(target.to_string_lossy().into_owned());
        track.part_path = None;
        track.bytes_received = size;
        track.bytes_total = Some(size);
        track.state = JobState::Completed;
        track.error = None;
        self.inner.storage.save_track_job(track).await?;
        self.inner
            .storage
            .refresh_job_progress(&track.job_id, None)
            .await?;
        let job = self
            .inner
            .storage
            .refresh_job_runtime_state(&track.job_id, None)
            .await?;
        emit_job(app, &job);
        Ok(())
    }

    async fn process_artwork(
        &self,
        app: &AppHandle,
        index: usize,
        mut artwork: ArtworkJobRecord,
        skip_verified_files: bool,
        cancellation: &CancellationToken,
    ) -> AppResult<()> {
        let artwork_url =
            Url::parse(&artwork.source_url).map_err(|_| AppError::invalid_url("原图链接无效"))?;
        artwork.state = JobState::Resolving;
        artwork.error = None;
        self.inner.storage.save_artwork_job(&artwork).await?;
        let job = self
            .inner
            .storage
            .refresh_job_runtime_state(&artwork.job_id, Some("专辑原图"))
            .await?;
        emit_job(app, &job);

        let requested = requested_artwork_path(&artwork, index, &artwork_url);
        let resource_key = ResourceKey {
            resource_url: artwork_url.to_string(),
            requested_target: requested.clone(),
        };
        let gate = self.inner.resources.gate_for(resource_key).await;
        let _resource_guard = match gate.execution.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                artwork.state = JobState::Waiting;
                self.inner.storage.save_artwork_job(&artwork).await?;
                let job = self
                    .inner
                    .storage
                    .refresh_job_runtime_state(&artwork.job_id, Some("专辑原图"))
                    .await?;
                emit_job(app, &job);
                tokio::select! {
                    _ = cancellation.cancelled() => return Err(canceled_error()),
                    guard = gate.execution.lock() => guard,
                }
            }
        };

        if let Some(completed) = reusable_completed_resource(&gate).await? {
            return self
                .complete_artwork(app, &mut artwork, &completed.target, completed.size)
                .await;
        }

        let _download_permit = self.inner.limiter.acquire(cancellation).await?;
        let prepared = gate.state.lock().await.prepared.clone();
        let (target, part, already_complete) = if let Some(prepared) = prepared {
            (prepared.target, prepared.part, false)
        } else {
            prepare_artwork_paths(
                &artwork,
                &requested,
                &artwork_url,
                skip_verified_files,
                &self.inner.downloader,
                cancellation,
            )
            .await?
        };
        artwork.target_path = Some(target.to_string_lossy().into_owned());
        artwork.part_path = (!already_complete).then(|| part.to_string_lossy().into_owned());
        if already_complete {
            let size = tokio::fs::metadata(&target)
                .await
                .map_err(file_error)?
                .len();
            set_completed_resource(&gate, target.clone(), size).await;
            return self
                .complete_artwork(app, &mut artwork, &target, size)
                .await;
        }
        gate.state.lock().await.prepared = Some(PreparedResource {
            target: target.clone(),
            part: part.clone(),
            owner_id: artwork.id.clone(),
        });
        artwork.state = JobState::Downloading;
        artwork.updated_at = timestamp();
        self.inner.storage.save_artwork_job(&artwork).await?;
        let job = self
            .inner
            .storage
            .refresh_job_runtime_state(&artwork.job_id, Some("专辑原图"))
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
            false,
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
                set_completed_resource(&gate, target.clone(), outcome.bytes_received).await;
                self.inner.storage.save_artwork_job(&artwork).await?;
                let job = self
                    .inner
                    .storage
                    .refresh_job_runtime_state(&artwork.job_id, None)
                    .await?;
                emit_job(app, &job);
                Ok(())
            }
            Err(error) if error.code == ErrorCode::Canceled => Err(error),
            Err(error) => {
                artwork.state = JobState::Failed;
                artwork.error = Some(error.clone());
                artwork.retry_count = artwork.retry_count.saturating_add(1);
                self.inner.storage.save_artwork_job(&artwork).await?;
                let job = self
                    .inner
                    .storage
                    .refresh_job_runtime_state(&artwork.job_id, None)
                    .await?;
                emit_job(app, &job);
                Err(error)
            }
        }
    }

    async fn complete_artwork(
        &self,
        app: &AppHandle,
        artwork: &mut ArtworkJobRecord,
        target: &Path,
        size: u64,
    ) -> AppResult<()> {
        artwork.target_path = Some(target.to_string_lossy().into_owned());
        artwork.part_path = None;
        artwork.bytes_received = size;
        artwork.bytes_total = Some(size);
        artwork.state = JobState::Completed;
        artwork.error = None;
        self.inner.storage.save_artwork_job(artwork).await?;
        let job = self
            .inner
            .storage
            .refresh_job_runtime_state(&artwork.job_id, None)
            .await?;
        emit_job(app, &job);
        Ok(())
    }

    async fn remove_track_part(&self, track: &TrackJobRecord) -> AppResult<()> {
        self.remove_owned_resource_part(
            track_resource_key(track),
            &track.id,
            track.part_path.as_deref(),
        )
        .await
    }

    async fn remove_artwork_part(&self, index: usize, artwork: &ArtworkJobRecord) -> AppResult<()> {
        self.remove_owned_resource_part(
            artwork_resource_key(artwork, index),
            &artwork.id,
            artwork.part_path.as_deref(),
        )
        .await
    }

    async fn remove_owned_resource_part(
        &self,
        key: Option<ResourceKey>,
        owner_id: &str,
        part_path: Option<&str>,
    ) -> AppResult<()> {
        let Some(part_path) = part_path else {
            return Ok(());
        };
        let Some(key) = key else {
            return remove_part_path(Some(part_path)).await;
        };
        let Some(gate) = self.inner.resources.existing_gate(&key).await else {
            return remove_part_path(Some(part_path)).await;
        };

        let mut state = gate.state.lock().await;
        let owns_part = state.prepared.as_ref().is_some_and(|prepared| {
            prepared.owner_id == owner_id && prepared.part == Path::new(part_path)
        });
        if owns_part {
            remove_part_path(Some(part_path)).await?;
            state.prepared = None;
        }
        Ok(())
    }
}

fn requested_track_path(track: &TrackJobRecord, media_url: &Url) -> AppResult<PathBuf> {
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
    Ok(PathBuf::from(&track.output_directory).join(track_filename(
        track.track.disc_number,
        track.track.index,
        &track.track.title,
        extension,
    )))
}

fn requested_artwork_path(artwork: &ArtworkJobRecord, index: usize, artwork_url: &Url) -> PathBuf {
    PathBuf::from(&artwork.output_directory).join(artwork_filename(index, artwork_url))
}

fn track_resource_key(track: &TrackJobRecord) -> Option<ResourceKey> {
    let resource_url = track.resolved_media_url.as_deref()?;
    let media_url = Url::parse(resource_url).ok()?;
    Some(ResourceKey {
        resource_url: media_url.to_string(),
        requested_target: requested_track_path(track, &media_url).ok()?,
    })
}

fn artwork_resource_key(artwork: &ArtworkJobRecord, index: usize) -> Option<ResourceKey> {
    let artwork_url = Url::parse(&artwork.source_url).ok()?;
    Some(ResourceKey {
        resource_url: artwork_url.to_string(),
        requested_target: requested_artwork_path(artwork, index, &artwork_url),
    })
}

async fn reusable_completed_resource(gate: &ResourceGate) -> AppResult<Option<CompletedResource>> {
    let completed = gate.state.lock().await.completed.clone();
    let Some(completed) = completed else {
        return Ok(None);
    };
    match tokio::fs::metadata(&completed.target).await {
        Ok(metadata) if metadata.len() == completed.size => Ok(Some(completed)),
        Ok(_) => {
            gate.state.lock().await.completed = None;
            Ok(None)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            gate.state.lock().await.completed = None;
            Ok(None)
        }
        Err(error) => Err(file_error(error)),
    }
}

async fn set_completed_resource(gate: &ResourceGate, target: PathBuf, size: u64) {
    let mut state = gate.state.lock().await;
    state.prepared = None;
    state.completed = Some(CompletedResource { target, size });
}

async fn prepare_paths(
    track: &TrackJobRecord,
    requested: &Path,
    media_url: &Url,
    skip_verified_files: bool,
    downloader: &DownloadClient,
    cancellation: &CancellationToken,
) -> AppResult<(PathBuf, PathBuf, bool)> {
    if let (Some(target), Some(part)) = (&track.target_path, &track.part_path) {
        return reconcile_persisted_paths(
            Path::new(target),
            Path::new(part),
            requested,
            track.bytes_total,
            cancellation,
            || downloader.remote_size(media_url),
        )
        .await;
    }
    if let Some(directory) = requested.parent() {
        tokio::fs::create_dir_all(directory)
            .await
            .map_err(file_error)?;
    }
    if skip_verified_files && tokio::fs::try_exists(requested).await.map_err(file_error)? {
        let remote_size = tokio::select! {
            _ = cancellation.cancelled() => return Err(canceled_error()),
            result = downloader.remote_size(media_url) => result?,
        };
        if let Some(remote_size) = remote_size {
            let local_size = tokio::fs::metadata(requested)
                .await
                .map_err(file_error)?
                .len();
            if local_size == remote_size {
                let filename = requested
                    .file_name()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| AppError::internal("无法生成临时文件名"))?;
                return Ok((
                    requested.to_owned(),
                    requested.with_file_name(format!("{filename}.part")),
                    true,
                ));
            }
        }
    }
    let (target, part) = reserve_unique_target_path(requested)
        .await
        .map_err(file_error)?;
    Ok((target, part, false))
}

async fn prepare_artwork_paths(
    artwork: &ArtworkJobRecord,
    requested: &Path,
    artwork_url: &Url,
    skip_verified_files: bool,
    downloader: &DownloadClient,
    cancellation: &CancellationToken,
) -> AppResult<(PathBuf, PathBuf, bool)> {
    if let (Some(target), Some(part)) = (&artwork.target_path, &artwork.part_path) {
        return reconcile_persisted_paths(
            Path::new(target),
            Path::new(part),
            requested,
            artwork.bytes_total,
            cancellation,
            || downloader.remote_artwork_size(artwork_url),
        )
        .await;
    }
    if let Some(directory) = requested.parent() {
        tokio::fs::create_dir_all(directory)
            .await
            .map_err(file_error)?;
    }
    if skip_verified_files && tokio::fs::try_exists(requested).await.map_err(file_error)? {
        let remote_size = tokio::select! {
            _ = cancellation.cancelled() => return Err(canceled_error()),
            result = downloader.remote_artwork_size(artwork_url) => result?,
        };
        if let Some(remote_size) = remote_size {
            let local_size = tokio::fs::metadata(requested)
                .await
                .map_err(file_error)?
                .len();
            if local_size == remote_size {
                let filename = requested
                    .file_name()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| AppError::internal("无法生成原图临时文件名"))?;
                let part = requested.with_file_name(format!("{filename}.part"));
                return Ok((requested.to_owned(), part, true));
            }
        }
    }
    let (target, part) = reserve_unique_target_path(requested)
        .await
        .map_err(file_error)?;
    Ok((target, part, false))
}

async fn reconcile_persisted_paths<F, Fut>(
    target: &Path,
    part: &Path,
    requested: &Path,
    bytes_total: Option<u64>,
    cancellation: &CancellationToken,
    remote_size: F,
) -> AppResult<(PathBuf, PathBuf, bool)>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = AppResult<Option<u64>>>,
{
    let metadata = match tokio::fs::metadata(target).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((target.to_owned(), part.to_owned(), false));
        }
        Err(error) => return Err(file_error(error)),
    };
    let expected_size = match bytes_total {
        Some(size) => Some(size),
        None => tokio::select! {
            _ = cancellation.cancelled() => return Err(canceled_error()),
            result = remote_size() => result?,
        },
    };

    if metadata.is_file() && expected_size == Some(metadata.len()) {
        if target != part {
            remove_file_if_exists(part).await?;
        }
        return Ok((target.to_owned(), part.to_owned(), true));
    }

    let (new_target, new_part) = reserve_unique_target_path(requested)
        .await
        .map_err(file_error)?;
    if target != part && part != new_part {
        if let Err(error) = migrate_part_file(part, &new_part).await {
            let _ = tokio::fs::remove_file(&new_part).await;
            return Err(error);
        }
    }
    Ok((new_target, new_part, false))
}

async fn migrate_part_file(source: &Path, destination: &Path) -> AppResult<()> {
    match tokio::fs::metadata(source).await {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(file_error(error)),
    }
    tokio::fs::copy(source, destination)
        .await
        .map_err(file_error)?;
    if let Err(error) = tokio::fs::remove_file(source).await {
        let _ = tokio::fs::remove_file(destination).await;
        return Err(file_error(error));
    }
    Ok(())
}

async fn remove_file_if_exists(path: &Path) -> AppResult<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(file_error(error)),
    }
}

async fn remove_part_path(path: Option<&str>) -> AppResult<()> {
    let Some(path) = path else {
        return Ok(());
    };
    remove_file_if_exists(Path::new(path)).await
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

fn canceled_error() -> AppError {
    AppError::new(ErrorCode::Canceled, "下载已取消", false)
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
    'adjust: loop {
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
                    continue 'adjust;
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

    fn resource_key(url: &str, target: &Path) -> ResourceKey {
        ResourceKey {
            resource_url: url.to_owned(),
            requested_target: target.to_owned(),
        }
    }

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
    async fn identical_resources_share_one_gate() {
        let coordinator = ResourceCoordinator::default();
        let key = resource_key(
            "https://media.khinsider.com/track.flac",
            Path::new("track.flac"),
        );

        let first = coordinator.gate_for(key.clone()).await;
        let second = coordinator.gate_for(key).await;

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn resource_gate_waits_until_the_owner_finishes() {
        let gate = Arc::new(ResourceGate::default());
        let owner = gate.execution.lock().await;
        let waiting_gate = gate.clone();
        let mut waiter = tokio::spawn(async move {
            let _guard = waiting_gate.execution.lock().await;
        });

        assert!(tokio::time::timeout(Duration::from_millis(20), &mut waiter)
            .await
            .is_err());
        drop(owner);
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("waiter acquires released resource")
            .expect("waiter task succeeds");
    }

    #[tokio::test]
    async fn completed_resource_is_reused_at_its_original_target() {
        let directory = std::env::temp_dir().join(format!(
            "khinfetch-resource-reuse-test-{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&directory)
            .await
            .expect("create reuse directory");
        let target = directory.join("track.flac");
        tokio::fs::write(&target, b"complete")
            .await
            .expect("write completed resource");
        let gate = ResourceGate::default();
        set_completed_resource(&gate, target.clone(), 8).await;

        let completed = reusable_completed_resource(&gate)
            .await
            .expect("validate completed resource")
            .expect("completed resource is reusable");

        assert_eq!(completed.target, target);
        assert_eq!(completed.size, 8);
        tokio::fs::remove_dir_all(directory)
            .await
            .expect("remove reuse directory");
    }

    #[tokio::test]
    async fn persisted_complete_target_is_reused_without_a_numbered_copy() {
        let directory = std::env::temp_dir().join(format!(
            "khinfetch-persisted-complete-test-{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&directory)
            .await
            .expect("create recovery directory");
        let target = directory.join("track.flac");
        let part = directory.join("track.flac.part");
        tokio::fs::write(&target, b"complete")
            .await
            .expect("write completed target");
        tokio::fs::write(&part, b"stale")
            .await
            .expect("write stale part");

        let (recovered_target, recovered_part, already_complete) = reconcile_persisted_paths(
            &target,
            &part,
            &target,
            Some(8),
            &CancellationToken::new(),
            || async { panic!("known size must not query the remote resource") },
        )
        .await
        .expect("reconcile completed target");

        assert_eq!(recovered_target, target);
        assert_eq!(recovered_part, part);
        assert!(already_complete);
        assert!(!tokio::fs::try_exists(&part)
            .await
            .expect("check stale part"));
        assert!(!tokio::fs::try_exists(directory.join("track (2).flac"))
            .await
            .expect("check numbered target"));
        tokio::fs::remove_dir_all(directory)
            .await
            .expect("remove recovery directory");
    }

    #[tokio::test]
    async fn persisted_mismatched_target_moves_the_part_to_a_new_reservation() {
        let directory = std::env::temp_dir().join(format!(
            "khinfetch-persisted-mismatch-test-{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&directory)
            .await
            .expect("create recovery directory");
        let target = directory.join("track.flac");
        let part = directory.join("track.flac.part");
        tokio::fs::write(&target, b"wrong")
            .await
            .expect("write mismatched target");
        tokio::fs::write(&part, b"resume")
            .await
            .expect("write resumable part");

        let (recovered_target, recovered_part, already_complete) = reconcile_persisted_paths(
            &target,
            &part,
            &target,
            Some(8),
            &CancellationToken::new(),
            || async { panic!("known size must not query the remote resource") },
        )
        .await
        .expect("reconcile mismatched target");

        assert_ne!(recovered_target, target);
        assert!(!already_complete);
        assert_eq!(
            tokio::fs::read(&target)
                .await
                .expect("read original target"),
            b"wrong"
        );
        assert_eq!(
            tokio::fs::read(&recovered_part)
                .await
                .expect("read migrated part"),
            b"resume"
        );
        assert!(!tokio::fs::try_exists(&part).await.expect("check old part"));
        tokio::fs::remove_dir_all(directory)
            .await
            .expect("remove recovery directory");
    }

    #[tokio::test]
    async fn different_urls_with_the_same_filename_use_different_gates() {
        let coordinator = ResourceCoordinator::default();
        let target = Path::new("track.mp3");
        let first = coordinator
            .gate_for(resource_key(
                "https://media.khinsider.com/first.mp3",
                target,
            ))
            .await;
        let second = coordinator
            .gate_for(resource_key(
                "https://media.khinsider.com/second.mp3",
                target,
            ))
            .await;

        assert!(!Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn artwork_jobs_reserve_distinct_targets() {
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
        let downloader = DownloadClient::new("test").expect("create download client");
        let cancellation = CancellationToken::new();
        let first_record = artwork_record("first-job", &directory);
        let first_requested = requested_artwork_path(&first_record, 0, &artwork_url);

        let (first_target, first_part, first_complete) = prepare_artwork_paths(
            &first_record,
            &first_requested,
            &artwork_url,
            false,
            &downloader,
            &cancellation,
        )
        .await
        .expect("prepare first artwork path");
        let second_record = artwork_record("second-job", &directory);
        let second_requested = requested_artwork_path(&second_record, 0, &artwork_url);
        let (second_target, second_part, second_complete) = prepare_artwork_paths(
            &second_record,
            &second_requested,
            &artwork_url,
            false,
            &downloader,
            &cancellation,
        )
        .await
        .expect("prepare second artwork path");

        assert_ne!(first_target, second_target);
        assert_ne!(first_part, second_part);
        assert!(!first_complete);
        assert!(!second_complete);

        tokio::fs::remove_dir_all(directory)
            .await
            .expect("remove test artwork directory");
    }

    #[tokio::test]
    async fn concurrency_adjustment_reacts_while_waiting_for_permits() {
        async fn wait_for_available(semaphore: &Semaphore, expected: usize) {
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    tokio::task::yield_now().await;
                    if semaphore.available_permits() == expected {
                        return;
                    }
                }
            })
            .await
            .expect("semaphore reaches expected availability");
        }

        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENCY as usize));
        let mut active = Vec::new();
        for _ in 0..MAX_CONCURRENCY {
            active.push(
                semaphore
                    .clone()
                    .acquire_owned()
                    .await
                    .expect("acquire active permit"),
            );
        }
        let (desired, receiver) = watch::channel(MAX_CONCURRENCY);
        let adjuster = tokio::spawn(adjust_permits(semaphore.clone(), receiver, Vec::new()));

        desired.send(1).expect("lower concurrency");
        drop(active.pop());
        wait_for_available(&semaphore, 0).await;
        drop(active.pop());
        wait_for_available(&semaphore, 0).await;

        desired.send(MAX_CONCURRENCY).expect("restore concurrency");
        wait_for_available(&semaphore, 2).await;

        drop(active);
        drop(desired);
        tokio::time::timeout(Duration::from_secs(1), adjuster)
            .await
            .expect("adjuster stops")
            .expect("adjuster succeeds");
    }
}
