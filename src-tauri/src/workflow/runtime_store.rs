use crate::db::models::FlowNodeDefinition;
use crate::time_hcm::SQL_NOW_HCM;

use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult, Row};

#[cfg(test)]
use super::types::{FlowNodeRun, FlowRun};

fn map_flow_node_definition_row(row: &Row) -> SqlResult<FlowNodeDefinition> {
    Ok(FlowNodeDefinition {
        id: row.get(0)?,
        flow_id: row.get(1)?,
        node_key: row.get(2)?,
        position: row.get(3)?,
        draft_config_json: row.get(4)?,
        published_config_json: row.get(5)?,
        draft_updated_at: row.get(6)?,
        published_at: row.get(7)?,
    })
}

pub fn list_flow_node_definitions(
    conn: &Connection,
    flow_id: i64,
) -> Result<Vec<FlowNodeDefinition>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, flow_id, node_key, position, draft_config_json, published_config_json, \
             draft_updated_at, published_at \
             FROM flow_nodes WHERE flow_id = ?1 ORDER BY position ASC, node_key ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([flow_id], map_flow_node_definition_row)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// Returns an existing `running` `flow_run` id or inserts one at the current `published_version`.
#[allow(dead_code)]
pub fn ensure_running_flow_run(
    conn: &Connection,
    flow_id: i64,
    trigger_reason: &str,
) -> Result<i64, String> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM flow_runs WHERE flow_id = ?1 AND status = 'running' \
             ORDER BY id DESC LIMIT 1",
            [flow_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let def_ver: i64 = conn
        .query_row(
            "SELECT published_version FROM flows WHERE id = ?1",
            [flow_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    conn.execute(
        &format!(
            "INSERT INTO flow_runs (flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (?1, ?2, 'running', {}, ?3)",
            SQL_NOW_HCM
        ),
        params![flow_id, def_ver, trigger_reason],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_latest_running_flow_run_id(
    conn: &Connection,
    flow_id: i64,
) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT id FROM flow_runs WHERE flow_id = ?1 AND status = 'running' \
         ORDER BY id DESC LIMIT 1",
        [flow_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

/// Ensures the active `flow_run` has a `record` node row in `running` state (for runtime UI).
#[allow(dead_code)]
pub fn upsert_running_record_node_run(
    conn: &Connection,
    flow_run_id: i64,
    flow_id: i64,
) -> Result<(), String> {
    let row: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, status FROM flow_node_runs \
             WHERE flow_run_id = ?1 AND node_key = 'record' AND status IN ('pending', 'running') \
             ORDER BY id DESC LIMIT 1",
            [flow_run_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    match row {
        Some((_id, status)) if status == "running" => Ok(()),
        Some((id, _)) => {
            conn.execute(
                &format!(
                    "UPDATE flow_node_runs SET status = 'running', \
                         started_at = COALESCE(started_at, {}) WHERE id = ?1",
                    SQL_NOW_HCM
                ),
                params![id],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        }
        None => {
            conn
                .execute(
                    &format!(
                        "INSERT INTO flow_node_runs (flow_run_id, flow_id, node_key, status, started_at) \
                         VALUES (?1, ?2, 'record', 'running', {})",
                        SQL_NOW_HCM
                    ),
                    params![flow_run_id, flow_id],
                )
                .map_err(|e| e.to_string())?;
            Ok(())
        }
    }
}

/// Cancels in-flight node runs + running `flow_runs`, then starts a new `running` row (publish restart).
pub fn restart_running_flow_runs_for_flow(
    conn: &mut Connection,
    flow_id: i64,
    trigger_reason: &str,
) -> Result<i64, String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let def_ver: i64 = tx
        .query_row(
            "SELECT published_version FROM flows WHERE id = ?1",
            [flow_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    tx.execute(
        &format!(
            "UPDATE flow_node_runs SET status = 'cancelled', ended_at = {} \
             WHERE flow_run_id IN (SELECT id FROM flow_runs WHERE flow_id = ?1 AND status = 'running') \
             AND status IN ('pending', 'running')",
            SQL_NOW_HCM
        ),
        [flow_id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        &format!(
            "UPDATE flow_runs SET status = 'cancelled', ended_at = {}, \
             error = 'Publish restart' \
             WHERE flow_id = ?1 AND status = 'running'",
            SQL_NOW_HCM
        ),
        [flow_id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        &format!(
            "INSERT INTO flow_runs (flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (?1, ?2, 'running', {}, ?3)",
            SQL_NOW_HCM
        ),
        params![flow_id, def_ver, trigger_reason],
    )
    .map_err(|e| e.to_string())?;
    let new_id = tx.last_insert_rowid();
    tx.commit().map_err(|e| e.to_string())?;
    Ok(new_id)
}

/// Marks the latest `running` `flow_run` for this flow completed or failed (recording cycle end).
/// Also closes the matching `record` `flow_node_runs` row when present.
#[allow(dead_code)]
pub fn finalize_latest_running_flow_run(
    conn: &mut Connection,
    flow_id: i64,
    success: bool,
    error_message: Option<&str>,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let run_id: Option<i64> = tx
        .query_row(
            "SELECT id FROM flow_runs WHERE flow_id = ?1 AND status = 'running' \
             ORDER BY id DESC LIMIT 1",
            [flow_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(run_id) = run_id else {
        tx.commit().map_err(|e| e.to_string())?;
        return Ok(());
    };
    if success {
        tx.execute(
            &format!(
                "UPDATE flow_node_runs SET status = 'completed', ended_at = {} \
                 WHERE flow_run_id = ?1 AND node_key = 'record' AND status = 'running'",
                SQL_NOW_HCM
            ),
            params![run_id],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            &format!(
                "UPDATE flow_runs SET status = 'completed', ended_at = {}, error = NULL WHERE id = ?1",
                SQL_NOW_HCM
            ),
            params![run_id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let err = error_message.unwrap_or("Recording failed");
        tx.execute(
            &format!(
                "UPDATE flow_node_runs SET status = 'failed', ended_at = {}, error = ?2 \
                 WHERE flow_run_id = ?1 AND node_key = 'record' AND status = 'running'",
                SQL_NOW_HCM
            ),
            params![run_id, err],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            &format!(
                "UPDATE flow_runs SET status = 'failed', ended_at = {}, error = ?2 WHERE id = ?1",
                SQL_NOW_HCM
            ),
            params![run_id, err],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[allow(dead_code)]
pub fn finalize_record_node_run(
    conn: &mut Connection,
    flow_id: i64,
    success: bool,
    error_message: Option<&str>,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let run_id: Option<i64> = tx
        .query_row(
            "SELECT id FROM flow_runs WHERE flow_id = ?1 AND status = 'running' ORDER BY id DESC LIMIT 1",
            [flow_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(run_id) = run_id else {
        tx.commit().map_err(|e| e.to_string())?;
        return Ok(());
    };

    if success {
        tx.execute(
            &format!(
                "UPDATE flow_node_runs SET status = 'completed', ended_at = {} WHERE flow_run_id = ?1 AND node_key = 'record' AND status = 'running'",
                SQL_NOW_HCM
            ),
            params![run_id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let err = error_message.unwrap_or("Recording failed");
        tx.execute(
            &format!(
                "UPDATE flow_node_runs SET status = 'failed', ended_at = {}, error = ?2 WHERE flow_run_id = ?1 AND node_key = 'record' AND status = 'running'",
                SQL_NOW_HCM
            ),
            params![run_id, err],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[allow(dead_code)]
pub fn finalize_record_node_run_by_flow_run_id(
    conn: &mut Connection,
    flow_run_id: i64,
    success: bool,
    error_message: Option<&str>,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if success {
        tx.execute(
            &format!(
                "UPDATE flow_node_runs SET status = 'completed', ended_at = {} WHERE flow_run_id = ?1 AND node_key = 'record' AND status = 'running'",
                SQL_NOW_HCM
            ),
            params![flow_run_id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let err = error_message.unwrap_or("Recording failed");
        tx.execute(
            &format!(
                "UPDATE flow_node_runs SET status = 'failed', ended_at = {}, error = ?2 WHERE flow_run_id = ?1 AND node_key = 'record' AND status = 'running'",
                SQL_NOW_HCM
            ),
            params![flow_run_id, err],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[allow(dead_code)]
pub fn finalize_flow_run_by_id(
    conn: &mut Connection,
    flow_run_id: i64,
    success: bool,
    error_message: Option<&str>,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if success {
        tx.execute(
            &format!(
                "UPDATE flow_runs SET status = 'completed', ended_at = {}, error = NULL WHERE id = ?1",
                SQL_NOW_HCM
            ),
            params![flow_run_id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let err = error_message.unwrap_or("Recording failed");
        tx.execute(
            &format!(
                "UPDATE flow_runs SET status = 'failed', ended_at = {}, error = ?2 WHERE id = ?1",
                SQL_NOW_HCM
            ),
            params![flow_run_id, err],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[allow(dead_code)]
pub fn cancel_flow_run_by_id(
    conn: &mut Connection,
    flow_run_id: i64,
    error_message: Option<&str>,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let err = error_message.unwrap_or("Recording cancelled");
    tx.execute(
        &format!(
            "UPDATE flow_node_runs SET status = 'cancelled', ended_at = {}, error = ?2 WHERE flow_run_id = ?1 AND node_key = 'record' AND status IN ('pending', 'running')",
            SQL_NOW_HCM
        ),
        params![flow_run_id, err],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        &format!(
            "UPDATE flow_runs SET status = 'cancelled', ended_at = {}, error = ?2 WHERE id = ?1",
            SQL_NOW_HCM
        ),
        params![flow_run_id, err],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn cancel_latest_running_flow_run(
    conn: &mut Connection,
    flow_id: i64,
    error_message: Option<&str>,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let run_id: Option<i64> = tx
        .query_row(
            "SELECT id FROM flow_runs WHERE flow_id = ?1 AND status = 'running' \
             ORDER BY id DESC LIMIT 1",
            [flow_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(run_id) = run_id else {
        tx.commit().map_err(|e| e.to_string())?;
        return Ok(());
    };
    let err = error_message.unwrap_or("Cancelled");
    tx.execute(
        &format!(
            "UPDATE flow_node_runs SET status = 'cancelled', ended_at = {}, error = ?2 \
             WHERE flow_run_id = ?1 AND status IN ('pending', 'running')",
            SQL_NOW_HCM
        ),
        params![run_id, err],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        &format!(
            "UPDATE flow_runs SET status = 'cancelled', ended_at = {}, error = ?2 WHERE id = ?1",
            SQL_NOW_HCM
        ),
        params![run_id, err],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// Completed step for `clip` / `caption` (linked to the clip row's `flow_run_id`).
pub fn append_completed_pipeline_node_run(
    conn: &Connection,
    flow_run_id: i64,
    flow_id: i64,
    node_key: &str,
    output_json: &str,
) -> Result<(), String> {
    if node_key != "clip" && node_key != "caption" {
        return Err(format!(
            "append_completed_pipeline_node_run: invalid node_key {node_key}"
        ));
    }
    conn.execute(
        &format!(
            "INSERT INTO flow_node_runs (flow_run_id, flow_id, node_key, status, started_at, ended_at, output_json) \
             VALUES (?1, ?2, ?3, 'completed', {}, {}, ?4)",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![flow_run_id, flow_id, node_key, output_json],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn append_failed_pipeline_node_run(
    conn: &mut Connection,
    flow_run_id: i64,
    flow_id: i64,
    node_key: &str,
    error_message: &str,
) -> Result<(), String> {
    if node_key != "clip" && node_key != "caption" {
        return Err(format!(
            "append_failed_pipeline_node_run: invalid node_key {node_key}"
        ));
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        &format!(
            "INSERT INTO flow_node_runs (flow_run_id, flow_id, node_key, status, started_at, ended_at, error) \
             VALUES (?1, ?2, ?3, 'failed', {}, {}, ?4)",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![flow_run_id, flow_id, node_key, error_message],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        &format!(
            "UPDATE flow_runs SET status = 'failed', ended_at = {}, error = ?2 WHERE id = ?1",
            SQL_NOW_HCM
        ),
        params![flow_run_id, error_message],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn create_run_with_completed_start_node(
    conn: &Connection,
    flow_id: i64,
    definition_version: i64,
    output_json: &str,
) -> Result<i64, String> {
    conn.execute(
        &format!(
            "INSERT INTO flow_runs (flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (?1, ?2, 'running', {}, 'start_live_detected')",
            SQL_NOW_HCM
        ),
        params![flow_id, definition_version],
    )
    .map_err(|e| e.to_string())?;
    let flow_run_id = conn.last_insert_rowid();

    conn.execute(
        &format!(
            "INSERT INTO flow_node_runs (flow_run_id, flow_id, node_key, status, started_at, ended_at, input_json, output_json) \
             VALUES (?1, ?2, 'start', 'completed', {}, {}, ?3, ?3)",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![flow_run_id, flow_id, output_json],
    )
    .map_err(|e| e.to_string())?;

    Ok(flow_run_id)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_last_room_id_from_latest_start_node_run(
    conn: &Connection,
    flow_id: i64,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT json_extract(fnr.output_json, '$.room_id') FROM flow_node_runs fnr \
         JOIN flow_runs fr ON fr.id = fnr.flow_run_id \
         WHERE fnr.flow_id = ?1 \
           AND fnr.node_key = 'start' \
           AND fnr.status = 'completed' \
           AND fnr.output_json IS NOT NULL \
           AND json_extract(fnr.output_json, '$.room_id') IS NOT NULL \
         ORDER BY fr.id DESC, fnr.id DESC LIMIT 1",
        [flow_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_last_completed_room_id_for_flow(
    conn: &Connection,
    flow_id: i64,
) -> Result<Option<String>, String> {
    let recording_room_id = conn
        .query_row(
            "SELECT r.room_id FROM recordings r \
             JOIN flow_runs fr ON fr.id = r.flow_run_id \
             WHERE r.flow_id = ?1 \
               AND r.room_id IS NOT NULL \
               AND trim(r.room_id) <> '' \
               AND fr.status IN ('completed', 'cancelled') \
             ORDER BY fr.id DESC, r.id DESC LIMIT 1",
            [flow_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if recording_room_id.is_some() {
        return Ok(recording_room_id);
    }

    load_last_room_id_from_latest_start_node_run(conn, flow_id)
}

#[cfg(test)]
fn map_flow_run_row(row: &Row) -> SqlResult<FlowRun> {
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

#[cfg(test)]
fn map_flow_node_run_row(row: &Row) -> SqlResult<FlowNodeRun> {
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

#[cfg(test)]
pub fn create_flow_run(
    conn: &Connection,
    flow_id: i64,
    definition_version: i64,
    trigger_reason: &str,
) -> Result<FlowRun, String> {
    conn.execute(
        &format!(
            "INSERT INTO flow_runs (flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (?1, ?2, 'running', {}, ?3)",
            SQL_NOW_HCM
        ),
        params![flow_id, definition_version, trigger_reason],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    read_flow_run(conn, id)
}

#[cfg(test)]
pub fn read_flow_run(conn: &Connection, id: i64) -> Result<FlowRun, String> {
    conn.query_row(
        "SELECT id, flow_id, definition_version, status, started_at, ended_at, trigger_reason, error \
         FROM flow_runs WHERE id = ?1",
        [id],
        map_flow_run_row,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
pub fn insert_flow_node_run_pending(
    conn: &Connection,
    flow_run_id: i64,
    flow_id: i64,
    node_key: &str,
) -> Result<FlowNodeRun, String> {
    conn.execute(
        "INSERT INTO flow_node_runs (flow_run_id, flow_id, node_key, status) VALUES (?1, ?2, ?3, 'pending')",
        params![flow_run_id, flow_id, node_key],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    read_flow_node_run(conn, id)
}

#[cfg(test)]
pub fn read_flow_node_run(conn: &Connection, id: i64) -> Result<FlowNodeRun, String> {
    conn.query_row(
        "SELECT id, flow_run_id, flow_id, node_key, status, started_at, ended_at, input_json, output_json, error \
         FROM flow_node_runs WHERE id = ?1",
        [id],
        map_flow_node_run_row,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_schema(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL DEFAULT '',
                type TEXT NOT NULL DEFAULT 'monitored'
            );
            CREATE TABLE flows (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                status TEXT NOT NULL DEFAULT 'idle',
                current_node TEXT,
                published_version INTEGER NOT NULL DEFAULT 1,
                draft_version INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'))
            );
            CREATE TABLE flow_nodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
                node_key TEXT NOT NULL,
                position INTEGER NOT NULL,
                draft_config_json TEXT NOT NULL DEFAULT '{}',
                published_config_json TEXT NOT NULL DEFAULT '{}',
                draft_updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                published_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                UNIQUE(flow_id, node_key)
            );
            CREATE TABLE flow_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
                definition_version INTEGER NOT NULL,
                status TEXT NOT NULL,
                started_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                ended_at TEXT,
                trigger_reason TEXT,
                error TEXT
            );
            CREATE TABLE flow_node_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                flow_run_id INTEGER NOT NULL REFERENCES flow_runs(id) ON DELETE CASCADE,
                flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
                node_key TEXT NOT NULL,
                status TEXT NOT NULL,
                started_at TEXT,
                ended_at TEXT,
                input_json TEXT,
                output_json TEXT,
                error TEXT
            );
            CREATE TABLE recordings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                room_id TEXT,
                status TEXT NOT NULL DEFAULT 'recording',
                started_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                ended_at TEXT,
                duration_seconds INTEGER NOT NULL DEFAULT 0,
                file_path TEXT,
                file_size_bytes INTEGER NOT NULL DEFAULT 0,
                stream_url TEXT,
                bitrate TEXT,
                error_message TEXT,
                auto_process INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                flow_id INTEGER REFERENCES flows(id) ON DELETE SET NULL,
                flow_run_id INTEGER REFERENCES flow_runs(id) ON DELETE SET NULL
            );",
        )
        .expect("schema");
    }

    #[test]
    fn create_flow_run_and_node_run_roundtrip() {
        let conn = Connection::open_in_memory().unwrap();
        in_memory_schema(&conn);
        conn.execute(
            "INSERT INTO flows (name, enabled, status) VALUES ('t', 1, 'idle')",
            [],
        )
        .unwrap();
        let flow_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json) \
             VALUES (?1, 'start', 1, '{}', '{}')",
            [flow_id],
        )
        .unwrap();

        let defs = list_flow_node_definitions(&conn, flow_id).unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].node_key, "start");

        let run = create_flow_run(&conn, flow_id, 1, "unit_test").unwrap();
        assert_eq!(run.flow_id, flow_id);
        assert_eq!(run.status, "running");

        let nr = insert_flow_node_run_pending(&conn, run.id, flow_id, "start").unwrap();
        assert_eq!(nr.status, "pending");
        let nr2 = read_flow_node_run(&conn, nr.id).unwrap();
        assert_eq!(nr2.node_key, "start");
    }

    #[test]
    fn load_last_completed_room_id_for_flow_reads_latest_room_from_recordings() {
        let conn = Connection::open_in_memory().unwrap();
        in_memory_schema(&conn);
        conn.execute(
            "INSERT INTO accounts (id, username, display_name, type) VALUES (1, 'shop_abc', 'Shop ABC', 'monitored')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version) \
             VALUES (1, 'Flow', 1, 'idle', 1, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, ended_at, trigger_reason) \
             VALUES (11, 1, 1, 'completed', datetime('now', '+7 hours'), datetime('now', '+7 hours'), 'test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO recordings (account_id, room_id, status, duration_seconds, file_size_bytes, flow_id, flow_run_id, created_at, started_at) \
             VALUES (1, '7312345', 'done', 0, 0, 1, 11, datetime('now', '+7 hours'), datetime('now', '+7 hours'))",
            [],
        )
        .unwrap();

        assert_eq!(
            load_last_completed_room_id_for_flow(&conn, 1)
                .unwrap()
                .as_deref(),
            Some("7312345")
        );
    }

    #[test]
    fn create_run_with_completed_start_node_creates_running_flow_run_and_completed_node_output() {
        let conn = Connection::open_in_memory().unwrap();
        in_memory_schema(&conn);
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version) \
             VALUES (1, 'Flow', 1, 'idle', 3, 3)",
            [],
        )
        .unwrap();

        let output_json = r#"{"account_id":9,"username":"shop_abc","room_id":"7312345","stream_url":"https://example.com/live.flv","viewer_count":77,"detected_at":"2026-04-18 09:30:00"}"#;

        let flow_run_id = create_run_with_completed_start_node(&conn, 1, 3, output_json).unwrap();

        let flow_run_row: (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, ended_at, trigger_reason FROM flow_runs WHERE id = ?1",
                [flow_run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(flow_run_row.0, "running");
        assert!(flow_run_row.1.is_none());
        assert_eq!(flow_run_row.2.as_deref(), Some("start_live_detected"));

        let node_row: (String, Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, started_at, input_json, output_json FROM flow_node_runs \
                 WHERE flow_run_id = ?1 AND node_key = 'start'",
                [flow_run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(node_row.0, "completed");
        assert!(node_row.1.is_some());
        assert_eq!(node_row.2.as_deref(), Some(output_json));
        assert_eq!(node_row.3.as_deref(), Some(output_json));
    }

    #[test]
    fn load_last_completed_room_id_for_flow_reads_room_id_from_start_output_when_recording_missing()
    {
        let conn = Connection::open_in_memory().unwrap();
        in_memory_schema(&conn);
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version) \
             VALUES (1, 'Flow', 1, 'idle', 1, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, ended_at, trigger_reason) \
             VALUES (11, 1, 1, 'completed', datetime('now', '+7 hours'), datetime('now', '+7 hours'), 'test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flow_node_runs (flow_run_id, flow_id, node_key, status, started_at, ended_at, output_json) \
             VALUES (11, 1, 'start', 'completed', datetime('now', '+7 hours'), datetime('now', '+7 hours'), ?1)",
            [r#"{"account_id":9,"username":"shop_abc","room_id":"7312345","stream_url":"https://example.com/live.flv","viewer_count":77,"detected_at":"2026-04-18 09:30:00"}"#],
        )
        .unwrap();

        assert_eq!(
            load_last_completed_room_id_for_flow(&conn, 1)
                .unwrap()
                .as_deref(),
            Some("7312345")
        );
    }

    #[test]
    fn finalize_record_node_run_by_flow_run_id_and_flow_run_by_id_mark_failed() {
        let conn = Connection::open_in_memory().unwrap();
        in_memory_schema(&conn);
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version) VALUES (1, 'Flow', 1, 'recording', 1, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) VALUES (11, 1, 1, 'running', datetime('now', '+7 hours'), 'test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flow_node_runs (flow_run_id, flow_id, node_key, status, started_at) VALUES (11, 1, 'record', 'running', datetime('now', '+7 hours'))",
            [],
        )
        .unwrap();
        let mut conn = conn;

        finalize_record_node_run_by_flow_run_id(&mut conn, 11, false, Some("ffmpeg failed"))
            .unwrap();
        finalize_flow_run_by_id(&mut conn, 11, false, Some("ffmpeg failed")).unwrap();

        let run_status: String = conn
            .query_row("SELECT status FROM flow_runs WHERE id = 11", [], |row| {
                row.get(0)
            })
            .unwrap();
        let node_status: String = conn
            .query_row(
                "SELECT status FROM flow_node_runs WHERE flow_run_id = 11 AND node_key = 'record'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(run_status, "failed");
        assert_eq!(node_status, "failed");
    }

    #[test]
    fn cancel_latest_running_flow_run_marks_run_and_record_node_cancelled() {
        let conn = Connection::open_in_memory().unwrap();
        in_memory_schema(&conn);
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version) VALUES (1, 'Flow', 1, 'recording', 1, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) VALUES (11, 1, 1, 'running', datetime('now', '+7 hours'), 'test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flow_node_runs (flow_run_id, flow_id, node_key, status, started_at) VALUES (11, 1, 'record', 'running', datetime('now', '+7 hours'))",
            [],
        )
        .unwrap();
        let mut conn = conn;

        cancel_latest_running_flow_run(&mut conn, 1, Some("Recording cancelled")).unwrap();

        let run_status: String = conn
            .query_row("SELECT status FROM flow_runs WHERE id = 11", [], |row| {
                row.get(0)
            })
            .unwrap();
        let node_status: String = conn
            .query_row(
                "SELECT status FROM flow_node_runs WHERE flow_run_id = 11 AND node_key = 'record'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(run_status, "cancelled");
        assert_eq!(node_status, "cancelled");
    }
}
