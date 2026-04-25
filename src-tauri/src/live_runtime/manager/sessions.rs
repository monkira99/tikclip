use super::{
    store::{
        failed_snapshot, load_flow_runtime_config, open_runtime_connection,
        update_flow_runtime_by_flow_id,
    },
    ActivePollTaskHandle, ActiveRecordingHandle, LiveRuntimeManager, LiveRuntimeState,
};
use crate::commands::flows::UpdateFlowRuntimeByAccountInput;
#[cfg(test)]
use crate::commands::live_runtime::FlowRuntimeSnapshot;
#[cfg(test)]
use crate::live_runtime::logs::FlowRuntimeLogEntry;
use crate::live_runtime::logs::FlowRuntimeLogLevel;
use crate::live_runtime::session::LiveRuntimeSession;
use crate::live_runtime::types::LiveRuntimeSessionSnapshot;
#[cfg(test)]
use crate::tiktok::types::LiveStatus;
use crate::time_hcm::now_timestamp_hcm;
use crate::workflow::runtime_store;
use log::warn;
use rusqlite::Connection;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

impl LiveRuntimeManager {
    pub(super) fn cancel_and_remove_poll_task(state: &mut LiveRuntimeState, flow_id: i64) {
        if let Some(handle) = state.active_poll_tasks_by_flow.remove(&flow_id) {
            handle.cancelled.store(true, Ordering::SeqCst);
            #[cfg(test)]
            {
                state
                    .cancelled_poll_generations_by_flow
                    .entry(flow_id)
                    .or_default()
                    .push(handle.generation);
            }
        }
    }

    pub(super) fn replace_poll_task_for_session(
        state: &mut LiveRuntimeState,
        flow_id: i64,
        session: &mut LiveRuntimeSession,
    ) -> ActivePollTaskHandle {
        Self::cancel_and_remove_poll_task(state, flow_id);
        let generation = session.bump_poll_generation();
        let handle = ActivePollTaskHandle {
            generation,
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        state
            .active_poll_tasks_by_flow
            .insert(flow_id, handle.clone());
        handle
    }

    pub fn bootstrap_enabled_flows(&self, conn: &mut Connection) -> Result<(), String> {
        let flow_ids = {
            let mut stmt = conn
                .prepare("SELECT id FROM flows WHERE enabled = 1 ORDER BY id ASC")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| row.get::<_, i64>(0))
                .map_err(|e| e.to_string())?;
            let mut flow_ids = Vec::new();
            for row in rows {
                flow_ids.push(row.map_err(|e| e.to_string())?);
            }
            flow_ids
        };

        for flow_id in flow_ids {
            self.cancel_orphaned_bootstrap_run(conn, flow_id)?;
            let _ = self.log_runtime_event(
                flow_id,
                None,
                None,
                "session",
                "session_bootstrap_started",
                FlowRuntimeLogLevel::Info,
                None,
                "Starting runtime session bootstrap for enabled flow",
                serde_json::json!({
                    "flow_id": flow_id,
                }),
            );
            if let Err(err) = self.start_flow_session(conn, flow_id) {
                let err_message = err.clone();
                let _ = self.log_runtime_event(
                    flow_id,
                    None,
                    None,
                    "session",
                    "session_bootstrap_failed",
                    FlowRuntimeLogLevel::Error,
                    None,
                    "Failed to bootstrap runtime session",
                    serde_json::json!({
                        "flow_id": flow_id,
                        "error": err_message,
                    }),
                );
                warn!(
                    "failed to bootstrap live runtime session for flow {}: {}",
                    flow_id, err
                );
            }
        }

        Ok(())
    }

