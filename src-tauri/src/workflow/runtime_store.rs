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

/// Ensures the active `flow_run` has a `record` node row in `running` state (for runtime UI).
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
            "CREATE TABLE flows (
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
}
