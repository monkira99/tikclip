use crate::db::models::{Clip, Flow, FlowNodeConfig};
use crate::time_hcm::SQL_NOW_HCM;
use crate::AppState;
use rusqlite::Result as SqlResult;
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;

const FLOW_NODE_KEYS: [&str; 5] = ["start", "record", "clip", "caption", "upload"];
const FLOW_STATUS_KEYS: [&str; 6] = [
    "idle",
    "watching",
    "recording",
    "processing",
    "error",
    "disabled",
];

fn is_valid_flow_node(node_key: &str) -> bool {
    FLOW_NODE_KEYS.contains(&node_key)
}

fn is_valid_flow_status(status: &str) -> bool {
    FLOW_STATUS_KEYS.contains(&status)
}

fn map_flow_row(row: &Row) -> SqlResult<Flow> {
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
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn map_flow_node_config_row(row: &Row) -> SqlResult<FlowNodeConfig> {
    Ok(FlowNodeConfig {
        id: row.get(0)?,
        flow_id: row.get(1)?,
        node_key: row.get(2)?,
        config_json: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

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
        flow_id: row.get(16)?,
        transcript_text: row.get(17)?,
        caption_text: row.get(18)?,
        caption_status: row.get(19)?,
        caption_error: row.get(20)?,
        caption_generated_at: row.get(21)?,
        created_at: row.get(22)?,
        updated_at: row.get(23)?,
    })
}

#[derive(Debug, Serialize)]
pub struct FlowListItem {
    pub id: i64,
    pub account_id: i64,
    pub account_username: String,
    pub name: String,
    pub enabled: bool,
    pub status: String,
    pub current_node: Option<String>,
    pub last_live_at: Option<String>,
    pub last_run_at: Option<String>,
    pub last_error: Option<String>,
    pub recordings_count: i64,
    pub clips_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct FlowDetail {
    pub flow: Flow,
    pub node_configs: Vec<FlowNodeConfig>,
    pub recordings_count: i64,
    pub clips_count: i64,
}

#[derive(Debug, Serialize)]
pub struct FlowRecording {
    pub id: i64,
    pub account_id: i64,
    pub account_username: String,
    pub room_id: Option<String>,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: i64,
    pub file_path: Option<String>,
    pub file_size_bytes: i64,
    pub sidecar_recording_id: Option<String>,
    pub error_message: Option<String>,
    pub flow_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateFlowInput {
    pub account_id: i64,
    pub name: String,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateFlowInput {
    pub name: Option<String>,
    pub status: Option<String>,
    pub current_node: Option<String>,
    pub last_live_at: Option<String>,
    pub last_run_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SaveFlowNodeConfigInput {
    pub flow_id: i64,
    pub node_key: String,
    pub config_json: String,
}

#[tauri::command]
pub fn list_flows(state: State<'_, AppState>) -> Result<Vec<FlowListItem>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT \
             f.id, f.account_id, a.username, f.name, f.enabled, f.status, f.current_node, \
             f.last_live_at, f.last_run_at, f.last_error, \
             (SELECT COUNT(*) FROM recordings r WHERE r.flow_id = f.id), \
             (SELECT COUNT(*) FROM clips c WHERE c.flow_id = f.id), \
             f.created_at, f.updated_at \
             FROM flows f \
             INNER JOIN accounts a ON a.id = f.account_id \
             ORDER BY f.updated_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(FlowListItem {
                id: row.get(0)?,
                account_id: row.get(1)?,
                account_username: row.get(2)?,
                name: row.get(3)?,
                enabled: row.get::<_, i64>(4)? != 0,
                status: row.get(5)?,
                current_node: row.get(6)?,
                last_live_at: row.get(7)?,
                last_run_at: row.get(8)?,
                last_error: row.get(9)?,
                recordings_count: row.get(10)?,
                clips_count: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
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
pub fn get_flow_detail(state: State<'_, AppState>, flow_id: i64) -> Result<FlowDetail, String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let flow = conn
        .query_row(
            "SELECT id, account_id, name, enabled, status, current_node, last_live_at, last_run_at, last_error, created_at, updated_at \
             FROM flows WHERE id = ?1",
            [flow_id],
            map_flow_row,
        )
        .map_err(|e| e.to_string())?;

    let mut node_stmt = conn
        .prepare(
            "SELECT id, flow_id, node_key, config_json, updated_at \
             FROM flow_node_configs \
             WHERE flow_id = ?1 \
             ORDER BY node_key ASC",
        )
        .map_err(|e| e.to_string())?;
    let node_rows = node_stmt
        .query_map([flow_id], map_flow_node_config_row)
        .map_err(|e| e.to_string())?;
    let mut node_configs = Vec::new();
    for row in node_rows {
        node_configs.push(row.map_err(|e| e.to_string())?);
    }

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

    Ok(FlowDetail {
        flow,
        node_configs,
        recordings_count,
        clips_count,
    })
}

#[tauri::command]
pub fn create_flow(state: State<'_, AppState>, input: CreateFlowInput) -> Result<i64, String> {
    if input.account_id <= 0 {
        return Err("account_id must be positive".to_string());
    }
    let flow_name = input.name.trim();
    if flow_name.is_empty() {
        return Err("name is required".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let account_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM accounts WHERE id = ?1",
            [input.account_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if account_exists == 0 {
        return Err(format!("unknown account_id {}", input.account_id));
    }

    let existing_flow: Option<i64> = conn
        .query_row(
            "SELECT id FROM flows WHERE account_id = ?1",
            [input.account_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(existing_id) = existing_flow {
        return Err(format!(
            "flow already exists for account_id {} (flow_id {})",
            input.account_id, existing_id
        ));
    }

    conn.execute(
        &format!(
            "INSERT INTO flows (account_id, name, enabled, status, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'idle', {}, {})",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![
            input.account_id,
            flow_name,
            if input.enabled.unwrap_or(true) {
                1i64
            } else {
                0i64
            }
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_flow(
    state: State<'_, AppState>,
    flow_id: i64,
    input: UpdateFlowInput,
) -> Result<(), String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }

    let mut sets: Vec<String> = Vec::new();
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx: usize = 1;

    if let Some(name) = input.name {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("name cannot be empty".to_string());
        }
        sets.push(format!("name = ?{idx}"));
        params_vec.push(Box::new(trimmed.to_string()));
        idx += 1;
    }

    if let Some(status) = input.status {
        let trimmed = status.trim();
        if trimmed.is_empty() {
            return Err("status cannot be empty".to_string());
        }
        if !is_valid_flow_status(trimmed) {
            return Err(format!("invalid status: {trimmed}"));
        }
        sets.push(format!("status = ?{idx}"));
        params_vec.push(Box::new(trimmed.to_string()));
        idx += 1;
    }

    if let Some(current_node) = input.current_node {
        let trimmed = current_node.trim().to_string();
        if trimmed.is_empty() {
            sets.push("current_node = NULL".to_string());
        } else {
            if !is_valid_flow_node(trimmed.as_str()) {
                return Err(format!("invalid current_node: {}", trimmed));
            }
            sets.push(format!("current_node = ?{idx}"));
            params_vec.push(Box::new(trimmed));
            idx += 1;
        }
    }

    if let Some(last_live_at) = input.last_live_at {
        let value = last_live_at.trim().to_string();
        if value.is_empty() {
            sets.push("last_live_at = NULL".to_string());
        } else {
            sets.push(format!("last_live_at = ?{idx}"));
            params_vec.push(Box::new(value));
            idx += 1;
        }
    }

    if let Some(last_run_at) = input.last_run_at {
        let value = last_run_at.trim().to_string();
        if value.is_empty() {
            sets.push("last_run_at = NULL".to_string());
        } else {
            sets.push(format!("last_run_at = ?{idx}"));
            params_vec.push(Box::new(value));
            idx += 1;
        }
    }

    if let Some(last_error) = input.last_error {
        let value = last_error.trim().to_string();
        if value.is_empty() {
            sets.push("last_error = NULL".to_string());
        } else {
            sets.push(format!("last_error = ?{idx}"));
            params_vec.push(Box::new(value));
            idx += 1;
        }
    }

    if sets.is_empty() {
        return Ok(());
    }

    sets.push(format!("updated_at = {SQL_NOW_HCM}"));
    let sql = format!("UPDATE flows SET {} WHERE id = ?{idx}", sets.join(", "));
    params_vec.push(Box::new(flow_id));

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|p| p.as_ref()).collect();
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let changed = conn
        .execute(sql.as_str(), params_refs.as_slice())
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err(format!("flow {flow_id} not found"));
    }
    Ok(())
}

#[tauri::command]
pub fn set_flow_enabled(
    state: State<'_, AppState>,
    flow_id: i64,
    enabled: bool,
) -> Result<(), String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let changed = conn
        .execute(
            &format!(
                "UPDATE flows SET enabled = ?1, updated_at = {} WHERE id = ?2",
                SQL_NOW_HCM
            ),
            params![if enabled { 1i64 } else { 0i64 }, flow_id],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err(format!("flow {flow_id} not found"));
    }
    Ok(())
}

#[tauri::command]
pub fn save_flow_node_config(
    state: State<'_, AppState>,
    input: SaveFlowNodeConfigInput,
) -> Result<FlowNodeConfig, String> {
    if input.flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }

    let node_key = input.node_key.trim();
    if !is_valid_flow_node(node_key) {
        return Err(format!("invalid node_key: {}", input.node_key));
    }

    let config_json = input.config_json.trim();
    if config_json.is_empty() {
        return Err("config_json is required".to_string());
    }
    serde_json::from_str::<serde_json::Value>(config_json)
        .map_err(|e| format!("config_json must be valid JSON: {e}"))?;

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let flow_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM flows WHERE id = ?1",
            [input.flow_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if flow_exists == 0 {
        return Err(format!("flow {} not found", input.flow_id));
    }

    conn.execute(
        &format!(
            "INSERT INTO flow_node_configs (flow_id, node_key, config_json, updated_at) \
             VALUES (?1, ?2, ?3, {}) \
             ON CONFLICT(flow_id, node_key) DO UPDATE SET \
               config_json = excluded.config_json, \
               updated_at = excluded.updated_at",
            SQL_NOW_HCM
        ),
        params![input.flow_id, node_key, config_json],
    )
    .map_err(|e| e.to_string())?;

    conn.query_row(
        "SELECT id, flow_id, node_key, config_json, updated_at \
         FROM flow_node_configs WHERE flow_id = ?1 AND node_key = ?2",
        params![input.flow_id, node_key],
        map_flow_node_config_row,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_recordings_by_flow(
    state: State<'_, AppState>,
    flow_id: i64,
) -> Result<Vec<FlowRecording>, String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT \
             r.id, r.account_id, a.username, r.room_id, r.status, r.started_at, r.ended_at, \
             r.duration_seconds, r.file_path, r.file_size_bytes, r.sidecar_recording_id, \
             r.error_message, r.flow_id, r.created_at \
             FROM recordings r \
             INNER JOIN accounts a ON a.id = r.account_id \
             WHERE r.flow_id = ?1 \
             ORDER BY r.started_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([flow_id], |row| {
            Ok(FlowRecording {
                id: row.get(0)?,
                account_id: row.get(1)?,
                account_username: row.get(2)?,
                room_id: row.get(3)?,
                status: row.get(4)?,
                started_at: row.get(5)?,
                ended_at: row.get(6)?,
                duration_seconds: row.get(7)?,
                file_path: row.get(8)?,
                file_size_bytes: row.get(9)?,
                sidecar_recording_id: row.get(10)?,
                error_message: row.get(11)?,
                flow_id: row.get(12)?,
                created_at: row.get(13)?,
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
pub fn list_clips_by_flow(state: State<'_, AppState>, flow_id: i64) -> Result<Vec<Clip>, String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT \
             c.id, c.recording_id, c.account_id, a.username, \
             c.title, c.file_path, c.thumbnail_path, c.duration_seconds, c.file_size_bytes, \
             c.start_time, c.end_time, c.status, c.quality_score, c.scene_type, c.ai_tags_json, \
             c.notes, c.flow_id, c.transcript_text, c.caption_text, c.caption_status, c.caption_error, c.caption_generated_at, c.created_at, c.updated_at \
             FROM clips c \
             INNER JOIN accounts a ON a.id = c.account_id \
             WHERE c.flow_id = ?1 \
             ORDER BY c.created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([flow_id], map_clip_row)
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}
