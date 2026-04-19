use crate::live_runtime::manager::LiveRuntimeManager;
use crate::live_runtime::session::runtime_current_node_for_status;
use crate::tiktok::types::LiveStatus;
use crate::workflow::runtime_store;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[cfg(test)]
use crate::live_runtime::types::LiveRuntimeSessionSnapshot;

#[cfg(test)]
pub fn list_live_runtime_sessions_from_manager(
    manager: &LiveRuntimeManager,
) -> Vec<LiveRuntimeSessionSnapshot> {
    manager.list_sessions()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct FlowRuntimeSnapshot {
    pub flow_id: i64,
    pub status: String,
    pub current_node: Option<String>,
    pub account_id: Option<i64>,
    pub username: String,
    pub last_live_at: Option<String>,
    pub last_error: Option<String>,
    pub active_flow_run_id: Option<i64>,
}

fn load_runtime_account_id(
    conn: &rusqlite::Connection,
    username: &str,
) -> Result<Option<i64>, String> {
    crate::live_runtime::account_binding::find_account_by_start_username(conn, Some(username))
        .map(|row| row.map(|(account_id, _)| account_id))
}

fn load_runtime_last_live_at(
    conn: &rusqlite::Connection,
    flow_id: i64,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT json_extract(published_config_json, '$.last_live_at') FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
        [flow_id],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}

fn load_runtime_current_node(
    conn: &rusqlite::Connection,
    flow_id: i64,
    status: &str,
) -> Result<Option<String>, String> {
    let current_node: Option<String> = conn
        .query_row(
            "SELECT current_node FROM flows WHERE id = ?1",
            [flow_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(current_node.or_else(|| runtime_current_node_for_status(status).map(str::to_string)))
}

pub fn list_live_runtime_snapshots_with_conn(
    conn: &rusqlite::Connection,
    manager: &LiveRuntimeManager,
) -> Result<Vec<FlowRuntimeSnapshot>, String> {
    manager
        .list_sessions()
        .into_iter()
        .map(|session| {
            Ok(FlowRuntimeSnapshot {
                flow_id: session.flow_id,
                status: session.status.clone(),
                current_node: load_runtime_current_node(conn, session.flow_id, &session.status)?,
                account_id: load_runtime_account_id(conn, &session.username)?,
                username: session.username,
                last_live_at: load_runtime_last_live_at(conn, session.flow_id)?,
                last_error: session.last_error,
                active_flow_run_id: runtime_store::load_latest_running_flow_run_id(
                    conn,
                    session.flow_id,
                )?,
            })
        })
        .collect()
}

#[tauri::command]
pub fn list_live_runtime_sessions(
    state: State<'_, AppState>,
    manager: State<'_, LiveRuntimeManager>,
) -> Result<Vec<FlowRuntimeSnapshot>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    list_live_runtime_snapshots_with_conn(&conn, &manager)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TriggerStartLiveDetectedInput {
    pub flow_id: i64,
    pub room_id: String,
    pub stream_url: Option<String>,
    pub viewer_count: Option<i64>,
}

pub fn trigger_start_live_detected_with_conn(
    conn: &rusqlite::Connection,
    manager: &LiveRuntimeManager,
    input: TriggerStartLiveDetectedInput,
) -> Result<Option<i64>, String> {
    if input.flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }
    if input.room_id.trim().is_empty() {
        return Err("room_id is required".to_string());
    }

    manager.handle_live_detected(
        conn,
        input.flow_id,
        &LiveStatus {
            room_id: input.room_id,
            stream_url: input.stream_url,
            viewer_count: input.viewer_count,
        },
    )
}

#[tauri::command]
pub fn trigger_start_live_detected(
    state: State<'_, AppState>,
    manager: State<'_, LiveRuntimeManager>,
    input: TriggerStartLiveDetectedInput,
) -> Result<Option<i64>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    trigger_start_live_detected_with_conn(&conn, &manager, input)
}

#[tauri::command]
pub fn mark_start_run_completed(
    state: State<'_, AppState>,
    manager: State<'_, LiveRuntimeManager>,
    flow_id: i64,
    room_id: Option<String>,
) -> Result<(), String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }

    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    manager.complete_active_run(&mut conn, flow_id, room_id.as_deref())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn mark_source_offline_with_conn(
    manager: &LiveRuntimeManager,
    flow_id: i64,
) -> Result<(), String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }
    manager.mark_source_offline(flow_id)
}

