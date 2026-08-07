use url::Url;

use crate::domain::{AppError, AppResult, SourceKind};

const KHINSIDER_HOST: &str = "downloads.khinsider.com";
const KHINSIDER_IMAGES_HOST: &str = "images.khinsider.com";
const MEDIA_HOST_SUFFIXES: [&str; 2] = ["vgmtreasurechest.com", "vgmsite.com"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceUrl {
    pub url: Url,
    pub kind: SourceKind,
}

pub fn validate_source_url(input: &str) -> AppResult<SourceUrl> {
    let url =
        Url::parse(input.trim()).map_err(|_| AppError::invalid_url("请输入有效的 HTTPS 链接"))?;
    validate_source(&url)
}

pub fn validate_source(url: &Url) -> AppResult<SourceUrl> {
    if !validate_html_url(url) {
        return Err(AppError::invalid_url(
            "只支持 downloads.khinsider.com 的 HTTPS 专辑或播放列表链接",
        ));
    }

    let path = url.path().trim_end_matches('/');
    let kind = if path.starts_with("/game-soundtracks/album/")
        && path.len() > "/game-soundtracks/album/".len()
    {
        SourceKind::Album
    } else if path.starts_with("/playlist/") && path.len() > "/playlist/".len() {
        SourceKind::Playlist
    } else {
        return Err(AppError::invalid_url(
            "链接必须指向 Khinsider 专辑或播放列表",
        ));
    };

    let mut normalized = url.clone();
    normalized.set_fragment(None);
    Ok(SourceUrl {
        url: normalized,
        kind,
    })
}

pub(crate) fn validate_html_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some(KHINSIDER_HOST)
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
}

pub fn validate_media_url(url: &Url) -> bool {
    if url.scheme() != "https"
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return false;
    }

    let Some(host) = url.host_str() else {
        return false;
    };
    let host_allowed = host == KHINSIDER_HOST
        || MEDIA_HOST_SUFFIXES
            .iter()
            .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")));
    if !host_allowed {
        return false;
    }

    let path = url.path().to_ascii_lowercase();
    [".mp3", ".flac", ".m4a", ".aac", ".ogg"]
        .iter()
        .any(|extension| path.ends_with(extension))
}

pub(crate) fn validate_artwork_url(url: &Url) -> bool {
    if url.scheme() != "https"
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return false;
    }

    let Some(host) = url.host_str() else {
        return false;
    };
    let host_allowed = host == KHINSIDER_HOST
        || host == KHINSIDER_IMAGES_HOST
        || MEDIA_HOST_SUFFIXES
            .iter()
            .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")));
    if !host_allowed {
        return false;
    }

    let path = url.path().to_ascii_lowercase();
    [".jpg", ".jpeg", ".png", ".webp", ".gif"]
        .iter()
        .any(|extension| path.ends_with(extension))
}
