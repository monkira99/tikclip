use crate::commands::flows::UpdateFlowRuntimeByAccountInput;
use crate::commands::recordings::finalize_rust_recording_row;
use crate::live_runtime::types::LiveRuntimeSessionSnapshot;
use crate::recording_runtime::types::{RecordingFinishInput, RecordingOutcome};
use crate::workflow::start_node;
use rusqlite::{Connection, OptionalExtension};

#[derive(Debug, Clone)]
pub(super) struct FlowRuntimeConfig {
    pub(super) flow_id: i64,
    pub(super) flow_name: String,
    pub(super) enabled: bool,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) definition_version: i64,
    pub(super) username: String,
    pub(super) lookup_key: String,
    pub(super) cookies_json: String,
    pub(super) proxy_url: Option<String>,
    pub(super) poll_interval_seconds: i64,
}

pub(super) fn load_sidecar_base_url(conn: &Connection) -> Result<Option<String>, String> {
    let port: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'sidecar_port'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(port.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(format!("http://127.0.0.1:{trimmed}"))
        }
    }))
}

pub(super) fn load_flow_runtime_config(
    conn: &Connection,
    flow_id: i64,
) -> Result<FlowRuntimeConfig, String> {
    let (loaded_flow_id, flow_name, enabled, definition_version, published_config_json): (
        i64,
        String,
        i64,
        i64,
        String,
    ) = conn
        .query_row(
            "SELECT f.id, f.name, f.enabled, f.published_version, n.published_config_json              FROM flows f              JOIN flow_nodes n ON n.flow_id = f.id AND n.node_key = 'start'              WHERE f.id = ?1",
            [flow_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;
    let config = start_node::parse_start_config(&published_config_json)?;

    Ok(FlowRuntimeConfig {
        flow_id: loaded_flow_id,
        flow_name,
        enabled: enabled != 0,
        definition_version,
        username: config.username.canonical.clone(),
        lookup_key: config.username.lookup_key,
        cookies_json: config.cookies_json,
        proxy_url: config.proxy_url,
        poll_interval_seconds: config.poll_interval_seconds,
    })
}

pub(super) fn load_record_duration_seconds(conn: &Connection, flow_id: i64) -> Result<i64, String> {
    let raw: String = conn
        .query_row(
            "SELECT published_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'record'",
            [flow_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(crate::workflow::record_node::parse_record_config(&raw)?.max_duration_seconds())
}

pub(super) fn update_flow_runtime_by_flow_id(
    conn: &Connection,
    flow_id: i64,
    input: &UpdateFlowRuntimeByAccountInput,
) -> Result<(), String> {
    let mut sets: Vec<String> = Vec::new();
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx: usize = 1;

    if let Some(status) = &input.status {
        sets.push(format!("status = ?{idx}"));
        params_vec.push(Box::new(status.trim().to_string()));
        idx += 1;
    }
    if let Some(current_node) = &input.current_node {
        let trimmed = current_node.trim();
        if trimmed.is_empty() {
            sets.push("current_node = NULL".to_string());
        } else {
            sets.push(format!("current_node = ?{idx}"));
            params_vec.push(Box::new(trimmed.to_string()));
            idx += 1;
        }
    }
    if !sets.is_empty() {
        sets.push(format!("updated_at = {}", crate::time_hcm::SQL_NOW_HCM));
        let sql = format!("UPDATE flows SET {} WHERE id = ?{idx}", sets.join(", "));
        params_vec.push(Box::new(flow_id));
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|value| value.as_ref()).collect();
        conn.execute(sql.as_str(), params_refs.as_slice())
            .map_err(|e| e.to_string())?;
    }

    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT draft_config_json, published_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
            [flow_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((draft_json, published_json)) = row else {
        return Ok(());
    };
    let mut draft: serde_json::Value =
        serde_json::from_str(&draft_json).unwrap_or_else(|_| serde_json::json!({}));
    let mut published: serde_json::Value =
        serde_json::from_str(&published_json).unwrap_or_else(|_| serde_json::json!({}));
    let draft_obj = draft
        .as_object_mut()
        .ok_or_else(|| "start draft_config_json must be a JSON object".to_string())?;
    let published_obj = published
        .as_object_mut()
        .ok_or_else(|| "start published_config_json must be a JSON object".to_string())?;
    for (key, value) in [
        ("last_live_at", input.last_live_at.as_ref()),
        ("last_run_at", input.last_run_at.as_ref()),
        ("last_error", input.last_error.as_ref()),
    ] {
        if let Some(value) = value {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                draft_obj.remove(key);
                published_obj.remove(key);
            } else {
                draft_obj.insert(key.to_string(), serde_json::json!(trimmed.to_string()));
                published_obj.insert(key.to_string(), serde_json::json!(trimmed.to_string()));
            }
        }
    }
    conn.execute(
        &format!(
            "UPDATE flow_nodes SET draft_config_json = ?1, published_config_json = ?2, draft_updated_at = {}, published_at = {} WHERE flow_id = ?3 AND node_key = 'start'",
            crate::time_hcm::SQL_NOW_HCM,
            crate::time_hcm::SQL_NOW_HCM
        ),
        rusqlite::params![
            serde_json::to_string(&draft).map_err(|e| e.to_string())?,
            serde_json::to_string(&published).map_err(|e| e.to_string())?,
            flow_id,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn finalize_latest_recording_row(
    conn: &Connection,
    flow_id: i64,
    room_id: Option<&str>,
    success: bool,
    error_message: Option<&str>,
) -> Result<(), String> {
    let recording: Option<(i64, i64, String, Option<String>, String)> = conn
        .query_row(
            "SELECT account_id, flow_run_id, sidecar_recording_id, file_path, room_id FROM recordings WHERE flow_id = ?1 AND status = 'recording' ORDER BY id DESC LIMIT 1",
            [flow_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((account_id, flow_run_id, external_recording_id, file_path, existing_room_id)) =
        recording
    else {
        return Ok(());
    };
    finalize_rust_recording_row(
        conn,
        &RecordingFinishInput {
            account_id,
            flow_id,
            flow_run_id,
            external_recording_id,
            room_id: room_id.map(str::to_string).unwrap_or(existing_room_id),
            file_path,
            error_message: error_message.map(str::to_string),
            duration_seconds: 0,
            file_size_bytes: 0,
            outcome: if success {
                RecordingOutcome::Success
            } else if matches!(
                error_message.map(str::trim),
                Some("Recording cancelled") | Some("Cancelled")
            ) {
                RecordingOutcome::Cancelled
            } else {
                RecordingOutcome::Failed
            },
        },
    )?;
    Ok(())
}

pub(super) fn open_runtime_connection(db_path: &std::path::Path) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| e.to_string())?;
    Ok(conn)
}

pub(super) fn failed_snapshot(
    flow_id: i64,
    config: &FlowRuntimeConfig,
    generation: u64,
    error: &str,
) -> LiveRuntimeSessionSnapshot {
    LiveRuntimeSessionSnapshot {
        flow_id,
        flow_name: config.flow_name.clone(),
        username: config.username.clone(),
        lookup_key: config.lookup_key.clone(),
        generation,
        status: "error".to_string(),
        last_error: Some(error.to_string()),
        last_checked_at: None,
        last_check_live: None,
        next_poll_at: None,
        poll_interval_seconds: Some(config.poll_interval_seconds),
    }
}
