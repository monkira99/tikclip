use crate::time_hcm::now_timestamp_hcm;
use crate::AppState;
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex as StdMutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter, State};

const DEFAULT_RAW_RETENTION_DAYS: i64 = 7;
const DEFAULT_ARCHIVE_RETENTION_DAYS: i64 = 0;
const DEFAULT_STORAGE_WARN_PERCENT: i64 = 80;
const DEFAULT_STORAGE_CLEANUP_PERCENT: i64 = 95;
const CLEANUP_INTERVAL_SECONDS: u64 = 30 * 60;
const BYTES_PER_GB: f64 = 1_073_741_824.0;

#[derive(Debug, Clone, Copy, Default)]
struct DirUsage {
    bytes: i64,
    files: i64,
}

#[derive(Debug, Clone, Copy)]
struct CleanupSettings {
    raw_retention_days: i64,
    archive_retention_days: i64,
    warn_percent: i64,
    cleanup_percent: i64,
}

#[derive(Debug, Serialize)]
pub struct StorageStats {
    pub recordings_bytes: i64,
    pub recordings_count: i64,
    pub clips_bytes: i64,
    pub clips_count: i64,
    pub products_bytes: i64,
    pub products_count: i64,
    /// Media quota usage excludes products by design.
    pub total_bytes: i64,
    pub quota_bytes: Option<i64>,
    pub usage_percent: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct StorageCleanupRunInput {
    pub raw_retention_days: i64,
    pub archive_retention_days: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageCleanupSummary {
    pub deleted_recordings: i64,
    pub deleted_clips: i64,
    pub freed_bytes: i64,
}

struct ArchivedClipRow {
    id: i64,
    file_path: String,
    thumbnail_path: Option<String>,
    updated_at: String,
}

pub struct StorageCleanupWorker {
    stop: Arc<AtomicBool>,
    handle: StdMutex<Option<JoinHandle<()>>>,
}

impl StorageCleanupWorker {
    pub fn start(app: AppHandle, db_path: PathBuf, storage_root: PathBuf) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                if let Err(err) = run_background_cleanup_cycle(&app, &db_path, &storage_root) {
                    log::warn!("storage cleanup cycle failed: {}", err);
                }
                sleep_until_next_cycle(&thread_stop);
            }
        });

        Self {
            stop,
            handle: StdMutex::new(Some(handle)),
        }
    }

    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        let Some(handle) = self.handle.lock().ok().and_then(|mut h| h.take()) else {
            return;
        };
        if handle.join().is_err() {
            log::warn!("storage cleanup worker thread panicked during shutdown");
        }
    }
}

fn sleep_until_next_cycle(stop: &AtomicBool) {
    for _ in 0..CLEANUP_INTERVAL_SECONDS {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        thread::sleep(Duration::from_secs(1));
    }
}

fn hcm_offset() -> FixedOffset {
    FixedOffset::east_opt(7 * 3600).expect("GMT+7 offset")
}

fn today_hcm() -> NaiveDate {
    Utc::now().with_timezone(&hcm_offset()).date_naive()
}

fn hcm_calendar_age_days_from_mtime(mtime: SystemTime, today: NaiveDate) -> i64 {
    let dt: DateTime<Utc> = mtime.into();
    let mtime_date = dt.with_timezone(&hcm_offset()).date_naive();
    today.signed_duration_since(mtime_date).num_days().max(0)
}

fn file_hcm_calendar_age_days(path: &Path, today: NaiveDate) -> i64 {
    path.metadata()
        .and_then(|m| m.modified())
        .map(|mtime| hcm_calendar_age_days_from_mtime(mtime, today))
        .unwrap_or(0)
}

fn db_timestamp_hcm_date(value: &str) -> Option<NaiveDate> {
    let date_part = value.get(0..10)?;
    NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()
}

fn add_len(total: &mut i64, len: u64) {
    let size = i64::try_from(len).unwrap_or(i64::MAX);
    *total = total.saturating_add(size);
}

fn dir_usage(path: &Path) -> DirUsage {
    let Ok(entries) = std::fs::read_dir(path) else {
        return DirUsage::default();
    };

    let mut usage = DirUsage::default();
    for entry in entries.flatten() {
        let Ok(md) = entry.metadata() else {
            continue;
        };
        let p = entry.path();
        if md.is_file() {
            add_len(&mut usage.bytes, md.len());
            usage.files = usage.files.saturating_add(1);
        } else if md.is_dir() {
            let child = dir_usage(&p);
            usage.bytes = usage.bytes.saturating_add(child.bytes);
            usage.files = usage.files.saturating_add(child.files);
        }
    }
    usage
}

