-- One-time: existing rows were stored as SQLite UTC `datetime('now')`; shift to GMT+7 wall clock.

UPDATE accounts SET
  created_at = datetime(created_at, '+7 hours'),
  updated_at = datetime(updated_at, '+7 hours'),
  last_live_at = CASE
    WHEN last_live_at IS NOT NULL AND length(trim(last_live_at)) > 0
    THEN datetime(last_live_at, '+7 hours')
    ELSE last_live_at
  END,
  last_checked_at = CASE
    WHEN last_checked_at IS NOT NULL AND length(trim(last_checked_at)) > 0
    THEN datetime(last_checked_at, '+7 hours')
    ELSE last_checked_at
  END;

UPDATE recordings SET
  started_at = datetime(started_at, '+7 hours'),
  ended_at = CASE
    WHEN ended_at IS NOT NULL AND length(trim(ended_at)) > 0
    THEN datetime(ended_at, '+7 hours')
    ELSE ended_at
  END,
  created_at = datetime(created_at, '+7 hours');

UPDATE clips SET
  created_at = datetime(created_at, '+7 hours'),
  updated_at = datetime(updated_at, '+7 hours');

UPDATE products SET
  created_at = datetime(created_at, '+7 hours');

UPDATE notifications SET
  created_at = datetime(created_at, '+7 hours');

UPDATE app_settings SET
  updated_at = datetime(updated_at, '+7 hours');
