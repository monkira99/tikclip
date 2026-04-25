use crate::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub clips_today: i64,
    /// Max of DB column sums, bytes at stored paths, and recursive size under `clips/`, `records/`,
    /// and legacy `recordings/`.
    pub storage_used_bytes: i64,
    /// From `app_settings.max_storage_gb` when set and positive; otherwise null (no quota in UI).
    pub storage_quota_gb: Option<f64>,
}

fn is_valid_ymd(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 {
        return false;
    }
    for (i, &c) in b.iter().enumerate() {
        if i == 4 || i == 7 {
            if c != b'-' {
                return false;
            }
        } else if !c.is_ascii_digit() {
            return false;
        }
    }
    true
}

/// Sum `metadata().len()` for every distinct non-empty path stored on clips/recordings (and clip thumbnails).
fn disk_usage_from_stored_paths(conn: &Connection) -> Result<i64, String> {
    let mut stmt = conn
        .prepare(
            "SELECT file_path FROM clips WHERE file_path IS NOT NULL AND TRIM(file_path) != '' \
             UNION \
             SELECT file_path FROM recordings WHERE file_path IS NOT NULL AND TRIM(file_path) != '' \
             UNION \
             SELECT thumbnail_path FROM clips WHERE thumbnail_path IS NOT NULL AND TRIM(thumbnail_path) != ''",
        )
        .map_err(|e| e.to_string())?;

    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    let mut total: i64 = 0;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let p: String = row.get(0).map_err(|e| e.to_string())?;
        let path = Path::new(p.trim());
        if let Ok(md) = path.metadata() {
            if md.is_file() {
                total = total.saturating_add(md.len() as i64);
            }
        }
    }
    Ok(total)
}

/// Sum sizes of all regular files under `root` (recursive). Used when DB paths are missing or stale.
fn sum_files_recursive(root: &Path) -> i64 {
    let mut total: i64 = 0;
    let Ok(entries) = std::fs::read_dir(root) else {
        return 0;
    };
    for entry in entries.flatten() {
        let Ok(md) = entry.metadata() else {
            continue;
        };
        let path = entry.path();
        if md.is_file() {
            total = total.saturating_add(md.len() as i64);
        } else if md.is_dir() {
            total = total.saturating_add(sum_files_recursive(&path));
        }
    }
    total
}

fn layout_media_usage_bytes(storage_root: &Path) -> i64 {
    let clips = storage_root.join("clips");
    let records = storage_root.join("records");
    let recordings_legacy = storage_root.join("recordings");
    sum_files_recursive(&clips)
        .saturating_add(sum_files_recursive(&records))
        .saturating_add(sum_files_recursive(&recordings_legacy))
}

#[tauri::command]
pub fn get_dashboard_stats(
    state: State<'_, AppState>,
    today_ymd: String,
) -> Result<DashboardStats, String> {
    let today = today_ymd.trim();
    if !is_valid_ymd(today) {
        return Err("today_ymd must be YYYY-MM-DD".to_string());
    }

    let storage_root: PathBuf = state.storage_path.clone();

    let (clips_today, sum_columns, sum_disk, storage_quota_gb): (i64, i64, i64, Option<f64>) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;

        // Match "today" by DB timestamp prefix OR by folder segment in file_path (sidecar uses
        // `YYYY-MM-DD` in paths; `created_at` is stored as GMT+7 wall clock).
        let needle = format!("/{today}/");
        let clips_today: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clips WHERE substr(created_at, 1, 10) = ?1 \
             OR instr(file_path, ?2) > 0 \
             OR instr(replace(file_path, '\\', '/'), ?2) > 0",
                params![today, &needle],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        let clip_bytes: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(file_size_bytes), 0) FROM clips",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let rec_bytes: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(file_size_bytes), 0) FROM recordings",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let sum_columns = clip_bytes.saturating_add(rec_bytes).max(0);
        let sum_disk = disk_usage_from_stored_paths(&conn).unwrap_or(0);

        let quota_raw: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                ["max_storage_gb"],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        let storage_quota_gb = quota_raw.and_then(|s| {
            let t = s.trim();
            if t.is_empty() {
                return None;
            }
            let v: f64 = t.parse().ok()?;
            if v > 0.0 {
                Some(v)
            } else {
                None
            }
        });

        (clips_today, sum_columns, sum_disk, storage_quota_gb)
    };

    let layout_bytes = layout_media_usage_bytes(&storage_root);
    let storage_used_bytes = sum_columns.max(sum_disk).max(layout_bytes);

    log::info!(
        "dashboard_stats: clips_today={} storage_used_bytes={} (db_sum={} disk_paths_sum={} layout_clips_records_sum={})",
        clips_today,
        storage_used_bytes,
        sum_columns,
        sum_disk,
        layout_bytes
    );

    Ok(DashboardStats {
        clips_today,
        storage_used_bytes,
        storage_quota_gb,
    })
}
