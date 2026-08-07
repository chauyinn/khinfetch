CREATE TABLE IF NOT EXISTS app_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    download_directory TEXT NOT NULL,
    concurrency INTEGER NOT NULL CHECK (concurrency BETWEEN 1 AND 8),
    preferred_quality TEXT NOT NULL,
    download_artwork INTEGER NOT NULL CHECK (download_artwork IN (0, 1)),
    skip_verified_files INTEGER NOT NULL CHECK (skip_verified_files IN (0, 1)),
    logging_enabled INTEGER NOT NULL CHECK (logging_enabled IN (0, 1)),
    theme TEXT NOT NULL,
    language TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO app_settings (
    id,
    download_directory,
    concurrency,
    preferred_quality,
    download_artwork,
    skip_verified_files,
    logging_enabled,
    theme,
    language,
    updated_at
) VALUES (1, '', 4, 'bestAvailable', 0, 1, 1, 'system', 'zh-cn', '0');

CREATE TABLE IF NOT EXISTS album_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    source_url TEXT NOT NULL,
    title TEXT NOT NULL,
    cover_url TEXT,
    year TEXT,
    platforms_json TEXT NOT NULL,
    album_type TEXT,
    source_kind TEXT NOT NULL,
    quality TEXT NOT NULL,
    state TEXT NOT NULL,
    selected_track_ids_json TEXT NOT NULL,
    total_tracks INTEGER NOT NULL,
    completed_tracks INTEGER NOT NULL,
    bytes_received INTEGER NOT NULL,
    bytes_total INTEGER,
    current_track_title TEXT,
    error_json TEXT,
    sequence INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_album_jobs_updated_at ON album_jobs (updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_album_jobs_source_url ON album_jobs (source_url);

CREATE TABLE IF NOT EXISTS track_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    job_id TEXT NOT NULL REFERENCES album_jobs (id) ON DELETE CASCADE,
    track_index INTEGER,
    title TEXT NOT NULL,
    duration_seconds INTEGER,
    track_page_url TEXT NOT NULL,
    resolved_media_url TEXT,
    available_formats_json TEXT NOT NULL,
    output_directory TEXT NOT NULL,
    target_path TEXT,
    part_path TEXT,
    bytes_received INTEGER NOT NULL,
    bytes_total INTEGER,
    state TEXT NOT NULL,
    error_json TEXT,
    retry_count INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_track_jobs_job_id ON track_jobs (job_id, track_index);
