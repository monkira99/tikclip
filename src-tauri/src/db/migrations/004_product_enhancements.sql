ALTER TABLE products ADD COLUMN tiktok_url TEXT;
ALTER TABLE products ADD COLUMN updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'));
