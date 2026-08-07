use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use url::Url;
use uuid::Uuid;

use crate::domain::{AlbumDetails, AlbumSummary, AppError, SourceKind, TrackSummary};

use super::url_policy::{validate_artwork_url, validate_media_url};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum ParseError {
    #[error("required page structure was not found: {0}")]
    Structure(&'static str),
    #[error("invalid link in page: {0}")]
    Link(String),
}

impl From<ParseError> for AppError {
    fn from(error: ParseError) -> Self {
        AppError::parse_changed(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedVariant {
    pub format: String,
    pub media_url: Url,
    pub size_bytes: Option<u64>,
}

pub(crate) fn parse_search_results(
    html: &str,
    base_url: &Url,
) -> Result<Vec<AlbumSummary>, ParseError> {
    let document = Html::parse_document(html);
    let row_selector = selector("table.albumList tr")?;
    let cell_selector = selector("td")?;
    let anchor_selector = selector("a[href]")?;
    let image_selector = selector("img[src]")?;
    let mut results = Vec::new();

    for row in document.select(&row_selector).skip(1) {
        let cells: Vec<_> = row.select(&cell_selector).collect();
        if cells.len() < 5 {
            continue;
        }

        let Some(album_anchor) = row.select(&anchor_selector).find(|anchor| {
            anchor.value().attr("href").is_some_and(is_album_path) && !text(*anchor).is_empty()
        }) else {
            continue;
        };
        let href = album_anchor
            .value()
            .attr("href")
            .ok_or(ParseError::Structure("album href"))?;
        let source_url = base_url
            .join(href)
            .map_err(|error| ParseError::Link(error.to_string()))?;
        let source_url = source_url.to_string();
        let title = text(album_anchor);
        if title.is_empty() {
            continue;
        }

        let cover_url = cells[0]
            .select(&image_selector)
            .find_map(|image| image.value().attr("src"))
            .and_then(|url| base_url.join(url).ok())
            .filter(validate_artwork_url)
            .map(|url| url.to_string());
        let platforms = cells[2]
            .select(&anchor_selector)
            .map(text)
            .filter(|value| !value.is_empty())
            .collect();
        let album_type = non_empty(text(cells[3]));
        let year = non_empty(text(cells[4]));

        results.push(AlbumSummary {
            source_url,
            title,
            cover_url,
            year,
            platforms,
            album_type,
            source_kind: SourceKind::Album,
        });
    }

    if results.is_empty() && document.select(&row_selector).next().is_none() {
        return Err(ParseError::Structure("album search table"));
    }
    Ok(results)
}

pub(crate) fn parse_album_details(
    html: &str,
    source_url: &Url,
    kind: SourceKind,
) -> Result<AlbumDetails, ParseError> {
    let document = Html::parse_document(html);
    let title_selector = selector("#pageContent > h2")?;
    let title = document
        .select(&title_selector)
        .map(text)
        .find(|value| !value.is_empty())
        .ok_or(ParseError::Structure("album title"))?;

    let metadata_selector = selector("#pageContent > p")?;
    let metadata = document
        .select(&metadata_selector)
        .map(text)
        .find(|value| value.contains("Platforms:") || value.contains("Year:"));
    let platforms = metadata
        .as_deref()
        .map(|value| split_label(value, "Platforms:", &metadata_labels()))
        .unwrap_or_default();
    let year = metadata
        .as_deref()
        .and_then(|value| non_empty(split_label(value, "Year:", &metadata_labels()).join(", ")));
    let publisher = metadata
        .as_deref()
        .and_then(|value| single_label(value, "Published by:", &metadata_labels()));
    let developer = metadata
        .as_deref()
        .and_then(|value| single_label(value, "Developed by:", &metadata_labels()));
    let album_type = metadata
        .as_deref()
        .and_then(|value| single_label(value, "Album type:", &metadata_labels()));

    let artwork_urls = parse_artwork(&document, source_url);
    let cover_url =
        parse_cover_preview(&document, source_url).or_else(|| artwork_urls.first().cloned());
    let available_formats = parse_formats(&document);
    let tracks = parse_tracks(&document, source_url, &available_formats)?;
    let summary = AlbumSummary {
        source_url: source_url.to_string(),
        title,
        cover_url,
        year,
        platforms,
        album_type,
        source_kind: kind,
    };

    Ok(AlbumDetails {
        summary,
        publisher,
        developer,
        artwork_urls,
        tracks,
        available_formats,
    })
}

pub(crate) fn parse_download_variants(
    html: &str,
    _track_page_url: &Url,
) -> Result<Vec<ResolvedVariant>, ParseError> {
    let document = Html::parse_document(html);
    let anchor_selector = selector("#pageContent a[href], a[href]")?;
    let mut variants = Vec::new();

    for anchor in document.select(&anchor_selector) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        let Ok(url) = Url::parse(href) else {
            continue;
        };
        if !validate_media_url(&url) {
            continue;
        }
        let Some(format) = media_format_from_anchor(anchor, &url) else {
            continue;
        };
        if variants
            .iter()
            .any(|variant: &ResolvedVariant| variant.media_url == url)
        {
            continue;
        }
        variants.push(ResolvedVariant {
            format,
            media_url: url,
            size_bytes: parse_size_bytes(&text(anchor)),
        });
    }

    if variants.is_empty() {
        return Err(ParseError::Structure("download variant"));
    }
    Ok(variants)
}

fn selector(value: &str) -> Result<Selector, ParseError> {
    Selector::parse(value).map_err(|_| ParseError::Structure("CSS selector"))
}

fn text(element: ElementRef<'_>) -> String {
    normalize_text(&element.text().collect::<Vec<_>>().join(" "))
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn non_empty(value: String) -> Option<String> {
    let value = normalize_text(&value);
    (!value.is_empty()).then_some(value)
}

fn is_album_path(href: &str) -> bool {
    href.starts_with("/game-soundtracks/album/") || href.contains("/game-soundtracks/album/")
}

fn metadata_labels() -> Vec<&'static str> {
    vec![
        "Platforms:",
        "Year:",
        "Published by:",
        "Developed by:",
        "Number of Files:",
        "Total Filesize:",
        "Date Added:",
        "Album type:",
        "Uploaded by:",
    ]
}

fn label_value<'a>(value: &'a str, label: &str, labels: &[&str]) -> Option<&'a str> {
    let start = value.find(label)? + label.len();
    let remaining = &value[start..];
    let end = labels
        .iter()
        .filter(|candidate| **candidate != label)
        .filter_map(|candidate| remaining.find(candidate))
        .min()
        .unwrap_or(remaining.len());
    Some(remaining[..end].trim())
}

fn split_label(value: &str, label: &str, labels: &[&str]) -> Vec<String> {
    label_value(value, label, labels)
        .unwrap_or_default()
        .split(',')
        .map(normalize_text)
        .filter(|part| !part.is_empty())
        .collect()
}

fn single_label(value: &str, label: &str, labels: &[&str]) -> Option<String> {
    non_empty(
        label_value(value, label, labels)
            .unwrap_or_default()
            .to_string(),
    )
}

fn parse_artwork(document: &Html, source_url: &Url) -> Vec<String> {
    let selectors = [
        "[class*='albumImage'] a[href]",
        "[class*='albumImage'] img[src]",
    ];
    let mut urls = Vec::new();
    for selector_text in selectors {
        let Ok(selector) = Selector::parse(selector_text) else {
            continue;
        };
        for element in document.select(&selector) {
            let attribute = if element.value().name() == "img" {
                "src"
            } else {
                "href"
            };
            let Some(value) = element.value().attr(attribute) else {
                continue;
            };
            let Ok(url) = source_url.join(value) else {
                continue;
            };
            if !validate_artwork_url(&url) {
                continue;
            }
            if !urls.contains(&url.to_string()) {
                urls.push(url.to_string());
            }
        }
        // Anchors point at the original files. Images are thumbnail fallbacks
        // only for pages that do not link original artwork.
        if !urls.is_empty() {
            break;
        }
    }
    urls
}

fn parse_cover_preview(document: &Html, source_url: &Url) -> Option<String> {
    let selector = Selector::parse("[class*='albumImage'] img[src]").ok()?;
    document.select(&selector).find_map(|element| {
        let value = element.value().attr("src")?;
        let url = source_url.join(value).ok()?;
        validate_artwork_url(&url).then(|| url.to_string())
    })
}

fn parse_formats(document: &Html) -> Vec<String> {
    let Ok(selector) = Selector::parse("#songlist_header th") else {
        return Vec::new();
    };
    document
        .select(&selector)
        .map(text)
        .map(|value| value.to_ascii_uppercase())
        .filter(|value| ["MP3", "FLAC", "ALAC", "AAC", "M4A", "OGG"].contains(&value.as_str()))
        .fold(Vec::new(), |mut formats, format| {
            if !formats.contains(&format) {
                formats.push(format);
            }
            formats
        })
}

fn parse_tracks(
    document: &Html,
    source_url: &Url,
    available_formats: &[String],
) -> Result<Vec<TrackSummary>, ParseError> {
    let row_selector = selector("#songlist tr")?;
    let cell_selector = selector("td")?;
    let anchor_selector = selector("a[href]")?;
    let duration_re =
        Regex::new(r"^(\d+):(\d{2})$").map_err(|_| ParseError::Structure("duration regex"))?;
    let columns = parse_track_columns(document)?;
    let mut tracks = Vec::new();

    for row in document.select(&row_selector) {
        if row
            .value()
            .attr("id")
            .is_some_and(|id| id.contains("songlist_"))
        {
            continue;
        }
        let Some(track_anchor) = row
            .select(&anchor_selector)
            .find(|anchor| anchor.value().attr("href").is_some_and(is_album_path))
        else {
            continue;
        };
        let href = track_anchor
            .value()
            .attr("href")
            .ok_or(ParseError::Structure("track href"))?;
        let track_url = source_url
            .join(href)
            .map_err(|error| ParseError::Link(error.to_string()))?;
        let cells: Vec<_> = row.select(&cell_selector).collect();
        let disc_number = parse_track_number(&cells, columns.disc_number);
        let index = parse_track_number(&cells, columns.index);
        let duration_seconds = row.select(&cell_selector).map(text).find_map(|value| {
            let captures = duration_re.captures(&value)?;
            let minutes: u32 = captures.get(1)?.as_str().parse().ok()?;
            let seconds: u32 = captures.get(2)?.as_str().parse().ok()?;
            (seconds < 60).then_some(minutes * 60 + seconds)
        });
        let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, track_url.as_str().as_bytes()).to_string();
        tracks.push(TrackSummary {
            id,
            disc_number,
            index,
            title: text(track_anchor),
            duration_seconds,
            track_page_url: track_url.to_string(),
            available_formats: available_formats.to_vec(),
        });
    }
    Ok(tracks)
}

