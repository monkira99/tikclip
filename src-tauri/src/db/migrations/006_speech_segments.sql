CREATE TABLE IF NOT EXISTS speech_segments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recording_id INTEGER NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
    start_time REAL NOT NULL,
    end_time REAL NOT NULL,
    text TEXT NOT NULL DEFAULT '',
    confidence REAL,
    created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'))
);

CREATE INDEX IF NOT EXISTS idx_speech_segments_recording
  ON speech_segments(recording_id);

ALTER TABLE clips ADD COLUMN transcript_text TEXT;
