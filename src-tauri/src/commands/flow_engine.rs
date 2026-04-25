use crate::live_runtime::manager::LiveRuntimeManager;
use crate::workflow::constants::is_valid_flow_node;
use crate::AppState;
use serde::Serialize;
use tauri::State;

mod drafts;
mod publish;
mod queries;
mod restart;

pub(crate) use drafts::apply_flow_node_draft;
#[cfg(test)]
pub(crate) use publish::publish_flow_definition_with_conn;
pub(crate) use publish::publish_flow_with_runtime_reconcile;
pub(crate) use queries::get_flow_definition_with_conn;
pub(crate) use restart::restart_flow_run_with_conn;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowEditorPayload {
    pub flow: crate::db::models::Flow,
    pub nodes: Vec<crate::db::models::FlowNodeDefinition>,
    pub runs: Vec<crate::workflow::types::FlowRun>,
    pub node_runs: Vec<crate::workflow::types::FlowNodeRun>,
    #[serde(rename = "recordings_count")]
    pub recordings_count: i64,
    #[serde(rename = "clips_count")]
    pub clips_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishFlowResult {
    pub flow_id: i64,
    pub is_running: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestartFlowRunResult {
    pub flow_id: i64,
    pub restarted: bool,
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

/// Cancels any active run for this flow and returns the live session to watcher mode.
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
mod tests;