    pub fn start_flow_session(&self, conn: &Connection, flow_id: i64) -> Result<(), String> {
        let config = load_flow_runtime_config(conn, flow_id)?;
        let poll_interval_seconds = config.poll_interval_seconds;
        let last_completed_room_id =
            runtime_store::load_last_completed_room_id_for_flow(conn, flow_id)?;
        let active_flow_run_id = runtime_store::load_latest_running_flow_run_id(conn, flow_id)?;
        if !config.enabled {
            self.stop_flow_session(flow_id)?;
            return Ok(());
        }

        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.failed_snapshots_by_flow.remove(&flow_id);
        if state.sessions_by_flow.contains_key(&flow_id) {
            return Ok(());
        }
        if let Some(owner) = state
            .lease_owner_by_lookup_key
            .get(config.lookup_key.as_str())
            .copied()
        {
            let error = format!(
                "username lease already held for {} by flow {}",
                config.lookup_key, owner
            );
            state.failed_snapshots_by_flow.insert(
                flow_id,
                failed_snapshot(flow_id, &config, 1, error.as_str()),
            );
            drop(state);
            let _ = self.log_runtime_event(
                flow_id,
                active_flow_run_id,
                None,
                "start",
                "lease_conflict",
                FlowRuntimeLogLevel::Warn,
                Some("start.username_conflict"),
                "Skipped session start because username lease is already held",
                serde_json::json!({
                    "lookup_key": config.lookup_key.as_str(),
                    "lease_owner_flow_id": owner,
                }),
            );
            return Err(error);
        }

        let lookup_key = config.lookup_key.clone();
        let session = LiveRuntimeSession::new(
            config.flow_id,
            config.flow_name,
            config.username,
            lookup_key.clone(),
            1,
            last_completed_room_id,
        );
        let mut session = session;
        if let Some(flow_run_id) = active_flow_run_id {
            session.mark_flow_run_started(flow_run_id);
        }
        let poll_task = Self::replace_poll_task_for_session(&mut state, flow_id, &mut session);
        state
            .lease_owner_by_lookup_key
            .insert(lookup_key.clone(), config.flow_id);
        state.sessions_by_flow.insert(flow_id, session);
        drop(state);
        let _ = self.log_runtime_event(
            flow_id,
            active_flow_run_id,
            None,
            "start",
            "lease_acquired",
            FlowRuntimeLogLevel::Info,
            None,
            "Acquired username lease for runtime session",
            serde_json::json!({
                "lookup_key": lookup_key.as_str(),
            }),
        );
        let _ = self.log_runtime_event(
            flow_id,
            active_flow_run_id,
            None,
            "session",
            "session_started",
            FlowRuntimeLogLevel::Info,
            None,
            "Started runtime session",
            serde_json::json!({
                "generation": 1,
                "lookup_key": lookup_key.as_str(),
                "restored_active_flow_run": active_flow_run_id.is_some(),
            }),
        );
        self.spawn_poll_loop_worker(flow_id, poll_interval_seconds, poll_task);
        self.emit_runtime_update_for_flow(conn, flow_id)?;
        Ok(())
    }

    fn cancel_orphaned_bootstrap_run(
        &self,
        conn: &mut Connection,
        flow_id: i64,
    ) -> Result<(), String> {
        let Some(flow_run_id) = runtime_store::load_latest_running_flow_run_id(conn, flow_id)?
        else {
            return Ok(());
        };

        conn.execute(
            &format!(
                "UPDATE recordings SET \
                 status = 'cancelled', \
                 ended_at = COALESCE(ended_at, {}), \
                 error_message = COALESCE(NULLIF(error_message, ''), 'Interrupted by app restart'), \
                 duration_seconds = CASE \
                   WHEN duration_seconds > 0 THEN duration_seconds \
                   WHEN started_at IS NOT NULL THEN MAX(0, CAST((julianday({}) - julianday(started_at)) * 86400 AS INTEGER)) \
                   ELSE 0 END \
                 WHERE flow_run_id = ?1 AND status = 'recording'",
                crate::time_hcm::SQL_NOW_HCM,
                crate::time_hcm::SQL_NOW_HCM
            ),
            [flow_run_id],
        )
        .map_err(|e| e.to_string())?;
        runtime_store::cancel_flow_run_by_id(
            conn,
            flow_run_id,
            Some("Interrupted by app restart"),
        )?;
        update_flow_runtime_by_flow_id(
            conn,
            flow_id,
            &UpdateFlowRuntimeByAccountInput {
                status: Some("watching".to_string()),
                current_node: Some("start".to_string()),
                last_live_at: None,
                last_run_at: Some(now_timestamp_hcm()),
                last_error: Some(String::new()),
            },
        )?;
        let _ = self.log_runtime_event(
            flow_id,
            Some(flow_run_id),
            None,
            "session",
            "orphaned_run_cancelled",
            FlowRuntimeLogLevel::Warn,
            None,
            "Cancelled stale runtime run during app bootstrap",
            serde_json::json!({
                "flow_run_id": flow_run_id,
            }),
        );
        Ok(())
    }

