use crate::db::models::{Flow, FlowNodeDefinition};
use crate::live_runtime::account_binding::find_account_by_start_username;
use crate::live_runtime::manager::LiveRuntimeManager;
use crate::time_hcm::SQL_NOW_HCM;
use crate::workflow::node_runner;
use crate::workflow::runtime_store;
use crate::workflow::types::{FlowNodeRun, FlowRun};
use crate::workflow::{record_node, start_node};
use crate::AppState;
use rusqlite::{params, Connection, Row};
use serde::Serialize;
use tauri::State;

const FLOW_NODE_KEYS: [&str; 5] = ["start", "record", "clip", "caption", "upload"];

fn is_valid_flow_node(node_key: &str) -> bool {
    FLOW_NODE_KEYS.contains(&node_key)
}

fn map_flow_run_row(row: &Row) -> rusqlite::Result<FlowRun> {
    Ok(FlowRun {
        id: row.get(0)?,
        flow_id: row.get(1)?,
        definition_version: row.get(2)?,
        status: row.get(3)?,
        started_at: row.get(4)?,
        ended_at: row.get(5)?,
        trigger_reason: row.get(6)?,
        error: row.get(7)?,
    })
}

fn map_flow_node_run_row(row: &Row) -> rusqlite::Result<FlowNodeRun> {
    Ok(FlowNodeRun {
        id: row.get(0)?,
        flow_run_id: row.get(1)?,
        flow_id: row.get(2)?,
        node_key: row.get(3)?,
        status: row.get(4)?,
        started_at: row.get(5)?,
        ended_at: row.get(6)?,
        input_json: row.get(7)?,
        output_json: row.get(8)?,
        error: row.get(9)?,
    })
}