fn resolve_usage_path(storage_root: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw.trim());
    if path.is_absolute() {
        path
    } else {
        storage_root.join(path)
    }
}

fn usage_from_paths(storage_root: &Path, paths: impl IntoIterator<Item = String>) -> DirUsage {
    let mut usage = DirUsage::default();
    for raw in paths {
        if raw.trim().is_empty() {
            continue;
        }
        let path = resolve_usage_path(storage_root, &raw);
        let Ok(md) = path.metadata() else {
            continue;
        };
        if !md.is_file() {
            continue;
        }
        add_len(&mut usage.bytes, md.len());
        usage.files = usage.files.saturating_add(1);
    }
    usage
}

fn db_file_size_usage(
    conn: &Connection,
    table: &str,
    file_path_column: &str,
) -> Result<DirUsage, String> {
    let sql = format!(
        "SELECT COALESCE(SUM(file_size_bytes), 0), COUNT(*) FROM {table} \
         WHERE {file_path_column} IS NOT NULL \
           AND TRIM({file_path_column}) != '' \
           AND file_size_bytes > 0"
    );
    conn.query_row(&sql, [], |row| {
        Ok(DirUsage {
            bytes: row.get(0)?,
            files: row.get(1)?,
        })
    })
    .map_err(|e| e.to_string())
}

fn query_string_column(conn: &Connection, sql: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn max_usage(usages: &[DirUsage]) -> DirUsage {
    usages
        .iter()
        .copied()
        .max_by_key(|usage| usage.bytes)
        .unwrap_or_default()
}

fn quota_bytes_from_gb(value: Option<f64>) -> Option<i64> {
    let gb = value?;
    if gb <= 0.0 || !gb.is_finite() {
        return None;
    }
    Some((gb * BYTES_PER_GB).round() as i64)
}

fn round_one(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn get_setting_trimmed(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty()))
}

fn read_i64_setting(conn: &Connection, key: &str, fallback: i64) -> Result<i64, String> {
    let Some(value) = get_setting_trimmed(conn, key)? else {
        return Ok(fallback);
    };
    value
        .parse::<i64>()
        .map_err(|_| format!("{key} must be an integer, got {value:?}"))
}

fn read_f64_setting(conn: &Connection, key: &str) -> Result<Option<f64>, String> {
    let Some(value) = get_setting_trimmed(conn, key)? else {
        return Ok(None);
    };
    let parsed = value
        .parse::<f64>()
        .map_err(|_| format!("{key} must be a number, got {value:?}"))?;
    Ok(if parsed > 0.0 { Some(parsed) } else { None })
}

fn read_cleanup_settings(conn: &Connection) -> Result<CleanupSettings, String> {
    let raw_retention_days = read_i64_setting(
        conn,
        "TIKCLIP_RAW_RETENTION_DAYS",
        DEFAULT_RAW_RETENTION_DAYS,
    )?
    .max(0);
    let archive_retention_days = read_i64_setting(
        conn,
        "TIKCLIP_ARCHIVE_RETENTION_DAYS",
        DEFAULT_ARCHIVE_RETENTION_DAYS,
    )?
    .max(0);
    let warn_percent = read_i64_setting(
        conn,
        "TIKCLIP_STORAGE_WARN_PERCENT",
        DEFAULT_STORAGE_WARN_PERCENT,
    )?
    .clamp(1, 100);
    let cleanup_percent = read_i64_setting(
        conn,
        "TIKCLIP_STORAGE_CLEANUP_PERCENT",
        DEFAULT_STORAGE_CLEANUP_PERCENT,
    )?
    .clamp(1, 100);

    Ok(CleanupSettings {
        raw_retention_days,
        archive_retention_days,
        warn_percent,
        cleanup_percent: cleanup_percent.max(warn_percent),
    })
}

