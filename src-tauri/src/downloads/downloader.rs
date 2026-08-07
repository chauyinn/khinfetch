use std::{path::Path, sync::Arc, time::Duration};

use futures_util::StreamExt;
use reqwest::{
    header::{CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE, RETRY_AFTER},
    redirect::Policy,
    Client, StatusCode,
};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{
    domain::{AppError, AppResult, ErrorCode},
    khinsider::{validate_artwork_url, validate_media_url},
};

const MAX_ATTEMPTS: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    pub bytes_received: u64,
    pub bytes_total: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadOutcome {
    pub bytes_received: u64,
    pub bytes_total: Option<u64>,
}

pub type ProgressCallback = Arc<dyn Fn(DownloadProgress) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DownloadKind {
    Media,
    Artwork,
}

impl DownloadKind {
    fn validates(self, url: &Url) -> bool {
        match self {
            Self::Media => validate_media_url(url),
            Self::Artwork => validate_artwork_url(url),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Media => "媒体",
            Self::Artwork => "原图",
        }
    }
}

struct DownloadRequest<'a> {
    kind: DownloadKind,
    url: &'a Url,
    target_path: &'a Path,
    part_path: &'a Path,
    cancellation: &'a CancellationToken,
    skip_existing: bool,
    on_progress: ProgressCallback,
}

#[derive(Clone)]
pub struct DownloadClient {
    media_client: Client,
    artwork_client: Client,
}

impl DownloadClient {
    pub fn new(version: &str) -> AppResult<Self> {
        Ok(Self {
            media_client: build_client(version, DownloadKind::Media)?,
            artwork_client: build_client(version, DownloadKind::Artwork)?,
        })
    }

    pub async fn download(
        &self,
        media_url: &Url,
        target_path: &Path,
        part_path: &Path,
        cancellation: &CancellationToken,
        on_progress: ProgressCallback,
    ) -> AppResult<DownloadOutcome> {
        self.download_with_skip(
            media_url,
            target_path,
            part_path,
            cancellation,
            false,
            on_progress,
        )
        .await
    }

    pub async fn download_with_skip(
        &self,
        media_url: &Url,
        target_path: &Path,
        part_path: &Path,
        cancellation: &CancellationToken,
        skip_existing: bool,
        on_progress: ProgressCallback,
    ) -> AppResult<DownloadOutcome> {
        self.download_resource(DownloadRequest {
            kind: DownloadKind::Media,
            url: media_url,
            target_path,
            part_path,
            cancellation,
            skip_existing,
            on_progress,
        })
        .await
    }

    pub async fn download_artwork_with_skip(
        &self,
        artwork_url: &Url,
        target_path: &Path,
        part_path: &Path,
        cancellation: &CancellationToken,
        skip_existing: bool,
        on_progress: ProgressCallback,
    ) -> AppResult<DownloadOutcome> {
        self.download_resource(DownloadRequest {
            kind: DownloadKind::Artwork,
            url: artwork_url,
            target_path,
            part_path,
            cancellation,
            skip_existing,
            on_progress,
        })
        .await
    }

    async fn download_resource(&self, request: DownloadRequest<'_>) -> AppResult<DownloadOutcome> {
        let DownloadRequest {
            kind,
            url,
            target_path,
            part_path,
            cancellation,
            skip_existing,
            on_progress,
        } = request;
        self.validate_url(url, kind)?;
        ensure_same_parent(target_path, part_path)?;
        if let Some(parent) = part_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(file_error)?;
        }
        if skip_existing {
            if let (Ok(metadata), Some(remote_size)) = (
                tokio::fs::metadata(target_path).await,
                self.remote_size_for(url, kind).await?,
            ) {
                if metadata.len() == remote_size {
                    return Ok(DownloadOutcome {
                        bytes_received: remote_size,
                        bytes_total: Some(remote_size),
                    });
                }
            }
        }

