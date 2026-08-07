use std::{
    borrow::Cow,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{Duration, SystemTime},
};

use regex::{Captures, Regex};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{filter::LevelFilter, layer::SubscriberExt, Layer};
use url::Url;

const LOG_FILE_NAME: &str = "khinfetch.log";
const MAX_LOG_FILE_SIZE: u64 = 10 * 1024 * 1024;
const MAX_LOG_FILES: usize = 5;
const LOG_RETENTION: Duration = Duration::from_secs(14 * 24 * 60 * 60);

pub struct LoggingHandle {
    _guard: WorkerGuard,
}

struct SizeRotatingFile {
    path: PathBuf,
    file: Option<File>,
    bytes_written: u64,
    max_file_size: u64,
    max_files: usize,
}

impl SizeRotatingFile {
    fn new(log_directory: &Path, max_file_size: u64, max_files: usize) -> io::Result<Self> {
        let path = log_directory.join(LOG_FILE_NAME);
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let bytes_written = file.metadata()?.len();
        Ok(Self {
            path,
            file: Some(file),
            bytes_written,
            max_file_size,
            max_files,
        })
    }

    fn rotate(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.file.take() {
            file.flush()?;
        }
        rotate_log_files(&self.path, self.max_files)?;
        self.file = Some(
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&self.path)?,
        );
        self.bytes_written = 0;
        Ok(())
    }
}

impl Write for SizeRotatingFile {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if !buffer.is_empty()
            && self.bytes_written > 0
            && self.bytes_written.saturating_add(buffer.len() as u64) > self.max_file_size
        {
            self.rotate()?;
        }

        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("日志文件未打开"))?
            .write_all(buffer)?;
        self.bytes_written = self.bytes_written.saturating_add(buffer.len() as u64);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("日志文件未打开"))?
            .flush()
    }
}

pub fn initialize(log_directory: &Path) -> io::Result<LoggingHandle> {
    fs::create_dir_all(log_directory)?;
    cleanup_old_logs(log_directory);

    let file_appender = SizeRotatingFile::new(log_directory, MAX_LOG_FILE_SIZE, MAX_LOG_FILES)?;
    let (writer, guard) = tracing_appender::non_blocking(file_appender);

    let console_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(io::stderr)
        .with_filter(LevelFilter::INFO);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(writer)
        .with_filter(LevelFilter::INFO);

    tracing::subscriber::set_global_default(
        tracing_subscriber::registry()
            .with(console_layer)
            .with(file_layer),
    )
    .map_err(|error| io::Error::other(format!("无法初始化日志: {error}")))?;

    Ok(LoggingHandle { _guard: guard })
}

fn rotate_log_files(current: &Path, max_files: usize) -> io::Result<()> {
    if max_files <= 1 {
        if current.exists() {
            fs::remove_file(current)?;
        }
        return Ok(());
    }

    let oldest = numbered_log_path(current, max_files - 1);
    if oldest.exists() {
        fs::remove_file(oldest)?;
    }
    for generation in (1..max_files - 1).rev() {
        let source = numbered_log_path(current, generation);
        if source.exists() {
            fs::rename(source, numbered_log_path(current, generation + 1))?;
        }
    }
    if current.exists() {
        fs::rename(current, numbered_log_path(current, 1))?;
    }
    Ok(())
}

fn numbered_log_path(current: &Path, generation: usize) -> PathBuf {
    current.with_file_name(format!("{LOG_FILE_NAME}.{generation}"))
}

pub fn redact(value: &str) -> Cow<'_, str> {
    let with_urls = url_regex().replace_all(value, |captures: &Captures<'_>| {
        let candidate = captures.get(0).map_or("", |matched| matched.as_str());
        match Url::parse(candidate) {
            Ok(url) => match url.host_str() {
                Some(host) => format!("{}://{host}/[redacted]", url.scheme()),
                None => "[redacted-url]".to_owned(),
            },
            Err(_) => "[redacted-url]".to_owned(),
        }
    });
    let with_windows_paths = windows_path_regex().replace_all(&with_urls, "[redacted-path]");
    let with_unix_paths = unix_path_regex().replace_all(&with_windows_paths, "[redacted-path]");

    if with_unix_paths == value {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(with_unix_paths.into_owned())
    }
}

fn url_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r#"https?://[^\s\"'<>]+"#).expect("valid URL redaction regex"))
}

fn windows_path_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?i)\b[A-Z]:\\[^\r\n\t\"']+"#).expect("valid Windows path redaction regex")
    })
}

fn unix_path_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?:/home|/Users|/tmp|/var/tmp|/mnt)/[^\s\"']+"#)
            .expect("valid Unix path redaction regex")
    })
}

fn cleanup_old_logs(log_directory: &Path) {
    let cutoff = SystemTime::now().checked_sub(LOG_RETENTION);
    let Ok(entries) = fs::read_dir(log_directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let log_suffix = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix(&format!("{LOG_FILE_NAME}.")));
        let Some(log_suffix) = log_suffix else {
            continue;
        };
        let exceeds_file_limit = log_suffix
            .parse::<usize>()
            .is_ok_and(|generation| generation >= MAX_LOG_FILES);
        let is_expired = cutoff.is_some_and(|cutoff| {
            entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .is_ok_and(|modified| modified < cutoff)
        });
        if exceeds_file_limit || is_expired {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_by_size_and_removes_the_oldest_file() {
        let directory = std::env::temp_dir().join(format!(
            "khinfetch-logging-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("system time after unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&directory).expect("create test log directory");

        let mut writer = SizeRotatingFile::new(&directory, 5, 3).expect("create log writer");
        writer.write_all(b"12345").expect("write current log");
        writer.write_all(b"678").expect("rotate once");
        writer.write_all(b"abcd").expect("rotate twice");
        writer
            .write_all(b"efghi")
            .expect("rotate and remove oldest");
        writer.flush().expect("flush current log");

        assert_eq!(fs::read(directory.join(LOG_FILE_NAME)).unwrap(), b"efghi");
        assert_eq!(
            fs::read(directory.join("khinfetch.log.1")).unwrap(),
            b"abcd"
        );
        assert_eq!(fs::read(directory.join("khinfetch.log.2")).unwrap(), b"678");
        assert!(!directory.join("khinfetch.log.3").exists());

        drop(writer);
        fs::remove_dir_all(directory).expect("remove test log directory");
    }
}