fn storage_stats(conn: &Connection, storage_root: &Path) -> Result<StorageStats, String> {
    let layout_records = dir_usage(&storage_root.join("records"));
    let layout_legacy_recordings = dir_usage(&storage_root.join("recordings"));
    let layout_recordings = DirUsage {
        bytes: layout_records
            .bytes
            .saturating_add(layout_legacy_recordings.bytes),
        files: layout_records
            .files
            .saturating_add(layout_legacy_recordings.files),
    };
    let db_recordings = db_file_size_usage(conn, "recordings", "file_path")?;
    let path_recordings = usage_from_paths(
        storage_root,
        query_string_column(
            conn,
            "SELECT DISTINCT file_path FROM recordings \
             WHERE file_path IS NOT NULL AND TRIM(file_path) != ''",
        )?,
    );
    let recordings = max_usage(&[layout_recordings, db_recordings, path_recordings]);

    let layout_clips = dir_usage(&storage_root.join("clips"));
    let db_clips = db_file_size_usage(conn, "clips", "file_path")?;
    let path_clips = usage_from_paths(
        storage_root,
        query_string_column(
            conn,
            "SELECT file_path FROM clips WHERE file_path IS NOT NULL AND TRIM(file_path) != '' \
             UNION \
             SELECT thumbnail_path FROM clips WHERE thumbnail_path IS NOT NULL AND TRIM(thumbnail_path) != ''",
        )?,
    );
    let clips = max_usage(&[layout_clips, db_clips, path_clips]);

    let products = dir_usage(&storage_root.join("products"));
    let total_bytes = recordings.bytes.saturating_add(clips.bytes);
    let quota_bytes = quota_bytes_from_gb(read_f64_setting(conn, "max_storage_gb")?);
    let usage_percent = quota_bytes
        .filter(|quota| *quota > 0)
        .map(|quota| round_one(total_bytes as f64 / quota as f64 * 100.0))
        .unwrap_or(0.0);

    Ok(StorageStats {
        recordings_bytes: recordings.bytes,
        recordings_count: recordings.files,
        clips_bytes: clips.bytes,
        clips_count: clips.files,
        products_bytes: products.bytes,
        products_count: products.files,
        total_bytes,
        quota_bytes,
        usage_percent,
    })
}

fn is_raw_media_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "flv" | "mp4" | "ts" | "mkv" | "m4a" | "aac"
    )
}

fn delete_file_for_cleanup(path: &Path) -> Option<i64> {
    let size = path
        .metadata()
        .ok()
        .filter(|m| m.is_file())
        .map(|m| i64::try_from(m.len()).unwrap_or(i64::MAX))
        .unwrap_or(0);
    match std::fs::remove_file(path) {
        Ok(()) => Some(size),
        Err(err) => {
            log::warn!("failed to delete storage file {}: {}", path.display(), err);
            None
        }
    }
}