    pub fn reconcile_flow(&self, conn: &Connection, flow_id: i64) -> Result<(), String> {
        let config = load_flow_runtime_config(conn, flow_id)?;
        let poll_interval_seconds = config.poll_interval_seconds;
        let last_completed_room_id =
            runtime_store::load_last_completed_room_id_for_flow(conn, flow_id)?;
        let active_flow_run_id = runtime_store::load_latest_running_flow_run_id(conn, flow_id)?;
        if !config.enabled {
            self.stop_flow_session(flow_id)?;
            return Ok(());
        }

        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        let next_generation = state
            .sessions_by_flow
            .get(&flow_id)
            .map(|session| session.generation() + 1)
            .unwrap_or(1);
        let current_lookup_key = state
            .sessions_by_flow
            .get(&flow_id)
            .as_ref()
            .map(|session| session.lookup_key().to_string());

        if let Some(owner) = state
            .lease_owner_by_lookup_key
            .get(config.lookup_key.as_str())
            .copied()
        {
            let conflicts_with_other_flow = owner != flow_id;
            let lookup_changed = current_lookup_key
                .as_deref()
                .map(|lookup_key| lookup_key != config.lookup_key)
                .unwrap_or(true);
            if conflicts_with_other_flow || lookup_changed {
                let error = format!(
                    "username lease already held for {} by flow {}",
                    config.lookup_key, owner
                );
                let previous = state.sessions_by_flow.remove(&flow_id);
                Self::cancel_and_remove_poll_task(&mut state, flow_id);
                if let Some(previous_session) = previous.as_ref() {
                    state
                        .lease_owner_by_lookup_key
                        .remove(previous_session.lookup_key());
                }
                if let Some(mut previous_session) = previous {
                    previous_session.fail(error.as_str());
                    state
                        .failed_snapshots_by_flow
                        .insert(flow_id, previous_session.snapshot());
                } else {
                    state.failed_snapshots_by_flow.insert(
                        flow_id,
                        failed_snapshot(flow_id, &config, next_generation, error.as_str()),
                    );
                }
                drop(state);
                let _ = self.log_runtime_event(
                    flow_id,
                    active_flow_run_id,
                    None,
                    "start",
                    "lease_conflict",
                    FlowRuntimeLogLevel::Warn,
                    Some("start.username_conflict"),
                    "Skipped session reconcile because username lease is already held",
                    serde_json::json!({
                        "lookup_key": config.lookup_key.as_str(),
                        "lease_owner_flow_id": owner,
                    }),
                );
                return Err(error);
            }
        }
        let previous = state.sessions_by_flow.remove(&flow_id);
        Self::cancel_and_remove_poll_task(&mut state, flow_id);
        if let Some(previous_session) = previous.as_ref() {
            state
                .lease_owner_by_lookup_key
                .remove(previous_session.lookup_key());
        }
        if let Some(mut previous_session) = previous {
            previous_session.teardown();
        }

        state
            .lease_owner_by_lookup_key
            .insert(config.lookup_key.clone(), flow_id);
        state.failed_snapshots_by_flow.remove(&flow_id);
        let lookup_key = config.lookup_key.clone();
        let mut session = LiveRuntimeSession::new(
            config.flow_id,
            config.flow_name,
            config.username,
            lookup_key.clone(),
            next_generation,
            last_completed_room_id,
        );
        session.set_poll_generation(next_generation.saturating_sub(1));
        if let Some(flow_run_id) = active_flow_run_id {
            session.mark_flow_run_started(flow_run_id);
        }
        let poll_task = Self::replace_poll_task_for_session(&mut state, flow_id, &mut session);
        state.sessions_by_flow.insert(flow_id, session);
        drop(state);
        let _ = self.log_runtime_event(
            flow_id,
            active_flow_run_id,
            None,
            "start",
            "lease_acquired",
            FlowRuntimeLogLevel::Info,
            None,
            "Acquired username lease for reconciled runtime session",
            serde_json::json!({
                "lookup_key": lookup_key.as_str(),
                "generation": next_generation,
            }),
        );
        let _ = self.log_runtime_event(
            flow_id,
            active_flow_run_id,
            None,
            "session",
            "session_started",
            FlowRuntimeLogLevel::Info,
            None,
            "Replaced runtime session after reconcile",
            serde_json::json!({
                "generation": next_generation,
                "lookup_key": lookup_key.as_str(),
            }),
        );
        self.spawn_poll_loop_worker(flow_id, poll_interval_seconds, poll_task);
        self.emit_runtime_update_for_flow(conn, flow_id)?;
        Ok(())
    }

