use std::time::Duration;

use reqwest::header::{ACCEPT, RETRY_AFTER};
use reqwest::{redirect::Policy, Client, StatusCode};
use url::Url;

use crate::domain::{AlbumDetails, AlbumSummary, AppError, AppResult};

use super::parser::{
    parse_album_details, parse_download_variants, parse_search_results, ResolvedVariant,
};
use super::url_policy::{validate_html_url, validate_source, validate_source_url, SourceUrl};

const BASE_URL: &str = "https://downloads.khinsider.com/";
const MAX_HTML_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct KhinsiderClient {
    client: Client,
    base_url: Url,
    user_agent: String,
}

impl KhinsiderClient {
    pub fn new(version: &str) -> AppResult<Self> {
        let user_agent = format!("khinfetch/{version}");
        let client = Client::builder()
            .user_agent(&user_agent)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 {
                    return attempt.error(std::io::Error::other("too many HTML redirects"));
                }
                if validate_html_url(attempt.url()) {
                    attempt.follow()
                } else {
                    attempt.error(std::io::Error::other("HTML redirect is not allowed"))
                }
            }))
            .build()
            .map_err(|error| AppError::internal(format!("无法初始化 HTTP 客户端: {error}")))?;
        let base_url =
            Url::parse(BASE_URL).map_err(|error| AppError::internal(error.to_string()))?;
        Ok(Self {
            client,
            base_url,
            user_agent,
        })
    }

    pub async fn search(&self, query: &str) -> AppResult<Vec<AlbumSummary>> {
        let mut url = self
            .base_url
            .join("search")
            .map_err(|error| AppError::internal(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("search", query)
            .append_pair("albumListSize", "compact");
        let (final_url, html) = self.fetch_html(url).await?;
        parse_search_results(&html, &final_url).map_err(Into::into)
    }

    pub async fn album_details(&self, source: &SourceUrl) -> AppResult<AlbumDetails> {
        let (final_url, html) = self.fetch_html(source.url.clone()).await?;
        let final_source = validate_source(&final_url)?;
        parse_album_details(&html, &final_source.url, final_source.kind).map_err(Into::into)
    }

    pub(crate) async fn resolve_track_variants(
        &self,
        track_page_url: &str,
    ) -> AppResult<Vec<ResolvedVariant>> {
        let source = validate_source_url(track_page_url)?;
        let (final_url, html) = self.fetch_html(source.url).await?;
        validate_source(&final_url)?;
        parse_download_variants(&html, &final_url).map_err(Into::into)
    }

    async fn fetch_html(&self, url: Url) -> AppResult<(Url, String)> {
        if !validate_html_url(&url) {
            return Err(AppError::invalid_url("请求地址不属于 Khinsider"));
        }

        let mut last_error = None;
        for attempt in 0..=3 {
            let response = self
                .client
                .get(url.clone())
                .header(ACCEPT, "text/html,application/xhtml+xml")
                .header("User-Agent", &self.user_agent)
                .send()
                .await;

            let response = match response {
                Ok(response) => response,
                Err(error) if attempt < 3 && (error.is_timeout() || error.is_connect()) => {
                    last_error = Some(error.to_string());
                    tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                    continue;
                }
                Err(error) => {
                    return Err(AppError::network(format!("网络请求失败: {error}"), true))
                }
            };

            let status = response.status();
            if status == StatusCode::TOO_MANY_REQUESTS {
                if attempt < 3 {
                    let delay = response
                        .headers()
                        .get(RETRY_AFTER)
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse::<u64>().ok())
                        .unwrap_or(1 << attempt);
                    tokio::time::sleep(Duration::from_secs(delay.min(30))).await;
                    continue;
                }
                return Err(AppError::rate_limited("站点请求过于频繁，请稍后重试"));
            }
            if status.is_server_error() && attempt < 3 {
                last_error = Some(format!("HTTP {status}"));
                tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                continue;
            }
            if !status.is_success() {
                let retryable = status.is_server_error() || status == StatusCode::REQUEST_TIMEOUT;
                return Err(AppError::network(
                    format!("站点返回 HTTP {status}"),
                    retryable,
                ));
            }
            if response
                .content_length()
                .is_some_and(|length| length > MAX_HTML_BYTES)
            {
                return Err(AppError::parse_changed("页面内容超过允许大小"));
            }

            let final_url = response.url().clone();
            let bytes = response
                .bytes()
                .await
                .map_err(|error| AppError::network(format!("读取页面失败: {error}"), true))?;
            if bytes.len() as u64 > MAX_HTML_BYTES {
                return Err(AppError::parse_changed("页面内容超过允许大小"));
            }
            return Ok((final_url, String::from_utf8_lossy(&bytes).into_owned()));
        }

        Err(AppError::network(
            last_error.unwrap_or_else(|| "网络请求失败".to_string()),
            true,
        ))
    }
}
