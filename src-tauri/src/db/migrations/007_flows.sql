CREATE TABLE IF NOT EXISTS flows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL UNIQUE REFERENCES accounts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'idle',
    current_node TEXT CHECK (current_node IN ('start', 'record', 'clip', 'caption', 'upload')),
    last_live_at TEXT,
    last_run_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'))
);

CREATE TABLE IF NOT EXISTS flow_node_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
    node_key TEXT NOT NULL CHECK (node_key IN ('start', 'record', 'clip', 'caption', 'upload')),
    config_json TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
    UNIQUE(flow_id, node_key)
);

ALTER TABLE recordings ADD COLUMN flow_id INTEGER REFERENCES flows(id) ON DELETE SET NULL;

ALTER TABLE clips ADD COLUMN flow_id INTEGER REFERENCES flows(id) ON DELETE SET NULL;
ALTER TABLE clips ADD COLUMN caption_text TEXT;
ALTER TABLE clips ADD COLUMN caption_status TEXT NOT NULL DEFAULT 'pending' CHECK (caption_status IN ('pending', 'generating', 'completed', 'failed'));
ALTER TABLE clips ADD COLUMN caption_error TEXT;
ALTER TABLE clips ADD COLUMN caption_generated_at TEXT;

CREATE INDEX IF NOT EXISTS idx_flows_status ON flows(status);
CREATE INDEX IF NOT EXISTS idx_recordings_flow ON recordings(flow_id);
CREATE INDEX IF NOT EXISTS idx_clips_flow ON clips(flow_id);
CREATE INDEX IF NOT EXISTS idx_clips_caption_status ON clips(caption_status);