    #[cfg(test)]
    pub fn session_is_polling_for_test(&self, flow_id: i64) -> bool {
        self.state
            .lock()
            .ok()
            .and_then(|state| {
                state
                    .sessions_by_flow
                    .get(&flow_id)
                    .map(LiveRuntimeSession::is_polling)
            })
            .unwrap_or(false)
    }

    #[allow(dead_code)]
    #[cfg(test)]
    pub fn session_last_completed_room_id_for_test(&self, flow_id: i64) -> Option<String> {
        self.state.lock().ok().and_then(|state| {
            state
                .sessions_by_flow
                .get(&flow_id)
                .and_then(|session| session.last_completed_room_id().map(str::to_string))
        })
    }

    #[cfg(test)]
    pub fn session_active_flow_run_id_for_test(&self, flow_id: i64) -> Option<i64> {
        self.state.lock().ok().and_then(|state| {
            state
                .sessions_by_flow
                .get(&flow_id)
                .and_then(LiveRuntimeSession::active_flow_run_id)
        })
    }

    #[cfg(test)]
    pub fn session_has_poll_task_for_test(&self, flow_id: i64) -> bool {
        self.state
            .lock()
            .ok()
            .map(|state| state.active_poll_tasks_by_flow.contains_key(&flow_id))
            .unwrap_or(false)
    }

    #[cfg(test)]
    pub fn session_generation_for_test(&self, flow_id: i64) -> Option<u64> {
        self.state.lock().ok().and_then(|state| {
            state
                .sessions_by_flow
                .get(&flow_id)
                .map(LiveRuntimeSession::poll_generation)
        })
    }