        let mut last_error = None;
        for attempt in 0..MAX_ATTEMPTS {
            if cancellation.is_cancelled() {
                return Err(canceled_error());
            }
            match self
                .download_once(kind, url, part_path, cancellation, &on_progress)
                .await
            {
                Ok(outcome) => {
                    tokio::fs::rename(part_path, target_path)
                        .await
                        .map_err(file_error)?;
                    return Ok(outcome);
                }
                Err(error) if error.code == ErrorCode::Canceled => return Err(error),
                Err(error) if error.retryable && attempt + 1 < MAX_ATTEMPTS => {
                    last_error = Some(error);
                    let delay = Duration::from_secs(1 << attempt);
                    tokio::select! {
                        _ = cancellation.cancelled() => return Err(canceled_error()),
                        _ = tokio::time::sleep(delay) => {}
                    }
                }
                Err(error) => return Err(error),
            }
        }
        Err(last_error.unwrap_or_else(|| AppError::network("下载失败", true)))
    }

    pub async fn remote_size(&self, media_url: &Url) -> AppResult<Option<u64>> {
        self.remote_size_for(media_url, DownloadKind::Media).await
    }

    async fn remote_size_for(&self, url: &Url, kind: DownloadKind) -> AppResult<Option<u64>> {
        self.validate_url(url, kind)?;
        let response = self.client(kind).head(url.clone()).send().await;
        let Ok(response) = response else {
            return Ok(None);
        };
        if !response.status().is_success() {
            return Ok(None);
        }
        Ok(response.content_length())
    }

    async fn download_once(
        &self,
        kind: DownloadKind,
        url: &Url,
        part_path: &Path,
        cancellation: &CancellationToken,
        on_progress: &ProgressCallback,
    ) -> AppResult<DownloadOutcome> {
        let existing = match tokio::fs::metadata(part_path).await {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => return Err(file_error(error)),
        };

        let mut request = self.client(kind).get(url.clone());
        if existing > 0 {
            request = request.header(RANGE, format!("bytes={existing}-"));
        }
        let response = request.send();
        tokio::pin!(response);
        let response = tokio::select! {
            _ = cancellation.cancelled() => return Err(canceled_error()),
            response = &mut response => response
                .map_err(|error| AppError::network(format!("下载请求失败: {error}"), true))?,
        };
        let status = response.status();

        if status == StatusCode::RANGE_NOT_SATISFIABLE && existing > 0 {
            reset_part(part_path).await?;
            return Err(AppError::network("服务端拒绝续传，已重置临时文件", true));
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            let wait = retry_after_seconds(&response).unwrap_or(1).min(30);
            tokio::select! {
                _ = cancellation.cancelled() => return Err(canceled_error()),
                _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
            }
            return Err(AppError::rate_limited(format!(
                "站点限流，请在 {wait} 秒后重试"
            )));
        }
        if status.is_server_error() || status == StatusCode::REQUEST_TIMEOUT {
            return Err(AppError::network(
                format!("下载服务器返回 HTTP {status}"),
                true,
            ));
        }
        if status != StatusCode::OK && status != StatusCode::PARTIAL_CONTENT {
            return Err(AppError::network(
                format!("下载服务器返回 HTTP {status}"),
                false,
            ));
        }
        if kind == DownloadKind::Artwork
            && !response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.to_ascii_lowercase().starts_with("image/"))
        {
            return Err(AppError::network("原图响应不是图片内容", false));
        }

        let resumed = status == StatusCode::PARTIAL_CONTENT && existing > 0;
        let offset = if resumed {
            let range_start = response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_content_range_start)
                .ok_or_else(|| AppError::network("续传响应缺少有效 Content-Range", true))?;
            if range_start != existing {
                reset_part(part_path).await?;
                return Err(AppError::network(
                    "续传响应起点不匹配，已重置临时文件",
                    true,
                ));
            }
            existing
        } else {
            0
        };
        let bytes_total = response_total(&response, offset);

        let mut options = tokio::fs::OpenOptions::new();
        options.create(true).write(true);
        if resumed {
            options.append(true);
        } else {
            options.truncate(true);
        }
        let mut file = options.open(part_path).await.map_err(file_error)?;
        let mut bytes_received = offset;
        (on_progress)(DownloadProgress {
            bytes_received,
            bytes_total,
        });

        let mut stream = response.bytes_stream();
        loop {
            let next = tokio::select! {
                biased;
                _ = cancellation.cancelled() => return Err(canceled_error()),
                next = stream.next() => next,
            };
            let Some(chunk) = next else {
                break;
            };
            let chunk =
                chunk.map_err(|error| AppError::network(format!("下载数据中断: {error}"), true))?;
            file.write_all(&chunk).await.map_err(file_error)?;
            bytes_received = bytes_received
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| AppError::internal("下载字节计数溢出"))?;
            (on_progress)(DownloadProgress {
                bytes_received,
                bytes_total,
            });
        }
        file.flush().await.map_err(file_error)?;
        file.sync_all().await.map_err(file_error)?;
        drop(file);

        if bytes_total.is_some_and(|total| total != bytes_received) {
            return Err(AppError::network("下载数据长度与响应不一致", true));
        }
        Ok(DownloadOutcome {
            bytes_received,
            bytes_total,
        })
    }

    fn validate_url(&self, url: &Url, kind: DownloadKind) -> AppResult<()> {
        if kind.validates(url) {
            Ok(())
        } else {
            Err(AppError::invalid_url(format!(
                "{}链接不在允许的域名范围内",
                kind.name()
            )))
        }
    }

    fn client(&self, kind: DownloadKind) -> &Client {
        match kind {
            DownloadKind::Media => &self.media_client,
            DownloadKind::Artwork => &self.artwork_client,
        }
    }
}

