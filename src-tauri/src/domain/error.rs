use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidUrl,
    Network,
    RateLimited,
    ParseChanged,
    PermissionDenied,
    DiskFull,
    Canceled,
    DuplicateJob,
    UnsupportedFormat,
    Storage,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl AppError {
    pub fn invalid_url(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidUrl, message, false)
    }

    pub fn network(message: impl Into<String>, retryable: bool) -> Self {
        Self::new(ErrorCode::Network, message, retryable)
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::RateLimited, message, true)
    }

    pub fn parse_changed(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ParseChanged, message, false)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message, false)
    }

    pub fn new(code: ErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }
}