fn map_flow_definition_row(row: &Row) -> rusqlite::Result<Flow> {
    Ok(Flow {
        id: row.get(0)?,
        account_id: row.get(1)?,
        name: row.get(2)?,
        enabled: row.get::<_, i64>(3)? != 0,
        status: row.get(4)?,
        current_node: row.get(5)?,
        last_live_at: row.get(6)?,
        last_run_at: row.get(7)?,
        last_error: row.get(8)?,
        published_version: row.get(9)?,
        draft_version: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

pub(crate) fn load_flow_definition(conn: &Connection, flow_id: i64) -> Result<Flow, String> {
    let flow = conn
        .query_row(
            "SELECT f.id, \
             0, \
             f.name, f.enabled, f.status, f.current_node, \
             json_extract(n.published_config_json, '$.last_live_at'), \
             json_extract(n.published_config_json, '$.last_run_at'), \
             json_extract(n.published_config_json, '$.last_error'), \
             f.published_version, f.draft_version, \
             f.created_at, f.updated_at \
             FROM flows f \
             LEFT JOIN flow_nodes n ON n.flow_id = f.id AND n.node_key = 'start' \
             WHERE f.id = ?1",
            [flow_id],
            map_flow_definition_row,
        )
        .map_err(|e| e.to_string())?;
    let start_username: Option<String> = conn
        .query_row(
            "SELECT json_extract(published_config_json, '$.username') FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
            [flow_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let account_id = find_account_by_start_username(conn, start_username.as_deref())?
        .map(|(account_id, _)| account_id)
        .unwrap_or(0);

    Ok(Flow { account_id, ..flow })
}

fn load_flow_runs(conn: &Connection, flow_id: i64) -> Result<Vec<FlowRun>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, flow_id, definition_version, status, started_at, ended_at, trigger_reason, error \
             FROM flow_runs WHERE flow_id = ?1 ORDER BY id DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([flow_id], map_flow_run_row)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn load_flow_node_runs(conn: &Connection, flow_id: i64) -> Result<Vec<FlowNodeRun>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, flow_run_id, flow_id, node_key, status, started_at, ended_at, input_json, output_json, error \
             FROM flow_node_runs WHERE flow_id = ?1 ORDER BY id DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([flow_id], map_flow_node_run_row)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowEditorPayload {
    pub flow: Flow,
    pub nodes: Vec<FlowNodeDefinition>,
    pub runs: Vec<FlowRun>,
    pub node_runs: Vec<FlowNodeRun>,
    #[serde(rename = "recordings_count")]
    pub recordings_count: i64,
    #[serde(rename = "clips_count")]
    pub clips_count: i64,
}

fn get_flow_definition_with_conn(
    conn: &Connection,
    flow_id: i64,
) -> Result<FlowEditorPayload, String> {
    let flow = load_flow_definition(conn, flow_id)?;
    let nodes = runtime_store::list_flow_node_definitions(conn, flow_id)?;
    if let Some(start_def) = nodes.iter().find(|d| d.node_key == "start") {
        if start_node::parse_start_config(start_def.published_config_json.as_str()).is_ok() {
            let preview = node_runner::run_node(start_def, None)?;
            let expected = node_runner::next_node_key("start");
            let ok = match (&preview.next_node, expected) {
                (Some(next), Some(exp)) => next == exp,
                (None, None) => true,
                _ => false,
            };
            if !ok {
                return Err("engine pipeline misaligned for start node".to_string());
            }
            log::trace!(
                "flow {} start engine preview status={} next={:?} err={:?} out={:?}",
                flow_id,
                preview.status,
                preview.next_node,
                preview.error,
                preview.output_json
            );
        }
    }
    let runs = load_flow_runs(conn, flow_id)?;
    let node_runs = load_flow_node_runs(conn, flow_id)?;
    let recordings_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM recordings WHERE flow_id = ?1",
            [flow_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let clips_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clips WHERE flow_id = ?1",
            [flow_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(FlowEditorPayload {
        flow,
        nodes,
        runs,
        node_runs,
        recordings_count,
        clips_count,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishFlowResult {
    pub flow_id: i64,
    pub is_running: bool,
}

/// Persists draft JSON for one node (engine reads published only until publish).
pub(crate) fn apply_flow_node_draft(
    conn: &Connection,
    flow_id: i64,
    node_key: &str,
    draft_config_json: &str,
) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(draft_config_json)
        .map_err(|e| format!("draft_config_json must be valid JSON: {e}"))?;
    let draft_config_json = match node_key {
        "start" => start_node::canonicalize_start_draft_config_json(draft_config_json)?,
        "record" => record_node::canonicalize_record_config_json(draft_config_json)?,
        _ => draft_config_json.to_string(),
    };
    let changed = conn
        .execute(
            &format!(
                "UPDATE flow_nodes SET draft_config_json = ?1, draft_updated_at = {} \
                 WHERE flow_id = ?2 AND node_key = ?3",
                SQL_NOW_HCM
            ),
            params![draft_config_json, flow_id, node_key],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err(format!("flow {flow_id} node {node_key} not found"));
    }
    Ok(())
}

fn validate_publishable_node_configs(
    conn: &Connection,
    flow_id: i64,
) -> Result<(Option<String>, Option<String>), String> {
    let mut stmt = conn
        .prepare(
            "SELECT node_key, draft_config_json FROM flow_nodes \
             WHERE flow_id = ?1 AND node_key IN ('start', 'record')",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([flow_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut canonical_start_json = None;
    let mut canonical_record_json = None;
    for row in rows {
        let (node_key, draft_config_json) = row.map_err(|e| e.to_string())?;
        match node_key.as_str() {
            "start" => {
                start_node::parse_start_config(&draft_config_json)
                    .map_err(|e| format!("invalid start config: {e}"))?;
                canonical_start_json = Some(
                    start_node::canonicalize_start_config_json(&draft_config_json)
                        .map_err(|e| format!("invalid start config: {e}"))?,
                );
            }
            "record" => {
                record_node::parse_record_config(&draft_config_json)
                    .map_err(|e| format!("invalid record config: {e}"))?;
                canonical_record_json = Some(
                    record_node::canonicalize_record_config_json(&draft_config_json)
                        .map_err(|e| format!("invalid record config: {e}"))?,
                );
            }
            _ => {}
        }
    }

    Ok((canonical_start_json, canonical_record_json))
}

fn publish_flow_definition_with_conn(
    conn: &mut Connection,
    flow_id: i64,
) -> Result<PublishFlowResult, String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let is_running: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM flow_runs WHERE flow_id = ?1 AND status = 'running')",
            [flow_id],
            |row| row.get::<_, i64>(0).map(|v| v != 0),
        )
        .map_err(|e| e.to_string())?;
    let flow_exists: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM flows WHERE id = ?1",
            [flow_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if flow_exists == 0 {
        return Err(format!("flow {flow_id} not found"));
    }

    let (canonical_start_json, canonical_record_json) =
        validate_publishable_node_configs(&tx, flow_id)?;

    tx.execute(
        &format!(
            "UPDATE flow_nodes SET published_config_json = draft_config_json, published_at = {} WHERE flow_id = ?1",
            SQL_NOW_HCM
        ),
        [flow_id],
    )
    .map_err(|e| e.to_string())?;
    if let Some(canonical_start_json) = canonical_start_json {
        tx.execute(
            "UPDATE flow_nodes SET published_config_json = ?1 WHERE flow_id = ?2 AND node_key = 'start'",
            params![canonical_start_json, flow_id],
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some(canonical_record_json) = canonical_record_json {
        tx.execute(
            "UPDATE flow_nodes SET published_config_json = ?1 WHERE flow_id = ?2 AND node_key = 'record'",
            params![canonical_record_json, flow_id],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.execute(
        &format!(
            "UPDATE flows SET published_version = draft_version + 1, draft_version = draft_version + 1, updated_at = {} WHERE id = ?1",
            SQL_NOW_HCM
        ),
        [flow_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    Ok(PublishFlowResult {
        flow_id,
        is_running,
    })
}

fn publish_flow_with_runtime_reconcile(
    conn: &mut Connection,
    runtime_manager: &LiveRuntimeManager,
    flow_id: i64,
) -> Result<PublishFlowResult, String> {
    let previous_published_version: i64 = conn
        .query_row(
            "SELECT published_version FROM flows WHERE id = ?1",
            [flow_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let previous_start_published_config: Option<String> = conn
        .query_row(
            "SELECT published_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
            [flow_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let previous_record_published_config: Option<String> = conn
        .query_row(
            "SELECT published_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'record'",
            [flow_id],
            |row| row.get(0),
        )
        .ok();

    let result = publish_flow_definition_with_conn(conn, flow_id)?;
    let enabled: bool = conn
        .query_row(
            "SELECT enabled FROM flows WHERE id = ?1",
            [flow_id],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )
        .map_err(|e| e.to_string())?;
    if !enabled {
        return Ok(result);
    }

    if let Err(err) = runtime_manager.reconcile_flow(conn, flow_id) {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            &format!(
                "UPDATE flows SET published_version = ?1, draft_version = draft_version - 1, updated_at = {} WHERE id = ?2",
                SQL_NOW_HCM
            ),
            params![previous_published_version, flow_id],
        )
        .map_err(|e| e.to_string())?;
        if let Some(previous_start_published_config) = previous_start_published_config {
            tx.execute(
                "UPDATE flow_nodes SET published_config_json = ?1 WHERE flow_id = ?2 AND node_key = 'start'",
                params![previous_start_published_config, flow_id],
            )
            .map_err(|e| e.to_string())?;
        }
        if let Some(previous_record_published_config) = previous_record_published_config {
            tx.execute(
                "UPDATE flow_nodes SET published_config_json = ?1 WHERE flow_id = ?2 AND node_key = 'record'",
                params![previous_record_published_config, flow_id],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        conn.execute(
            &format!(
                "UPDATE flows SET status = 'error', current_node = 'start', updated_at = {} WHERE id = ?1",
                SQL_NOW_HCM
            ),
            [flow_id],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            &format!(
                "UPDATE flow_nodes SET published_config_json = json_set(COALESCE(published_config_json, '{{}}'), '$.last_error', ?1), published_at = {} WHERE flow_id = ?2 AND node_key = 'start'",
                SQL_NOW_HCM
            ),
            params![err.as_str(), flow_id],
        )
        .map_err(|e| e.to_string())?;
        return Err(err);
    }

    Ok(result)
}

#[tauri::command]
pub fn get_flow_definition(
    state: State<'_, AppState>,
    flow_id: i64,
) -> Result<FlowEditorPayload, String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    get_flow_definition_with_conn(&conn, flow_id)
}

#[tauri::command]
pub fn save_flow_node_draft(
    state: State<'_, AppState>,
    flow_id: i64,
    node_key: String,
    draft_config_json: String,
) -> Result<(), String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }
    let node_key = node_key.trim();
    if !is_valid_flow_node(node_key) {
        return Err(format!("invalid node_key: {node_key}"));
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM flows WHERE id = ?1",
            [flow_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if exists == 0 {
        return Err(format!("flow {flow_id} not found"));
    }
    apply_flow_node_draft(&conn, flow_id, node_key, draft_config_json.trim())
}

#[tauri::command]
pub fn publish_flow_definition(
    state: State<'_, AppState>,
    runtime_manager: State<'_, LiveRuntimeManager>,
    flow_id: i64,
) -> Result<PublishFlowResult, String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    publish_flow_with_runtime_reconcile(&mut conn, &runtime_manager, flow_id)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestartFlowRunResult {
    pub flow_id: i64,
    pub new_run_id: i64,
}

fn restart_flow_run_with_conn(
    conn: &mut Connection,
    runtime_manager: &LiveRuntimeManager,
    flow_id: i64,
) -> Result<RestartFlowRunResult, String> {
    let flow_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM flows WHERE id = ?1",
            [flow_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if flow_exists == 0 {
        return Err(format!("flow {flow_id} not found"));
    }

    let new_run_id = runtime_manager.restart_active_run(conn, flow_id)?;
    Ok(RestartFlowRunResult {
        flow_id,
        new_run_id,
    })
}

/// Cancels any `running` `flow_runs` / active `flow_node_runs` for this flow and inserts a new `running` run.
#[tauri::command]
pub fn restart_flow_run(
    state: State<'_, AppState>,
    runtime_manager: State<'_, LiveRuntimeManager>,
    flow_id: i64,
) -> Result<RestartFlowRunResult, String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    restart_flow_run_with_conn(&mut conn, &runtime_manager, flow_id)
}

#[cfg(test)]
mod tests {
    use super::{
        apply_flow_node_draft, get_flow_definition_with_conn, publish_flow_definition_with_conn,
        publish_flow_with_runtime_reconcile, restart_flow_run_with_conn,
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
            "tikclip-flow-engine-test-{}-{}-{}.db",
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

    fn insert_flow_with_nodes(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO flows (name, enabled, status) VALUES ('t', 1, 'idle')",
            [],
        )
        .expect("insert flow");
        let flow_id = conn.last_insert_rowid();

        for (node_key, position, config_json) in [
            ("start", 1i64, r#"{"username":"shop_abc"}"#),
            ("record", 2i64, r#"{"max_duration_minutes":5}"#),
            ("clip", 3i64, "{}"),
            ("caption", 4i64, "{}"),
            ("upload", 5i64, "{}"),
        ] {
            conn.execute(
                "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json) \
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![flow_id, node_key, position, config_json],
            )
            .expect("insert flow node");
        }

        flow_id
    }

    fn insert_flow_with_username(conn: &Connection, flow_id: i64, username: &str) {
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version) VALUES (?1, ?2, 1, 'idle', 1, 1)",
            params![flow_id, format!("Flow {flow_id}")],
        )
        .expect("insert flow");

        for (node_key, position, config_json) in [
            ("start", 1i64, format!(r#"{{"username":"{username}"}}"#)),
            ("record", 2i64, r#"{"max_duration_minutes":5}"#.to_string()),
            ("clip", 3i64, "{}".to_string()),
            ("caption", 4i64, "{}".to_string()),
            ("upload", 5i64, "{}".to_string()),
        ] {
            conn.execute(
                "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json) VALUES (?1, ?2, ?3, ?4, ?4)",
                params![flow_id, node_key, position, config_json],
            )
            .expect("insert flow node");
        }
    }

    #[test]
    fn publish_flow_definition_rejects_empty_start_username() {
        let (mut conn, path) = open_temp_db();
        let flow_id = insert_flow_with_nodes(&conn);
        conn.execute(
            "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = ?2 AND node_key = 'start'",
            params![r#"{"username":"   @   "}"#, flow_id],
        )
        .expect("update start draft");

        let err = publish_flow_definition_with_conn(&mut conn, flow_id).unwrap_err();

        assert!(err.contains("username is required"));
        let published_username: String = conn
            .query_row(
                "SELECT published_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
                [flow_id],
                |row| row.get(0),
            )
            .expect("read published start");
        assert_eq!(published_username, r#"{"username":"shop_abc"}"#);

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn publish_flow_definition_rejects_malformed_record_duration() {
        let (mut conn, path) = open_temp_db();
        let flow_id = insert_flow_with_nodes(&conn);
        conn.execute(
            "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = ?2 AND node_key = 'record'",
            params![r#"{"max_duration_minutes":"oops"}"#, flow_id],
        )
        .expect("update record draft");

        let err = publish_flow_definition_with_conn(&mut conn, flow_id).unwrap_err();

        assert!(err.contains("invalid record config"));
        let published_record: String = conn
            .query_row(
                "SELECT published_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'record'",
                [flow_id],
                |row| row.get(0),
            )
            .expect("read published record");
        assert_eq!(published_record, r#"{"max_duration_minutes":5}"#);

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn publish_flow_definition_canonicalizes_start_username_before_persist() {
        let (mut conn, path) = open_temp_db();
        let flow_id = insert_flow_with_nodes(&conn);
        conn.execute(
            "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = ?2 AND node_key = 'start'",
            params![
                r#"{"username":" @shop_abc ","cookies_json":"{}","proxy_url":"","poll_interval_seconds":20,"retry_limit":3}"#,
                flow_id
            ],
        )
        .expect("update start draft");

        publish_flow_definition_with_conn(&mut conn, flow_id).expect("publish flow");

        let published_username: String = conn
            .query_row(
                "SELECT json_extract(published_config_json, '$.username') FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
                [flow_id],
                |row| row.get(0),
            )
            .expect("read canonical username");
        assert_eq!(published_username, "shop_abc");

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn publish_flow_definition_canonicalizes_start_legacy_keys_to_snake_case() {
        let (mut conn, path) = open_temp_db();
        let flow_id = insert_flow_with_nodes(&conn);
        conn.execute(
            "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = ?2 AND node_key = 'start'",
            params![
                r#"{"username":" @shop_abc ","cookiesJson":"{}","proxyUrl":"http://127.0.0.1:9000","pollIntervalSeconds":20,"retryLimit":4}"#,
                flow_id
            ],
        )
        .expect("update start draft");

        publish_flow_definition_with_conn(&mut conn, flow_id).expect("publish flow");

        let published_config: String = conn
            .query_row(
                "SELECT published_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
                [flow_id],
                |row| row.get(0),
            )
            .expect("read published start config");
        assert_eq!(
            published_config,
            r#"{"cookies_json":"{}","poll_interval_seconds":20,"proxy_url":"http://127.0.0.1:9000","retry_limit":4,"username":"shop_abc"}"#
        );

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn publish_flow_definition_canonicalizes_record_legacy_keys_to_snake_case() {
        let (mut conn, path) = open_temp_db();
        let flow_id = insert_flow_with_nodes(&conn);
        conn.execute(
            "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = ?2 AND node_key = 'record'",
            params![r#"{"maxDurationSeconds":61}"#, flow_id],
        )
        .expect("update record draft");

        publish_flow_definition_with_conn(&mut conn, flow_id).expect("publish flow");

        let published_config: String = conn
            .query_row(
                "SELECT published_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'record'",
                [flow_id],
                |row| row.get(0),
            )
            .expect("read published record config");
        assert_eq!(published_config, r#"{"max_duration_minutes":2}"#);

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn publish_flow_definition_advances_published_version_to_new_runtime_version() {
        let (mut conn, path) = open_temp_db();
        let flow_id = insert_flow_with_nodes(&conn);
        conn.execute(
            "UPDATE flows SET published_version = 2, draft_version = 3 WHERE id = ?1",
            [flow_id],
        )
        .expect("seed versions");

        let result = publish_flow_definition_with_conn(&mut conn, flow_id).expect("publish flow");

        assert_eq!(result.flow_id, flow_id);
        assert!(!result.is_running);
        let (published_version, draft_version): (i64, i64) = conn
            .query_row(
                "SELECT published_version, draft_version FROM flows WHERE id = ?1",
                [flow_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read versions");
        assert_eq!(published_version, 4);
        assert_eq!(draft_version, 4);

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn flow_editor_payload_serializes_count_fields_in_snake_case() {
        let (conn, path) = open_temp_db();
        let flow_id = insert_flow_with_nodes(&conn);

        let payload = get_flow_definition_with_conn(&conn, flow_id).expect("load flow definition");
        let value = serde_json::to_value(&payload).expect("serialize flow editor payload");

        assert!(value.get("nodeRuns").is_some());
        assert!(value.get("recordings_count").is_some());
        assert!(value.get("clips_count").is_some());
        assert!(value.get("recordingsCount").is_none());
        assert!(value.get("clipsCount").is_none());

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn apply_flow_node_draft_canonicalizes_start_username() {
        let (conn, path) = open_temp_db();
        let flow_id = insert_flow_with_nodes(&conn);

        super::apply_flow_node_draft(
            &conn,
            flow_id,
            "start",
            r#"{"username":" @shop_abc ","cookies_json":"{}"}"#,
        )
        .expect("save draft");

        let draft_username: String = conn
            .query_row(
                "SELECT json_extract(draft_config_json, '$.username') FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
                [flow_id],
                |row| row.get(0),
            )
            .expect("read canonical draft username");
        assert_eq!(draft_username, "shop_abc");

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn apply_flow_node_draft_allows_blank_start_username() {
        let (conn, path) = open_temp_db();
        let flow_id = insert_flow_with_nodes(&conn);

        apply_flow_node_draft(
            &conn,
            flow_id,
            "start",
            r#"{"username":"   ","cookiesJson":"{}","proxyUrl":"http://127.0.0.1:9000","pollIntervalSeconds":20,"retryLimit":4}"#,
        )
        .expect("save incomplete draft");

        let draft_config: String = conn
            .query_row(
                "SELECT draft_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
                [flow_id],
                |row| row.get(0),
            )
            .expect("read draft config");
        assert_eq!(
            draft_config,
            r#"{"cookies_json":"{}","poll_interval_seconds":20,"proxy_url":"http://127.0.0.1:9000","retry_limit":4,"username":""}"#
        );

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn get_flow_definition_allows_blank_start_username_for_new_flow() {
        let (conn, path) = open_temp_db();
        conn.execute(
            "INSERT INTO flows (name, enabled, status, published_version, draft_version) VALUES ('t', 1, 'idle', 1, 1)",
            [],
        )
        .expect("insert flow");
        let flow_id = conn.last_insert_rowid();
        for (node_key, position, config_json) in [
            (
                "start",
                1i64,
                r#"{"username":"","cookies_json":"","proxy_url":"","poll_interval_seconds":60,"retry_limit":3}"#,
            ),
            ("record", 2i64, r#"{"max_duration_minutes":5}"#),
            ("clip", 3i64, "{}"),
            ("caption", 4i64, "{}"),
            ("upload", 5i64, "{}"),
        ] {
            conn.execute(
                "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json) VALUES (?1, ?2, ?3, ?4, ?4)",
                params![flow_id, node_key, position, config_json],
            )
            .expect("insert flow node");
        }

        let result = get_flow_definition_with_conn(&conn, flow_id);

        assert!(result.is_ok());

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn publish_failure_due_to_reconcile_conflict_does_not_advance_published_db_state() {
        let (mut conn, path) = open_temp_db();
        insert_flow_with_username(&conn, 1, "shop_abc");
        insert_flow_with_username(&conn, 2, "shop_xyz");
        let runtime_manager = LiveRuntimeManager::new();

        runtime_manager
            .start_flow_session(&conn, 1)
            .expect("start flow 1 session");
        runtime_manager
            .start_flow_session(&conn, 2)
            .expect("start flow 2 session");
        conn.execute(
            "UPDATE flows SET published_version = 3, draft_version = 4 WHERE id = 1",
            [],
        )
        .expect("seed versions");
        conn.execute(
            "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = 1 AND node_key = 'start'",
            [r#"{"username":"shop_xyz"}"#],
        )
        .expect("set conflicting draft");

        let err = publish_flow_with_runtime_reconcile(&mut conn, &runtime_manager, 1).unwrap_err();

        assert!(err.contains("username lease already held"));
        let (published_version, draft_version, published_username): (i64, i64, String) = conn
            .query_row(
                "SELECT f.published_version, f.draft_version, json_extract(n.published_config_json, '$.username') \
                 FROM flows f JOIN flow_nodes n ON n.flow_id = f.id AND n.node_key = 'start' \
                 WHERE f.id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read persisted publish state");
        assert_eq!(published_version, 3);
        assert_eq!(draft_version, 4);
        assert_eq!(published_username, "shop_abc");

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn publish_failure_due_to_reconcile_conflict_leaves_no_active_old_session_running() {
        let (mut conn, path) = open_temp_db();
        insert_flow_with_username(&conn, 1, "shop_abc");
        insert_flow_with_username(&conn, 2, "shop_xyz");
        let runtime_manager = LiveRuntimeManager::new();

        runtime_manager
            .start_flow_session(&conn, 1)
            .expect("start flow 1 session");
        runtime_manager
            .start_flow_session(&conn, 2)
            .expect("start flow 2 session");
        conn.execute(
            "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = 1 AND node_key = 'start'",
            [r#"{"username":"shop_xyz"}"#],
        )
        .expect("set conflicting draft");

        let err = publish_flow_with_runtime_reconcile(&mut conn, &runtime_manager, 1).unwrap_err();

        assert!(err.contains("username lease already held"));
        let flow_one_snapshot = runtime_manager
            .list_sessions()
            .into_iter()
            .find(|snapshot| snapshot.flow_id == 1)
            .expect("failed runtime snapshot should remain");
        assert_eq!(flow_one_snapshot.status, "error");
        assert_eq!(flow_one_snapshot.lookup_key, "shop_abc");
        assert!(flow_one_snapshot
            .last_error
            .unwrap_or_default()
            .contains("username lease already held"));

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn publish_failure_due_to_reconcile_conflict_sets_runtime_error_state() {
        let (mut conn, path) = open_temp_db();
        insert_flow_with_username(&conn, 1, "shop_abc");
        insert_flow_with_username(&conn, 2, "shop_xyz");
        let runtime_manager = LiveRuntimeManager::new();

        runtime_manager
            .start_flow_session(&conn, 1)
            .expect("start flow 1 session");
        runtime_manager
            .start_flow_session(&conn, 2)
            .expect("start flow 2 session");
        conn.execute(
            "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = 1 AND node_key = 'start'",
            [r#"{"username":"shop_xyz"}"#],
        )
        .expect("set conflicting draft");

        let err = publish_flow_with_runtime_reconcile(&mut conn, &runtime_manager, 1).unwrap_err();

        assert!(err.contains("username lease already held"));
        let (status, current_node, last_error): (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, current_node, json_extract(published_config_json, '$.last_error') \
                 FROM flows f LEFT JOIN flow_nodes n ON n.flow_id = f.id AND n.node_key = 'start' \
                 WHERE f.id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read runtime error state");
        assert_eq!(status, "error");
        assert_eq!(current_node.as_deref(), Some("start"));
        assert!(last_error
            .unwrap_or_default()
            .contains("username lease already held"));

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn restart_flow_run_updates_runtime_session_state_coherently() {
        let (mut conn, path) = open_temp_db();
        insert_flow_with_username(&conn, 1, "shop_abc");
        let runtime_manager = LiveRuntimeManager::new();

        runtime_manager
            .start_flow_session(&conn, 1)
            .expect("start flow session");
        let first_run_id = runtime_manager
            .handle_live_detected(
                &conn,
                1,
                &crate::tiktok::types::LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("create initial run");

        let result =
            restart_flow_run_with_conn(&mut conn, &runtime_manager, 1).expect("restart flow run");

        assert_eq!(result.flow_id, 1);
        assert_ne!(result.new_run_id, first_run_id);
        assert_eq!(
            runtime_manager.session_active_flow_run_id_for_test(1),
            Some(result.new_run_id)
        );

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn restart_flow_run_fails_cleanly_without_mutating_db_when_session_missing() {
        let (mut conn, path) = open_temp_db();
        insert_flow_with_username(&conn, 1, "shop_abc");
        let runtime_manager = LiveRuntimeManager::new();
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (11, 1, 1, 'running', datetime('now','+7 hours'), 'test')",
            [],
        )
        .expect("insert running flow run");
        conn.execute(
            "INSERT INTO flow_node_runs (flow_run_id, flow_id, node_key, status, started_at) \
             VALUES (11, 1, 'record', 'running', datetime('now','+7 hours'))",
            [],
        )
        .expect("insert running node run");

        let err = restart_flow_run_with_conn(&mut conn, &runtime_manager, 1).unwrap_err();

        assert!(err.contains("missing live runtime session"));
        let runs: Vec<(i64, String)> = {
            let mut stmt = conn
                .prepare("SELECT id, status FROM flow_runs WHERE flow_id = 1 ORDER BY id ASC")
                .expect("prepare flow run query");
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("query flow runs")
                .map(|row| row.expect("map flow run row"))
                .collect()
        };
        assert_eq!(runs, vec![(11, "running".to_string())]);

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn publish_flow_runtime_reconcile_keeps_one_fresh_poll_task() {
        let (mut conn, path) = open_temp_db();
        insert_flow_with_username(&conn, 1, "shop_abc");
        let runtime_manager = LiveRuntimeManager::new();
        runtime_manager
            .start_flow_session(&conn, 1)
            .expect("start flow session");

        assert_eq!(runtime_manager.session_generation_for_test(1), Some(1));
        assert_eq!(runtime_manager.active_poll_task_count_for_test(), 1);

        publish_flow_with_runtime_reconcile(&mut conn, &runtime_manager, 1)
            .expect("publish with reconcile");

        assert!(runtime_manager.session_has_poll_task_for_test(1));
        assert_eq!(runtime_manager.active_poll_task_count_for_test(), 1);
        assert_eq!(runtime_manager.session_generation_for_test(1), Some(2));
        assert_eq!(
            runtime_manager.cancelled_poll_generations_for_test(1),
            vec![1]
        );

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn restart_flow_run_does_not_replace_poll_task() {
        let (mut conn, path) = open_temp_db();
        insert_flow_with_username(&conn, 1, "shop_abc");
        let runtime_manager = LiveRuntimeManager::new();
        runtime_manager
            .start_flow_session(&conn, 1)
            .expect("start flow session");

        assert_eq!(runtime_manager.session_generation_for_test(1), Some(1));
        assert_eq!(runtime_manager.active_poll_task_count_for_test(), 1);

        restart_flow_run_with_conn(&mut conn, &runtime_manager, 1).expect("restart flow run");

        assert!(runtime_manager.session_has_poll_task_for_test(1));
        assert_eq!(runtime_manager.active_poll_task_count_for_test(), 1);
        assert_eq!(runtime_manager.session_generation_for_test(1), Some(1));
        assert!(runtime_manager
            .cancelled_poll_generations_for_test(1)
            .is_empty());

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }
}
