use crate::time_hcm::now_timestamp_hcm;
use crate::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertNotificationInput {
    pub notification_type: String,
    pub title: String,
    pub message: String,
    pub account_id: Option<i64>,
    pub recording_id: Option<i64>,
    pub clip_id: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationListItem {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub read: bool,
    pub created_at: String,
}

/// Insert a notification row (sidecar / in-app events).
#[tauri::command]
pub fn insert_notification(
    state: State<'_, AppState>,
    input: InsertNotificationInput,
) -> Result<i64, String> {
    let t = input.notification_type.trim();
    if t.is_empty() {
        return Err("notification_type is required".to_string());
    }
    let title = input.title.trim();
    if title.is_empty() {
        return Err("title is required".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let ts = now_timestamp_hcm();
    conn.execute(
        "INSERT INTO notifications (type, title, message, account_id, recording_id, clip_id, is_read, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
        params![
            t,
            title,
            input.message,
            input.account_id,
            input.recording_id,
            input.clip_id,
            ts,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn list_notifications(
    state: State<'_, AppState>,
    limit: i64,
) -> Result<Vec<NotificationListItem>, String> {
    let cap = limit.clamp(1, 500);
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, type, title, message, is_read, created_at FROM notifications \
             ORDER BY id DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([cap], |row| {
            Ok(NotificationListItem {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                body: row.get(3)?,
                read: row.get::<_, i64>(4)? != 0,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub fn mark_notification_read(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let n = conn
        .execute(
            "UPDATE notifications SET is_read = 1 WHERE id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err(format!("notification id {id} not found"));
    }
    Ok(())
}

#[tauri::command]
pub fn mark_all_notifications_read(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE notifications SET is_read = 1 WHERE is_read = 0", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}
