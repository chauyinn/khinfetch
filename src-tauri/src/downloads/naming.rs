use std::path::{Path, PathBuf};

use percent_encoding::percent_decode_str;
use url::Url;

const MAX_COMPONENT_CHARS: usize = 120;

pub fn sanitize_component(input: &str) -> String {
    let mut value: String = input
        .trim()
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .take(MAX_COMPONENT_CHARS)
        .collect();
    value = value.trim_end_matches([' ', '.']).to_owned();
    if value.is_empty() {
        value.push_str("untitled");
    }

    let stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0');
    if reserved {
        value.insert(0, '_');
    }
    value
}

pub fn track_filename(
    disc_number: Option<u32>,
    index: Option<u32>,
    title: &str,
    extension: &str,
) -> String {
    let title = sanitize_component(title);
    let extension = extension
        .trim_start_matches('.')
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
        .to_ascii_lowercase();
    let extension = if extension.is_empty() {
        "bin"
    } else {
        &extension
    };
    match index {
        Some(index) => {
            let disc_number = disc_number.unwrap_or(1);
            format!("{disc_number:02}-{index:02} - {title}.{extension}")
        }
        _ => format!("{title}.{extension}"),
    }
}

pub fn artwork_filename(index: usize, url: &Url) -> String {
    let encoded = url
        .path_segments()
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
        .unwrap_or("artwork");
    let decoded = percent_decode_str(encoded).decode_utf8_lossy();
    let path = Path::new(decoded.as_ref());
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(sanitize_component)
        .unwrap_or_else(|| "artwork".to_owned());
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("bin")
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
        .to_ascii_lowercase();
    format!("{:03} - {stem}.{extension}", index + 1)
}

pub async fn unique_target_path(path: &Path) -> std::io::Result<PathBuf> {
    if tokio::fs::try_exists(path).await? {
        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("download");
        let extension = path.extension().and_then(|value| value.to_str());
        for number in 2..=10_000 {
            let filename = match extension {
                Some(extension) => format!("{stem} ({number}).{extension}"),
                None => format!("{stem} ({number})"),
            };
            let candidate = parent.join(filename);
            if !tokio::fs::try_exists(&candidate).await? {
                return Ok(candidate);
            }
        }
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "unable to allocate a unique filename",
        ));
    }
    Ok(path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::track_filename;

    #[test]
    fn track_filename_includes_disc_and_track_numbers() {
        assert_eq!(
            track_filename(Some(2), Some(3), "Test Track", "FLAC"),
            "02-03 - Test Track.flac"
        );
        assert_eq!(
            track_filename(Some(1), Some(3), "Test Track", "FLAC"),
            "01-03 - Test Track.flac"
        );
        assert_eq!(
            track_filename(None, Some(3), "Test Track", "FLAC"),
            "01-03 - Test Track.flac"
        );
    }
}
