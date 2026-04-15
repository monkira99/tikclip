use crate::commands::flow_engine;
use crate::db::models::{Clip, FlowNodeConfig};
use crate::time_hcm::SQL_NOW_HCM;
use crate::workflow::runtime_store;
use crate::AppState;
use chrono::{SecondsFormat, Utc};
use log::warn;
use rusqlite::Result as SqlResult;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
        flow_run_id: row.get(17)?,
        transcript_text: row.get(18)?,
        caption_text: row.get(19)?,
        caption_status: row.get(20)?,
        caption_error: row.get(21)?,
        caption_generated_at: row.get(22)?,
        created_at: row.get(23)?,
        updated_at: row.get(24)?,
    })
}

fn flow_id_for_account(conn: &rusqlite::Connection, account_id: i64) -> SqlResult<Option<i64>> {
    conn.query_row(
        "SELECT n.flow_id FROM flow_nodes n \
         JOIN accounts a ON a.id = ?1 \
         WHERE n.node_key = 'start' \
           AND trim(json_extract(n.published_config_json, '$.username')) != '' \
           AND lower(json_extract(n.published_config_json, '$.username')) = lower(a.username) \
         LIMIT 1",
        [account_id],
        |row| row.get(0),
    )
    .optional()
}

fn merge_start_runtime_fields(
    conn: &rusqlite::Connection,
    flow_id: i64,
    last_live_at: Option<&str>,
    last_run_at: Option<&str>,
    last_error: Option<&str>,
) -> Result<(), String> {
    if last_live_at.is_none() && last_run_at.is_none() && last_error.is_none() {
        return Ok(());
    }
    let row_result = conn.query_row(
        "SELECT draft_config_json, published_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
        [flow_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    );
    let (draft, published) = match row_result {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    let mut d: Value = serde_json::from_str(&draft).unwrap_or_else(|_| json!({}));
    let mut p: Value = serde_json::from_str(&published).unwrap_or_else(|_| json!({}));
    let obj_d = d
        .as_object_mut()
        .ok_or_else(|| "start draft_config_json must be a JSON object".to_string())?;
    let obj_p = p
        .as_object_mut()
        .ok_or_else(|| "start published_config_json must be a JSON object".to_string())?;

    if let Some(v) = last_live_at {
        let t = v.trim();
        if t.is_empty() {
            obj_d.remove("last_live_at");
            obj_p.remove("last_live_at");
        } else {
            obj_d.insert("last_live_at".to_string(), json!(t.to_string()));
            obj_p.insert("last_live_at".to_string(), json!(t.to_string()));
        }
    }
    if let Some(v) = last_run_at {
        let t = v.trim();
        if t.is_empty() {
            obj_d.remove("last_run_at");
            obj_p.remove("last_run_at");
        } else {
            obj_d.insert("last_run_at".to_string(), json!(t.to_string()));
            obj_p.insert("last_run_at".to_string(), json!(t.to_string()));
        }
    }
    if let Some(v) = last_error {
        let t = v.trim();
        if t.is_empty() {
            obj_d.remove("last_error");
            obj_p.remove("last_error");
        } else {
            obj_d.insert("last_error".to_string(), json!(t.to_string()));
            obj_p.insert("last_error".to_string(), json!(t.to_string()));
        }
    }

    let draft_s = serde_json::to_string(&d).map_err(|e| e.to_string())?;
    let pub_s = serde_json::to_string(&p).map_err(|e| e.to_string())?;
    conn.execute(
        &format!(
            "UPDATE flow_nodes SET draft_config_json = ?1, published_config_json = ?2, draft_updated_at = {}, published_at = {} WHERE flow_id = ?3 AND node_key = 'start'",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![draft_s, pub_s, flow_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn get_setting_trimmed(conn: &rusqlite::Connection, key: &str) -> Result<Option<String>, String> {
    let result = conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(value) => {
            let t = value.trim().to_string();
            Ok(if t.is_empty() { None } else { Some(t) })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn get_int_setting_or_default(conn: &rusqlite::Connection, key: &str, default: i64) -> i64 {
    match get_setting_trimmed(conn, key) {
        Ok(Some(raw)) => match raw.parse::<i64>() {
            Ok(value) => value,
            Err(_) => {
                warn!(
                    "invalid integer app_settings value for key '{}': '{}'; using default {}",
                    key, raw, default
                );
                default
            }
        },
        Ok(None) => default,
        Err(err) => {
            warn!(
                "failed to read app_settings key '{}': {}; using default {}",
                key, err, default
            );
            default
        }
    }
}

fn get_float_setting_or_default(conn: &rusqlite::Connection, key: &str, default: f64) -> f64 {
    match get_setting_trimmed(conn, key) {
        Ok(Some(raw)) => match raw.parse::<f64>() {
            Ok(value) => value,
            Err(_) => {
                warn!(
                    "invalid float app_settings value for key '{}': '{}'; using default {}",
                    key, raw, default
                );
                default
            }
        },
        Ok(None) => default,
        Err(err) => {
            warn!(
                "failed to read app_settings key '{}': {}; using default {}",
                key, err, default
            );
            default
        }
    }
}

fn get_bool_setting_or_default(conn: &rusqlite::Connection, key: &str, default: bool) -> bool {
    match get_setting_trimmed(conn, key) {
        Ok(Some(raw)) => {
            let t = raw.trim().to_ascii_lowercase();
            if matches!(t.as_str(), "1" | "true" | "yes" | "on") {
                true
            } else if matches!(t.as_str(), "0" | "false" | "no" | "off") {
                false
            } else {
                warn!(
                    "invalid boolean app_settings value for key '{}': '{}'; using default {}",
                    key, raw, default
                );
                default
            }
        }
        Ok(None) => default,
        Err(err) => {
            warn!(
                "failed to read app_settings key '{}': {}; using default {}",
                key, err, default
            );
            default
        }
    }
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
    pub published_version: i64,
    pub draft_version: i64,
    pub recordings_count: i64,
    pub clips_count: i64,
    pub captions_count: i64,
    pub created_at: String,
    pub updated_at: String,
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
pub struct UpdateFlowRuntimeByAccountInput {
    pub status: Option<String>,
    pub current_node: Option<String>,
    pub last_live_at: Option<String>,
    pub last_run_at: Option<String>,
    pub last_error: Option<String>,
}

fn ws_clock_tag() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn apply_flow_runtime_patch_for_account(
    conn: &rusqlite::Connection,
    account_id: i64,
    input: &UpdateFlowRuntimeByAccountInput,
) -> Result<(), String> {
    let touch_telemetry =
        input.last_live_at.is_some() || input.last_run_at.is_some() || input.last_error.is_some();

    let flow_id: Option<i64> = flow_id_for_account(conn, account_id).map_err(|e| e.to_string())?;

    let Some(flow_id) = flow_id else {
        return Ok(());
    };

    let mut sets: Vec<String> = Vec::new();
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx: usize = 1;

    if let Some(status) = &input.status {
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

    if let Some(current_node) = &input.current_node {
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

    if sets.is_empty() && !touch_telemetry {
        return Ok(());
    }

    if !sets.is_empty() {
        sets.push(format!("updated_at = {SQL_NOW_HCM}"));
        let sql = format!("UPDATE flows SET {} WHERE id = ?{idx}", sets.join(", "));
        params_vec.push(Box::new(flow_id));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        conn.execute(sql.as_str(), params_refs.as_slice())
            .map_err(|e| e.to_string())?;
    }

    if touch_telemetry {
        merge_start_runtime_fields(
            conn,
            flow_id,
            input.last_live_at.as_deref(),
            input.last_run_at.as_deref(),
            input.last_error.as_deref(),
        )?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ApplySidecarFlowRuntimeHintInput {
    pub account_id: i64,
    /// `account_live`, `account_offline`, `recording_started`, `recording_finished`, `clip_ready`, `caption_ready`
    pub hint: String,
    pub worker_status: Option<String>,
    pub error_message: Option<String>,
    /// Desktop DB clip row; used to append `clip`/`caption` `flow_node_runs` and to resolve `account_id` for `caption_ready`.
    pub clip_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SaveFlowNodeConfigInput {
    pub flow_id: i64,
    pub node_key: String,
    pub config_json: String,
}

fn append_pipeline_hint_node_run(
    conn: &Connection,
    hint: &str,
    clip_id: i64,
) -> Result<(), String> {
    let node_key = match hint {
        "clip_ready" => "clip",
        "caption_ready" => "caption",
        _ => return Ok(()),
    };
    let row: Option<(Option<i64>, Option<i64>)> = conn
        .query_row(
            "SELECT flow_run_id, flow_id FROM clips WHERE id = ?1",
            [clip_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((Some(flow_run_id), Some(flow_id))) = row else {
        return Ok(());
    };
    let output = serde_json::json!({ "clip_id": clip_id }).to_string();
    runtime_store::append_completed_pipeline_node_run(
        conn,
        flow_run_id,
        flow_id,
        node_key,
        output.as_str(),
    )
}

#[tauri::command]
pub fn list_flows(state: State<'_, AppState>) -> Result<Vec<FlowListItem>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT \
             f.id, \
             COALESCE(a_match.id, 0), \
             COALESCE(NULLIF(trim(json_extract(n.published_config_json, '$.username')), ''), a_match.username, ''), \
             f.name, f.enabled, f.status, f.current_node, \
             json_extract(n.published_config_json, '$.last_live_at'), \
             json_extract(n.published_config_json, '$.last_run_at'), \
             json_extract(n.published_config_json, '$.last_error'), \
             f.published_version, f.draft_version, \
             (SELECT COUNT(*) FROM recordings r WHERE r.flow_id = f.id), \
             (SELECT COUNT(*) FROM clips c WHERE c.flow_id = f.id), \
             (SELECT COUNT(*) FROM clips c WHERE c.flow_id = f.id AND c.caption_text IS NOT NULL AND trim(c.caption_text) <> ''), \
             f.created_at, f.updated_at \
             FROM flows f \
             LEFT JOIN flow_nodes n ON n.flow_id = f.id AND n.node_key = 'start' \
             LEFT JOIN accounts a_match ON lower(a_match.username) = lower(json_extract(n.published_config_json, '$.username')) \
               AND trim(json_extract(n.published_config_json, '$.username')) != '' \
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
                published_version: row.get(10)?,
                draft_version: row.get(11)?,
                recordings_count: row.get(12)?,
                clips_count: row.get(13)?,
                captions_count: row.get(14)?,
                created_at: row.get(15)?,
                updated_at: row.get(16)?,
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
pub fn create_flow(state: State<'_, AppState>, input: CreateFlowInput) -> Result<i64, String> {
    let flow_name = input.name.trim();
    if flow_name.is_empty() {
        return Err("name is required".to_string());
    }

    let mut conn = state.db.lock().map_err(|e| e.to_string())?;

    let record_max_duration_minutes =
        get_int_setting_or_default(&conn, "recording_max_minutes", 5).max(1);

    let clip_min_duration_sec = get_int_setting_or_default(&conn, "clip_min_duration", 15).max(1);
    let clip_max_duration_sec =
        get_int_setting_or_default(&conn, "clip_max_duration", 90).max(clip_min_duration_sec);
    let clip_auto_process_after_record =
        get_bool_setting_or_default(&conn, "auto_process_after_record", true);
    let clip_audio_processing_enabled =
        get_bool_setting_or_default(&conn, "audio_processing_enabled", true);
    let clip_speech_merge_gap_sec =
        get_float_setting_or_default(&conn, "speech_merge_gap_sec", 0.5).max(0.0);
    let clip_speech_cut_tolerance_sec =
        get_float_setting_or_default(&conn, "speech_cut_tolerance_sec", 1.5).max(0.0);
    let clip_stt_num_threads = get_int_setting_or_default(&conn, "stt_num_threads", 4).max(1);
    let clip_stt_quantize = get_setting_trimmed(&conn, "stt_quantize")?
        .map(|q| match q.trim().to_ascii_lowercase().as_str() {
            "fp32" | "float32" => "fp32".to_string(),
            "int8" => "int8".to_string(),
            _ => "auto".to_string(),
        })
        .unwrap_or_else(|| "auto".to_string());

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        &format!(
            "INSERT INTO flows (name, enabled, status, published_version, draft_version, created_at, updated_at) \
             VALUES (?1, ?2, 'idle', 1, 1, {}, {})",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![
            flow_name,
            if input.enabled.unwrap_or(true) {
                1i64
            } else {
                0i64
            }
        ],
    )
    .map_err(|e| e.to_string())?;
    let flow_id = tx.last_insert_rowid();

    let start_config = json!({
        "username": "",
        "cookies_json": "",
        "proxy_url": "",
        "auto_record": 1,
        "poll_interval_seconds": 60,
        "watcher_mode": "live_polling",
        "retry_limit": 3,
    })
    .to_string();
    let record_config = json!({
        "max_duration_minutes": record_max_duration_minutes,
    })
    .to_string();
    let clip_config = json!({
        "auto_process_after_record": clip_auto_process_after_record,
        "clip_min_duration": clip_min_duration_sec,
        "clip_max_duration": clip_max_duration_sec,
        "audio_processing_enabled": clip_audio_processing_enabled,
        "speech_merge_gap_sec": clip_speech_merge_gap_sec,
        "speech_cut_tolerance_sec": clip_speech_cut_tolerance_sec,
        "stt_num_threads": clip_stt_num_threads,
        "stt_quantize": clip_stt_quantize,
    })
    .to_string();
    let caption_config = json!({
        "inherit_global_defaults": true,
    })
    .to_string();
    let upload_config = json!({
        "inherit_global_defaults": true,
    })
    .to_string();

    for (node_key, position, config_json) in [
        ("start", 1i64, start_config.as_str()),
        ("record", 2i64, record_config.as_str()),
        ("clip", 3i64, clip_config.as_str()),
        ("caption", 4i64, caption_config.as_str()),
        ("upload", 5i64, upload_config.as_str()),
    ] {
        tx.execute(
            &format!(
                "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
                 VALUES (?1, ?2, ?3, ?4, ?4, {}, {})",
                SQL_NOW_HCM, SQL_NOW_HCM
            ),
            params![flow_id, node_key, position, config_json],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(flow_id)
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

    let touch_telemetry =
        input.last_live_at.is_some() || input.last_run_at.is_some() || input.last_error.is_some();

    if sets.is_empty() && !touch_telemetry {
        return Ok(());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;

    if !sets.is_empty() {
        sets.push(format!("updated_at = {SQL_NOW_HCM}"));
        let sql = format!("UPDATE flows SET {} WHERE id = ?{idx}", sets.join(", "));
        params_vec.push(Box::new(flow_id));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let changed = conn
            .execute(sql.as_str(), params_refs.as_slice())
            .map_err(|e| e.to_string())?;
        if changed == 0 {
            return Err(format!("flow {flow_id} not found"));
        }
    } else if touch_telemetry {
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
    }

    if touch_telemetry {
        merge_start_runtime_fields(
            &conn,
            flow_id,
            input.last_live_at.as_deref(),
            input.last_run_at.as_deref(),
            input.last_error.as_deref(),
        )?;
    }
    Ok(())
}

#[tauri::command]
pub fn update_flow_runtime_by_account(
    state: State<'_, AppState>,
    account_id: i64,
    input: UpdateFlowRuntimeByAccountInput,
) -> Result<(), String> {
    if account_id <= 0 {
        return Err("account_id must be positive".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    apply_flow_runtime_patch_for_account(&conn, account_id, &input)
}

/// Maps high-level sidecar events to `flows` / Start-node telemetry (desktop engine boundary).
#[tauri::command]
pub fn apply_sidecar_flow_runtime_hint(
    state: State<'_, AppState>,
    input: ApplySidecarFlowRuntimeHintInput,
) -> Result<(), String> {
    let hint = input.hint.trim();
    if hint.is_empty() {
        return Err("hint is required".to_string());
    }

    let tag = ws_clock_tag();
    let patch = match hint {
        "account_live" => UpdateFlowRuntimeByAccountInput {
            status: Some("watching".into()),
            current_node: Some("start".into()),
            last_live_at: Some(tag),
            last_run_at: None,
            last_error: Some(String::new()),
        },
        "account_offline" => UpdateFlowRuntimeByAccountInput {
            status: Some("idle".into()),
            current_node: Some(String::new()),
            last_live_at: None,
            last_run_at: None,
            last_error: None,
        },
        "recording_started" => UpdateFlowRuntimeByAccountInput {
            status: Some("recording".into()),
            current_node: Some("record".into()),
            last_live_at: None,
            last_run_at: Some(tag),
            last_error: Some(String::new()),
        },
        "recording_finished" => {
            let worker = input
                .worker_status
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if worker == "error" || worker == "failed" {
                let msg = input
                    .error_message
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Recording failed");
                UpdateFlowRuntimeByAccountInput {
                    status: Some("error".into()),
                    current_node: Some("record".into()),
                    last_live_at: None,
                    last_run_at: Some(tag),
                    last_error: Some(msg.to_string()),
                }
            } else if worker == "completed" || worker == "done" {
                UpdateFlowRuntimeByAccountInput {
                    status: Some("processing".into()),
                    current_node: Some("record".into()),
                    last_live_at: None,
                    last_run_at: Some(tag),
                    last_error: Some(String::new()),
                }
            } else {
                let last_err = input.error_message.as_ref().and_then(|s| {
                    let t = s.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                });
                UpdateFlowRuntimeByAccountInput {
                    status: Some("idle".into()),
                    current_node: Some(String::new()),
                    last_live_at: None,
                    last_run_at: Some(tag),
                    last_error: last_err,
                }
            }
        }
        "clip_ready" => UpdateFlowRuntimeByAccountInput {
            status: Some("processing".into()),
            current_node: Some("clip".into()),
            last_live_at: None,
            last_run_at: Some(tag),
            last_error: Some(String::new()),
        },
        "caption_ready" => UpdateFlowRuntimeByAccountInput {
            status: Some("processing".into()),
            current_node: Some("caption".into()),
            last_live_at: None,
            last_run_at: Some(tag),
            last_error: Some(String::new()),
        },
        _ => return Ok(()),
    };

    let mut conn = state.db.lock().map_err(|e| e.to_string())?;

    let mut account_id = input.account_id;
    if hint == "caption_ready" {
        if let Some(cid) = input.clip_id {
            if cid > 0 {
                let acct: Option<i64> = conn
                    .query_row("SELECT account_id FROM clips WHERE id = ?1", [cid], |r| {
                        r.get(0)
                    })
                    .optional()
                    .map_err(|e| e.to_string())?;
                if let Some(a) = acct {
                    account_id = a;
                }
            }
        }
    }
    if account_id <= 0 {
        return Err("account_id must be positive (or pass clip_id for caption_ready)".to_string());
    }

    apply_flow_runtime_patch_for_account(&conn, account_id, &patch)?;

    let flow_id_opt = flow_id_for_account(&conn, account_id).map_err(|e| e.to_string())?;
    if let Some(fid) = flow_id_opt {
        if hint == "recording_started" {
            let run_id = runtime_store::ensure_running_flow_run(&conn, fid, "recording_started")?;
            runtime_store::upsert_running_record_node_run(&conn, run_id, fid)?;
        } else if hint == "recording_finished" {
            let worker = input
                .worker_status
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_lowercase();
            let (success, err): (bool, Option<String>) = if worker == "error" || worker == "failed"
            {
                let msg = input
                    .error_message
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Recording failed")
                    .to_string();
                (false, Some(msg))
            } else {
                (true, None)
            };
            runtime_store::finalize_latest_running_flow_run(
                &mut conn,
                fid,
                success,
                err.as_deref(),
            )?;
        }
    }

    if matches!(hint, "clip_ready" | "caption_ready") {
        if let Some(cid) = input.clip_id {
            if cid > 0 {
                append_pipeline_hint_node_run(&conn, hint, cid)?;
            }
        }
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

    let node_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM flow_nodes WHERE flow_id = ?1 AND node_key = ?2",
            params![input.flow_id, node_key],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if node_exists > 0 {
        flow_engine::apply_flow_node_draft(&conn, input.flow_id, node_key, config_json)?;
    } else {
        serde_json::from_str::<serde_json::Value>(config_json)
            .map_err(|e| format!("config_json must be valid JSON: {e}"))?;
        let position: i64 = match node_key {
            "start" => 1,
            "record" => 2,
            "clip" => 3,
            "caption" => 4,
            "upload" => 5,
            _ => 1,
        };
        conn.execute(
            &format!(
                "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
                 VALUES (?1, ?2, ?3, ?4, ?4, {}, {})",
                SQL_NOW_HCM, SQL_NOW_HCM
            ),
            params![input.flow_id, node_key, position, config_json],
        )
        .map_err(|e| e.to_string())?;
    }

    conn.query_row(
        "SELECT id, flow_id, node_key, draft_config_json, draft_updated_at \
         FROM flow_nodes WHERE flow_id = ?1 AND node_key = ?2",
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
             c.notes, c.flow_id, c.flow_run_id, c.transcript_text, c.caption_text, c.caption_status, c.caption_error, c.caption_generated_at, c.created_at, c.updated_at \
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
