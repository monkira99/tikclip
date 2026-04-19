PRAGMA foreign_keys = OFF;

ALTER TABLE flows RENAME TO flows_legacy;
ALTER TABLE flow_node_configs RENAME TO flow_node_configs_legacy;

CREATE TABLE flows (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  status TEXT NOT NULL DEFAULT 'idle' CHECK (status IN ('idle', 'watching', 'recording', 'processing', 'error', 'disabled')),
  current_node TEXT CHECK (current_node IN ('start', 'record', 'clip', 'caption', 'upload')),
  published_version INTEGER NOT NULL DEFAULT 1,
  draft_version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'))
);

CREATE TABLE flow_nodes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
  node_key TEXT NOT NULL CHECK (node_key IN ('start', 'record', 'clip', 'caption', 'upload')),
  position INTEGER NOT NULL,
  draft_config_json TEXT NOT NULL DEFAULT '{}',
  published_config_json TEXT NOT NULL DEFAULT '{}',
  draft_updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
  published_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
  UNIQUE(flow_id, node_key)
);

CREATE TABLE flow_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
  definition_version INTEGER NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled')),
  started_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
  ended_at TEXT,
  trigger_reason TEXT,
  error TEXT
);

CREATE TABLE flow_node_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flow_run_id INTEGER NOT NULL REFERENCES flow_runs(id) ON DELETE CASCADE,
  flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
  node_key TEXT NOT NULL CHECK (node_key IN ('start', 'record', 'clip', 'caption', 'upload')),
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'skipped', 'cancelled')),
  started_at TEXT,
  ended_at TEXT,
  input_json TEXT,
  output_json TEXT,
  error TEXT
);

CREATE INDEX IF NOT EXISTS idx_flows_status ON flows(status);
CREATE INDEX IF NOT EXISTS idx_flow_runs_flow ON flow_runs(flow_id);

INSERT INTO flows (id, name, enabled, status, current_node, published_version, draft_version, created_at, updated_at)
SELECT
  id,
  name,
  enabled,
  CASE
    WHEN status IN ('idle', 'watching', 'recording', 'processing', 'error', 'disabled') THEN status
    ELSE 'idle'
  END,
  current_node,
  1,
  1,
  created_at,
  updated_at
FROM flows_legacy;

INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json)
SELECT
  f.id,
  'start',
  1,
  json_object(
    'username', CASE
      WHEN trim(COALESCE(a.username, '')) = '' THEN ''
      WHEN substr(trim(COALESCE(a.username, '')), 1, 1) = '@' THEN trim(substr(trim(COALESCE(a.username, '')), 2))
      ELSE trim(COALESCE(a.username, ''))
    END,
    'cookies_json', COALESCE(a.cookies_json, ''),
    'proxy_url', COALESCE(a.proxy_url, ''),
    'poll_interval_seconds', 60,
    'retry_limit', 0,
    'last_live_at', f.last_live_at,
    'last_run_at', f.last_run_at,
    'last_error', f.last_error
  ),
  json_object(
    'username', CASE
      WHEN trim(COALESCE(a.username, '')) = '' THEN ''
      WHEN substr(trim(COALESCE(a.username, '')), 1, 1) = '@' THEN trim(substr(trim(COALESCE(a.username, '')), 2))
      ELSE trim(COALESCE(a.username, ''))
    END,
    'cookies_json', COALESCE(a.cookies_json, ''),
    'proxy_url', COALESCE(a.proxy_url, ''),
    'poll_interval_seconds', 60,
    'retry_limit', 0,
    'last_live_at', f.last_live_at,
    'last_run_at', f.last_run_at,
    'last_error', f.last_error
  )
FROM flows_legacy f
LEFT JOIN accounts a ON a.id = f.account_id;

INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at)
SELECT
  n.flow_id,
  n.node_key,
  CASE n.node_key
    WHEN 'record' THEN 2
    WHEN 'clip' THEN 3
    WHEN 'caption' THEN 4
    WHEN 'upload' THEN 5
    ELSE 2
  END,
  CASE n.node_key
    WHEN 'record' THEN json_object(
      'max_duration_minutes', CASE
        WHEN json_extract(n.config_json, '$.max_duration_minutes') IS NOT NULL THEN MAX(CAST(json_extract(n.config_json, '$.max_duration_minutes') AS INTEGER), 1)
        WHEN json_extract(n.config_json, '$.maxDurationMinutes') IS NOT NULL THEN MAX(CAST(json_extract(n.config_json, '$.maxDurationMinutes') AS INTEGER), 1)
        WHEN json_extract(n.config_json, '$.maxDuration') IS NOT NULL THEN MAX(CAST(json_extract(n.config_json, '$.maxDuration') AS INTEGER), 1)
        WHEN json_extract(n.config_json, '$.maxDurationSeconds') IS NOT NULL THEN MAX((CAST(json_extract(n.config_json, '$.maxDurationSeconds') AS INTEGER) + 59) / 60, 1)
        WHEN json_extract(n.config_json, '$.durationSeconds') IS NOT NULL THEN MAX((CAST(json_extract(n.config_json, '$.durationSeconds') AS INTEGER) + 59) / 60, 1)
        ELSE 5
      END
    )
    ELSE n.config_json
  END,
  CASE n.node_key
    WHEN 'record' THEN json_object(
      'max_duration_minutes', CASE
        WHEN json_extract(n.config_json, '$.max_duration_minutes') IS NOT NULL THEN MAX(CAST(json_extract(n.config_json, '$.max_duration_minutes') AS INTEGER), 1)
        WHEN json_extract(n.config_json, '$.maxDurationMinutes') IS NOT NULL THEN MAX(CAST(json_extract(n.config_json, '$.maxDurationMinutes') AS INTEGER), 1)
        WHEN json_extract(n.config_json, '$.maxDuration') IS NOT NULL THEN MAX(CAST(json_extract(n.config_json, '$.maxDuration') AS INTEGER), 1)
        WHEN json_extract(n.config_json, '$.maxDurationSeconds') IS NOT NULL THEN MAX((CAST(json_extract(n.config_json, '$.maxDurationSeconds') AS INTEGER) + 59) / 60, 1)
        WHEN json_extract(n.config_json, '$.durationSeconds') IS NOT NULL THEN MAX((CAST(json_extract(n.config_json, '$.durationSeconds') AS INTEGER) + 59) / 60, 1)
        ELSE 5
      END
    )
    ELSE n.config_json
  END,
  n.updated_at,
  n.updated_at
FROM flow_node_configs_legacy n
WHERE n.node_key IN ('record', 'clip', 'caption', 'upload');

INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at)
SELECT
  f.id,
  v.node_key,
  v.position,
  '{}',
  '{}',
  datetime('now', '+7 hours'),
  datetime('now', '+7 hours')
FROM flows f
JOIN (
  SELECT 'start' AS node_key, 1 AS position
  UNION ALL SELECT 'record', 2
  UNION ALL SELECT 'clip', 3
  UNION ALL SELECT 'caption', 4
  UNION ALL SELECT 'upload', 5
) AS v
WHERE NOT EXISTS (
  SELECT 1 FROM flow_nodes n WHERE n.flow_id = f.id AND n.node_key = v.node_key
);

ALTER TABLE recordings ADD COLUMN flow_run_id INTEGER REFERENCES flow_runs(id) ON DELETE SET NULL;
ALTER TABLE clips ADD COLUMN flow_run_id INTEGER REFERENCES flow_runs(id) ON DELETE SET NULL;

DROP TABLE flow_node_configs_legacy;
DROP TABLE flows_legacy;

PRAGMA foreign_keys = ON;
