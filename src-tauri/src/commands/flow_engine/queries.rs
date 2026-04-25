use super::FlowEditorPayload;
use crate::db::models::Flow;
use crate::live_runtime::account_binding::find_account_by_start_username;
use crate::workflow::node_runner;
use crate::workflow::runtime_store;
use crate::workflow::start_node;
use crate::workflow::types::{FlowNodeRun, FlowRun};
use rusqlite::{Connection, Row};

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

pub(crate) fn get_flow_definition_with_conn(
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
