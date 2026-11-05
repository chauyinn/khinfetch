use std::{fs::File, path::Path, str::FromStr, sync::Arc};

use chrono::{SecondsFormat, Utc};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{from_str, to_string, to_value};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    Row, SqlitePool,
};

use crate::domain::{
    AlbumJob, AlbumSummary, AppError, AppResult, AppSettings, AppSettingsPatch, ErrorCode,
    JobState, TrackSummary,
};

const MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct Storage {
    pool: SqlitePool,
    // Retain ownership until the last storage handle (including download workers) is dropped.
    _owner_lock: Option<Arc<File>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackJobRecord {
    pub id: String,
    pub job_id: String,
    pub track: TrackSummary,
    pub resolved_media_url: Option<String>,
    pub output_directory: String,
    pub target_path: Option<String>,
    pub part_path: Option<String>,
    pub bytes_received: u64,
    pub bytes_total: Option<u64>,
    pub state: JobState,
    pub error: Option<AppError>,
    pub retry_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkJobRecord {
    pub id: String,
    pub job_id: String,
    pub source_url: String,
    pub output_directory: String,
    pub target_path: Option<String>,
    pub part_path: Option<String>,
    pub bytes_received: u64,
    pub bytes_total: Option<u64>,
    pub state: JobState,
    pub error: Option<AppError>,
    pub retry_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

impl Storage {
    pub async fn open(path: &Path) -> AppResult<Self> {
        let in_memory = path.to_string_lossy() == ":memory:";
        let owner_lock = if in_memory {
            None
        } else {
            // Protect recovery even when two processes race through application startup.
            let mut lock_path = path.as_os_str().to_os_string();
            lock_path.push(".lock");
            let lock = File::options()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(Path::new(&lock_path))
                .map_err(|error| storage_error(format!("无法打开数据库所有权锁: {error}")))?;
            lock.try_lock()
                .map_err(|error| storage_error(format!("无法独占打开应用数据库: {error}")))?;
            Some(Arc::new(lock))
        };
        let options = if in_memory {
            SqliteConnectOptions::from_str("sqlite::memory:")
                .map_err(|error| storage_error(format!("无法配置内存数据库: {error}")))?
        } else {
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal)
        }
        .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(if in_memory { 1 } else { 5 })
            .connect_with(options)
            .await
            .map_err(|error| storage_error(format!("无法打开数据库: {error}")))?;

        MIGRATIONS
            .run(&pool)
            .await
            .map_err(|error| storage_error(format!("数据库迁移失败: {error}")))?;

        let recovery_timestamp = timestamp();
        let mut recovery = pool
            .begin()
            .await
            .map_err(|error| storage_error(format!("启动恢复事务失败: {error}")))?;
        sqlx::query(
            "UPDATE album_jobs
             SET state = 'interrupted', current_track_title = NULL,
                 sequence = sequence + 1, updated_at = ?
             WHERE state IN ('queued', 'resolving', 'waiting', 'downloading')",
        )
        .bind(&recovery_timestamp)
        .execute(&mut *recovery)
        .await
        .map_err(|error| storage_error(format!("恢复中断任务失败: {error}")))?;
        sqlx::query(
            "UPDATE track_jobs
             SET state = 'interrupted', updated_at = ?
             WHERE state IN ('queued', 'resolving', 'waiting', 'downloading')",
        )
        .bind(&recovery_timestamp)
        .execute(&mut *recovery)
        .await
        .map_err(|error| storage_error(format!("恢复中断曲目失败: {error}")))?;
        sqlx::query(
            "UPDATE artwork_jobs
             SET state = 'interrupted', updated_at = ?
             WHERE state IN ('queued', 'resolving', 'waiting', 'downloading')",
        )
        .bind(&recovery_timestamp)
        .execute(&mut *recovery)
        .await
        .map_err(|error| storage_error(format!("恢复中断原图失败: {error}")))?;
        recovery
            .commit()
            .await
            .map_err(|error| storage_error(format!("提交启动恢复事务失败: {error}")))?;

        Ok(Self {
            pool,
            _owner_lock: owner_lock,
        })
    }

    pub async fn open_memory() -> AppResult<Self> {
        Self::open(Path::new(":memory:")).await
    }

    pub async fn list_jobs(&self) -> AppResult<Vec<AlbumJob>> {
        let rows = sqlx::query(
            "SELECT id, source_url, title, cover_url, year, platforms_json, album_type,
                    source_kind, quality, state, selected_track_ids_json,
                    selected_track_titles_json, selected_formats_json, total_tracks,
                    completed_tracks, bytes_received, bytes_total, current_track_title,
                    error_json, sequence, created_at, updated_at
             FROM album_jobs
             ORDER BY updated_at DESC, created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| storage_error(format!("读取下载任务失败: {error}")))?;

        rows.into_iter().map(row_to_job).collect()
    }

    pub async fn get_job(&self, job_id: &str) -> AppResult<Option<AlbumJob>> {
        let row = sqlx::query(
            "SELECT id, source_url, title, cover_url, year, platforms_json, album_type,
                    source_kind, quality, state, selected_track_ids_json,
                    selected_track_titles_json, selected_formats_json, total_tracks,
                    completed_tracks, bytes_received, bytes_total, current_track_title,
                    error_json, sequence, created_at, updated_at
             FROM album_jobs WHERE id = ?",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| storage_error(format!("读取下载任务失败: {error}")))?;
        row.map(row_to_job).transpose()
    }

    pub async fn get_settings(&self) -> AppResult<AppSettings> {
        let row = sqlx::query(
            "SELECT download_directory, concurrency, preferred_quality, download_artwork,
                    skip_verified_files, logging_enabled, theme, language
             FROM app_settings WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|error| storage_error(format!("读取设置失败: {error}")))?;

        row_to_settings(row)
    }

    pub async fn update_settings(&self, patch: &AppSettingsPatch) -> AppResult<AppSettings> {
        validate_settings_patch(patch)?;
        // Apply only supplied fields and return this write's snapshot atomically.
        let row = sqlx::query(
            "UPDATE app_settings
             SET download_directory = COALESCE(?, download_directory),
                 concurrency = COALESCE(?, concurrency),
                 preferred_quality = COALESCE(?, preferred_quality),
                 download_artwork = COALESCE(?, download_artwork),
                 skip_verified_files = COALESCE(?, skip_verified_files), logging_enabled = 1,
                 theme = COALESCE(?, theme), language = COALESCE(?, language), updated_at = ?
             WHERE id = 1
             RETURNING download_directory, concurrency, preferred_quality, download_artwork,
                       skip_verified_files, logging_enabled, theme, language",
        )
        .bind(patch.download_directory.as_deref().map(str::trim))
        .bind(patch.concurrency.map(i64::from))
        .bind(patch.preferred_quality.as_ref().map(token).transpose()?)
        .bind(patch.download_artwork)
        .bind(patch.skip_verified_files)
        .bind(patch.theme.as_ref().map(token).transpose()?)
        .bind(patch.language.as_ref().map(token).transpose()?)
        .bind(timestamp())
        .fetch_one(&self.pool)
        .await
        .map_err(|error| storage_error(format!("保存设置失败: {error}")))?;
        row_to_settings(row)
    }

    pub async fn insert_job(&self, job: &AlbumJob) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO album_jobs (
                id, source_url, title, cover_url, year, platforms_json, album_type,
                source_kind, quality, state, selected_track_ids_json,
                selected_track_titles_json, selected_formats_json, total_tracks,
                completed_tracks, bytes_received, bytes_total, current_track_title,
                error_json, sequence, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&job.id)
        .bind(&job.album.source_url)
        .bind(&job.album.title)
        .bind(&job.album.cover_url)
        .bind(&job.album.year)
        .bind(json(&job.album.platforms)?)
        .bind(&job.album.album_type)
        .bind(token(&job.album.source_kind)?)
        .bind(token(&job.quality)?)
        .bind(token(&job.state)?)
        .bind(json(&job.selected_track_ids)?)
        .bind(json(&job.selected_track_titles)?)
        .bind(json(&job.selected_formats)?)
        .bind(i64::from(job.total_tracks))
        .bind(i64::from(job.completed_tracks))
        .bind(
            i64::try_from(job.bytes_received)
                .map_err(|_| storage_error("字节数超出 SQLite 范围"))?,
        )
        .bind(
            job.bytes_total
                .map(i64::try_from)
                .transpose()
                .map_err(|_| storage_error("总字节数超出 SQLite 范围"))?,
        )
        .bind(&job.current_track_title)
        .bind(job.error.as_ref().map(json).transpose()?)
        .bind(i64::try_from(job.sequence).map_err(|_| storage_error("序列号超出 SQLite 范围"))?)
        .bind(&job.created_at)
        .bind(&job.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|error| storage_error(format!("写入下载任务失败: {error}")))?;
        Ok(())
    }

    pub async fn insert_job_with_tracks(
        &self,
        job: &AlbumJob,
        tracks: &[TrackJobRecord],
        download_artwork: bool,
        artworks: &[ArtworkJobRecord],
    ) -> AppResult<()> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| storage_error(format!("启动入队事务失败: {error}")))?;
        sqlx::query(
            "INSERT INTO album_jobs (
                id, source_url, title, cover_url, year, platforms_json, album_type,
                source_kind, quality, download_artwork, state, selected_track_ids_json,
                selected_track_titles_json, selected_formats_json, total_tracks,
                completed_tracks, bytes_received, bytes_total, current_track_title,
                error_json, sequence, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&job.id)
        .bind(&job.album.source_url)
        .bind(&job.album.title)
        .bind(&job.album.cover_url)
        .bind(&job.album.year)
        .bind(json(&job.album.platforms)?)
        .bind(&job.album.album_type)
        .bind(token(&job.album.source_kind)?)
        .bind(token(&job.quality)?)
        .bind(download_artwork)
        .bind(token(&job.state)?)
        .bind(json(&job.selected_track_ids)?)
        .bind(json(&job.selected_track_titles)?)
        .bind(json(&job.selected_formats)?)
        .bind(i64::from(job.total_tracks))
        .bind(i64::from(job.completed_tracks))
        .bind(
            i64::try_from(job.bytes_received)
                .map_err(|_| storage_error("字节数超出 SQLite 范围"))?,
        )
        .bind(
            job.bytes_total
                .map(i64::try_from)
                .transpose()
                .map_err(|_| storage_error("总字节数超出 SQLite 范围"))?,
        )
        .bind(&job.current_track_title)
        .bind(job.error.as_ref().map(json).transpose()?)
        .bind(i64::try_from(job.sequence).map_err(|_| storage_error("序列号超出 SQLite 范围"))?)
        .bind(&job.created_at)
        .bind(&job.updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| storage_error(format!("写入下载任务失败: {error}")))?;

        for track in tracks {
            sqlx::query(
                "INSERT INTO track_jobs (
                    id, job_id, disc_number, track_index, title, duration_seconds, track_page_url,
                    resolved_media_url, available_formats_json, output_directory, target_path, part_path,
                    bytes_received, bytes_total, state, error_json, retry_count,
                    created_at, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&track.id)
            .bind(&track.job_id)
            .bind(track.track.disc_number.map(i64::from))
            .bind(track.track.index.map(i64::from))
            .bind(&track.track.title)
            .bind(track.track.duration_seconds.map(i64::from))
            .bind(&track.track.track_page_url)
            .bind(&track.resolved_media_url)
            .bind(json(&track.track.available_formats)?)
            .bind(&track.output_directory)
            .bind(&track.target_path)
            .bind(&track.part_path)
            .bind(
                i64::try_from(track.bytes_received)
                    .map_err(|_| storage_error("字节数超出 SQLite 范围"))?,
            )
            .bind(
                track
                    .bytes_total
                    .map(i64::try_from)
                    .transpose()
                    .map_err(|_| storage_error("总字节数超出 SQLite 范围"))?,
            )
            .bind(token(&track.state)?)
            .bind(track.error.as_ref().map(json).transpose()?)
            .bind(i64::from(track.retry_count))
            .bind(&track.created_at)
            .bind(&track.updated_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| storage_error(format!("写入曲目任务失败: {error}")))?;
        }
        for artwork in artworks {
            sqlx::query(
                "INSERT INTO artwork_jobs (
                    id, job_id, source_url, output_directory, target_path, part_path,
                    bytes_received, bytes_total, state, error_json, retry_count,
                    created_at, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&artwork.id)
            .bind(&artwork.job_id)
            .bind(&artwork.source_url)
            .bind(&artwork.output_directory)
            .bind(&artwork.target_path)
            .bind(&artwork.part_path)
            .bind(
                i64::try_from(artwork.bytes_received)
                    .map_err(|_| storage_error("原图字节数超出 SQLite 范围"))?,
            )
            .bind(
                artwork
                    .bytes_total
                    .map(i64::try_from)
                    .transpose()
                    .map_err(|_| storage_error("原图总字节数超出 SQLite 范围"))?,
            )
            .bind(token(&artwork.state)?)
            .bind(artwork.error.as_ref().map(json).transpose()?)
            .bind(i64::from(artwork.retry_count))
            .bind(&artwork.created_at)
            .bind(&artwork.updated_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| storage_error(format!("写入原图任务失败: {error}")))?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| storage_error(format!("提交入队事务失败: {error}")))
    }

    pub async fn set_job_state(
        &self,
        job_id: &str,
        state: JobState,
        error: Option<&AppError>,
        current_track_title: Option<&str>,
    ) -> AppResult<AlbumJob> {
        let result = sqlx::query(
            "UPDATE album_jobs
             SET state = ?, error_json = ?, current_track_title = ?,
                 sequence = sequence + 1, updated_at = ?
             WHERE id = ?",
        )
        .bind(token(&state)?)
        .bind(error.map(json).transpose()?)
        .bind(current_track_title)
        .bind(timestamp())
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|error| storage_error(format!("更新任务状态失败: {error}")))?;
        if result.rows_affected() == 0 {
            return Err(storage_error("下载任务不存在"));
        }
        self.get_job(job_id)
            .await?
            .ok_or_else(|| storage_error("下载任务不存在"))
    }

    pub async fn refresh_job_progress(
        &self,
        job_id: &str,
        current_track_title: Option<&str>,
    ) -> AppResult<AlbumJob> {
        let tracks = self.list_track_jobs(job_id).await?;
        let completed_tracks = tracks
            .iter()
            .filter(|track| track.state == JobState::Completed)
            .count() as u32;
        let bytes_received = tracks
            .iter()
            .try_fold(0_u64, |total, track| {
                total.checked_add(track.bytes_received)
            })
            .ok_or_else(|| storage_error("任务字节计数溢出"))?;
        let bytes_total = match tracks
            .iter()
            .map(|track| track.bytes_total)
            .collect::<Option<Vec<_>>>()
        {
            Some(values) => Some(
                values
                    .into_iter()
                    .try_fold(0_u64, |total, value| total.checked_add(value))
                    .ok_or_else(|| storage_error("任务总字节数溢出"))?,
            ),
            None => None,
        };
        sqlx::query(
            "UPDATE album_jobs
             SET completed_tracks = ?, bytes_received = ?, bytes_total = ?,
                 current_track_title = ?, sequence = sequence + 1, updated_at = ?
             WHERE id = ?",
        )
        .bind(i64::from(completed_tracks))
        .bind(i64::try_from(bytes_received).map_err(|_| storage_error("字节数超出 SQLite 范围"))?)
        .bind(
            bytes_total
                .map(i64::try_from)
                .transpose()
                .map_err(|_| storage_error("总字节数超出 SQLite 范围"))?,
        )
        .bind(current_track_title)
        .bind(timestamp())
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|error| storage_error(format!("更新任务进度失败: {error}")))?;
        self.get_job(job_id)
            .await?
            .ok_or_else(|| storage_error("下载任务不存在"))
    }

    pub async fn refresh_job_runtime_state(
        &self,
        job_id: &str,
        current_track_title: Option<&str>,
    ) -> AppResult<AlbumJob> {
        let result = sqlx::query(
            "UPDATE album_jobs
             SET state = CASE
                     WHEN EXISTS (
                         SELECT 1 FROM track_jobs
                         WHERE track_jobs.job_id = album_jobs.id AND state = 'downloading'
                     ) OR EXISTS (
                         SELECT 1 FROM artwork_jobs
                         WHERE artwork_jobs.job_id = album_jobs.id AND state = 'downloading'
                     ) THEN 'downloading'
                     WHEN EXISTS (
                         SELECT 1 FROM track_jobs
                         WHERE track_jobs.job_id = album_jobs.id AND state = 'resolving'
                     ) OR EXISTS (
                         SELECT 1 FROM artwork_jobs
                         WHERE artwork_jobs.job_id = album_jobs.id AND state = 'resolving'
                     ) THEN 'resolving'
                     WHEN EXISTS (
                         SELECT 1 FROM track_jobs
                         WHERE track_jobs.job_id = album_jobs.id AND state = 'waiting'
                     ) OR EXISTS (
                         SELECT 1 FROM artwork_jobs
                         WHERE artwork_jobs.job_id = album_jobs.id AND state = 'waiting'
                     ) THEN 'waiting'
                     WHEN EXISTS (
                         SELECT 1 FROM track_jobs
                         WHERE track_jobs.job_id = album_jobs.id AND state = 'queued'
                     ) OR EXISTS (
                         SELECT 1 FROM artwork_jobs
                         WHERE artwork_jobs.job_id = album_jobs.id AND state = 'queued'
                     ) THEN 'queued'
                     ELSE state
                 END,
                 current_track_title = ?, sequence = sequence + 1, updated_at = ?
             WHERE id = ?",
        )
        .bind(current_track_title)
        .bind(timestamp())
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|error| storage_error(format!("更新任务运行状态失败: {error}")))?;
        if result.rows_affected() == 0 {
            return Err(storage_error("下载任务不存在"));
        }
        self.get_job(job_id)
            .await?
            .ok_or_else(|| storage_error("下载任务不存在"))
    }

    pub async fn remove_job(&self, job_id: &str) -> AppResult<()> {
        sqlx::query("DELETE FROM album_jobs WHERE id = ?")
            .bind(job_id)
            .execute(&self.pool)
            .await
            .map_err(|error| storage_error(format!("移除下载任务失败: {error}")))?;
        Ok(())
    }

    pub async fn insert_track_job(&self, job: &TrackJobRecord) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO track_jobs (
                id, job_id, disc_number, track_index, title, duration_seconds, track_page_url,
                resolved_media_url, available_formats_json, output_directory, target_path, part_path,
                bytes_received, bytes_total, state, error_json, retry_count,
                created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&job.id)
        .bind(&job.job_id)
        .bind(job.track.disc_number.map(i64::from))
        .bind(job.track.index.map(i64::from))
        .bind(&job.track.title)
        .bind(job.track.duration_seconds.map(i64::from))
        .bind(&job.track.track_page_url)
        .bind(&job.resolved_media_url)
        .bind(json(&job.track.available_formats)?)
        .bind(&job.output_directory)
        .bind(&job.target_path)
        .bind(&job.part_path)
        .bind(
            i64::try_from(job.bytes_received)
                .map_err(|_| storage_error("字节数超出 SQLite 范围"))?,
        )
        .bind(
            job.bytes_total
                .map(i64::try_from)
                .transpose()
                .map_err(|_| storage_error("总字节数超出 SQLite 范围"))?,
        )
        .bind(token(&job.state)?)
        .bind(job.error.as_ref().map(json).transpose()?)
        .bind(i64::from(job.retry_count))
        .bind(&job.created_at)
        .bind(&job.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|error| storage_error(format!("写入曲目任务失败: {error}")))?;
        Ok(())
    }

    pub async fn list_track_jobs(&self, job_id: &str) -> AppResult<Vec<TrackJobRecord>> {
        let rows = sqlx::query(
            "SELECT id, job_id, disc_number, track_index, title, duration_seconds, track_page_url,
                    resolved_media_url, available_formats_json, output_directory, target_path, part_path,
                    bytes_received, bytes_total, state, error_json, retry_count,
                    created_at, updated_at
             FROM track_jobs WHERE job_id = ? ORDER BY disc_number, track_index, created_at",
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| storage_error(format!("读取曲目任务失败: {error}")))?;
        rows.into_iter().map(row_to_track_job).collect()
    }

    pub async fn save_track_job(&self, track: &TrackJobRecord) -> AppResult<()> {
        let result = sqlx::query(
            "UPDATE track_jobs
             SET resolved_media_url = ?, target_path = ?, part_path = ?,
                 bytes_received = ?, bytes_total = ?, state = ?, error_json = ?,
                 retry_count = ?, updated_at = ?
             WHERE id = ? AND job_id = ?",
        )
        .bind(&track.resolved_media_url)
        .bind(&track.target_path)
        .bind(&track.part_path)
        .bind(
            i64::try_from(track.bytes_received)
                .map_err(|_| storage_error("字节数超出 SQLite 范围"))?,
        )
        .bind(
            track
                .bytes_total
                .map(i64::try_from)
                .transpose()
                .map_err(|_| storage_error("总字节数超出 SQLite 范围"))?,
        )
        .bind(token(&track.state)?)
        .bind(track.error.as_ref().map(json).transpose()?)
        .bind(i64::from(track.retry_count))
        .bind(timestamp())
        .bind(&track.id)
        .bind(&track.job_id)
        .execute(&self.pool)
        .await
        .map_err(|error| storage_error(format!("保存曲目任务失败: {error}")))?;
        if result.rows_affected() == 0 {
            return Err(storage_error("曲目任务不存在"));
        }
        Ok(())
    }

    pub async fn job_downloads_artwork(&self, job_id: &str) -> AppResult<bool> {
        let row = sqlx::query("SELECT download_artwork FROM album_jobs WHERE id = ?")
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| storage_error(format!("读取原图下载设置失败: {error}")))?
            .ok_or_else(|| storage_error("下载任务不存在"))?;
        Ok(row_i64(&row, "download_artwork")? != 0)
    }

    pub async fn list_artwork_jobs(&self, job_id: &str) -> AppResult<Vec<ArtworkJobRecord>> {
        let rows = sqlx::query(
            "SELECT id, job_id, source_url, output_directory, target_path, part_path,
                    bytes_received, bytes_total, state, error_json, retry_count,
                    created_at, updated_at
             FROM artwork_jobs WHERE job_id = ? ORDER BY created_at, id",
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| storage_error(format!("读取原图任务失败: {error}")))?;
        rows.into_iter().map(row_to_artwork_job).collect()
    }

    pub async fn save_artwork_job(&self, artwork: &ArtworkJobRecord) -> AppResult<()> {
        let result = sqlx::query(
            "UPDATE artwork_jobs
             SET target_path = ?, part_path = ?, bytes_received = ?, bytes_total = ?,
                 state = ?, error_json = ?, retry_count = ?, updated_at = ?
             WHERE id = ? AND job_id = ?",
        )
        .bind(&artwork.target_path)
        .bind(&artwork.part_path)
        .bind(
            i64::try_from(artwork.bytes_received)
                .map_err(|_| storage_error("原图字节数超出 SQLite 范围"))?,
        )
        .bind(
            artwork
                .bytes_total
                .map(i64::try_from)
                .transpose()
                .map_err(|_| storage_error("原图总字节数超出 SQLite 范围"))?,
        )
        .bind(token(&artwork.state)?)
        .bind(artwork.error.as_ref().map(json).transpose()?)
        .bind(i64::from(artwork.retry_count))
        .bind(timestamp())
        .bind(&artwork.id)
        .bind(&artwork.job_id)
        .execute(&self.pool)
        .await
        .map_err(|error| storage_error(format!("保存原图任务失败: {error}")))?;
        if result.rows_affected() == 0 {
            return Err(storage_error("原图任务不存在"));
        }
        Ok(())
    }
}