fn build_client(version: &str, kind: DownloadKind) -> AppResult<Client> {
    Client::builder()
        .user_agent(format!("khinfetch/{version}"))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .redirect(Policy::custom(move |attempt| {
            if attempt.previous().len() >= 5 {
                return attempt.error(std::io::Error::other("too many download redirects"));
            }
            if kind.validates(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error(std::io::Error::other("download redirect is not allowed"))
            }
        }))
        .build()
        .map_err(|error| AppError::internal(format!("无法初始化下载客户端: {error}")))
}

fn response_total(response: &reqwest::Response, offset: u64) -> Option<u64> {
    if response.status() == StatusCode::PARTIAL_CONTENT {
        response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_content_range_total)
            .or_else(|| {
                response
                    .content_length()
                    .and_then(|length| offset.checked_add(length))
            })
    } else {
        response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
            .or_else(|| response.content_length())
    }
}

fn parse_content_range_start(value: &str) -> Option<u64> {
    value
        .strip_prefix("bytes ")?
        .split('-')
        .next()?
        .parse()
        .ok()
}

fn parse_content_range_total(value: &str) -> Option<u64> {
    value.rsplit('/').next()?.parse().ok()
}

fn retry_after_seconds(response: &reqwest::Response) -> Option<u64> {
    response
        .headers()
        .get(RETRY_AFTER)?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

fn ensure_same_parent(target_path: &Path, part_path: &Path) -> AppResult<()> {
    if target_path.parent() != part_path.parent() {
        return Err(AppError::internal("最终文件和 .part 文件必须位于同一目录"));
    }
    Ok(())
}

async fn reset_part(path: &Path) -> AppResult<()> {
    let file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .await
        .map_err(file_error)?;
    file.sync_all().await.map_err(file_error)
}

fn canceled_error() -> AppError {
    AppError::new(ErrorCode::Canceled, "下载已取消", false)
}

fn file_error(error: std::io::Error) -> AppError {
    let code = match error.raw_os_error() {
        Some(28 | 39 | 112) => ErrorCode::DiskFull,
        _ if error.kind() == std::io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
        _ => ErrorCode::Internal,
    };
    AppError::new(code, format!("文件操作失败: {error}"), false)
}
