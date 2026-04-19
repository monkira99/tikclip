PRAGMA foreign_keys = OFF;

ALTER TABLE recordings RENAME TO recordings_legacy_009;
ALTER TABLE clips RENAME TO clips_legacy_009;
ALTER TABLE clip_products RENAME TO clip_products_legacy_009;
ALTER TABLE notifications RENAME TO notifications_legacy_009;
ALTER TABLE speech_segments RENAME TO speech_segments_legacy_009;

CREATE TABLE recordings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    room_id TEXT,
    status TEXT NOT NULL DEFAULT 'recording' CHECK (status IN ('recording', 'done', 'error', 'processing', 'cancelled')),
    started_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
    ended_at TEXT,
    duration_seconds INTEGER NOT NULL DEFAULT 0,
    file_path TEXT,
    file_size_bytes INTEGER NOT NULL DEFAULT 0,
    stream_url TEXT,
    bitrate TEXT,
    error_message TEXT,
    auto_process INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
    flow_id INTEGER REFERENCES flows(id) ON DELETE SET NULL,
    flow_run_id INTEGER REFERENCES flow_runs(id) ON DELETE SET NULL,
    sidecar_recording_id TEXT
);

CREATE TABLE clips (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recording_id INTEGER NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    title TEXT,
    file_path TEXT NOT NULL,
    thumbnail_path TEXT,
    duration_seconds INTEGER NOT NULL DEFAULT 0,
    file_size_bytes INTEGER NOT NULL DEFAULT 0,
    start_time REAL NOT NULL DEFAULT 0,
    end_time REAL NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'ready', 'posted', 'archived')),
    quality_score REAL,
    scene_type TEXT CHECK (scene_type IN ('product_intro', 'highlight', 'general')),
    ai_tags_json TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
    flow_id INTEGER REFERENCES flows(id) ON DELETE SET NULL,
    caption_text TEXT,
    caption_status TEXT NOT NULL DEFAULT 'pending' CHECK (caption_status IN ('pending', 'generating', 'completed', 'failed')),
    caption_error TEXT,
    caption_generated_at TEXT,
    transcript_text TEXT,
    flow_run_id INTEGER REFERENCES flow_runs(id) ON DELETE SET NULL
);

CREATE TABLE clip_products (
    clip_id INTEGER NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
    product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    PRIMARY KEY (clip_id, product_id)
);

CREATE TABLE notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    type TEXT NOT NULL,
    title TEXT NOT NULL,
    message TEXT NOT NULL DEFAULT '',
    account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
    recording_id INTEGER REFERENCES recordings(id) ON DELETE SET NULL,
    clip_id INTEGER REFERENCES clips(id) ON DELETE SET NULL,
    is_read INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'))
);

CREATE TABLE speech_segments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recording_id INTEGER NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
    start_time REAL NOT NULL,
    end_time REAL NOT NULL,
    text TEXT NOT NULL DEFAULT '',
    confidence REAL,
    created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'))
);

INSERT INTO recordings (
    id, account_id, room_id, status, started_at, ended_at, duration_seconds,
    file_path, file_size_bytes, stream_url, bitrate, error_message, auto_process,
    created_at, flow_id, flow_run_id, sidecar_recording_id
)
SELECT
    id, account_id, room_id, status, started_at, ended_at, duration_seconds,
    file_path, file_size_bytes, stream_url, bitrate, error_message, auto_process,
    created_at, flow_id, flow_run_id, sidecar_recording_id
FROM recordings_legacy_009;

INSERT INTO clips (
    id, recording_id, account_id, title, file_path, thumbnail_path, duration_seconds,
    file_size_bytes, start_time, end_time, status, quality_score, scene_type,
    ai_tags_json, notes, created_at, updated_at, flow_id, caption_text,
    caption_status, caption_error, caption_generated_at, transcript_text, flow_run_id
)
SELECT
    id, recording_id, account_id, title, file_path, thumbnail_path, duration_seconds,
    file_size_bytes, start_time, end_time, status, quality_score, scene_type,
    ai_tags_json, notes, created_at, updated_at, flow_id, caption_text,
    caption_status, caption_error, caption_generated_at, transcript_text, flow_run_id
FROM clips_legacy_009;

INSERT INTO clip_products (clip_id, product_id)
SELECT clip_id, product_id FROM clip_products_legacy_009;

INSERT INTO notifications (id, type, title, message, account_id, recording_id, clip_id, is_read, created_at)
SELECT id, type, title, message, account_id, recording_id, clip_id, is_read, created_at
FROM notifications_legacy_009;

INSERT INTO speech_segments (id, recording_id, start_time, end_time, text, confidence, created_at)
SELECT id, recording_id, start_time, end_time, text, confidence, created_at
FROM speech_segments_legacy_009;

DROP TABLE speech_segments_legacy_009;
DROP TABLE notifications_legacy_009;
DROP TABLE clip_products_legacy_009;
DROP TABLE clips_legacy_009;
DROP TABLE recordings_legacy_009;

CREATE INDEX IF NOT EXISTS idx_recordings_account ON recordings(account_id);
CREATE INDEX IF NOT EXISTS idx_recordings_status ON recordings(status);
CREATE INDEX IF NOT EXISTS idx_recordings_flow ON recordings(flow_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_recordings_sidecar_recording_id
  ON recordings(sidecar_recording_id) WHERE sidecar_recording_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_clips_recording ON clips(recording_id);
CREATE INDEX IF NOT EXISTS idx_clips_account ON clips(account_id);
CREATE INDEX IF NOT EXISTS idx_clips_status ON clips(status);
CREATE INDEX IF NOT EXISTS idx_clips_flow ON clips(flow_id);
CREATE INDEX IF NOT EXISTS idx_clips_caption_status ON clips(caption_status);
CREATE UNIQUE INDEX IF NOT EXISTS idx_clips_recording_file
  ON clips(recording_id, file_path);
CREATE INDEX IF NOT EXISTS idx_notifications_read ON notifications(is_read);
CREATE INDEX IF NOT EXISTS idx_speech_segments_recording
  ON speech_segments(recording_id);

PRAGMA foreign_keys = ON;
