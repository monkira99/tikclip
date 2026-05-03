use crate::live_runtime::account_binding::find_flow_id_for_account;
use crate::live_runtime::manager::LiveRuntimeManager;
use crate::recording_runtime::types::{
    RecordingFinishInput, RecordingStartInput, RustRecordingUpsertInput,
};
use crate::recording_runtime::worker;
use crate::time_hcm::SQL_NOW_HCM;
use crate::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SyncRecordingInput {
    pub external_recording_id: String,
    pub account_id: i64,
    pub status: String,
    pub duration_seconds: i64,
    pub file_size_bytes: i64,
    pub file_path: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ActiveRustRecordingStatus {
    pub recording_id: String,
    pub account_id: i64,
    pub username: String,
    pub status: String,
    pub duration_seconds: i64,
    pub file_size_bytes: i64,
    pub file_path: Option<String>,
    pub error_message: Option<String>,
}

fn map_external_recording_status(status: &str) -> &'static str {
    match status {
        "completed" | "stopped" => "done",
        "error" => "error",
        "processing" => "processing",
        _ => "recording",
    }
}

#[allow(dead_code)]
pub fn map_rust_recording_status(runtime_status: &str) -> &'static str {
    match runtime_status.trim() {
        "pending" | "recording" => "recording",
        "completed" | "stopped" => "done",
        "cancelled" => "cancelled",
        "error" | "failed" => "error",
        _ => "recording",
    }
}

