use super::PublishFlowResult;
use crate::live_runtime::manager::LiveRuntimeManager;
use crate::time_hcm::SQL_NOW_HCM;
use crate::workflow::{record_node, start_node};
use rusqlite::{params, Connection};

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

pub(crate) fn publish_flow_definition_with_conn(
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

pub(crate) fn publish_flow_with_runtime_reconcile(
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