fn clear_deleted_recording_path(conn: &Connection, path: &Path) -> Result<(), String> {
    let path_string = path.to_string_lossy();
    conn.execute(
        "UPDATE recordings SET file_path = NULL, file_size_bytes = 0 WHERE file_path = ?1",
        params![path_string.as_ref()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn delete_old_recordings_under_dir<F>(
    conn: &Connection,
    rec_dir: &Path,
    retention_days: i64,
    age_days: &F,
) -> Result<(i64, i64), String>
where
    F: Fn(&Path) -> i64,
{
    if retention_days <= 0 || !rec_dir.is_dir() {
        return Ok((0, 0));
    }

    let mut deleted = 0_i64;
    let mut freed = 0_i64;
    let entries = std::fs::read_dir(rec_dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(md) = entry.metadata() else {
            continue;
        };
        if md.is_dir() {
            let (child_deleted, child_freed) =
                delete_old_recordings_under_dir(conn, &path, retention_days, age_days)?;
            deleted = deleted.saturating_add(child_deleted);
            freed = freed.saturating_add(child_freed);
            continue;
        }
        if !md.is_file() || !is_raw_media_file(&path) || age_days(&path) < retention_days {
            continue;
        }
        let Some(size) = delete_file_for_cleanup(&path) else {
            continue;
        };
        clear_deleted_recording_path(conn, &path)?;
        deleted = deleted.saturating_add(1);
        freed = freed.saturating_add(size);
    }

    Ok((deleted, freed))
}

fn delete_old_recordings(
    conn: &Connection,
    storage_root: &Path,
    retention_days: i64,
    today: NaiveDate,
) -> Result<(i64, i64), String> {
    let age_days = |path: &Path| file_hcm_calendar_age_days(path, today);
    let (records_count, records_freed) = delete_old_recordings_under_dir(
        conn,
        &storage_root.join("records"),
        retention_days,
        &age_days,
    )?;
    let (legacy_count, legacy_freed) = delete_old_recordings_under_dir(
        conn,
        &storage_root.join("recordings"),
        retention_days,
        &age_days,
    )?;
    Ok((
        records_count.saturating_add(legacy_count),
        records_freed.saturating_add(legacy_freed),
    ))
}

fn archived_clip_rows(conn: &Connection) -> Result<Vec<ArchivedClipRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, file_path, thumbnail_path, updated_at \
             FROM clips \
             WHERE status = 'archived' \
             ORDER BY updated_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ArchivedClipRow {
                id: row.get(0)?,
                file_path: row.get(1)?,
                thumbnail_path: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn delete_archived_clips(
    conn: &Connection,
    retention_days: i64,
    today: NaiveDate,
) -> Result<(i64, i64), String> {
    if retention_days <= 0 {
        return Ok((0, 0));
    }

    let mut deleted = 0_i64;
    let mut freed = 0_i64;
    for clip in archived_clip_rows(conn)? {
        let Some(updated_date) = db_timestamp_hcm_date(&clip.updated_at) else {
            log::warn!(
                "skipping archived clip {} cleanup: invalid updated_at {:?}",
                clip.id,
                clip.updated_at
            );
            continue;
        };
        let age = today.signed_duration_since(updated_date).num_days().max(0);
        if age < retention_days {
            continue;
        }

        if let Some(size) = delete_file_for_cleanup(Path::new(&clip.file_path)) {
            freed = freed.saturating_add(size);
        }
        if let Some(thumbnail) = clip
            .thumbnail_path
            .as_deref()
            .filter(|p| !p.trim().is_empty())
        {
            if let Some(size) = delete_file_for_cleanup(Path::new(thumbnail)) {
                freed = freed.saturating_add(size);
            }
        }
        conn.execute("DELETE FROM clips WHERE id = ?1", params![clip.id])
            .map_err(|e| e.to_string())?;
        deleted = deleted.saturating_add(1);
    }

    Ok((deleted, freed))
}

fn insert_cleanup_notification(
    conn: &Connection,
    kind: &str,
    title: &str,
    message: &str,
) -> Result<i64, String> {
    let ts = now_timestamp_hcm();
    conn.execute(
        "INSERT INTO notifications (type, title, message, is_read, created_at) \
         VALUES (?1, ?2, ?3, 0, ?4)",
        params![kind, title, message, ts],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

fn emit_storage_event(app: Option<&AppHandle>, event: &str, payload: serde_json::Value) {
    let Some(app) = app else {
        return;
    };
    if let Err(err) = app.emit(event, payload) {
        log::warn!("failed to emit {event}: {err}");
    }
}

fn maybe_emit_cleanup_notifications(
    conn: &Connection,
    app: Option<&AppHandle>,
    settings: CleanupSettings,
    summary: &StorageCleanupSummary,
    stats: &StorageStats,
) {
    if let Some(quota_bytes) = stats.quota_bytes.filter(|q| *q > 0) {
        if stats.usage_percent >= settings.warn_percent as f64 {
            let critical = stats.usage_percent >= settings.cleanup_percent as f64;
            let title = if critical {
                "Dung lượng gần đầy"
            } else {
                "Cảnh báo dung lượng"
            };
            let body = format!(
                "Đang dùng khoảng {:.1}% quota cấu hình",
                stats.usage_percent
            );
            let notification_id =
                insert_cleanup_notification(conn, "storage_warning", title, &body).ok();
            emit_storage_event(
                app,
                "storage_warning",
                serde_json::json!({
                    "usage_percent": stats.usage_percent,
                    "quota_bytes": quota_bytes,
                    "total_bytes": stats.total_bytes,
                    "critical": critical,
                    "notification_id": notification_id,
                }),
            );
        }
    }

    if summary.freed_bytes > 0 {
        let mb = summary.freed_bytes as f64 / (1024.0 * 1024.0);
        let body = format!(
            "Đã xóa {} file ghi, {} clip; giải phóng ~{mb:.1} MB",
            summary.deleted_recordings, summary.deleted_clips
        );
        let notification_id =
            insert_cleanup_notification(conn, "cleanup_completed", "Dọn dẹp hoàn tất", &body).ok();
        emit_storage_event(
            app,
            "cleanup_completed",
            serde_json::json!({
                "deleted_recordings": summary.deleted_recordings,
                "deleted_clips": summary.deleted_clips,
                "freed_bytes": summary.freed_bytes,
                "notification_id": notification_id,
            }),
        );
    }
}

fn run_cleanup_cycle(
    conn: &Connection,
    storage_root: &Path,
    raw_retention_days: Option<i64>,
    archive_retention_days: Option<i64>,
    app: Option<&AppHandle>,
) -> Result<StorageCleanupSummary, String> {
    let settings = read_cleanup_settings(conn)?;
    let raw_days = raw_retention_days
        .unwrap_or(settings.raw_retention_days)
        .max(0);
    let archive_days = archive_retention_days
        .unwrap_or(settings.archive_retention_days)
        .max(0);
    let today = today_hcm();

    let (deleted_recordings, freed_recordings) =
        delete_old_recordings(conn, storage_root, raw_days, today)?;
    let (deleted_clips, freed_clips) = delete_archived_clips(conn, archive_days, today)?;
    let summary = StorageCleanupSummary {
        deleted_recordings,
        deleted_clips,
        freed_bytes: freed_recordings.saturating_add(freed_clips),
    };
    let stats = storage_stats(conn, storage_root)?;
    maybe_emit_cleanup_notifications(conn, app, settings, &summary, &stats);
    Ok(summary)
}

fn run_background_cleanup_cycle(
    app: &AppHandle,
    db_path: &Path,
    storage_root: &Path,
) -> Result<(), String> {
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .map_err(|e| e.to_string())?;
    let summary = run_cleanup_cycle(&conn, storage_root, None, None, Some(app))?;
    log::debug!(
        "storage cleanup cycle finished: deleted_recordings={} deleted_clips={} freed_bytes={}",
        summary.deleted_recordings,
        summary.deleted_clips,
        summary.freed_bytes
    );
    Ok(())
}

#[tauri::command]
pub fn get_storage_stats(state: State<'_, AppState>) -> Result<StorageStats, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    storage_stats(&conn, &state.storage_path)
}

#[tauri::command]
pub fn run_storage_cleanup_now(
    app: AppHandle,
    state: State<'_, AppState>,
    input: StorageCleanupRunInput,
) -> Result<StorageCleanupSummary, String> {
    if input.raw_retention_days < 0 || input.archive_retention_days < 0 {
        return Err("retention days must be non-negative".to_string());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    run_cleanup_cycle(
        &conn,
        &state.storage_path,
        Some(input.raw_retention_days),
        Some(input.archive_retention_days),
        Some(&app),
    )
}

#[tauri::command]
pub fn list_activity_feed(
    state: State<'_, AppState>,
    limit: i64,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, type, title, message, account_id, recording_id, clip_id, created_at \
             FROM notifications \
             ORDER BY created_at DESC \
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "type": row.get::<_, String>(1)?,
                "title": row.get::<_, String>(2)?,
                "message": row.get::<_, String>(3)?,
                "account_id": row.get::<_, Option<i64>>(4)?,
                "recording_id": row.get::<_, Option<i64>>(5)?,
                "clip_id": row.get::<_, Option<i64>>(6)?,
                "created_at": row.get::<_, String>(7)?,
            }))
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(name: &str) -> PathBuf {
        let n = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "tikclip-storage-test-{name}-{}-{n}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE recordings (
                id INTEGER PRIMARY KEY,
                file_path TEXT,
                file_size_bytes INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE clips (
                id INTEGER PRIMARY KEY,
                file_path TEXT NOT NULL,
                thumbnail_path TEXT,
                file_size_bytes INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             CREATE TABLE notifications (
                id INTEGER PRIMARY KEY,
                type TEXT NOT NULL,
                title TEXT NOT NULL,
                message TEXT NOT NULL DEFAULT '',
                is_read INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
             );",
        )
        .expect("create schema");
        conn
    }

    fn write_file(path: &Path, bytes: usize) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, vec![b'x'; bytes]).expect("write file");
    }

    #[test]
    fn storage_stats_excludes_products_from_quota_total() {
        let root = temp_dir("stats");
        write_file(&root.join("records/a.mp4"), 10);
        write_file(&root.join("recordings/b.flv"), 20);
        write_file(&root.join("clips/c.mp4"), 30);
        write_file(&root.join("products/p.jpg"), 40);
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('max_storage_gb', '0.0000001')",
            [],
        )
        .expect("insert setting");

        let stats = storage_stats(&conn, &root).expect("stats");

        assert_eq!(stats.recordings_bytes, 30);
        assert_eq!(stats.clips_bytes, 30);
        assert_eq!(stats.products_bytes, 40);
        assert_eq!(stats.total_bytes, 60);
        assert!(stats.usage_percent > 0.0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn raw_cleanup_deletes_file_and_nulls_recording_fields() {
        let root = temp_dir("raw-cleanup");
        let file = root.join("records/shop/old.mp4");
        write_file(&file, 12);
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO recordings (id, file_path, file_size_bytes) VALUES (1, ?1, 12)",
            [file.to_string_lossy().as_ref()],
        )
        .expect("insert recording");

        let (deleted, freed) =
            delete_old_recordings_under_dir(&conn, &root.join("records"), 7, &|_| 7)
                .expect("cleanup");

        assert_eq!(deleted, 1);
        assert_eq!(freed, 12);
        assert!(!file.exists());
        let row: (Option<String>, i64) = conn
            .query_row(
                "SELECT file_path, file_size_bytes FROM recordings WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("recording row");
        assert_eq!(row, (None, 0));
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM recordings", [], |row| row.get(0))
            .expect("count recordings");
        assert_eq!(count, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archived_cleanup_only_deletes_archived_clips_over_retention() {
        let root = temp_dir("archived-cleanup");
        let old_archived = root.join("clips/old.mp4");
        let old_thumb = root.join("clips/old.jpg");
        let ready = root.join("clips/ready.mp4");
        let fresh_archived = root.join("clips/fresh.mp4");
        write_file(&old_archived, 10);
        write_file(&old_thumb, 3);
        write_file(&ready, 20);
        write_file(&fresh_archived, 30);
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO clips (id, file_path, thumbnail_path, status, updated_at) VALUES (1, ?1, ?2, 'archived', '2026-04-20 10:00:00')",
            params![old_archived.to_string_lossy().as_ref(), old_thumb.to_string_lossy().as_ref()],
        )
        .expect("insert old archived");
        conn.execute(
            "INSERT INTO clips (id, file_path, status, updated_at) VALUES (2, ?1, 'ready', '2026-04-20 10:00:00')",
            [ready.to_string_lossy().as_ref()],
        )
        .expect("insert ready");
        conn.execute(
            "INSERT INTO clips (id, file_path, status, updated_at) VALUES (3, ?1, 'archived', '2026-04-24 10:00:00')",
            [fresh_archived.to_string_lossy().as_ref()],
        )
        .expect("insert fresh archived");
        let today = NaiveDate::from_ymd_opt(2026, 4, 25).expect("today");

        let (deleted, freed) = delete_archived_clips(&conn, 3, today).expect("cleanup");

        assert_eq!(deleted, 1);
        assert_eq!(freed, 13);
        assert!(!old_archived.exists());
        assert!(!old_thumb.exists());
        assert!(ready.exists());
        assert!(fresh_archived.exists());
        let ids: Vec<i64> = {
            let mut stmt = conn
                .prepare("SELECT id FROM clips ORDER BY id")
                .expect("prepare");
            stmt.query_map([], |row| row.get(0))
                .expect("query")
                .map(|row| row.expect("row"))
                .collect()
        };
        assert_eq!(ids, vec![2, 3]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn quota_warning_uses_media_total_and_thresholds() {
        let root = temp_dir("quota");
        write_file(&root.join("clips/c.mp4"), 110);
        write_file(&root.join("products/p.jpg"), 900);
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('max_storage_gb', '0.0000001')",
            [],
        )
        .expect("insert quota");

        let settings = read_cleanup_settings(&conn).expect("settings");
        let stats = storage_stats(&conn, &root).expect("stats");

        assert_eq!(stats.total_bytes, 110);
        assert_eq!(stats.products_bytes, 900);
        assert!(stats.usage_percent >= settings.warn_percent as f64);
        assert!(stats.usage_percent >= settings.cleanup_percent as f64);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hcm_calendar_age_uses_calendar_days() {
        let today = NaiveDate::from_ymd_opt(2026, 4, 25).expect("today");
        let dt = DateTime::parse_from_rfc3339("2026-04-24T23:30:00+07:00")
            .expect("parse")
            .with_timezone(&Utc);

        let age = hcm_calendar_age_days_from_mtime(SystemTime::from(dt), today);

        assert_eq!(age, 1);
    }
}