#[allow(dead_code)]
pub fn upsert_rust_recording(
    conn: &Connection,
    input: &RustRecordingUpsertInput,
) -> Result<i64, String> {
    if input.external_recording_id.trim().is_empty() {
        return Err("external_recording_id is required".to_string());
    }

    let status = map_rust_recording_status(input.runtime_status.as_str());
    let is_terminal = matches!(status, "done" | "error" | "cancelled");
    let existing_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM recordings WHERE external_recording_id = ?1",
            [&input.external_recording_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some(id) = existing_id {
        let existing_row: (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, ended_at, error_message FROM recordings WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|e| e.to_string())?;
        let was_terminal = matches!(existing_row.0.as_str(), "done" | "error");
        let next_status = if was_terminal {
            existing_row.0.as_str()
        } else {
            status
        };
        let next_error_message = match (was_terminal, is_terminal, input.error_message.as_deref()) {
            (true, true, Some(message)) if !message.trim().is_empty() => Some(message),
            (true, _, _) => existing_row.2.as_deref(),
            (false, true, Some(message)) if !message.trim().is_empty() => Some(message),
            (false, _, _) => input.error_message.as_deref(),
        };

        conn.execute(
            &format!(
                "UPDATE recordings SET \
                 status = ?1, duration_seconds = ?2, file_size_bytes = ?3, \
                 file_path = COALESCE(?4, file_path), \
                 error_message = ?5, \
                 ended_at = CASE \
                     WHEN ?6 != 0 AND ended_at IS NULL THEN {} \
                     ELSE ended_at END \
                 WHERE id = ?7",
                SQL_NOW_HCM
            ),
            params![
                next_status,
                input.duration_seconds,
                input.file_size_bytes,
                input.file_path.as_deref(),
                next_error_message,
                if existing_row.1.is_some() || is_terminal {
                    1i64
                } else {
                    0i64
                },
                id,
            ],
        )
        .map_err(|e| e.to_string())?;

        if input.room_id.is_some() {
            conn.execute(
                "UPDATE recordings SET room_id = COALESCE(room_id, ?1) WHERE id = ?2",
                params![input.room_id.as_deref(), id],
            )
            .map_err(|e| e.to_string())?;
        }

        if input.flow_run_id.is_some() {
            conn.execute(
                "UPDATE recordings SET flow_run_id = COALESCE(flow_run_id, ?1) WHERE id = ?2",
                params![input.flow_run_id, id],
            )
            .map_err(|e| e.to_string())?;
        }

        conn.execute(
            "UPDATE recordings SET flow_id = COALESCE(flow_id, ?1) WHERE id = ?2",
            params![input.flow_id, id],
        )
        .map_err(|e| e.to_string())?;

        Ok(id)
    } else {
        conn.execute(
            &format!(
                "INSERT INTO recordings (\
                   account_id, room_id, status, duration_seconds, file_size_bytes, flow_id, flow_run_id, \
                   file_path, error_message, external_recording_id, started_at, created_at, ended_at\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, {}, {}, \
                   CASE WHEN ?11 != 0 THEN {} ELSE NULL END)",
                SQL_NOW_HCM, SQL_NOW_HCM, SQL_NOW_HCM
            ),
            params![
                input.account_id,
                input.room_id.as_deref(),
                status,
                input.duration_seconds,
                input.file_size_bytes,
                input.flow_id,
                input.flow_run_id,
                input.file_path.as_deref(),
                input.error_message.as_deref(),
                &input.external_recording_id,
                if is_terminal { 1i64 } else { 0i64 },
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(conn.last_insert_rowid())
    }
}

#[allow(dead_code)]
pub fn start_rust_recording_row(
    conn: &Connection,
    input: &RecordingStartInput,
) -> Result<(i64, String), String> {
    let output_path = worker::build_recording_output_path(input);
    if let Some(parent) = std::path::Path::new(&output_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let recording_id = upsert_rust_recording(
        conn,
        &RustRecordingUpsertInput {
            account_id: input.account_id,
            flow_id: input.flow_id,
            flow_run_id: Some(input.flow_run_id),
            external_recording_id: input.external_recording_id.clone(),
            runtime_status: "recording".to_string(),
            room_id: Some(input.room_id.clone()),
            file_path: Some(output_path.clone()),
            error_message: None,
            duration_seconds: 0,
            file_size_bytes: 0,
        },
    )?;
    conn.execute(
        "UPDATE recordings SET stream_url = ?1 WHERE id = ?2",
        params![&input.stream_url, recording_id],
    )
    .map_err(|e| e.to_string())?;
    Ok((recording_id, output_path))
}

#[allow(dead_code)]
pub fn finalize_rust_recording_row(
    conn: &Connection,
    input: &RecordingFinishInput,
) -> Result<i64, String> {
    upsert_rust_recording(
        conn,
        &RustRecordingUpsertInput {
            account_id: input.account_id,
            flow_id: input.flow_id,
            flow_run_id: Some(input.flow_run_id),
            external_recording_id: input.external_recording_id.clone(),
            runtime_status: match input.outcome {
                crate::recording_runtime::types::RecordingOutcome::Success => "completed",
                crate::recording_runtime::types::RecordingOutcome::Failed => "error",
                crate::recording_runtime::types::RecordingOutcome::Cancelled => "cancelled",
            }
            .to_string(),
            room_id: Some(input.room_id.clone()),
            file_path: input.file_path.clone(),
            error_message: input.error_message.clone(),
            duration_seconds: input.duration_seconds,
            file_size_bytes: input.file_size_bytes,
        },
    )
}

#[tauri::command]
pub fn list_active_rust_recordings(
    state: State<'_, AppState>,
    runtime_manager: State<'_, LiveRuntimeManager>,
) -> Result<Vec<ActiveRustRecordingStatus>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    runtime_manager.list_active_rust_recordings(&conn)
}

/// Shared upsert used by runtime clip/recording reconciliation.
pub(super) fn sync_recording_from_external_key_conn(
    conn: &Connection,
    input: &SyncRecordingInput,
) -> Result<i64, String> {
    if input.external_recording_id.trim().is_empty() {
        return Err("external_recording_id is required".to_string());
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

    let mapped = map_external_recording_status(input.status.trim());
    let path = input.file_path.as_deref();
    let err = input.error_message.as_deref();
    let flow_id = find_flow_id_for_account(conn, input.account_id)?;

    let flow_run_id: Option<i64> = match flow_id {
        Some(fid) => conn
            .query_row(
                "SELECT id FROM flow_runs WHERE flow_id = ?1 AND status = 'running' \
                 ORDER BY id DESC LIMIT 1",
                [fid],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?,
        None => None,
    };

    let existing_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM recordings WHERE external_recording_id = ?1",
            [&input.external_recording_id],
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
                 flow_id = COALESCE(flow_id, ?6), \
                 flow_run_id = COALESCE(flow_run_id, ?7), \
                 ended_at = CASE \
                     WHEN ?1 IN ('done', 'error', 'processing') AND ended_at IS NULL \
                     THEN {} ELSE ended_at END \
                 WHERE id = ?8",
                SQL_NOW_HCM
            ),
            params![
                mapped,
                input.duration_seconds,
                input.file_size_bytes,
                path,
                err,
                flow_id,
                flow_run_id,
                id
            ],
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
                   account_id, room_id, status, duration_seconds, file_size_bytes, flow_id, flow_run_id, \
                   file_path, error_message, external_recording_id, started_at, created_at, ended_at\
                 ) VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, {}, {}, \
                   CASE WHEN ?10 != 0 THEN {} ELSE NULL END)",
                SQL_NOW_HCM, SQL_NOW_HCM, SQL_NOW_HCM
            ),
            params![
                input.account_id,
                mapped,
                input.duration_seconds,
                input.file_size_bytes,
                flow_id,
                flow_run_id,
                path,
                err,
                &input.external_recording_id,
                ended_flag,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        map_rust_recording_status, sync_recording_from_external_key_conn, upsert_rust_recording,
        SyncRecordingInput,
    };
    use crate::recording_runtime::types::RustRecordingUpsertInput;
    use rusqlite::Connection;

    fn open_in_memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL DEFAULT '',
                type TEXT NOT NULL DEFAULT 'monitored',
                created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'))
            );
            CREATE TABLE flows (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                status TEXT NOT NULL DEFAULT 'idle',
                current_node TEXT,
                published_version INTEGER NOT NULL DEFAULT 1,
                draft_version INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'))
            );
            CREATE TABLE flow_nodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
                node_key TEXT NOT NULL,
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
                status TEXT NOT NULL,
                started_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                ended_at TEXT,
                trigger_reason TEXT,
                error TEXT
            );
            CREATE TABLE recordings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                room_id TEXT,
                status TEXT NOT NULL DEFAULT 'recording' CHECK (status IN ('recording', 'done', 'error', 'processing', 'cancelled')),
                started_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                ended_at TEXT,
                duration_seconds INTEGER NOT NULL DEFAULT 0,
                file_path TEXT,
                file_size_bytes INTEGER NOT NULL DEFAULT 0,
                stream_url TEXT,
                bitrate TEXT,
                error_message TEXT,
                auto_process INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                flow_id INTEGER REFERENCES flows(id) ON DELETE SET NULL,
                flow_run_id INTEGER REFERENCES flow_runs(id) ON DELETE SET NULL,
                external_recording_id TEXT
            );
            CREATE UNIQUE INDEX idx_recordings_external_recording_id ON recordings(external_recording_id) WHERE external_recording_id IS NOT NULL;",
        )
        .expect("create test schema");
        conn
    }

    fn seed_runtime_recording_ownership(conn: &Connection) {
        conn.execute(
            "INSERT INTO accounts (id, username, display_name, type, created_at, updated_at) \
             VALUES (9, 'shop_abc', 'Shop ABC', 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert account 9");
        conn.execute(
            "INSERT INTO accounts (id, username, display_name, type, created_at, updated_at) \
             VALUES (12, 'shop_xyz', 'Shop XYZ', 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert account 12");
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) \
             VALUES (3, 'Flow 3', 1, 'watching', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert flow 3");
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) \
             VALUES (7, 'Flow 7', 1, 'watching', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert flow 7");
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (11, 3, 1, 'running', datetime('now','+7 hours'), 'test')",
            [],
        )
        .expect("insert flow run 11");
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (22, 7, 1, 'running', datetime('now','+7 hours'), 'rebind-attempt')",
            [],
        )
        .expect("insert flow run 22");
    }

    fn insert_account_and_flow(conn: &Connection) {
        conn.execute(
            "INSERT INTO accounts (id, username, display_name, type, created_at, updated_at) \
             VALUES (1, 'shop_abc', 'A', 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert account");
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) \
             VALUES (11, 'Flow', 1, 'idle', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert flow");
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
             VALUES (11, 'start', 1, '{\"username\":\"shop_abc\"}', '{\"username\":\"shop_abc\"}', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert start node");
    }

    #[test]
    fn sync_recording_from_external_key_keeps_existing_flow_and_run_binding_on_later_updates() {
        let conn = open_in_memory_db();
        insert_account_and_flow(&conn);
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (21, 11, 1, 'running', datetime('now','+7 hours'), 'first')",
            [],
        )
        .expect("insert first running run");

        let recording_id = sync_recording_from_external_key_conn(
            &conn,
            &SyncRecordingInput {
                external_recording_id: "ext-123".to_string(),
                account_id: 1,
                status: "recording".to_string(),
                duration_seconds: 5,
                file_size_bytes: 10,
                file_path: None,
                error_message: None,
            },
        )
        .expect("insert recording");
        conn.execute(
            "UPDATE flow_runs SET status = 'cancelled', ended_at = datetime('now','+7 hours') WHERE id = 21",
            [],
        )
        .expect("cancel old run");
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (22, 11, 1, 'running', datetime('now','+7 hours'), 'second')",
            [],
        )
        .expect("insert second running run");

        sync_recording_from_external_key_conn(
            &conn,
            &SyncRecordingInput {
                external_recording_id: "ext-123".to_string(),
                account_id: 1,
                status: "completed".to_string(),
                duration_seconds: 15,
                file_size_bytes: 20,
                file_path: Some("/tmp/out.mp4".to_string()),
                error_message: None,
            },
        )
        .expect("update recording");

        let binding: (Option<i64>, Option<i64>, String) = conn
            .query_row(
                "SELECT flow_id, flow_run_id, status FROM recordings WHERE id = ?1",
                [recording_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read recording binding");
        assert_eq!(binding.0, Some(11));
        assert_eq!(binding.1, Some(21));
        assert_eq!(binding.2, "done");
    }

    #[test]
    fn upsert_rust_recording_keeps_account_flow_and_run_ownership() {
        let conn = open_in_memory_db();
        seed_runtime_recording_ownership(&conn);

        let recording_id = upsert_rust_recording(
            &conn,
            &RustRecordingUpsertInput {
                account_id: 9,
                flow_id: 3,
                flow_run_id: Some(11),
                external_recording_id: "ext-123".to_string(),
                runtime_status: "recording".to_string(),
                room_id: Some("7312345".to_string()),
                file_path: None,
                error_message: None,
                duration_seconds: 0,
                file_size_bytes: 0,
            },
        )
        .expect("insert rust recording");

        upsert_rust_recording(
            &conn,
            &RustRecordingUpsertInput {
                account_id: 12,
                flow_id: 7,
                flow_run_id: Some(22),
                external_recording_id: "ext-123".to_string(),
                runtime_status: "completed".to_string(),
                room_id: Some("9999999".to_string()),
                file_path: Some("/tmp/out.mp4".to_string()),
                error_message: None,
                duration_seconds: 33,
                file_size_bytes: 44,
            },
        )
        .expect("update rust recording without rebinding");

        let row: (i64, Option<i64>, Option<i64>, Option<String>, String, Option<String>) = conn
            .query_row(
                "SELECT account_id, flow_id, flow_run_id, room_id, status, file_path FROM recordings WHERE id = ?1",
                [recording_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("read rust recording row");

        assert_eq!(row.0, 9);
        assert_eq!(row.1, Some(3));
        assert_eq!(row.2, Some(11));
        assert_eq!(row.3.as_deref(), Some("7312345"));
        assert_eq!(row.4, "done");
        assert_eq!(row.5.as_deref(), Some("/tmp/out.mp4"));
    }

    #[test]
    fn map_rust_recording_status_maps_runtime_states_to_db_states() {
        assert_eq!(map_rust_recording_status("pending"), "recording");
        assert_eq!(map_rust_recording_status("recording"), "recording");
        assert_eq!(map_rust_recording_status("completed"), "done");
        assert_eq!(map_rust_recording_status("stopped"), "done");
        assert_eq!(map_rust_recording_status("error"), "error");
    }

    #[test]
    fn completed_or_error_recording_sets_ended_at_and_error_message() {
        let conn = open_in_memory_db();
        seed_runtime_recording_ownership(&conn);

        let recording_id = upsert_rust_recording(
            &conn,
            &RustRecordingUpsertInput {
                account_id: 9,
                flow_id: 3,
                flow_run_id: Some(11),
                external_recording_id: "ext-456".to_string(),
                runtime_status: "error".to_string(),
                room_id: Some("7319999".to_string()),
                file_path: Some("/tmp/out.mp4".to_string()),
                error_message: Some("ffmpeg failed".to_string()),
                duration_seconds: 12,
                file_size_bytes: 99,
            },
        )
        .expect("insert failed rust recording");

        let row: (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, ended_at, error_message FROM recordings WHERE id = ?1",
                [recording_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read terminal rust recording");

        assert_eq!(row.0, "error");
        assert!(row.1.is_some());
        assert_eq!(row.2.as_deref(), Some("ffmpeg failed"));
    }

    #[test]
    fn upsert_rust_recording_does_not_regress_done_row_on_late_progress_update() {
        let conn = open_in_memory_db();
        seed_runtime_recording_ownership(&conn);

        let recording_id = upsert_rust_recording(
            &conn,
            &RustRecordingUpsertInput {
                account_id: 9,
                flow_id: 3,
                flow_run_id: Some(11),
                external_recording_id: "ext-late-done".to_string(),
                runtime_status: "completed".to_string(),
                room_id: Some("7312345".to_string()),
                file_path: Some("/tmp/done.mp4".to_string()),
                error_message: None,
                duration_seconds: 120,
                file_size_bytes: 1000,
            },
        )
        .expect("insert completed row");

        upsert_rust_recording(
            &conn,
            &RustRecordingUpsertInput {
                account_id: 9,
                flow_id: 3,
                flow_run_id: Some(11),
                external_recording_id: "ext-late-done".to_string(),
                runtime_status: "recording".to_string(),
                room_id: Some("7312345".to_string()),
                file_path: None,
                error_message: None,
                duration_seconds: 130,
                file_size_bytes: 1100,
            },
        )
        .expect("apply late progress update");

        let row: (String, Option<String>) = conn
            .query_row(
                "SELECT status, ended_at FROM recordings WHERE id = ?1",
                [recording_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read terminal row");

        assert_eq!(row.0, "done");
        assert!(row.1.is_some());
    }

    #[test]
    fn upsert_rust_recording_preserves_terminal_error_message_until_new_terminal_error() {
        let conn = open_in_memory_db();
        seed_runtime_recording_ownership(&conn);

        let recording_id = upsert_rust_recording(
            &conn,
            &RustRecordingUpsertInput {
                account_id: 9,
                flow_id: 3,
                flow_run_id: Some(11),
                external_recording_id: "ext-late-error".to_string(),
                runtime_status: "error".to_string(),
                room_id: Some("7319999".to_string()),
                file_path: Some("/tmp/error.mp4".to_string()),
                error_message: Some("ffmpeg failed".to_string()),
                duration_seconds: 12,
                file_size_bytes: 99,
            },
        )
        .expect("insert error row");

        upsert_rust_recording(
            &conn,
            &RustRecordingUpsertInput {
                account_id: 9,
                flow_id: 3,
                flow_run_id: Some(11),
                external_recording_id: "ext-late-error".to_string(),
                runtime_status: "recording".to_string(),
                room_id: Some("7319999".to_string()),
                file_path: None,
                error_message: Some("late progress noise".to_string()),
                duration_seconds: 13,
                file_size_bytes: 100,
            },
        )
        .expect("apply late progress update");

        let after_progress: (String, Option<String>) = conn
            .query_row(
                "SELECT status, error_message FROM recordings WHERE id = ?1",
                [recording_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read row after progress update");
        assert_eq!(after_progress.0, "error");
        assert_eq!(after_progress.1.as_deref(), Some("ffmpeg failed"));

        upsert_rust_recording(
            &conn,
            &RustRecordingUpsertInput {
                account_id: 9,
                flow_id: 3,
                flow_run_id: Some(11),
                external_recording_id: "ext-late-error".to_string(),
                runtime_status: "error".to_string(),
                room_id: Some("7319999".to_string()),
                file_path: None,
                error_message: Some("disk full".to_string()),
                duration_seconds: 14,
                file_size_bytes: 101,
            },
        )
        .expect("apply terminal error update");

        let final_error: Option<String> = conn
            .query_row(
                "SELECT error_message FROM recordings WHERE id = ?1",
                [recording_id],
                |row| row.get(0),
            )
            .expect("read final error message");
        assert_eq!(final_error.as_deref(), Some("disk full"));
    }

    #[test]
    fn map_rust_recording_status_maps_cancelled_to_cancelled() {
        assert_eq!(map_rust_recording_status("cancelled"), "cancelled");
    }

    #[test]
    fn cancelled_rust_recording_sets_terminal_status_and_error_message() {
        let conn = open_in_memory_db();
        seed_runtime_recording_ownership(&conn);

        let recording_id = upsert_rust_recording(
            &conn,
            &RustRecordingUpsertInput {
                account_id: 9,
                flow_id: 3,
                flow_run_id: Some(11),
                external_recording_id: "ext-cancelled".to_string(),
                runtime_status: "cancelled".to_string(),
                room_id: Some("7319999".to_string()),
                file_path: Some("/tmp/out.mp4".to_string()),
                error_message: Some("Recording cancelled".to_string()),
                duration_seconds: 0,
                file_size_bytes: 0,
            },
        )
        .expect("insert cancelled rust recording");

        let row: (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, ended_at, error_message FROM recordings WHERE id = ?1",
                [recording_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read cancelled recording row");

        assert_eq!(row.0, "cancelled");
        assert!(row.1.is_some());
        assert_eq!(row.2.as_deref(), Some("Recording cancelled"));
    }
}