fn validate_settings_patch(patch: &AppSettingsPatch) -> AppResult<()> {
    if let Some(value) = &patch.download_directory {
        let value = value.trim();
        if value.is_empty() || value.contains('\0') || !Path::new(value).is_absolute() {
            return Err(AppError::new(
                ErrorCode::PermissionDenied,
                "下载目录必须是有效的绝对路径",
                false,
            ));
        }
    }
    if let Some(value) = patch.concurrency {
        if !(1..=8).contains(&value) {
            return Err(AppError::invalid_url("并发数必须在 1 到 8 之间"));
        }
    }
    Ok(())
}

fn row_to_settings(row: sqlx::sqlite::SqliteRow) -> AppResult<AppSettings> {
    let concurrency = row_i64(&row, "concurrency")?;
    let concurrency =
        u8::try_from(concurrency).map_err(|_| storage_error("数据库中的并发数无效"))?;
    if !(1..=8).contains(&concurrency) {
        return Err(storage_error("数据库中的并发数超出范围"));
    }

    Ok(AppSettings {
        download_directory: row
            .try_get("download_directory")
            .map_err(sqlx_error_to_app)?,
        concurrency,
        preferred_quality: parse_token(row_string(&row, "preferred_quality")?)?,
        download_artwork: row_i64(&row, "download_artwork")? != 0,
        skip_verified_files: row_i64(&row, "skip_verified_files")? != 0,
        logging_enabled: true,
        theme: parse_token(row_string(&row, "theme")?)?,
        language: parse_token(row_string(&row, "language")?)?,
    })
}

