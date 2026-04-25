use super::RestartFlowRunResult;
use crate::live_runtime::manager::LiveRuntimeManager;
use rusqlite::Connection;

pub(crate) fn restart_flow_run_with_conn(
    conn: &mut Connection,
    runtime_manager: &LiveRuntimeManager,
    flow_id: i64,
) -> Result<RestartFlowRunResult, String> {
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

    runtime_manager.restart_active_run(conn, flow_id)?;
    Ok(RestartFlowRunResult {
        flow_id,
        restarted: true,
    })
}