    #[cfg(test)]
    pub fn active_poll_task_count_for_test(&self) -> usize {
        self.state
            .lock()
            .ok()
            .map(|state| state.active_poll_tasks_by_flow.len())
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub fn cancelled_poll_generations_for_test(&self, flow_id: i64) -> Vec<u64> {
        self.state
            .lock()
            .ok()
            .and_then(|state| {
                state
                    .cancelled_poll_generations_by_flow
                    .get(&flow_id)
                    .cloned()
            })
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub fn drain_worker_threads_for_test(&self) {
        if let Ok(mut handles) = self.worker_threads.lock() {
            for handle in handles.drain(..) {
                let _ = handle.join();
            }
        }
    }

    #[cfg(test)]
    pub fn take_latest_runtime_event_for_test(&self) -> Option<FlowRuntimeSnapshot> {
        self.latest_runtime_event
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
    }

    #[cfg(test)]
    pub fn fail_runtime_log_emit_for_test(&self, error: &str) {
        if let Ok(mut slot) = self.runtime_log_emit_error.lock() {
            *slot = Some(error.to_string());
        }
    }

    #[cfg(test)]
    pub fn log_runtime_event_for_test(
        &self,
        flow_id: i64,
        flow_run_id: Option<i64>,
        stage: &str,
        event: &str,
        message: &str,
    ) {
        self.log_runtime_event(
            flow_id,
            flow_run_id,
            None,
            stage,
            event,
            FlowRuntimeLogLevel::Info,
            None,
            message,
            serde_json::json!({}),
        )
        .expect("append runtime log for test");
    }

    #[cfg(test)]
    pub fn list_runtime_logs_for_test(
        &self,
        flow_id: Option<i64>,
        limit: Option<usize>,
    ) -> Vec<FlowRuntimeLogEntry> {
        self.list_runtime_logs(flow_id, limit)
            .expect("list runtime logs for test")
    }

    #[cfg(test)]
    pub fn with_stubbed_live_status_for_test(
        &self,
        statuses: Vec<Result<Option<LiveStatus>, String>>,
    ) {
        if let Ok(mut queue) = self.stubbed_live_statuses.lock() {
            *queue = statuses.into_iter().collect();
        }
    }

    #[cfg(test)]
    pub fn run_one_poll_iteration_for_test(
        &self,
        conn: &Connection,
        flow_id: i64,
    ) -> Result<(), String> {
        self.poll_flow_once(conn, flow_id)
    }

    #[cfg(test)]
    pub fn with_before_poll_apply_hook_for_test<F>(&self, hook: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        if let Ok(mut slot) = self.before_poll_apply_hook.lock() {
            *slot = Some(Arc::new(hook));
        }
    }

    pub fn stop_flow_session(
        &self,
        flow_id: i64,
    ) -> Result<Vec<LiveRuntimeSessionSnapshot>, String> {
        if let Some(db_path) = &self.runtime_db_path {
            let mut conn = open_runtime_connection(db_path)?;
            let handle = self
                .state
                .lock()
                .map_err(|e| e.to_string())?
                .active_recordings_by_flow
                .get(&flow_id)
                .cloned();
            if let Some(handle) = handle {
                handle.cancelled.store(true, Ordering::SeqCst);
                if let Some(cancel_process) = &handle.cancel_process {
                    cancel_process()?;
                }
                self.finalize_recording_by_key(
                    &mut conn,
                    &handle.external_recording_id,
                    None,
                    false,
                    Some("Recording cancelled"),
                )?;
            }
        }
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        let mut stopped = Vec::new();
        if let Some(mut session) = state.sessions_by_flow.remove(&flow_id) {
            Self::cancel_and_remove_poll_task(&mut state, flow_id);
            state.lease_owner_by_lookup_key.remove(session.lookup_key());
            state.active_recordings_by_flow.remove(&flow_id);
            session.teardown();
            stopped.push(session.snapshot());
        }
        state.failed_snapshots_by_flow.remove(&flow_id);
        Ok(stopped)
    }

    pub fn shutdown(&self) -> Result<Vec<LiveRuntimeSessionSnapshot>, String> {
        if let Some(db_path) = &self.runtime_db_path {
            let recording_handles: Vec<ActiveRecordingHandle> = self
                .state
                .lock()
                .map_err(|e| e.to_string())?
                .active_recordings_by_flow
                .values()
                .cloned()
                .collect();
            let mut conn = open_runtime_connection(db_path)?;
            for handle in recording_handles {
                handle.cancelled.store(true, Ordering::SeqCst);
                if let Some(cancel_process) = &handle.cancel_process {
                    cancel_process()?;
                }
                self.finalize_recording_by_key(
                    &mut conn,
                    &handle.external_recording_id,
                    None,
                    false,
                    Some("Recording cancelled"),
                )?;
            }
        }
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        let flow_ids_to_cancel: Vec<i64> =
            state.active_poll_tasks_by_flow.keys().copied().collect();
        for flow_id in flow_ids_to_cancel {
            Self::cancel_and_remove_poll_task(&mut state, flow_id);
        }
        let mut stopped = Vec::new();
        for (_, mut session) in state.sessions_by_flow.drain() {
            session.teardown();
            stopped.push(session.snapshot());
        }
        stopped.sort_by_key(|snapshot| snapshot.flow_id);
        state.lease_owner_by_lookup_key.clear();
        state.active_poll_tasks_by_flow.clear();
        state.active_recordings_by_flow.clear();
        state.failed_snapshots_by_flow.clear();
        Ok(stopped)
    }

    pub fn list_sessions(&self) -> Vec<LiveRuntimeSessionSnapshot> {
        match self.state.lock() {
            Ok(state) => {
                let mut sessions: Vec<_> = state
                    .sessions_by_flow
                    .values()
                    .map(LiveRuntimeSession::snapshot)
                    .collect();
                sessions.extend(state.failed_snapshots_by_flow.values().cloned());
                sessions.sort_by_key(|snapshot| snapshot.flow_id);
                sessions
            }
            Err(_) => Vec::new(),
        }
    }
}