fn row_to_job(row: sqlx::sqlite::SqliteRow) -> AppResult<AlbumJob> {
    let bytes_received = u64_from_i64(row_i64(&row, "bytes_received")?, "已接收字节数")?;
    let bytes_total = row
        .try_get::<Option<i64>, _>("bytes_total")
        .map_err(sqlx_error_to_app)?
        .map(|value| u64_from_i64(value, "总字节数"))
        .transpose()?;
    let total_tracks = u32::try_from(row_i64(&row, "total_tracks")?)
        .map_err(|_| storage_error("数据库中的曲目总数无效"))?;
    let completed_tracks = u32::try_from(row_i64(&row, "completed_tracks")?)
        .map_err(|_| storage_error("数据库中的已完成曲目数无效"))?;
    let sequence = u64_from_i64(row_i64(&row, "sequence")?, "任务序列号")?;
    let error_json = row
        .try_get::<Option<String>, _>("error_json")
        .map_err(sqlx_error_to_app)?;

    Ok(AlbumJob {
        id: row_string(&row, "id")?,
        album: AlbumSummary {
            source_url: row_string(&row, "source_url")?,
            title: row_string(&row, "title")?,
            cover_url: row.try_get("cover_url").map_err(sqlx_error_to_app)?,
            year: row.try_get("year").map_err(sqlx_error_to_app)?,
            platforms: parse_json(row_string(&row, "platforms_json")?)?,
            album_type: row.try_get("album_type").map_err(sqlx_error_to_app)?,
            source_kind: parse_token(row_string(&row, "source_kind")?)?,
        },
        quality: parse_token(row_string(&row, "quality")?)?,
        selected_formats: parse_json(row_string(&row, "selected_formats_json")?)?,
        state: parse_token(row_string(&row, "state")?)?,
        selected_track_ids: parse_json(row_string(&row, "selected_track_ids_json")?)?,
        selected_track_titles: parse_json(row_string(&row, "selected_track_titles_json")?)?,
        total_tracks,
        completed_tracks,
        bytes_received,
        bytes_total,
        current_track_title: row
            .try_get("current_track_title")
            .map_err(sqlx_error_to_app)?,
        error: error_json
            .map(|value| from_str(&value))
            .transpose()
            .map_err(json_error)?,
        sequence,
        created_at: row_string(&row, "created_at")?,
        updated_at: row_string(&row, "updated_at")?,
    })
}

