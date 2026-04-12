use crate::AppState;
use rusqlite::params;
use tauri::State;

#[tauri::command]
pub fn delete_recording_files(state: State<'_, AppState>, recording_id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let file_path: Option<String> = conn
        .query_row(
            "SELECT file_path FROM recordings WHERE id = ?1",
            [recording_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if let Some(ref path) = file_path {
        let _ = std::fs::remove_file(path);
    }

    conn.execute(
        "UPDATE recordings SET file_path = NULL, file_size_bytes = 0 WHERE id = ?1",
        [recording_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn list_recordings_for_cleanup(
    state: State<'_, AppState>,
    retention_days: i64,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, account_id, file_path, file_size_bytes, ended_at \
             FROM recordings \
             WHERE status = 'done' \
               AND file_path IS NOT NULL \
               AND ended_at IS NOT NULL \
               AND julianday('now', '+7 hours') - julianday(ended_at) > ?1 \
             ORDER BY ended_at ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![retention_days], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "account_id": row.get::<_, i64>(1)?,
                "file_path": row.get::<_, Option<String>>(2)?,
                "file_size_bytes": row.get::<_, i64>(3)?,
                "ended_at": row.get::<_, Option<String>>(4)?,
            }))
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
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
