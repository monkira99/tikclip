use crate::time_hcm::SQL_NOW_HCM;
use crate::workflow::{clip_node, record_node, start_node};
use rusqlite::{params, Connection};

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
        "clip" => clip_node::canonicalize_clip_config_json(draft_config_json)?,
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
