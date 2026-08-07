ALTER TABLE track_jobs
ADD COLUMN disc_number INTEGER;

ALTER TABLE album_jobs
ADD COLUMN selected_track_titles_json TEXT NOT NULL DEFAULT '[]';

UPDATE album_jobs
SET selected_track_titles_json = COALESCE(
    (
        SELECT json_group_array(title)
        FROM track_jobs
        WHERE track_jobs.job_id = album_jobs.id
    ),
    '[]'
);