fn row_to_track_job(row: sqlx::sqlite::SqliteRow) -> AppResult<TrackJobRecord> {
    let duration_seconds = row
        .try_get::<Option<i64>, _>("duration_seconds")
        .map_err(sqlx_error_to_app)?
        .map(|value| u32::try_from(value).map_err(|_| storage_error("数据库中的曲目时长无效")))
        .transpose()?;
    let bytes_received = u64_from_i64(row_i64(&row, "bytes_received")?, "曲目已接收字节数")?;
    let bytes_total = row
        .try_get::<Option<i64>, _>("bytes_total")
        .map_err(sqlx_error_to_app)?
        .map(|value| u64_from_i64(value, "曲目总字节数"))
        .transpose()?;
    let retry_count = u32::try_from(row_i64(&row, "retry_count")?)
        .map_err(|_| storage_error("数据库中的重试次数无效"))?;
    let error_json = row
        .try_get::<Option<String>, _>("error_json")
        .map_err(sqlx_error_to_app)?;

    Ok(TrackJobRecord {
        id: row_string(&row, "id")?,
        job_id: row_string(&row, "job_id")?,
        track: TrackSummary {
            id: row_string(&row, "id")?,
            disc_number: row
                .try_get::<Option<i64>, _>("disc_number")
                .map_err(sqlx_error_to_app)?
                .map(|value| {
                    u32::try_from(value).map_err(|_| storage_error("数据库中的 CD 序号无效"))
                })
                .transpose()?,
            index: row
                .try_get::<Option<i64>, _>("track_index")
                .map_err(sqlx_error_to_app)?
                .map(|value| {
                    u32::try_from(value).map_err(|_| storage_error("数据库中的曲目序号无效"))
                })
                .transpose()?,
            title: row_string(&row, "title")?,
            duration_seconds,
            track_page_url: row_string(&row, "track_page_url")?,
            available_formats: parse_json(row_string(&row, "available_formats_json")?)?,
            available_variants: Vec::new(),
        },
        resolved_media_url: row
            .try_get("resolved_media_url")
            .map_err(sqlx_error_to_app)?,
        output_directory: row_string(&row, "output_directory")?,
        target_path: row.try_get("target_path").map_err(sqlx_error_to_app)?,
        part_path: row.try_get("part_path").map_err(sqlx_error_to_app)?,
        bytes_received,
        bytes_total,
        state: parse_token(row_string(&row, "state")?)?,
        error: error_json
            .map(|value| from_str(&value))
            .transpose()
            .map_err(json_error)?,
        retry_count,
        created_at: row_string(&row, "created_at")?,
        updated_at: row_string(&row, "updated_at")?,
    })
}

