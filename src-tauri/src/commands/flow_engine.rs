use crate::db::models::{Flow, FlowNodeDefinition};
use crate::time_hcm::SQL_NOW_HCM;
use crate::workflow::node_runner;
use crate::workflow::runtime_store;
use crate::workflow::types::{FlowNodeRun, FlowRun};
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
    conn.query_row(
        "SELECT f.id, \
         COALESCE(a_match.id, 0), \
         f.name, f.enabled, f.status, f.current_node, \
         json_extract(n.published_config_json, '$.last_live_at'), \
         json_extract(n.published_config_json, '$.last_run_at'), \
         json_extract(n.published_config_json, '$.last_error'), \
         f.published_version, f.draft_version, \
         f.created_at, f.updated_at \
         FROM flows f \
         LEFT JOIN flow_nodes n ON n.flow_id = f.id AND n.node_key = 'start' \
         LEFT JOIN accounts a_match ON lower(a_match.username) = lower(json_extract(n.published_config_json, '$.username')) \
           AND trim(json_extract(n.published_config_json, '$.username')) != '' \
         WHERE f.id = ?1",
        [flow_id],
        map_flow_definition_row,
    )
    .map_err(|e| e.to_string())
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
    pub recordings_count: i64,
    pub clips_count: i64,
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

#[tauri::command]
pub fn get_flow_definition(
    state: State<'_, AppState>,
    flow_id: i64,
) -> Result<FlowEditorPayload, String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let flow = load_flow_definition(&conn, flow_id)?;
    let nodes = runtime_store::list_flow_node_definitions(&conn, flow_id)?;
    if let Some(start_def) = nodes.iter().find(|d| d.node_key == "start") {
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
    let runs = load_flow_runs(&conn, flow_id)?;
    let node_runs = load_flow_node_runs(&conn, flow_id)?;
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
    flow_id: i64,
) -> Result<PublishFlowResult, String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
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
    tx.execute(
        &format!(
            "UPDATE flow_nodes SET published_config_json = draft_config_json, published_at = {} WHERE flow_id = ?1",
            SQL_NOW_HCM
        ),
        [flow_id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        &format!(
            "UPDATE flows SET published_version = draft_version, draft_version = draft_version + 1, updated_at = {} WHERE id = ?1",
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestartFlowRunResult {
    pub flow_id: i64,
    pub new_run_id: i64,
}

/// Cancels any `running` `flow_runs` / active `flow_node_runs` for this flow and inserts a new `running` run.
#[tauri::command]
pub fn restart_flow_run(
    state: State<'_, AppState>,
    flow_id: i64,
) -> Result<RestartFlowRunResult, String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
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
    let new_run_id =
        runtime_store::restart_running_flow_runs_for_flow(&mut conn, flow_id, "publish_restart")?;
    Ok(RestartFlowRunResult {
        flow_id,
        new_run_id,
    })
}