#[derive(Default)]
struct TrackColumns {
    disc_number: Option<usize>,
    index: Option<usize>,
}

fn parse_track_columns(document: &Html) -> Result<TrackColumns, ParseError> {
    let header_selector = selector("#songlist_header th")?;
    let mut columns = TrackColumns::default();
    let mut column_index = 0;

    for header in document.select(&header_selector) {
        match text(header).trim().to_ascii_uppercase().as_str() {
            "CD" => columns.disc_number = Some(column_index),
            "#" => columns.index = Some(column_index),
            _ => {}
        }
        let colspan = header
            .value()
            .attr("colspan")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(1);
        column_index = column_index.saturating_add(colspan);
    }

    Ok(columns)
}

fn parse_track_number(cells: &[ElementRef<'_>], column: Option<usize>) -> Option<u32> {
    let cell = cells.get(column?)?;
    text(*cell).trim().trim_end_matches('.').parse().ok()
}

fn media_format(url: &Url) -> Option<String> {
    let extension = url.path().rsplit('.').next()?.to_ascii_uppercase();
    ["MP3", "FLAC", "M4A", "AAC", "OGG"]
        .contains(&extension.as_str())
        .then_some(extension)
}

fn media_format_from_anchor(anchor: ElementRef<'_>, url: &Url) -> Option<String> {
    let label = text(anchor).to_ascii_uppercase();
    for format in ["FLAC", "ALAC", "AAC", "MP3"] {
        if label.contains(format) {
            return Some(format.to_owned());
        }
    }
    media_format(url)
}

fn parse_size_bytes(value: &str) -> Option<u64> {
    let value = normalize_text(value);
    let mut parts = value.split_whitespace();
    let amount: f64 = parts.next()?.parse().ok()?;
    let unit = parts.next()?.to_ascii_uppercase();
    let multiplier = match unit.as_str() {
        "KB" => 1024.0,
        "MB" => 1024.0 * 1024.0,
        "GB" => 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((amount * multiplier) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_search_results() {
        let rows = (0..51)
            .map(|index| {
                format!(
                    r#"<tr>
                      <td><img src="https://images.khinsider.com/image{index}.jpg"></td>
                      <td><a href="/game-soundtracks/album/album-{index}">Album {index}</a></td>
                      <td><a href="/game-soundtracks/browse/Platform">Platform</a></td>
                      <td>Soundtrack</td>
                      <td>2026</td>
                    </tr>"#
                )
            })
            .collect::<String>();
        let html = format!(r#"<table class="albumList"><tr><th>Header</th></tr>{rows}</table>"#);
        let base_url = Url::parse("https://downloads.khinsider.com/search?search=album")
            .expect("valid search URL");

        let results = parse_search_results(&html, &base_url).expect("parse search results");

        assert_eq!(results.len(), 51);
    }

    #[test]
    fn parses_disc_and_track_columns_from_songlist_header() {
        let document = Html::parse_document(
            r#"
            <table id="songlist">
              <tr id="songlist_header">
                <th></th><th>CD</th><th>#</th><th colspan="2">Song Name</th>
                <th>MP3</th><th>FLAC</th>
              </tr>
              <tr>
                <td></td><td>2</td><td>3.</td>
                <td><a href="/game-soundtracks/album/example/2-03.track.mp3">Track</a></td>
                <td>1:05</td><td>1 MB</td><td>2 MB</td>
              </tr>
            </table>
            "#,
        );
        let source_url =
            Url::parse("https://downloads.khinsider.com/game-soundtracks/album/example")
                .expect("valid source URL");

        let tracks =
            parse_tracks(&document, &source_url, &["MP3".to_owned()]).expect("parse track list");

        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].disc_number, Some(2));
        assert_eq!(tracks[0].index, Some(3));
    }
}