fn row_to_artwork_job(row: sqlx::sqlite::SqliteRow) -> AppResult<ArtworkJobRecord> {
    let bytes_received = u64_from_i64(row_i64(&row, "bytes_received")?, "原图已接收字节数")?;
    let bytes_total = row
        .try_get::<Option<i64>, _>("bytes_total")
        .map_err(sqlx_error_to_app)?
        .map(|value| u64_from_i64(value, "原图总字节数"))
        .transpose()?;
    let retry_count = u32::try_from(row_i64(&row, "retry_count")?)
        .map_err(|_| storage_error("数据库中的原图重试次数无效"))?;
    let error_json = row
        .try_get::<Option<String>, _>("error_json")
        .map_err(sqlx_error_to_app)?;

    Ok(ArtworkJobRecord {
        id: row_string(&row, "id")?,
        job_id: row_string(&row, "job_id")?,
        source_url: row_string(&row, "source_url")?,
        output_directory: row_string(&row, "output_directory")?,
        target_path: row.try_get("target_path").map_err(sqlx_error_to_app)?,
        part_path: row.try_get("part_path").map_err(sqlx_error_to_app)?,
        bytes_received,
        bytes_total,
        state: parse_token(row_string(&row, "state")?)?,
        error: error_json
            .map(|value| from_str(&value))
            .transpose()
            .map_err(json_error)?,
        retry_count,
        created_at: row_string(&row, "created_at")?,
        updated_at: row_string(&row, "updated_at")?,
    })
}

