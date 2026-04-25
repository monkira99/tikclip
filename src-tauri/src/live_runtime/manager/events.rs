use super::LiveRuntimeManager;
use crate::commands::live_runtime::FlowRuntimeSnapshot;
use crate::live_runtime::logs::{format_terminal_entry, FlowRuntimeLogEntry, FlowRuntimeLogLevel};
use rusqlite::Connection;
use tauri::Emitter;

impl LiveRuntimeManager {
    fn runtime_snapshot_for_event(
        &self,
        conn: &Connection,
        flow_id: i64,
    ) -> Result<Option<FlowRuntimeSnapshot>, String> {
        crate::commands::live_runtime::list_live_runtime_snapshots_with_conn(conn, self)
            .map(|rows| rows.into_iter().find(|row| row.flow_id == flow_id))
    }

    pub(super) fn emit_runtime_update_for_flow(
        &self,
        conn: &Connection,
        flow_id: i64,
    ) -> Result<(), String> {
        let Some(snapshot) = self.runtime_snapshot_for_event(conn, flow_id)? else {
            return Ok(());
        };
        #[cfg(test)]
        {
            if let Ok(mut slot) = self.latest_runtime_event.lock() {
                *slot = Some(snapshot.clone());
            }
        }
        if let Some(app_handle) = &self.app_handle {
            app_handle
                .emit("flow-runtime-updated", snapshot)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn emit_runtime_log_entry(&self, entry: &FlowRuntimeLogEntry) -> Result<(), String> {
        #[cfg(test)]
        {
            if let Some(err) = self
                .runtime_log_emit_error
                .lock()
                .map_err(|e| e.to_string())?
                .clone()
            {
                return Err(err);
            }
        }

        if let Some(app_handle) = &self.app_handle {
            app_handle
                .emit("flow-runtime-log", entry.clone())
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    #[expect(
        clippy::too_many_arguments,
        reason = "Runtime log helper keeps structured fields explicit at the manager boundary"
    )]
    pub fn log_runtime_event(
        &self,
        flow_id: i64,
        flow_run_id: Option<i64>,
        external_recording_id: Option<&str>,
        stage: &str,
        event: &str,
        level: FlowRuntimeLogLevel,
        code: Option<&str>,
        message: &str,
        context: serde_json::Value,
    ) -> Result<(), String> {
        let entry = FlowRuntimeLogEntry::new(
            flow_id,
            flow_run_id,
            external_recording_id,
            stage,
            event,
            level,
            code,
            message,
            context,
        );

        let terminal_entry = format_terminal_entry(&entry);

        match &entry.level {
            FlowRuntimeLogLevel::Debug => {
                log::debug!("{}", terminal_entry);
            }
            FlowRuntimeLogLevel::Info => {
                log::info!("{}", terminal_entry);
            }
            FlowRuntimeLogLevel::Warn => {
                log::warn!("{}", terminal_entry);
            }
            FlowRuntimeLogLevel::Error => {
                log::error!("{}", terminal_entry);
            }
        }

        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.log_buffer.push(entry.clone());
        drop(state);

        if let Err(err) = self.emit_runtime_log_entry(&entry) {
            log::warn!(
                "flow_runtime failed to emit UI log event for flow={} run={:?} stage={} event={}: {}",
                entry.flow_id,
                entry.flow_run_id,
                entry.stage,
                entry.event,
                err
            );
        }

        Ok(())
    }

    pub fn list_runtime_logs(
        &self,
        flow_id: Option<i64>,
        limit: Option<usize>,
    ) -> Result<Vec<FlowRuntimeLogEntry>, String> {
        let mut rows = self
            .state
            .lock()
            .map_err(|e| e.to_string())?
            .log_buffer
            .entries();

        if let Some(flow_id) = flow_id {
            rows.retain(|row| row.flow_id == flow_id);
        }

        if let Some(limit) = limit {
            if limit == 0 {
                return Ok(Vec::new());
            }
            if rows.len() > limit {
                rows = rows.split_off(rows.len() - limit);
            }
        }

        Ok(rows)
    }
}
