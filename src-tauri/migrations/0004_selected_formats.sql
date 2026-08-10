ALTER TABLE album_jobs
ADD COLUMN selected_formats_json TEXT NOT NULL DEFAULT '[]';