fn row_string(row: &sqlx::sqlite::SqliteRow, column: &str) -> AppResult<String> {
    row.try_get(column).map_err(sqlx_error_to_app)
}

fn row_i64(row: &sqlx::sqlite::SqliteRow, column: &str) -> AppResult<i64> {
    row.try_get(column).map_err(sqlx_error_to_app)
}

fn parse_json<T: DeserializeOwned>(value: String) -> AppResult<T> {
    from_str(&value).map_err(json_error)
}

fn parse_token<T: DeserializeOwned>(value: String) -> AppResult<T> {
    from_str(&format!("\"{value}\"")).map_err(json_error)
}

fn json<T: Serialize>(value: &T) -> AppResult<String> {
    to_string(value).map_err(json_error)
}

fn token<T: Serialize>(value: &T) -> AppResult<String> {
    let value = to_value(value).map_err(json_error)?;
    value
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| storage_error("无法序列化枚举设置"))
}

fn u64_from_i64(value: i64, name: &str) -> AppResult<u64> {
    u64::try_from(value).map_err(|_| storage_error(format!("数据库中的{name}无效")))
}

pub(crate) fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn json_error(error: serde_json::Error) -> AppError {
    storage_error(format!("数据库 JSON 数据无效: {error}"))
}

fn sqlx_error_to_app(error: sqlx::Error) -> AppError {
    storage_error(format!("数据库读取失败: {error}"))
}

