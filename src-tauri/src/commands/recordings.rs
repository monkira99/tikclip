use crate::time_hcm::SQL_NOW_HCM;
use crate::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SyncRecordingFromSidecarInput {
    pub sidecar_recording_id: String,
    pub account_id: i64,
    pub status: String,
    pub duration_seconds: i64,
    pub file_size_bytes: i64,
    pub file_path: Option<String>,
    pub error_message: Option<String>,
}

fn map_sidecar_recording_status(sidecar: &str) -> &'static str {
    match sidecar {
        "completed" | "stopped" => "done",
        "error" => "error",
        "processing" => "processing",
        _ => "recording",
    }
}

/// Shared upsert used by the Tauri command and `insert_clip_from_sidecar`.
pub(super) fn sync_recording_from_sidecar_conn(
    conn: &Connection,
    input: &SyncRecordingFromSidecarInput,
) -> Result<i64, String> {
    if input.sidecar_recording_id.trim().is_empty() {
        return Err("sidecar_recording_id is required".to_string());
    }

    let acct_ok: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM accounts WHERE id = ?1",
            [input.account_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if acct_ok == 0 {
        return Err(format!("unknown account_id {}", input.account_id));
    }

    let mapped = map_sidecar_recording_status(input.status.trim());
    let path = input.file_path.as_deref();
    let err = input.error_message.as_deref();

    let existing_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM recordings WHERE sidecar_recording_id = ?1",
            [&input.sidecar_recording_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some(id) = existing_id {
        conn.execute(
            &format!(
                "UPDATE recordings SET \
                 status = ?1, duration_seconds = ?2, \
                 file_size_bytes = CASE WHEN ?3 > 0 THEN ?3 ELSE file_size_bytes END, \
                 file_path = COALESCE(?4, file_path), \
                 error_message = COALESCE(?5, error_message), \
                 ended_at = CASE \
                   WHEN ?1 IN ('done', 'error', 'processing') AND ended_at IS NULL \
                   THEN {} ELSE ended_at END \
                 WHERE id = ?6",
                SQL_NOW_HCM
            ),
            params![mapped, input.duration_seconds, input.file_size_bytes, path, err, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(id)
    } else {
        let ended_flag: i64 = if matches!(mapped, "done" | "error" | "processing") {
            1
        } else {
            0
        };
        conn.execute(
            &format!(
                "INSERT INTO recordings (\
                   account_id, room_id, status, duration_seconds, file_size_bytes, \
                   file_path, error_message, sidecar_recording_id, started_at, created_at, ended_at\
                 ) VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, ?7, {}, {}, \
                   CASE WHEN ?8 != 0 THEN {} ELSE NULL END)",
                SQL_NOW_HCM, SQL_NOW_HCM, SQL_NOW_HCM
            ),
            params![
                input.account_id,
                mapped,
                input.duration_seconds,
                input.file_size_bytes,
                path,
                err,
                &input.sidecar_recording_id,
                ended_flag,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }
}

/// Upsert a `recordings` row keyed by `sidecar_recording_id` (sidecar UUID).
#[tauri::command]
pub fn sync_recording_from_sidecar(
    state: State<'_, AppState>,
    input: SyncRecordingFromSidecarInput,
) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sync_recording_from_sidecar_conn(&conn, &input)
}
