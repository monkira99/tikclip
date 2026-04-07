use crate::db::models::Clip;
use crate::AppState;
use rusqlite::Result as SqlResult;
use rusqlite::{params, OptionalExtension, Row};
use serde::Deserialize;
use tauri::State;

use super::recordings::{sync_recording_from_sidecar_conn, SyncRecordingFromSidecarInput};

fn map_clip_row(row: &Row) -> SqlResult<Clip> {
    Ok(Clip {
        id: row.get(0)?,
        recording_id: row.get(1)?,
        account_id: row.get(2)?,
        account_username: row.get(3)?,
        title: row.get(4)?,
        file_path: row.get(5)?,
        thumbnail_path: row.get(6)?,
        duration_seconds: row.get(7)?,
        file_size_bytes: row.get(8)?,
        start_time: row.get(9)?,
        end_time: row.get(10)?,
        status: row.get(11)?,
        quality_score: row.get(12)?,
        scene_type: row.get(13)?,
        ai_tags_json: row.get(14)?,
        notes: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

/// Lists clips joined with `accounts.username` as `account_username`.
/// Column order matches `clips` table in `db/migrations/001_initial.sql` plus joined username.
#[tauri::command]
pub fn list_clips(state: State<'_, AppState>) -> Result<Vec<Clip>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT \
             c.id, c.recording_id, c.account_id, a.username, \
             c.title, c.file_path, c.thumbnail_path, c.duration_seconds, c.file_size_bytes, \
             c.start_time, c.end_time, c.status, c.quality_score, c.scene_type, c.ai_tags_json, \
             c.notes, c.created_at, c.updated_at \
             FROM clips c \
             INNER JOIN accounts a ON a.id = c.account_id \
             ORDER BY c.created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], map_clip_row)
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InsertClipFromSidecarInput {
    pub sidecar_recording_id: String,
    pub account_id: i64,
    pub file_path: String,
    pub thumbnail_path: String,
    pub duration_sec: f64,
    pub start_sec: f64,
    pub end_sec: f64,
}

/// Persist one generated clip when the sidecar emits `clip_ready` over WebSocket.
#[tauri::command]
pub fn insert_clip_from_sidecar(
    state: State<'_, AppState>,
    input: InsertClipFromSidecarInput,
) -> Result<i64, String> {
    if input.sidecar_recording_id.trim().is_empty() {
        return Err("sidecar_recording_id is required".to_string());
    }
    if input.file_path.trim().is_empty() {
        return Err("file_path is required".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let mut rec_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM recordings WHERE sidecar_recording_id = ?1",
            [&input.sidecar_recording_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if rec_id.is_none() {
        sync_recording_from_sidecar_conn(
            &conn,
            &SyncRecordingFromSidecarInput {
                sidecar_recording_id: input.sidecar_recording_id.clone(),
                account_id: input.account_id,
                status: "done".to_string(),
                duration_seconds: 0,
                file_size_bytes: 0,
                file_path: None,
                error_message: None,
            },
        )?;
        rec_id = Some(
            conn.query_row(
                "SELECT id FROM recordings WHERE sidecar_recording_id = ?1",
                [&input.sidecar_recording_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?,
        );
    }

    let recording_id = rec_id.expect("recording row must exist after upsert");

    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM clips WHERE recording_id = ?1 AND file_path = ?2",
            params![recording_id, &input.file_path],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(id) = existing {
        return Ok(id);
    }

    let file_size_bytes: i64 = std::fs::metadata(&input.file_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    let duration_seconds = input.duration_sec.round().max(0.0) as i64;

    let thumb = input.thumbnail_path.trim();
    let thumb_opt = if thumb.is_empty() { None } else { Some(thumb) };

    conn.execute(
        "INSERT INTO clips (\
           recording_id, account_id, title, file_path, thumbnail_path, \
           duration_seconds, file_size_bytes, start_time, end_time, status\
         ) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, 'ready')",
        params![
            recording_id,
            input.account_id,
            &input.file_path,
            thumb_opt,
            duration_seconds,
            file_size_bytes,
            input.start_sec,
            input.end_sec,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