fn storage_error(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::Storage, message, true)
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        AlbumJob, AlbumSummary, AppSettingsPatch, JobState, LanguagePreference, QualityPreference,
        SourceKind, ThemePreference, TrackSummary,
    };

    use super::{Storage, TrackJobRecord};

    fn example_job(state: JobState) -> AlbumJob {
        AlbumJob {
            id: "job".to_owned(),
            album: AlbumSummary {
                source_url: "https://downloads.khinsider.com/game-soundtracks/album/example"
                    .to_owned(),
                title: "Example".to_owned(),
                cover_url: None,
                year: None,
                platforms: Vec::new(),
                album_type: None,
                source_kind: SourceKind::Album,
            },
            quality: QualityPreference::BestAvailable,
            selected_formats: vec!["FLAC".to_owned()],
            state,
            selected_track_ids: vec!["source-track".to_owned()],
            selected_track_titles: vec!["Track".to_owned()],
            total_tracks: 1,
            completed_tracks: 0,
            bytes_received: 0,
            bytes_total: None,
            current_track_title: None,
            error: None,
            sequence: 1,
            created_at: "0".to_owned(),
            updated_at: "0".to_owned(),
        }
    }

    fn example_track(state: JobState) -> TrackJobRecord {
        TrackJobRecord {
            id: "track".to_owned(),
            job_id: "job".to_owned(),
            track: TrackSummary {
                id: "source-track".to_owned(),
                disc_number: Some(2),
                index: Some(3),
                title: "Track".to_owned(),
                duration_seconds: Some(65),
                track_page_url:
                    "https://downloads.khinsider.com/game-soundtracks/album/example/track"
                        .to_owned(),
                available_formats: vec!["FLAC".to_owned()],
                available_variants: Vec::new(),
            },
            resolved_media_url: None,
            output_directory: "C:\\Downloads\\Example".to_owned(),
            target_path: None,
            part_path: None,
            bytes_received: 0,
            bytes_total: None,
            state,
            error: None,
            retry_count: 0,
            created_at: "0".to_owned(),
            updated_at: "0".to_owned(),
        }
    }

    #[tokio::test]
    async fn applies_all_migrations_to_a_fresh_database() {
        let storage = Storage::open_memory()
            .await
            .expect("apply database migrations");

        assert!(storage.list_jobs().await.expect("list jobs").is_empty());
    }

    #[tokio::test]
    async fn second_storage_owner_cannot_recover_an_active_database() {
        let directory =
            std::env::temp_dir().join(format!("khinfetch-owner-test-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&directory).await.unwrap();
        let database_path = directory.join("owner.sqlite3");
        let first = Storage::open(&database_path).await.expect("first owner");
        first
            .insert_job(&example_job(JobState::Downloading))
            .await
            .unwrap();
        first
            .insert_track_job(&example_track(JobState::Downloading))
            .await
            .unwrap();

        assert!(Storage::open(&database_path).await.is_err());
        assert_eq!(
            first.get_job("job").await.unwrap().unwrap().state,
            JobState::Downloading
        );
        assert_eq!(first.get_job("job").await.unwrap().unwrap().sequence, 1);
        assert_eq!(
            first.list_track_jobs("job").await.unwrap()[0].state,
            JobState::Downloading
        );

        let worker = first.clone();
        drop(first);
        assert!(Storage::open(&database_path).await.is_err());
        worker.pool.close().await;
        drop(worker);

        // The sidecar can remain on disk: OS ownership ends when the last handle closes.
        let restarted = Storage::open(&database_path)
            .await
            .expect("restart after owner exits");
        assert_eq!(
            restarted.get_job("job").await.unwrap().unwrap().state,
            JobState::Interrupted
        );
        assert_eq!(
            restarted.list_track_jobs("job").await.unwrap()[0].state,
            JobState::Interrupted
        );
        restarted.pool.close().await;
        drop(restarted);
        tokio::fs::remove_dir_all(&directory).await.unwrap();
    }

    #[tokio::test]
    async fn concurrent_settings_patches_preserve_independent_fields() {
        let storage = Storage::open_memory().await.expect("open database");
        let concurrency = AppSettingsPatch {
            concurrency: Some(8),
            ..Default::default()
        };
        let artwork = AppSettingsPatch {
            download_artwork: Some(true),
            ..Default::default()
        };
        let appearance = AppSettingsPatch {
            theme: Some(ThemePreference::Dark),
            language: Some(LanguagePreference::En),
            ..Default::default()
        };

        let (a, b, c) = tokio::join!(
            storage.update_settings(&concurrency),
            storage.update_settings(&artwork),
            storage.update_settings(&appearance),
        );
        assert_eq!(a.expect("save concurrency").concurrency, 8);
        assert!(b.expect("save artwork preference").download_artwork);
        assert_eq!(c.expect("save appearance").theme, ThemePreference::Dark);

        let settings = storage.get_settings().await.expect("read settings");
        assert_eq!(settings.concurrency, 8);
        assert!(settings.download_artwork);
        assert_eq!(settings.theme, ThemePreference::Dark);
        assert_eq!(settings.language, LanguagePreference::En);
    }

    #[tokio::test]
    async fn concurrent_settings_writes_return_their_own_snapshot() {
        let storage = Storage::open_memory().await.expect("open database");
        let first = AppSettingsPatch {
            concurrency: Some(5),
            ..Default::default()
        };
        let second = AppSettingsPatch {
            concurrency: Some(8),
            ..Default::default()
        };
        let (a, b) = tokio::join!(
            storage.update_settings(&first),
            storage.update_settings(&second)
        );
        assert_eq!(a.expect("save first patch").concurrency, 5);
        assert_eq!(b.expect("save second patch").concurrency, 8);
    }

    #[tokio::test]
    async fn settings_patch_preserves_omitted_fields_and_accepts_false() {
        let storage = Storage::open_memory().await.expect("open database");
        let directory = std::env::temp_dir().join("khinfetch-settings");
        let original = storage
            .update_settings(&AppSettingsPatch {
                download_directory: Some(format!("  {}  ", directory.display())),
                concurrency: Some(7),
                preferred_quality: Some(QualityPreference::Mp3),
                download_artwork: Some(true),
                theme: Some(ThemePreference::Dark),
                language: Some(LanguagePreference::En),
                ..Default::default()
            })
            .await
            .expect("save initial settings");
        assert_eq!(original.download_directory, directory.to_string_lossy());
        let updated = storage
            .update_settings(&AppSettingsPatch {
                download_artwork: Some(false),
                skip_verified_files: Some(false),
                ..Default::default()
            })
            .await
            .expect("disable boolean preferences");
        let mut expected = original;
        expected.download_artwork = false;
        expected.skip_verified_files = false;
        assert_eq!(updated, expected);
        assert_eq!(storage.get_settings().await.unwrap(), expected);
        assert_eq!(
            storage
                .update_settings(&AppSettingsPatch::default())
                .await
                .unwrap(),
            expected
        );
    }

    #[tokio::test]
    async fn invalid_settings_patch_does_not_partially_update_fields() {
        let storage = Storage::open_memory().await.expect("open database");
        let original = storage.get_settings().await.unwrap();
        for patch in [
            AppSettingsPatch {
                concurrency: Some(0),
                download_artwork: Some(true),
                ..Default::default()
            },
            AppSettingsPatch {
                concurrency: Some(9),
                download_artwork: Some(true),
                ..Default::default()
            },
            AppSettingsPatch {
                download_directory: Some("relative/path".to_owned()),
                concurrency: Some(8),
                ..Default::default()
            },
            AppSettingsPatch {
                download_directory: Some("  ".to_owned()),
                concurrency: Some(8),
                ..Default::default()
            },
        ] {
            assert!(storage.update_settings(&patch).await.is_err());
            assert_eq!(storage.get_settings().await.unwrap(), original);
        }
    }

    #[tokio::test]
    async fn persists_track_metadata_and_job_selection_titles() {
        let storage = Storage::open_memory()
            .await
            .expect("apply database migrations");
        let job = example_job(JobState::Queued);
        storage.insert_job(&job).await.expect("insert album job");
        storage
            .insert_track_job(&example_track(JobState::Queued))
            .await
            .expect("insert track job");

        let persisted_job = storage
            .get_job(&job.id)
            .await
            .expect("read album job")
            .expect("album job exists");
        let persisted_tracks = storage
            .list_track_jobs(&job.id)
            .await
            .expect("read track jobs");

        assert_eq!(persisted_job.selected_track_titles, vec!["Track"]);
        assert_eq!(persisted_job.selected_formats, vec!["FLAC"]);
        assert_eq!(persisted_tracks[0].track.disc_number, Some(2));
        assert_eq!(persisted_tracks[0].track.index, Some(3));
    }

    #[tokio::test]
    async fn runtime_state_prioritizes_downloads_over_waiters() {
        let storage = Storage::open_memory()
            .await
            .expect("apply database migrations");
        let mut job = example_job(JobState::Resolving);
        job.total_tracks = 2;
        storage.insert_job(&job).await.expect("insert album job");
        let waiting = example_track(JobState::Waiting);
        storage
            .insert_track_job(&waiting)
            .await
            .expect("insert waiting track");
        let mut downloading = example_track(JobState::Downloading);
        downloading.id = "track-2".to_owned();
        downloading.track.id = "source-track-2".to_owned();
        downloading.track.index = Some(4);
        storage
            .insert_track_job(&downloading)
            .await
            .expect("insert downloading track");

        let active = storage
            .refresh_job_runtime_state(&job.id, Some("Track"))
            .await
            .expect("refresh active state");
        assert_eq!(active.state, JobState::Downloading);

        downloading.state = JobState::Completed;
        storage
            .save_track_job(&downloading)
            .await
            .expect("complete downloading track");
        let waiting_job = storage
            .refresh_job_runtime_state(&job.id, Some("Track"))
            .await
            .expect("refresh waiting state");
        assert_eq!(waiting_job.state, JobState::Waiting);
    }

    #[tokio::test]
    async fn queued_jobs_are_interrupted_when_database_reopens() {
        let directory =
            std::env::temp_dir().join(format!("khinfetch-recovery-test-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&directory)
            .await
            .expect("create recovery directory");
        let database_path = directory.join("recovery.sqlite3");
        {
            let storage = Storage::open(&database_path)
                .await
                .expect("open recovery database");
            storage
                .insert_job(&example_job(JobState::Queued))
                .await
                .expect("insert queued job");
            storage
                .insert_track_job(&example_track(JobState::Queued))
                .await
                .expect("insert queued track");
            storage.pool.close().await;
        }

        let storage = Storage::open(&database_path)
            .await
            .expect("reopen recovery database");
        let job = storage
            .get_job("job")
            .await
            .expect("read recovered job")
            .expect("recovered job exists");
        let tracks = storage
            .list_track_jobs("job")
            .await
            .expect("read recovered tracks");

        assert_eq!(job.state, JobState::Interrupted);
        assert_eq!(job.sequence, 2);
        assert_eq!(tracks[0].state, JobState::Interrupted);
        storage.pool.close().await;
        drop(storage);
        tokio::fs::remove_dir_all(&directory)
            .await
            .expect("remove recovery directory");
    }

    #[tokio::test]
    async fn waiting_jobs_are_interrupted_when_database_reopens() {
        let directory = std::env::temp_dir().join(format!(
            "khinfetch-waiting-recovery-test-{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&directory)
            .await
            .expect("create recovery directory");
        let database_path = directory.join("recovery.sqlite3");
        {
            let storage = Storage::open(&database_path)
                .await
                .expect("open recovery database");
            storage
                .insert_job(&example_job(JobState::Waiting))
                .await
                .expect("insert waiting job");
            storage
                .insert_track_job(&example_track(JobState::Waiting))
                .await
                .expect("insert waiting track");
            storage.pool.close().await;
        }

        let storage = Storage::open(&database_path)
            .await
            .expect("reopen recovery database");
        let job = storage
            .get_job("job")
            .await
            .expect("read recovered job")
            .expect("recovered job exists");
        let tracks = storage
            .list_track_jobs("job")
            .await
            .expect("read recovered tracks");

        assert_eq!(job.state, JobState::Interrupted);
        assert_eq!(tracks[0].state, JobState::Interrupted);
        storage.pool.close().await;
        drop(storage);
        tokio::fs::remove_dir_all(&directory)
            .await
            .expect("remove recovery directory");
    }
}