#[tauri::command]
#[cfg_attr(test, allow(dead_code))]
pub fn mark_source_offline(
    manager: State<'_, LiveRuntimeManager>,
    flow_id: i64,
) -> Result<(), String> {
    mark_source_offline_with_conn(&manager, flow_id)
}

#[cfg(test)]
mod tests {
    use super::{
        list_live_runtime_sessions_from_manager, list_live_runtime_snapshots_with_conn,
        mark_source_offline_with_conn, trigger_start_live_detected_with_conn,
        TriggerStartLiveDetectedInput,
    };
    use crate::db::init::initialize_database;
    use crate::live_runtime::manager::LiveRuntimeManager;
    use rusqlite::{params, Connection};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn open_temp_db() -> (Connection, PathBuf) {
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tikclip-live-runtime-command-test-{}-{}-{}.db",
            std::process::id(),
            counter,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let conn = initialize_database(&path).expect("init db");
        (conn, path)
    }

    fn insert_flow(conn: &Connection, flow_id: i64, username: &str) {
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) \
             VALUES (?1, ?2, 1, 'idle', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            params![flow_id, format!("Flow {flow_id}")],
        )
        .expect("insert flow");
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
             VALUES (?1, 'start', 1, ?2, ?2, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            params![flow_id, format!(r#"{{"username":"{username}"}}"#)],
        )
        .expect("insert start node");
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
             VALUES (?1, 'record', 2, '{\"max_duration_minutes\":5}', '{\"max_duration_minutes\":5}', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            params![flow_id],
        )
        .expect("insert record node");
    }

    #[test]
    fn session_listing_command_returns_snapshots() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 7, "shop_abc");
        let manager = LiveRuntimeManager::new();
        manager.start_flow_session(&conn, 7).expect("start flow");

