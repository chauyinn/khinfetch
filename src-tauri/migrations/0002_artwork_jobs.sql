ALTER TABLE album_jobs
ADD COLUMN download_artwork INTEGER NOT NULL DEFAULT 0
CHECK (download_artwork IN (0, 1));

CREATE TABLE IF NOT EXISTS artwork_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    job_id TEXT NOT NULL REFERENCES album_jobs (id) ON DELETE CASCADE,
    source_url TEXT NOT NULL,
    output_directory TEXT NOT NULL,
    target_path TEXT,
    part_path TEXT,
    bytes_received INTEGER NOT NULL,
    bytes_total INTEGER,
    state TEXT NOT NULL,
    error_json TEXT,
    retry_count INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (job_id, source_url)
);

CREATE INDEX IF NOT EXISTS idx_artwork_jobs_job_id
ON artwork_jobs (job_id, created_at);
