-- Map sidecar UUID (WebSocket / FFmpeg pipeline) to local INTEGER recordings.id
ALTER TABLE recordings ADD COLUMN sidecar_recording_id TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_recordings_sidecar_recording_id
  ON recordings(sidecar_recording_id) WHERE sidecar_recording_id IS NOT NULL;

-- Idempotent clip inserts when the same file is reported again
CREATE UNIQUE INDEX IF NOT EXISTS idx_clips_recording_file
  ON clips(recording_id, file_path);