        let snapshots = list_live_runtime_sessions_from_manager(&manager);

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].flow_id, 7);
        assert_eq!(snapshots[0].flow_name, "Flow 7");
        assert_eq!(snapshots[0].username, "shop_abc");

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn session_listing_command_shows_failure_state_after_restart_conflict() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 7, "shop_abc");
        insert_flow(&conn, 8, "shop_xyz");
        let manager = LiveRuntimeManager::new();
        manager.start_flow_session(&conn, 7).expect("start flow 7");
        manager.start_flow_session(&conn, 8).expect("start flow 8");
        conn.execute(
            "UPDATE flow_nodes SET published_config_json = ?1 WHERE flow_id = 7 AND node_key = 'start'",
            [r#"{"username":"shop_xyz"}"#],
        )
        .expect("make conflicting published config");

        let err = manager.reconcile_flow(&conn, 7).unwrap_err();

        assert!(err.contains("username lease already held"));
        let snapshots = list_live_runtime_sessions_from_manager(&manager);
        let snapshot = snapshots
            .into_iter()
            .find(|snapshot| snapshot.flow_id == 7)
            .expect("failed snapshot");
        assert_eq!(snapshot.status, "error");
        assert!(snapshot
            .last_error
            .unwrap_or_default()
            .contains("username lease already held"));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn session_listing_command_shows_failure_state_after_enable_start_error() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 7, "shop_abc");
        insert_flow(&conn, 8, "@shop_abc");
        let manager = LiveRuntimeManager::new();
        manager.start_flow_session(&conn, 7).expect("start flow 7");

        let err = manager.start_flow_session(&conn, 8).unwrap_err();

        assert!(err.contains("username lease already held"));
        let snapshots = list_live_runtime_sessions_from_manager(&manager);
        let snapshot = snapshots
            .into_iter()
            .find(|snapshot| snapshot.flow_id == 8)
            .expect("failed snapshot");
        assert_eq!(snapshot.status, "error");
        assert!(snapshot
            .last_error
            .unwrap_or_default()
            .contains("username lease already held"));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn trigger_start_live_detected_command_uses_manager_backed_transition() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 7, "shop_abc");
        let manager = LiveRuntimeManager::new();
        manager.start_flow_session(&conn, 7).expect("start flow");

        let flow_run_id = trigger_start_live_detected_with_conn(
            &conn,
            &manager,
            TriggerStartLiveDetectedInput {
                flow_id: 7,
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(77),
            },
        )
        .expect("trigger live detected")
        .expect("created run");

        let output_json: String = conn
            .query_row(
                "SELECT output_json FROM flow_node_runs WHERE flow_run_id = ?1 AND node_key = 'start'",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read start output");
        assert!(output_json.contains("7312345"));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn runtime_snapshot_listing_exposes_frontend_runtime_fields_for_active_recording() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 7, "shop_abc");
        let manager = LiveRuntimeManager::new();
        manager.start_flow_session(&conn, 7).expect("start flow");
        let flow_run_id = trigger_start_live_detected_with_conn(
            &conn,
            &manager,
            TriggerStartLiveDetectedInput {
                flow_id: 7,
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(77),
            },
        )
        .expect("trigger live detected")
        .expect("created run");
        let account_id: i64 = conn
            .query_row(
                "SELECT id FROM accounts WHERE username = 'shop_abc' ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("read account id");

        let snapshots =
            list_live_runtime_snapshots_with_conn(&conn, &manager).expect("list runtime snapshots");
        let snapshot = snapshots
            .into_iter()
            .find(|snapshot| snapshot.flow_id == 7)
            .expect("runtime snapshot");

        assert_eq!(snapshot.status, "recording");
        assert_eq!(snapshot.current_node.as_deref(), Some("record"));
        assert_eq!(snapshot.active_flow_run_id, Some(flow_run_id));
        assert_eq!(snapshot.account_id, Some(account_id));
        assert_eq!(snapshot.username, "shop_abc");

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn runtime_snapshot_listing_preserves_caption_stage_after_downstream_progress() {
        let (mut conn, path) = open_temp_db();
        insert_flow(&conn, 7, "shop_abc");
        let manager = LiveRuntimeManager::new();
        manager.start_flow_session(&conn, 7).expect("start flow");
        let flow_run_id = trigger_start_live_detected_with_conn(
            &conn,
            &manager,
            TriggerStartLiveDetectedInput {
                flow_id: 7,
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(77),
            },
        )
        .expect("trigger live detected")
        .expect("created run");
        manager
            .complete_active_run(&mut conn, 7, Some("7312345"))
            .expect("complete active run");
        conn.execute("UPDATE flows SET current_node = 'caption' WHERE id = 7", [])
            .expect("set caption stage");

        let snapshots =
            list_live_runtime_snapshots_with_conn(&conn, &manager).expect("list runtime snapshots");
        let snapshot = snapshots
            .into_iter()
            .find(|snapshot| snapshot.flow_id == 7)
            .expect("runtime snapshot");

        assert_eq!(snapshot.status, "processing");
        assert_eq!(snapshot.current_node.as_deref(), Some("caption"));
        assert_eq!(snapshot.active_flow_run_id, Some(flow_run_id));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn mark_source_offline_command_resets_same_room_dedupe_for_next_live_cycle() {
        let (mut conn, path) = open_temp_db();
        insert_flow(&conn, 7, "shop_abc");
        let manager = LiveRuntimeManager::new();
        manager.start_flow_session(&conn, 7).expect("start flow");
        let first_run_id = trigger_start_live_detected_with_conn(
            &conn,
            &manager,
            TriggerStartLiveDetectedInput {
                flow_id: 7,
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(77),
            },
        )
        .expect("trigger first live")
        .expect("created first run");
        manager
            .complete_active_run(&mut conn, 7, Some("7312345"))
            .expect("complete first run");

        let duplicate = trigger_start_live_detected_with_conn(
            &conn,
            &manager,
            TriggerStartLiveDetectedInput {
                flow_id: 7,
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(88),
            },
        )
        .expect("trigger duplicate live");
        assert_eq!(duplicate, None);

        mark_source_offline_with_conn(&manager, 7).expect("mark source offline");

        let second_run_id = trigger_start_live_detected_with_conn(
            &conn,
            &manager,
            TriggerStartLiveDetectedInput {
                flow_id: 7,
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(99),
            },
        )
        .expect("trigger live after offline")
        .expect("created second run");

        assert_ne!(first_run_id, second_run_id);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }
}
