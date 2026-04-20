use crate::commands::flows::UpdateFlowRuntimeByAccountInput;
use crate::commands::live_runtime::FlowRuntimeSnapshot;
use crate::commands::recordings::{finalize_rust_recording_row, start_rust_recording_row};
use crate::live_runtime::account_binding::{
    resolve_or_create_account_for_username, ResolveAccountResult,
};
use crate::live_runtime::logs::{
    format_terminal_entry, FlowRuntimeLogBuffer, FlowRuntimeLogEntry, FlowRuntimeLogLevel,
};
use crate::live_runtime::session::LiveRuntimeSession;
use crate::live_runtime::types::LiveRuntimeSessionSnapshot;
use crate::recording_runtime::types::{
    RecordingFinishInput, RecordingOutcome, RecordingStartInput,
};
#[cfg(test)]
use crate::recording_runtime::worker::RecordingProcessHandle;
use crate::recording_runtime::worker::{self, RecordingProcessRunner, RecordingRunner};
use crate::tiktok::types::LiveStatus;
use crate::time_hcm::now_timestamp_hcm;
use crate::workflow::runtime_store;
use crate::workflow::start_node;
use log::warn;
use rusqlite::{Connection, OptionalExtension};
use std::collections::HashMap;
#[cfg(test)]
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{AppHandle, Emitter};

#[derive(Default)]
struct LiveRuntimeState {
    sessions_by_flow: HashMap<i64, LiveRuntimeSession>,
    lease_owner_by_lookup_key: HashMap<String, i64>,
    failed_snapshots_by_flow: HashMap<i64, LiveRuntimeSessionSnapshot>,
    active_poll_tasks_by_flow: HashMap<i64, ActivePollTaskHandle>,
    active_recordings_by_flow: HashMap<i64, ActiveRecordingHandle>,
    #[cfg(test)]
    cancelled_poll_generations_by_flow: HashMap<i64, Vec<u64>>,
    log_buffer: FlowRuntimeLogBuffer,
}

#[derive(Clone)]
struct ActiveRecordingHandle {
    external_recording_id: String,
    cancelled: Arc<AtomicBool>,
    cancel_process: Option<Arc<dyn Fn() -> Result<(), String> + Send + Sync>>,
}

#[derive(Clone)]
struct ActivePollTaskHandle {
    generation: u64,
    cancelled: Arc<AtomicBool>,
}

#[derive(Clone)]
struct PollIterationToken {
    generation: u64,
    cancelled: Arc<AtomicBool>,
}

#[cfg(test)]
type StubbedLiveStatuses = Arc<Mutex<VecDeque<Result<Option<LiveStatus>, String>>>>;
#[cfg(test)]
type BeforePollApplyHook = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;

pub struct LiveRuntimeManager {
    state: Arc<Mutex<LiveRuntimeState>>,
    recording_runner: RecordingRunner,
    recording_process_runner: RecordingProcessRunner,
    runtime_db_path: Option<PathBuf>,
    auto_spawn_recording_execution: bool,
    app_handle: Option<AppHandle>,
    #[cfg(test)]
    runtime_log_emit_error: Arc<Mutex<Option<String>>>,
    #[cfg(test)]
    latest_runtime_event: Arc<Mutex<Option<FlowRuntimeSnapshot>>>,
    #[cfg(test)]
    worker_threads: Arc<Mutex<Vec<std::thread::JoinHandle<()>>>>,
    #[cfg(test)]
    stubbed_live_statuses: StubbedLiveStatuses,
    #[cfg(test)]
    before_poll_apply_hook: BeforePollApplyHook,
}

impl Clone for LiveRuntimeManager {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            recording_runner: self.recording_runner.clone(),
            recording_process_runner: self.recording_process_runner.clone(),
            runtime_db_path: self.runtime_db_path.clone(),
            auto_spawn_recording_execution: self.auto_spawn_recording_execution,
            app_handle: self.app_handle.clone(),
            #[cfg(test)]
            runtime_log_emit_error: Arc::clone(&self.runtime_log_emit_error),
            #[cfg(test)]
            latest_runtime_event: Arc::clone(&self.latest_runtime_event),
            #[cfg(test)]
            worker_threads: Arc::clone(&self.worker_threads),
            #[cfg(test)]
            stubbed_live_statuses: Arc::clone(&self.stubbed_live_statuses),
            #[cfg(test)]
            before_poll_apply_hook: Arc::clone(&self.before_poll_apply_hook),
        }
    }
}

#[derive(Debug, Clone)]
struct FlowRuntimeConfig {
    flow_id: i64,
    flow_name: String,
    enabled: bool,
    #[cfg_attr(not(test), allow(dead_code))]
    definition_version: i64,
    username: String,
    lookup_key: String,
    cookies_json: String,
    proxy_url: Option<String>,
    poll_interval_seconds: i64,
}

impl LiveRuntimeManager {
    fn cancel_and_remove_poll_task(state: &mut LiveRuntimeState, flow_id: i64) {
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

    fn replace_poll_task_for_session(
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

    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(LiveRuntimeState::default())),
            recording_runner: RecordingRunner::ffmpeg(),
            recording_process_runner: RecordingProcessRunner::ffmpeg(),
            runtime_db_path: None,
            auto_spawn_recording_execution: !cfg!(test),
            app_handle: None,
            #[cfg(test)]
            runtime_log_emit_error: Arc::new(Mutex::new(None)),
            #[cfg(test)]
            latest_runtime_event: Arc::new(Mutex::new(None)),
            #[cfg(test)]
            worker_threads: Arc::new(Mutex::new(Vec::new())),
            #[cfg(test)]
            stubbed_live_statuses: Arc::new(Mutex::new(VecDeque::new())),
            #[cfg(test)]
            before_poll_apply_hook: Arc::new(Mutex::new(None)),
        }
    }

    #[allow(dead_code)]
    pub fn with_runtime_db_path(db_path: PathBuf) -> Self {
        Self {
            state: Arc::new(Mutex::new(LiveRuntimeState::default())),
            recording_runner: RecordingRunner::ffmpeg(),
            recording_process_runner: RecordingProcessRunner::ffmpeg(),
            runtime_db_path: Some(db_path),
            auto_spawn_recording_execution: !cfg!(test),
            app_handle: None,
            #[cfg(test)]
            runtime_log_emit_error: Arc::new(Mutex::new(None)),
            #[cfg(test)]
            latest_runtime_event: Arc::new(Mutex::new(None)),
            #[cfg(test)]
            worker_threads: Arc::new(Mutex::new(Vec::new())),
            #[cfg(test)]
            stubbed_live_statuses: Arc::new(Mutex::new(VecDeque::new())),
            #[cfg(test)]
            before_poll_apply_hook: Arc::new(Mutex::new(None)),
        }
    }

    #[cfg(test)]
    pub fn with_recording_runner_for_test(
        db_path: PathBuf,
        recording_runner: RecordingRunner,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(LiveRuntimeState::default())),
            recording_runner: recording_runner.clone(),
            recording_process_runner: RecordingProcessRunner::from_fn(
                move |input: &RecordingStartInput, output_path: &str| {
                    let runner = recording_runner.clone();
                    let output_path = output_path.to_string();
                    let input = input.clone();
                    Ok(RecordingProcessHandle::from_parts(
                        Box::new(move || runner.run(&input, &output_path)),
                        Arc::new(|| Ok(())),
                    ))
                },
            ),
            runtime_db_path: Some(db_path),
            auto_spawn_recording_execution: false,
            app_handle: None,
            runtime_log_emit_error: Arc::new(Mutex::new(None)),
            latest_runtime_event: Arc::new(Mutex::new(None)),
            #[cfg(test)]
            worker_threads: Arc::new(Mutex::new(Vec::new())),
            #[cfg(test)]
            stubbed_live_statuses: Arc::new(Mutex::new(VecDeque::new())),
            #[cfg(test)]
            before_poll_apply_hook: Arc::new(Mutex::new(None)),
        }
    }

    #[cfg(test)]
    pub fn with_recording_runner_autospawn_for_test(
        db_path: PathBuf,
        recording_runner: RecordingRunner,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(LiveRuntimeState::default())),
            recording_runner: recording_runner.clone(),
            recording_process_runner: RecordingProcessRunner::from_fn(
                move |input: &RecordingStartInput, output_path: &str| {
                    let runner = recording_runner.clone();
                    let output_path = output_path.to_string();
                    let input = input.clone();
                    Ok(RecordingProcessHandle::from_parts(
                        Box::new(move || runner.run(&input, &output_path)),
                        Arc::new(|| Ok(())),
                    ))
                },
            ),
            runtime_db_path: Some(db_path),
            auto_spawn_recording_execution: true,
            app_handle: None,
            runtime_log_emit_error: Arc::new(Mutex::new(None)),
            latest_runtime_event: Arc::new(Mutex::new(None)),
            worker_threads: Arc::new(Mutex::new(Vec::new())),
            stubbed_live_statuses: Arc::new(Mutex::new(VecDeque::new())),
            before_poll_apply_hook: Arc::new(Mutex::new(None)),
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn with_recording_process_runner_for_test(
        db_path: PathBuf,
        recording_process_runner: RecordingProcessRunner,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(LiveRuntimeState::default())),
            recording_runner: RecordingRunner::from_fn(|_, _| {
                Err("recording_runner not used in process-runner test".to_string())
            }),
            recording_process_runner,
            runtime_db_path: Some(db_path),
            auto_spawn_recording_execution: true,
            app_handle: None,
            runtime_log_emit_error: Arc::new(Mutex::new(None)),
            latest_runtime_event: Arc::new(Mutex::new(None)),
            worker_threads: Arc::new(Mutex::new(Vec::new())),
            stubbed_live_statuses: Arc::new(Mutex::new(VecDeque::new())),
            before_poll_apply_hook: Arc::new(Mutex::new(None)),
        }
    }

    pub fn attach_app_handle(&mut self, app_handle: AppHandle) {
        self.app_handle = Some(app_handle);
    }

    fn runtime_snapshot_for_event(
        &self,
        conn: &Connection,
        flow_id: i64,
    ) -> Result<Option<FlowRuntimeSnapshot>, String> {
        crate::commands::live_runtime::list_live_runtime_snapshots_with_conn(conn, self)
            .map(|rows| rows.into_iter().find(|row| row.flow_id == flow_id))
    }

    fn emit_runtime_update_for_flow(&self, conn: &Connection, flow_id: i64) -> Result<(), String> {
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

    fn load_sidecar_base_url(conn: &Connection) -> Result<Option<String>, String> {
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

    fn resolve_live_status_for_poll(
        &self,
        config: &FlowRuntimeConfig,
    ) -> Result<Option<LiveStatus>, String> {
        #[cfg(test)]
        {
            if let Some(next) = self
                .stubbed_live_statuses
                .lock()
                .map_err(|e| e.to_string())?
                .pop_front()
            {
                return next;
            }
        }

        crate::tiktok::check_live::resolve_live_status(&crate::tiktok::types::LiveCheckConfig {
            username: config.username.as_str(),
            cookies_json: config.cookies_json.as_str(),
            proxy_url: config.proxy_url.as_deref(),
        })
    }

    fn poll_iteration_token(&self, flow_id: i64) -> Result<Option<PollIterationToken>, String> {
        let state = self.state.lock().map_err(|e| e.to_string())?;
        let Some(task) = state.active_poll_tasks_by_flow.get(&flow_id) else {
            return Ok(None);
        };
        Ok(Some(PollIterationToken {
            generation: task.generation,
            cancelled: Arc::clone(&task.cancelled),
        }))
    }

    fn is_poll_iteration_stale(
        &self,
        flow_id: i64,
        token: &PollIterationToken,
    ) -> Result<bool, String> {
        if token.cancelled.load(Ordering::SeqCst) {
            return Ok(true);
        }

        let state = self.state.lock().map_err(|e| e.to_string())?;
        let Some(task) = state.active_poll_tasks_by_flow.get(&flow_id) else {
            return Ok(true);
        };
        if task.generation != token.generation || task.cancelled.load(Ordering::SeqCst) {
            return Ok(true);
        }

        let Some(session) = state.sessions_by_flow.get(&flow_id) else {
            return Ok(true);
        };
        Ok(!session.is_polling())
    }

    pub fn poll_flow_once(&self, conn: &Connection, flow_id: i64) -> Result<(), String> {
        let config = load_flow_runtime_config(conn, flow_id)?;
        if !config.enabled {
            return Ok(());
        }

        let Some(token) = self.poll_iteration_token(flow_id)? else {
            return Ok(());
        };
        if self.is_poll_iteration_stale(flow_id, &token)? {
            return Ok(());
        }

        let live_status = self.resolve_live_status_for_poll(&config)?;
        #[cfg(test)]
        {
            if let Some(hook) = self
                .before_poll_apply_hook
                .lock()
                .map_err(|e| e.to_string())?
                .clone()
            {
                hook();
            }
        }
        if self.is_poll_iteration_stale(flow_id, &token)? {
            return Ok(());
        }

        if let Some(status) = live_status {
            let _ = self.handle_live_detected(conn, flow_id, &status)?;
        } else {
            self.mark_source_offline(flow_id)?;
        }

        Ok(())
    }

    fn spawn_poll_loop_worker(
        &self,
        flow_id: i64,
        poll_interval_seconds: i64,
        handle: ActivePollTaskHandle,
    ) {
        #[cfg(test)]
        {
            let _ = (flow_id, poll_interval_seconds, handle);
        }

        #[cfg(not(test))]
        {
            let Some(db_path) = self.runtime_db_path.clone() else {
                return;
            };
            let runtime_manager = self.clone();
            let poll_interval = std::time::Duration::from_secs(poll_interval_seconds.max(1) as u64);
            let token = PollIterationToken {
                generation: handle.generation,
                cancelled: Arc::clone(&handle.cancelled),
            };

            std::thread::spawn(move || loop {
                let is_stale = runtime_manager
                    .is_poll_iteration_stale(flow_id, &token)
                    .unwrap_or(true);
                if is_stale {
                    break;
                }

                let conn = match open_runtime_connection(&db_path) {
                    Ok(conn) => conn,
                    Err(err) => {
                        log::warn!(
                            "flow_runtime poll worker failed opening DB for flow {}: {}",
                            flow_id,
                            err
                        );
                        break;
                    }
                };
                if let Err(err) = runtime_manager.poll_flow_once(&conn, flow_id) {
                    log::warn!(
                        "flow_runtime poll worker iteration failed for flow {}: {}",
                        flow_id,
                        err
                    );
                }

                let is_stale = runtime_manager
                    .is_poll_iteration_stale(flow_id, &token)
                    .unwrap_or(true);
                if is_stale {
                    break;
                }
                std::thread::sleep(poll_interval);
            });
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "Structured handoff logging keeps runtime identifiers explicit at the sidecar boundary"
    )]
    fn handoff_recording_to_sidecar_processing(
        &self,
        conn: &Connection,
        flow_id: i64,
        flow_run_id: i64,
        external_recording_id: &str,
        account_id: i64,
        username: &str,
        file_path: &str,
    ) -> Result<(), String> {
        let Some(sidecar_base_url) = Self::load_sidecar_base_url(conn)? else {
            let _ = self.log_runtime_event(
                flow_id,
                Some(flow_run_id),
                Some(external_recording_id),
                "clip",
                "sidecar_handoff_failed",
                FlowRuntimeLogLevel::Warn,
                Some("handoff.sidecar_unavailable"),
                "Skipped sidecar handoff because sidecar base URL is unavailable",
                serde_json::json!({
                    "sidecar_url_present": false,
                    "file_path_present": true,
                }),
            );
            return Ok(());
        };
        let body = serde_json::json!({
            "recording_id": external_recording_id,
            "username": username,
            "file_path": file_path,
            "account_id": account_id,
        });
        let handoff_result = tokio::runtime::Runtime::new()
            .map_err(|e| e.to_string())?
            .block_on(async {
                reqwest::Client::new()
                    .post(format!("{sidecar_base_url}/api/video/process"))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?
                    .error_for_status()
                    .map_err(|e| e.to_string())?;
                Ok::<(), String>(())
            });
        if let Err(err) = handoff_result {
            let _ = self.log_runtime_event(
                flow_id,
                Some(flow_run_id),
                Some(external_recording_id),
                "clip",
                "sidecar_handoff_failed",
                FlowRuntimeLogLevel::Error,
                Some("handoff.http_failed"),
                "Failed to hand off recording to sidecar processing",
                serde_json::json!({
                    "sidecar_url_present": true,
                    "file_path_present": true,
                    "error": err,
                }),
            );
            return Err(err);
        }
        let _ = self.log_runtime_event(
            flow_id,
            Some(flow_run_id),
            Some(external_recording_id),
            "clip",
            "sidecar_handoff_completed",
            FlowRuntimeLogLevel::Info,
            None,
            "Handed recording off to sidecar processing",
            serde_json::json!({
                "sidecar_url_present": true,
                "file_path_present": true,
            }),
        );
        Ok(())
    }

    pub fn bootstrap_enabled_flows(&self, conn: &Connection) -> Result<(), String> {
        let mut stmt = conn
            .prepare("SELECT id FROM flows WHERE enabled = 1 ORDER BY id ASC")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(|e| e.to_string())?;

        for row in rows {
            let flow_id = row.map_err(|e| e.to_string())?;
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

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn handle_live_detected(
        &self,
        conn: &Connection,
        flow_id: i64,
        live_status: &LiveStatus,
    ) -> Result<Option<i64>, String> {
        let config = load_flow_runtime_config(conn, flow_id)?;
        let session_state = {
            let state = self.state.lock().map_err(|e| e.to_string())?;
            state
                .sessions_by_flow
                .get(&flow_id)
                .map(|session| {
                    (
                        session.active_flow_run_id(),
                        session.last_completed_room_id().map(str::to_string),
                    )
                })
                .ok_or_else(|| format!("missing live runtime session for flow {flow_id}"))?
        };
        let session_active = session_state.0;
        let last_completed_room_id = session_state.1;

        if session_active.is_some() {
            return Ok(None);
        }
        if live_status.room_id.trim().is_empty() {
            return Ok(None);
        }
        let _ = self.log_runtime_event(
            flow_id,
            None,
            None,
            "start",
            "live_detected",
            FlowRuntimeLogLevel::Info,
            None,
            "Detected live room while polling runtime session",
            serde_json::json!({
                "room_id": live_status.room_id.as_str(),
                "viewer_count": live_status.viewer_count,
                "has_stream_url": live_status.stream_url.is_some(),
            }),
        );
        if !crate::live_runtime::session::should_start_new_run(
            Some(live_status.room_id.as_str()),
            last_completed_room_id.as_deref(),
            true,
        ) {
            let _ = self.log_runtime_event(
                flow_id,
                None,
                None,
                "start",
                "run_creation_skipped_dedupe",
                FlowRuntimeLogLevel::Info,
                None,
                "Skipped run creation because room was already completed",
                serde_json::json!({
                    "room_id": live_status.room_id.as_str(),
                    "last_completed_room_id": last_completed_room_id,
                }),
            );
            return Ok(None);
        }
        let Some(stream_url) = live_status.stream_url.as_deref() else {
            let _ = self.log_runtime_event(
                flow_id,
                None,
                None,
                "start",
                "stream_url_missing",
                FlowRuntimeLogLevel::Warn,
                Some("start.stream_url_missing"),
                "Skipped run creation because live stream URL is missing",
                serde_json::json!({
                    "room_id": live_status.room_id.as_str(),
                    "viewer_count": live_status.viewer_count,
                }),
            );
            return Ok(None);
        };

        let account_id = resolve_account_id(resolve_or_create_account_for_username(
            conn,
            config.username.as_str(),
        )?);
        let output_json = serde_json::to_string(&start_node::StartOutput {
            account_id,
            username: config.username,
            room_id: live_status.room_id.clone(),
            stream_url: stream_url.to_string(),
            viewer_count: live_status.viewer_count,
            detected_at: now_timestamp_hcm(),
        })
        .map_err(|e| e.to_string())?;
        let flow_run_id = runtime_store::create_run_with_completed_start_node(
            conn,
            flow_id,
            config.definition_version,
            output_json.as_str(),
        )?;
        runtime_store::upsert_running_record_node_run(conn, flow_run_id, flow_id)?;
        let start_input = RecordingStartInput {
            account_id,
            flow_id,
            flow_run_id,
            room_id: live_status.room_id.clone(),
            stream_url: stream_url.to_string(),
            max_duration_seconds: load_record_duration_seconds(conn, flow_id)?,
            external_recording_id: worker::generate_external_recording_id(flow_run_id),
        };
        let _ = self.log_runtime_event(
            flow_id,
            Some(flow_run_id),
            Some(start_input.external_recording_id.as_str()),
            "start",
            "run_created",
            FlowRuntimeLogLevel::Info,
            None,
            "Created runtime flow run for live room",
            serde_json::json!({
                "room_id": live_status.room_id.as_str(),
                "has_stream_url": true,
            }),
        );
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        let cancel_flag = Arc::new(AtomicBool::new(false));
        state.active_recordings_by_flow.insert(
            flow_id,
            ActiveRecordingHandle {
                external_recording_id: start_input.external_recording_id.clone(),
                cancelled: Arc::clone(&cancel_flag),
                cancel_process: None,
            },
        );
        start_rust_recording_row(conn, &start_input)?;
        drop(state);
        if self.auto_spawn_recording_execution {
            self.spawn_recording_execution(start_input.clone(), cancel_flag)?;
        }
        update_flow_runtime_by_flow_id(
            conn,
            flow_id,
            &UpdateFlowRuntimeByAccountInput {
                status: Some("recording".to_string()),
                current_node: Some("record".to_string()),
                last_live_at: None,
                last_run_at: Some(now_timestamp_hcm()),
                last_error: Some(String::new()),
            },
        )?;
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        let session = state
            .sessions_by_flow
            .get_mut(&flow_id)
            .ok_or_else(|| format!("missing live runtime session for flow {flow_id}"))?;
        session.mark_flow_run_started(flow_run_id);
        drop(state);
        self.emit_runtime_update_for_flow(conn, flow_id)?;
        Ok(Some(flow_run_id))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn complete_active_run(
        &self,
        conn: &mut Connection,
        flow_id: i64,
        room_id: Option<&str>,
    ) -> Result<(), String> {
        let flow_run_id = runtime_store::load_latest_running_flow_run_id(conn, flow_id)?
            .ok_or_else(|| format!("missing running flow_run for flow {flow_id}"))?;
        let room_id = match room_id {
            Some(room_id) => Some(room_id.to_string()),
            None => runtime_store::load_last_room_id_from_latest_start_node_run(conn, flow_id)?,
        };
        finalize_latest_recording_row(conn, flow_id, room_id.as_deref(), true, None)?;
        runtime_store::finalize_record_node_run(conn, flow_id, true, None)?;
        update_flow_runtime_by_flow_id(
            conn,
            flow_id,
            &UpdateFlowRuntimeByAccountInput {
                status: Some("processing".to_string()),
                current_node: Some("clip".to_string()),
                last_live_at: None,
                last_run_at: Some(now_timestamp_hcm()),
                last_error: Some(String::new()),
            },
        )?;
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.active_recordings_by_flow.remove(&flow_id);
        if let Some(session) = state.sessions_by_flow.get_mut(&flow_id) {
            session.mark_downstream_stage(flow_run_id, "processing");
        }
        drop(state);
        let _ = self.log_runtime_event(
            flow_id,
            Some(flow_run_id),
            None,
            "record",
            "record_completed",
            FlowRuntimeLogLevel::Info,
            None,
            "Recording completed successfully",
            serde_json::json!({
                "room_id": room_id,
            }),
        );
        self.emit_runtime_update_for_flow(conn, flow_id)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn fail_active_run(
        &self,
        conn: &mut Connection,
        flow_id: i64,
        room_id: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), String> {
        let flow_run_id = runtime_store::load_latest_running_flow_run_id(conn, flow_id)?;
        let room_id = match room_id {
            Some(room_id) => Some(room_id.to_string()),
            None => runtime_store::load_last_room_id_from_latest_start_node_run(conn, flow_id)?,
        };
        finalize_latest_recording_row(conn, flow_id, room_id.as_deref(), false, error_message)?;
        runtime_store::finalize_latest_running_flow_run(conn, flow_id, false, error_message)?;
        update_flow_runtime_by_flow_id(
            conn,
            flow_id,
            &UpdateFlowRuntimeByAccountInput {
                status: Some("error".to_string()),
                current_node: Some("record".to_string()),
                last_live_at: None,
                last_run_at: Some(now_timestamp_hcm()),
                last_error: Some(error_message.unwrap_or("Recording failed").to_string()),
            },
        )?;

        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.active_recordings_by_flow.remove(&flow_id);
        if let Some(session) = state.sessions_by_flow.get_mut(&flow_id) {
            session.mark_flow_run_completed(room_id.as_deref());
        }
        drop(state);
        let _ = self.log_runtime_event(
            flow_id,
            flow_run_id,
            None,
            "record",
            "record_failed",
            FlowRuntimeLogLevel::Error,
            None,
            "Recording failed and active run was finalized",
            serde_json::json!({
                "room_id": room_id,
                "error": error_message,
            }),
        );
        self.emit_runtime_update_for_flow(conn, flow_id)?;
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn finalize_recording_by_key(
        &self,
        conn: &mut Connection,
        external_recording_id: &str,
        room_id: Option<&str>,
        success: bool,
        error_message: Option<&str>,
    ) -> Result<(), String> {
        type RecordingFinalizeRow = (i64, i64, i64, Option<String>, Option<String>);
        let row: Option<RecordingFinalizeRow> = conn
            .query_row(
                "SELECT account_id, flow_id, flow_run_id, room_id, file_path FROM recordings WHERE sidecar_recording_id = ?1",
                [external_recording_id],
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
            .optional()
            .map_err(|e| e.to_string())?;
        let Some((account_id, flow_id, flow_run_id, existing_room_id, file_path)) = row else {
            return Ok(());
        };

        finalize_rust_recording_row(
            conn,
            &RecordingFinishInput {
                account_id,
                flow_id,
                flow_run_id,
                external_recording_id: external_recording_id.to_string(),
                room_id: room_id
                    .map(str::to_string)
                    .or(existing_room_id)
                    .unwrap_or_default(),
                file_path: file_path.clone(),
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

        if success {
            runtime_store::finalize_record_node_run_by_flow_run_id(conn, flow_run_id, true, None)?;
            update_flow_runtime_by_flow_id(
                conn,
                flow_id,
                &UpdateFlowRuntimeByAccountInput {
                    status: Some("processing".to_string()),
                    current_node: Some("clip".to_string()),
                    last_live_at: None,
                    last_run_at: Some(now_timestamp_hcm()),
                    last_error: Some(String::new()),
                },
            )?;
            let mut state = self.state.lock().map_err(|e| e.to_string())?;
            state.active_recordings_by_flow.remove(&flow_id);
            if let Some(session) = state.sessions_by_flow.get_mut(&flow_id) {
                session.mark_downstream_stage(flow_run_id, "processing");
            }
            drop(state);
            let _ = self.log_runtime_event(
                flow_id,
                Some(flow_run_id),
                Some(external_recording_id),
                "record",
                "record_completed",
                FlowRuntimeLogLevel::Info,
                None,
                "Recording finalized successfully",
                serde_json::json!({
                    "room_id": room_id,
                }),
            );
            if let Some(file_path) = file_path.as_deref() {
                if let Err(err) = self.handoff_recording_to_sidecar_processing(
                    conn,
                    flow_id,
                    flow_run_id,
                    external_recording_id,
                    account_id,
                    &load_flow_runtime_config(conn, flow_id)?.username,
                    file_path,
                ) {
                    runtime_store::append_failed_pipeline_node_run(
                        conn,
                        flow_run_id,
                        flow_id,
                        "clip",
                        err.as_str(),
                    )?;
                    update_flow_runtime_by_flow_id(
                        conn,
                        flow_id,
                        &UpdateFlowRuntimeByAccountInput {
                            status: Some("error".to_string()),
                            current_node: Some("clip".to_string()),
                            last_live_at: None,
                            last_run_at: Some(now_timestamp_hcm()),
                            last_error: Some(err.clone()),
                        },
                    )?;
                    let mut state = self.state.lock().map_err(|e| e.to_string())?;
                    if let Some(session) = state.sessions_by_flow.get_mut(&flow_id) {
                        session.mark_downstream_stage(flow_run_id, "error");
                    }
                    drop(state);
                    self.emit_runtime_update_for_flow(conn, flow_id)?;
                    return Ok(());
                }
            }
            self.emit_runtime_update_for_flow(conn, flow_id)?;
        } else {
            let is_cancel = matches!(
                error_message.map(str::trim),
                Some("Recording cancelled") | Some("Cancelled")
            );
            if is_cancel {
                runtime_store::cancel_flow_run_by_id(
                    conn,
                    flow_run_id,
                    Some("Recording cancelled"),
                )?;
                update_flow_runtime_by_flow_id(
                    conn,
                    flow_id,
                    &UpdateFlowRuntimeByAccountInput {
                        status: Some("disabled".to_string()),
                        current_node: Some("record".to_string()),
                        last_live_at: None,
                        last_run_at: Some(now_timestamp_hcm()),
                        last_error: Some("Recording cancelled".to_string()),
                    },
                )?;
            } else {
                runtime_store::finalize_record_node_run_by_flow_run_id(
                    conn,
                    flow_run_id,
                    false,
                    error_message,
                )?;
                runtime_store::finalize_flow_run_by_id(conn, flow_run_id, false, error_message)?;
                update_flow_runtime_by_flow_id(
                    conn,
                    flow_id,
                    &UpdateFlowRuntimeByAccountInput {
                        status: Some("error".to_string()),
                        current_node: Some("record".to_string()),
                        last_live_at: None,
                        last_run_at: Some(now_timestamp_hcm()),
                        last_error: Some(error_message.unwrap_or("Recording failed").to_string()),
                    },
                )?;
            }

            let mut state = self.state.lock().map_err(|e| e.to_string())?;
            state.active_recordings_by_flow.remove(&flow_id);
            if let Some(session) = state.sessions_by_flow.get_mut(&flow_id) {
                session.mark_flow_run_completed(room_id);
            }
            drop(state);
            let _ = self.log_runtime_event(
                flow_id,
                Some(flow_run_id),
                Some(external_recording_id),
                "record",
                if is_cancel {
                    "record_cancelled"
                } else {
                    "record_failed"
                },
                if is_cancel {
                    FlowRuntimeLogLevel::Warn
                } else {
                    FlowRuntimeLogLevel::Error
                },
                None,
                if is_cancel {
                    "Recording was cancelled"
                } else {
                    "Recording finalized with failure"
                },
                serde_json::json!({
                    "room_id": room_id,
                    "error": error_message,
                }),
            );
            self.emit_runtime_update_for_flow(conn, flow_id)?;
        }

        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn mark_source_offline(&self, flow_id: i64) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        if let Some(session) = state.sessions_by_flow.get_mut(&flow_id) {
            session.mark_source_offline();
        }
        let db_path = self.runtime_db_path.clone();
        drop(state);
        let _ = self.log_runtime_event(
            flow_id,
            None,
            None,
            "session",
            "source_offline_marked",
            FlowRuntimeLogLevel::Info,
            None,
            "Marked source offline and reset dedupe state",
            serde_json::json!({
                "flow_id": flow_id,
                "dedupe_reset": true,
            }),
        );
        if let Some(db_path) = db_path {
            let conn = open_runtime_connection(&db_path)?;
            self.emit_runtime_update_for_flow(&conn, flow_id)?;
        }
        Ok(())
    }

    pub fn restart_active_run(&self, conn: &mut Connection, flow_id: i64) -> Result<i64, String> {
        {
            let state = self.state.lock().map_err(|e| e.to_string())?;
            if !state.sessions_by_flow.contains_key(&flow_id) {
                return Err(format!("missing live runtime session for flow {flow_id}"));
            }
        }

        let new_run_id =
            runtime_store::restart_running_flow_runs_for_flow(conn, flow_id, "publish_restart")?;

        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        let session = state
            .sessions_by_flow
            .get_mut(&flow_id)
            .ok_or_else(|| format!("missing live runtime session for flow {flow_id}"))?;
        session.mark_flow_run_started(new_run_id);
        Ok(new_run_id)
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

fn load_flow_runtime_config(conn: &Connection, flow_id: i64) -> Result<FlowRuntimeConfig, String> {
    let (loaded_flow_id, flow_name, enabled, definition_version, published_config_json): (
        i64,
        String,
        i64,
        i64,
        String,
    ) = conn
        .query_row(
            "SELECT f.id, f.name, f.enabled, f.published_version, n.published_config_json \
             FROM flows f \
             JOIN flow_nodes n ON n.flow_id = f.id AND n.node_key = 'start' \
             WHERE f.id = ?1",
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

fn load_record_duration_seconds(conn: &Connection, flow_id: i64) -> Result<i64, String> {
    let raw: String = conn
        .query_row(
            "SELECT published_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'record'",
            [flow_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(crate::workflow::record_node::parse_record_config(&raw)?.max_duration_seconds())
}

fn update_flow_runtime_by_flow_id(
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

fn finalize_latest_recording_row(
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

fn open_runtime_connection(db_path: &std::path::Path) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| e.to_string())?;
    Ok(conn)
}

impl LiveRuntimeManager {
    fn spawn_recording_execution(
        &self,
        start_input: RecordingStartInput,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let Some(db_path) = self.runtime_db_path.clone() else {
            return Ok(());
        };
        let runner = self.recording_process_runner.clone();
        let runtime_manager = self.clone();
        let execution = worker::build_recording_execution(&start_input);

        let _ = self.log_runtime_event(
            start_input.flow_id,
            Some(start_input.flow_run_id),
            Some(start_input.external_recording_id.as_str()),
            "record",
            "record_spawned",
            FlowRuntimeLogLevel::Info,
            None,
            "Spawned Rust-owned recording worker",
            serde_json::json!({
                "room_id": start_input.room_id.as_str(),
            }),
        );

        let handle = std::thread::spawn(move || {
            let process = match runner.spawn(&execution.start, execution.output_path.as_str()) {
                Ok(process) => process,
                Err(err) => {
                    let Ok(mut conn) = open_runtime_connection(&db_path) else {
                        return;
                    };
                    let _ = runtime_manager.finalize_recording_by_key(
                        &mut conn,
                        execution.start.external_recording_id.as_str(),
                        Some(execution.start.room_id.as_str()),
                        false,
                        Some(err.as_str()),
                    );
                    return;
                }
            };
            let cancel_handle = process.cancel_handle();
            let mut should_cancel_after_registration = cancel_flag.load(Ordering::SeqCst);
            if let Ok(mut state) = runtime_manager.state.lock() {
                if let Some(active) = state
                    .active_recordings_by_flow
                    .get_mut(&execution.start.flow_id)
                {
                    active.cancel_process = Some(cancel_handle.clone());
                    if active.cancelled.load(Ordering::SeqCst) {
                        should_cancel_after_registration = true;
                    }
                }
            }
            if should_cancel_after_registration {
                let _ = cancel_handle();
            }

            let finish_result = process.wait();
            let Ok(mut conn) = open_runtime_connection(&db_path) else {
                return;
            };

            if cancel_flag.load(Ordering::SeqCst) {
                return;
            }

            match finish_result {
                Ok(finish) => {
                    let _ = runtime_manager.finalize_recording_by_key(
                        &mut conn,
                        finish.external_recording_id.as_str(),
                        Some(finish.room_id.as_str()),
                        matches!(finish.outcome, RecordingOutcome::Success),
                        match finish.outcome {
                            RecordingOutcome::Success => finish.error_message.as_deref(),
                            RecordingOutcome::Failed => finish.error_message.as_deref(),
                            RecordingOutcome::Cancelled => Some("Recording cancelled"),
                        },
                    );
                }
                Err(err) => {
                    let _ = runtime_manager.finalize_recording_by_key(
                        &mut conn,
                        execution.start.external_recording_id.as_str(),
                        Some(execution.start.room_id.as_str()),
                        false,
                        Some(err.as_str()),
                    );
                }
            }
        });

        #[cfg(test)]
        {
            self.worker_threads
                .lock()
                .map_err(|e| e.to_string())?
                .push(handle);
        }

        #[cfg(not(test))]
        {
            let _ = handle;
        }

        Ok(())
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn resolve_account_id(result: ResolveAccountResult) -> i64 {
    match result {
        ResolveAccountResult::Existing { account_id }
        | ResolveAccountResult::Created { account_id } => account_id,
    }
}

fn failed_snapshot(
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
    }
}

#[cfg(test)]
mod tests {
    use super::LiveRuntimeManager;
    use crate::db::init::initialize_database;
    use crate::live_runtime::logs::FlowRuntimeLogLevel;
    use crate::live_runtime::session::{
        lock_teardown_test_guard_for_test, reset_teardown_call_count_for_test,
        teardown_call_count_for_test,
    };
    use crate::recording_runtime::types::RecordingOutcome;
    use crate::tiktok::types::LiveStatus;
    use rusqlite::{params, Connection};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn open_temp_db() -> (Connection, PathBuf) {
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tikclip-live-runtime-manager-test-{}-{}-{}.db",
            std::process::id(),
            counter,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let conn = initialize_database(&path).expect("init db");
        (conn, path)
    }

    fn insert_flow(conn: &Connection, flow_id: i64, enabled: bool, username: &str) {
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'idle', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            params![flow_id, format!("Flow {flow_id}"), if enabled { 1 } else { 0 }],
        )
        .expect("insert flow");
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
             VALUES (?1, 'start', 1, ?2, ?2, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            params![flow_id, format!(r#"{{"username":"{username}"}}"#)],
        )
        .expect("insert start node");
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
             VALUES (?1, 'record', 2, '{\"max_duration_minutes\":5}', '{\"max_duration_minutes\":5}', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            params![flow_id],
        )
        .expect("insert record node");
    }

    fn sidecar_request_server() -> (String, Arc<Mutex<Vec<String>>>, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind sidecar test server");
        let addr = listener.local_addr().expect("test server addr");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let requests_for_thread = Arc::clone(&requests);
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .expect("set read timeout");
                let mut buffer = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    match stream.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            buffer.extend_from_slice(&chunk[..n]);
                            if n < chunk.len() {
                                break;
                            }
                        }
                        Err(err)
                            if err.kind() == std::io::ErrorKind::WouldBlock
                                || err.kind() == std::io::ErrorKind::TimedOut =>
                        {
                            break;
                        }
                        Err(_) => break,
                    }
                }
                requests_for_thread
                    .lock()
                    .expect("lock requests")
                    .push(String::from_utf8_lossy(&buffer).to_string());
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
                );
            }
        });
        (format!("http://{}", addr), requests, handle)
    }

    fn in_memory_runtime_schema(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL DEFAULT '',
                type TEXT NOT NULL DEFAULT 'monitored',
                created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                auto_record INTEGER NOT NULL DEFAULT 0,
                priority INTEGER NOT NULL DEFAULT 0
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
        .expect("create runtime schema");
    }

    #[test]
    fn bootstrap_enabled_flows_starts_enabled_flows_once() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        insert_flow(&conn, 2, false, "shop_xyz");
        let manager = LiveRuntimeManager::new();

        manager
            .bootstrap_enabled_flows(&conn)
            .expect("first bootstrap");
        manager
            .bootstrap_enabled_flows(&conn)
            .expect("second bootstrap");

        let sessions = manager.list_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].flow_id, 1);
        assert_eq!(sessions[0].lookup_key, "shop_abc");
        assert_eq!(sessions[0].generation, 1);
        let logs = manager.list_runtime_logs_for_test(Some(1), Some(10));
        assert!(logs.iter().any(|entry| {
            entry.event == "session_bootstrap_started"
                && entry.stage == "session"
                && entry
                    .context
                    .get("flow_id")
                    .and_then(|value| value.as_i64())
                    == Some(1)
        }));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn bootstrap_enabled_flows_restores_active_flow_run_id_from_running_db_run() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (41, 1, 1, 'running', datetime('now','+7 hours'), 'test')",
            [],
        )
        .expect("insert running flow run");
        let manager = LiveRuntimeManager::new();

        manager
            .bootstrap_enabled_flows(&conn)
            .expect("bootstrap enabled flows");
        let flow_run_id = manager.session_active_flow_run_id_for_test(1);

        assert_eq!(flow_run_id, Some(41));
        let duplicate_attempt = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live while run restored");
        assert_eq!(duplicate_attempt, None);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn bootstrap_enabled_flows_skips_bad_flow_and_starts_valid_enabled_flow() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        insert_flow(&conn, 2, true, "shop_xyz");
        conn.execute(
            "UPDATE flow_nodes SET published_config_json = ?1 WHERE flow_id = 2 AND node_key = 'start'",
            [r#"{"username":"   @   "}"#],
        )
        .expect("break flow config");
        let manager = LiveRuntimeManager::new();

        manager
            .bootstrap_enabled_flows(&conn)
            .expect("bootstrap should continue");

        let sessions = manager.list_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].flow_id, 1);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn acquire_username_lease_rejects_second_flow_with_same_lookup_key() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "Shop_ABC");
        insert_flow(&conn, 2, true, "@shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start first");
        let err = manager.start_flow_session(&conn, 2).unwrap_err();

        assert!(err.contains("username lease already held"));
        let sessions = manager.list_sessions();
        assert_eq!(sessions.len(), 2);
        let failed = sessions
            .iter()
            .find(|snapshot| snapshot.flow_id == 2)
            .expect("failed snapshot retained");
        assert_eq!(failed.status, "error");
        assert!(failed
            .last_error
            .clone()
            .unwrap_or_default()
            .contains("username lease already held"));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reconcile_flow_after_publish_restarts_session_once() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start initial");
        manager.reconcile_flow(&conn, 1).expect("reconcile");

        let sessions = manager.list_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].flow_id, 1);
        assert_eq!(sessions[0].generation, 2);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reconcile_flow_restores_active_flow_run_id_from_running_db_run() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start session");
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (51, 1, 1, 'running', datetime('now','+7 hours'), 'test')",
            [],
        )
        .expect("insert running flow run");

        manager.reconcile_flow(&conn, 1).expect("reconcile flow");

        assert_eq!(manager.session_active_flow_run_id_for_test(1), Some(51));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reconcile_flow_surfaces_failure_snapshot_when_replacement_fails() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        insert_flow(&conn, 2, true, "shop_xyz");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start first");
        manager.start_flow_session(&conn, 2).expect("start second");
        conn.execute(
            "UPDATE flow_nodes SET published_config_json = ?1 WHERE flow_id = 1 AND node_key = 'start'",
            [r#"{"username":"shop_xyz"}"#],
        )
        .expect("change username to conflicting lease");

        let err = manager.reconcile_flow(&conn, 1).unwrap_err();

        assert!(err.contains("username lease already held"));
        let sessions = manager.list_sessions();
        assert_eq!(sessions.len(), 2);
        let session = sessions
            .iter()
            .find(|snapshot| snapshot.flow_id == 1)
            .expect("failed runtime snapshot retained");
        assert_eq!(session.lookup_key, "shop_abc");
        assert_eq!(session.generation, 1);
        assert_eq!(session.status, "error");
        assert!(session
            .last_error
            .clone()
            .unwrap_or_default()
            .contains("username lease already held"));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn shutdown_clears_sessions_and_leases() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start flow");
        manager.shutdown().expect("shutdown");

        assert!(manager.list_sessions().is_empty());
        manager.start_flow_session(&conn, 1).expect("start again");
        assert_eq!(manager.list_sessions().len(), 1);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn log_runtime_event_appends_entry_to_manager_buffer() {
        let manager = LiveRuntimeManager::new();

        manager.log_runtime_event_for_test(
            7,
            Some(42),
            "record",
            "record_spawned",
            "Spawned worker",
        );

        let logs = manager.list_runtime_logs_for_test(None, None);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].flow_id, 7);
        assert_eq!(logs[0].event, "record_spawned");
    }

    #[test]
    fn log_runtime_event_keeps_buffered_entry_when_emit_fails() {
        let manager = LiveRuntimeManager::new();
        manager.fail_runtime_log_emit_for_test("simulated emit failure");

        let result = manager.log_runtime_event(
            7,
            Some(42),
            None,
            "record",
            "record_spawned",
            FlowRuntimeLogLevel::Info,
            None,
            "Spawned worker",
            serde_json::json!({}),
        );

        assert!(result.is_ok());
        let logs = manager.list_runtime_logs_for_test(Some(7), None);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].event, "record_spawned");
    }

    #[test]
    fn stop_and_shutdown_invoke_session_teardown_hook() {
        let _guard = lock_teardown_test_guard_for_test();
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        insert_flow(&conn, 2, true, "shop_xyz");
        let manager = LiveRuntimeManager::new();
        reset_teardown_call_count_for_test();

        manager.start_flow_session(&conn, 1).expect("start first");
        manager.start_flow_session(&conn, 2).expect("start second");
        let stopped = manager.stop_flow_session(1).expect("stop one session");

        assert_eq!(stopped.len(), 1);
        assert_eq!(stopped[0].status, "stopped");
        assert_eq!(teardown_call_count_for_test(), 1);
        let shutdown = manager.shutdown().expect("shutdown");
        assert_eq!(shutdown.len(), 1);
        assert_eq!(shutdown[0].status, "stopped");
        assert_eq!(teardown_call_count_for_test(), 2);
        assert!(manager.list_sessions().is_empty());

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn restored_last_completed_room_id_prevents_duplicate_run_for_same_on_air_room() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        in_memory_runtime_schema(&conn);
        insert_flow(&conn, 1, true, "shop_abc");
        conn.execute(
            "INSERT INTO accounts (id, username, display_name, type, created_at, updated_at) \
             VALUES (1, 'shop_abc', 'Shop ABC', 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert account");
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, ended_at, trigger_reason) \
             VALUES (11, 1, 1, 'completed', datetime('now','+7 hours'), datetime('now','+7 hours'), 'test')",
            [],
        )
        .expect("insert completed flow run");
        conn.execute(
            "INSERT INTO recordings (account_id, room_id, status, duration_seconds, file_size_bytes, flow_id, flow_run_id, created_at, started_at) \
             VALUES (1, '7312345', 'done', 0, 0, 1, 11, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert recording");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live");

        assert_eq!(flow_run_id, None);
        let running_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM flow_runs WHERE flow_id = 1 AND status = 'running'",
                [],
                |row| row.get(0),
            )
            .expect("count running runs");
        assert_eq!(running_count, 0);
    }

    #[test]
    fn handle_live_detected_does_not_create_run_when_stream_url_is_missing() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: None,
                    viewer_count: Some(77),
                },
            )
            .expect("handle live without stream url");

        assert_eq!(flow_run_id, None);
        assert!(manager.session_is_polling_for_test(1));
        let logs = manager.list_runtime_logs_for_test(Some(1), Some(10));
        assert!(logs.iter().any(|entry| {
            entry.event == "stream_url_missing"
                && entry.code.as_deref() == Some("start.stream_url_missing")
                && entry.stage == "start"
        }));
        let run_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM flow_runs WHERE flow_id = 1",
                [],
                |row| row.get(0),
            )
            .expect("count flow runs");
        assert_eq!(run_count, 0);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn successful_record_completion_keeps_flow_run_alive_for_downstream_stages() {
        let (mut conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::with_recording_runner_for_test(
            path.clone(),
            crate::recording_runtime::worker::RecordingRunner::from_fn(|input, output_path| {
                Ok(crate::recording_runtime::types::RecordingFinishInput {
                    account_id: input.account_id,
                    flow_id: input.flow_id,
                    flow_run_id: input.flow_run_id,
                    external_recording_id: input.external_recording_id.clone(),
                    room_id: input.room_id.clone(),
                    file_path: Some(output_path.to_string()),
                    error_message: None,
                    duration_seconds: input.max_duration_seconds,
                    file_size_bytes: 1,
                    outcome: RecordingOutcome::Success,
                })
            }),
        );

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created flow run");

        manager
            .complete_active_run(&mut conn, 1, Some("7312345"))
            .expect("complete run");

        let flow_run_status: String = conn
            .query_row(
                "SELECT status FROM flow_runs WHERE id = ?1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read flow run status");
        assert_eq!(flow_run_status, "running");
        assert!(!manager.session_is_polling_for_test(1));
        assert_eq!(
            manager.session_active_flow_run_id_for_test(1),
            Some(flow_run_id)
        );

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn successful_record_completion_without_explicit_room_id_keeps_active_run_alive() {
        let (mut conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::with_recording_runner_for_test(
            path.clone(),
            crate::recording_runtime::worker::RecordingRunner::from_fn(|input, output_path| {
                Ok(crate::recording_runtime::types::RecordingFinishInput {
                    account_id: input.account_id,
                    flow_id: input.flow_id,
                    flow_run_id: input.flow_run_id,
                    external_recording_id: input.external_recording_id.clone(),
                    room_id: input.room_id.clone(),
                    file_path: Some(output_path.to_string()),
                    error_message: None,
                    duration_seconds: input.max_duration_seconds,
                    file_size_bytes: 1,
                    outcome: RecordingOutcome::Success,
                })
            }),
        );

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created flow run");

        manager
            .complete_active_run(&mut conn, 1, None)
            .expect("complete run");

        let flow_run_status: String = conn
            .query_row(
                "SELECT status FROM flow_runs WHERE id = ?1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read flow run status");
        assert_eq!(flow_run_status, "running");
        assert_eq!(
            manager.session_active_flow_run_id_for_test(1),
            Some(flow_run_id)
        );

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn handle_live_detected_starts_rust_owned_execution_without_waiting_for_sidecar_finish() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created flow run");

        let row: (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, file_path, sidecar_recording_id FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read recording row");

        assert_eq!(row.0, "recording");
        assert!(row
            .1
            .as_deref()
            .is_some_and(|value| value.ends_with(".mp4")));
        assert!(row
            .2
            .as_deref()
            .is_some_and(|value| value.starts_with("rust-recording-")));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rust_owned_recording_worker_completes_without_sidecar_finish_signal() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::with_recording_runner_autospawn_for_test(
            path.clone(),
            crate::recording_runtime::worker::RecordingRunner::from_fn(|input, output_path| {
                Ok(crate::recording_runtime::types::RecordingFinishInput {
                    account_id: input.account_id,
                    flow_id: input.flow_id,
                    flow_run_id: input.flow_run_id,
                    external_recording_id: input.external_recording_id.clone(),
                    room_id: input.room_id.clone(),
                    file_path: Some(output_path.to_string()),
                    error_message: None,
                    duration_seconds: input.max_duration_seconds,
                    file_size_bytes: 321,
                    outcome: RecordingOutcome::Success,
                })
            }),
        );

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created flow run");

        manager.drain_worker_threads_for_test();
        let reader = crate::db::init::initialize_database(&path).expect("open reader");
        let status: String = reader
            .query_row(
                "SELECT status FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("query finalized recording status");
        let flow_run_status: String = reader
            .query_row(
                "SELECT status FROM flow_runs WHERE id = ?1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read flow run status");

        assert_eq!(status, "done");
        assert_eq!(flow_run_status, "running");

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn manager_with_runtime_db_path_does_not_skip_rust_execution() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::with_recording_runner_for_test(
            path.clone(),
            crate::recording_runtime::worker::RecordingRunner::from_fn(|input, output_path| {
                Ok(crate::recording_runtime::types::RecordingFinishInput {
                    account_id: input.account_id,
                    flow_id: input.flow_id,
                    flow_run_id: input.flow_run_id,
                    external_recording_id: input.external_recording_id.clone(),
                    room_id: input.room_id.clone(),
                    file_path: Some(output_path.to_string()),
                    error_message: None,
                    duration_seconds: input.max_duration_seconds,
                    file_size_bytes: 1,
                    outcome: RecordingOutcome::Success,
                })
            }),
        );

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created flow run");

        let recording_key: String = conn
            .query_row(
                "SELECT sidecar_recording_id FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read recording key");

        assert!(recording_key.starts_with("rust-recording-"));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn finalize_active_recording_uses_durable_external_recording_key() {
        let (mut conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        insert_flow(&conn, 2, true, "shop_xyz");
        let manager = LiveRuntimeManager::new();

        manager
            .start_flow_session(&conn, 1)
            .expect("start first session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created flow run");
        let external_recording_id: String = conn
            .query_row(
                "SELECT sidecar_recording_id FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read recording key");

        conn.execute(
            "UPDATE accounts SET username = 'shop_moved' WHERE id = 1",
            [],
        )
        .expect("move account away from original flow");

        manager
            .finalize_recording_by_key(
                &mut conn,
                &external_recording_id,
                Some("7312345"),
                true,
                None,
            )
            .expect("finalize by durable key");

        let row: (String, Option<i64>) = conn
            .query_row(
                "SELECT status, flow_run_id FROM recordings WHERE sidecar_recording_id = ?1",
                [&external_recording_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read finalized row");

        assert_eq!(row.0, "done");
        assert_eq!(row.1, Some(flow_run_id));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn successful_finalization_keeps_session_active_run_and_moves_to_processing() {
        let (mut conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created flow run");
        let external_recording_id: String = conn
            .query_row(
                "SELECT sidecar_recording_id FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read recording key");

        manager
            .finalize_recording_by_key(
                &mut conn,
                &external_recording_id,
                Some("7312345"),
                true,
                None,
            )
            .expect("finalize success");

        assert_eq!(
            manager.session_active_flow_run_id_for_test(1),
            Some(flow_run_id)
        );
        assert!(!manager.session_is_polling_for_test(1));
        let snapshot = manager
            .list_sessions()
            .into_iter()
            .find(|snapshot| snapshot.flow_id == 1)
            .expect("session snapshot");
        assert_eq!(snapshot.status, "processing");

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn successful_finalization_triggers_sidecar_processing_handoff_with_external_key() {
        let (mut conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let (sidecar_base, requests, server_handle) = sidecar_request_server();
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES ('sidecar_port', ?1, datetime('now','+7 hours'))",
            [sidecar_base.trim_start_matches("http://127.0.0.1:")],
        )
        .expect("insert sidecar port setting");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created flow run");
        let recording: (String, String) = conn
            .query_row(
                "SELECT sidecar_recording_id, file_path FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read recording row");

        manager
            .finalize_recording_by_key(&mut conn, &recording.0, Some("7312345"), true, None)
            .expect("finalize success");

        server_handle.join().expect("join sidecar server");
        let requests = requests.lock().expect("lock requests");
        let payload = requests.first().expect("captured handoff request");
        let body = payload.split("\r\n\r\n").nth(1).expect("http request body");
        let body_json: serde_json::Value =
            serde_json::from_str(body).expect("parse handoff request json");

        assert!(payload.contains("POST /api/video/process HTTP/1.1"));
        assert_eq!(
            body_json.get("recording_id").and_then(|v| v.as_str()),
            Some(recording.0.as_str())
        );
        assert_eq!(
            body_json.get("account_id").and_then(|v| v.as_i64()),
            Some(1)
        );
        assert_eq!(
            body_json.get("username").and_then(|v| v.as_str()),
            Some("shop_abc")
        );
        assert_eq!(
            body_json.get("file_path").and_then(|v| v.as_str()),
            Some(recording.1.as_str())
        );
        let logs = manager.list_runtime_logs_for_test(Some(1), Some(20));
        let record_completed_index = logs
            .iter()
            .position(|entry| {
                entry.event == "record_completed"
                    && entry.flow_run_id == Some(flow_run_id)
                    && entry.external_recording_id.as_deref() == Some(recording.0.as_str())
                    && entry.stage == "record"
            })
            .expect("record_completed log index");
        let handoff_completed_index = logs
            .iter()
            .position(|entry| {
                entry.event == "sidecar_handoff_completed"
                    && entry.flow_run_id == Some(flow_run_id)
                    && entry.external_recording_id.as_deref() == Some(recording.0.as_str())
                    && entry.stage == "clip"
            })
            .expect("sidecar_handoff_completed log index");
        assert!(record_completed_index < handoff_completed_index);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn successful_finalization_emits_runtime_update_event_on_stage_transition() {
        let (mut conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created flow run");
        let external_recording_id: String = conn
            .query_row(
                "SELECT sidecar_recording_id FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read recording key");

        manager
            .finalize_recording_by_key(
                &mut conn,
                &external_recording_id,
                Some("7312345"),
                true,
                None,
            )
            .expect("finalize success");

        let snapshot = manager
            .take_latest_runtime_event_for_test()
            .expect("runtime event snapshot");
        assert_eq!(snapshot.flow_id, 1);
        assert_eq!(snapshot.status, "processing");
        assert_eq!(snapshot.current_node.as_deref(), Some("clip"));
        assert_eq!(snapshot.active_flow_run_id, Some(flow_run_id));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn handoff_failure_marks_clip_stage_failed_and_emits_runtime_update() {
        let (mut conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES ('sidecar_port', '1', datetime('now','+7 hours'))",
            [],
        )
        .expect("insert invalid sidecar port");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created flow run");
        let external_recording_id: String = conn
            .query_row(
                "SELECT sidecar_recording_id FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read recording key");

        manager
            .finalize_recording_by_key(
                &mut conn,
                &external_recording_id,
                Some("7312345"),
                true,
                None,
            )
            .expect("finalize success with handoff failure handling");

        let flow_run_status: String = conn
            .query_row(
                "SELECT status FROM flow_runs WHERE id = ?1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read flow run status");
        let clip_node_status: Option<String> = conn
            .query_row(
                "SELECT status FROM flow_node_runs WHERE flow_run_id = ?1 AND node_key = 'clip' ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| row.get(0),
            )
            .ok();
        let flow_status: (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, current_node, json_extract(published_config_json, '$.last_error') FROM flows f JOIN flow_nodes n ON n.flow_id = f.id AND n.node_key = 'start' WHERE f.id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read flow status");
        let snapshot = manager
            .take_latest_runtime_event_for_test()
            .expect("runtime event snapshot");

        assert_eq!(flow_run_status, "failed");
        assert_eq!(clip_node_status.as_deref(), Some("failed"));
        assert_eq!(flow_status.0, "error");
        assert_eq!(flow_status.1.as_deref(), Some("clip"));
        let last_error = flow_status.2.unwrap_or_default();
        assert!(!last_error.is_empty());
        assert!(
            last_error.contains("error")
                || last_error.contains("refused")
                || last_error.contains("connect")
        );
        assert_eq!(snapshot.status, "error");
        assert_eq!(snapshot.current_node.as_deref(), Some("clip"));
        let logs = manager.list_runtime_logs_for_test(Some(1), Some(20));
        let record_completed_index = logs
            .iter()
            .position(|entry| {
                entry.event == "record_completed"
                    && entry.flow_run_id == Some(flow_run_id)
                    && entry.external_recording_id.as_deref()
                        == Some(external_recording_id.as_str())
                    && entry.stage == "record"
            })
            .expect("record_completed log index");
        let handoff_failed_index = logs
            .iter()
            .position(|entry| {
                entry.event == "sidecar_handoff_failed"
                    && entry.flow_run_id == Some(flow_run_id)
                    && entry.external_recording_id.as_deref()
                        == Some(external_recording_id.as_str())
                    && entry.stage == "clip"
                    && entry.code.as_deref() == Some("handoff.http_failed")
            })
            .expect("sidecar_handoff_failed log index");
        assert!(record_completed_index < handoff_failed_index);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn cancelled_session_path_tears_down_exactly_once() {
        let _guard = lock_teardown_test_guard_for_test();
        reset_teardown_call_count_for_test();
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::with_recording_process_runner_for_test(
            path.clone(),
            crate::recording_runtime::worker::RecordingProcessRunner::from_fn(
                |input, output_path| {
                    let cancelled = Arc::new(Mutex::new(false));
                    let cancelled_for_wait = Arc::clone(&cancelled);
                    let cancelled_for_cancel = Arc::clone(&cancelled);
                    let output_path = output_path.to_string();
                    let input = input.clone();
                    Ok(
                        crate::recording_runtime::worker::RecordingProcessHandle::from_parts(
                            Box::new(move || {
                                std::thread::sleep(std::time::Duration::from_millis(300));
                                let outcome =
                                    if *cancelled_for_wait.lock().expect("lock cancel flag") {
                                        RecordingOutcome::Cancelled
                                    } else {
                                        RecordingOutcome::Success
                                    };
                                Ok(crate::recording_runtime::types::RecordingFinishInput {
                                    account_id: input.account_id,
                                    flow_id: input.flow_id,
                                    flow_run_id: input.flow_run_id,
                                    external_recording_id: input.external_recording_id.clone(),
                                    room_id: input.room_id.clone(),
                                    file_path: Some(output_path.clone()),
                                    error_message: None,
                                    duration_seconds: 0,
                                    file_size_bytes: 0,
                                    outcome,
                                })
                            }),
                            Arc::new(move || {
                                *cancelled_for_cancel.lock().expect("lock cancel flag") = true;
                                Ok(())
                            }),
                        ),
                    )
                },
            ),
        );

        manager.start_flow_session(&conn, 1).expect("start session");
        manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created run");

        manager.stop_flow_session(1).expect("cancel flow session");
        manager.drain_worker_threads_for_test();

        assert_eq!(teardown_call_count_for_test(), 1);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn finalize_by_external_recording_key_updates_targeted_run_not_latest_running_run() {
        let (mut conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start session");
        let first_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle first live")
            .expect("created first run");
        let first_recording_key: String = conn
            .query_row(
                "SELECT sidecar_recording_id FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [first_run_id],
                |row| row.get(0),
            )
            .expect("read first recording key");

        conn.execute(
            "UPDATE flow_runs SET status = 'running' WHERE id = ?1",
            [first_run_id],
        )
        .expect("keep first run running for targeted finalize test");
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) VALUES (999, 1, 1, 'running', datetime('now','+7 hours'), 'test')",
            [],
        )
        .expect("insert newer running flow run");
        conn.execute(
            "INSERT INTO flow_node_runs (flow_run_id, flow_id, node_key, status, started_at) VALUES (999, 1, 'record', 'running', datetime('now','+7 hours'))",
            [],
        )
        .expect("insert newer running record node");

        manager
            .finalize_recording_by_key(&mut conn, &first_recording_key, Some("7312345"), true, None)
            .expect("finalize first recording by key");

        let first_node_status: String = conn
            .query_row(
                "SELECT status FROM flow_node_runs WHERE flow_run_id = ?1 AND node_key = 'record' ORDER BY id DESC LIMIT 1",
                [first_run_id],
                |row| row.get(0),
            )
            .expect("read first node status");
        let second_node_status: String = conn
            .query_row(
                "SELECT status FROM flow_node_runs WHERE flow_run_id = 999 AND node_key = 'record' ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("read second node status");

        assert_eq!(first_node_status, "completed");
        assert_eq!(second_node_status, "running");

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn stop_and_shutdown_cancel_tracked_rust_recording_execution_and_finalize_rows() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        insert_flow(&conn, 2, true, "shop_xyz");
        let manager = LiveRuntimeManager::with_recording_runner_autospawn_for_test(
            path.clone(),
            crate::recording_runtime::worker::RecordingRunner::from_fn(|input, output_path| {
                std::thread::sleep(std::time::Duration::from_millis(200));
                Ok(crate::recording_runtime::types::RecordingFinishInput {
                    account_id: input.account_id,
                    flow_id: input.flow_id,
                    flow_run_id: input.flow_run_id,
                    external_recording_id: input.external_recording_id.clone(),
                    room_id: input.room_id.clone(),
                    file_path: Some(output_path.to_string()),
                    error_message: None,
                    duration_seconds: input.max_duration_seconds,
                    file_size_bytes: 0,
                    outcome: RecordingOutcome::Success,
                })
            }),
        );

        manager
            .start_flow_session(&conn, 1)
            .expect("start first session");
        manager
            .start_flow_session(&conn, 2)
            .expect("start second session");
        let first_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle first live")
            .expect("first run created");
        let second_run_id = manager
            .handle_live_detected(
                &conn,
                2,
                &LiveStatus {
                    room_id: "8888".to_string(),
                    stream_url: Some("https://example.com/other.flv".to_string()),
                    viewer_count: Some(55),
                },
            )
            .expect("handle second live")
            .expect("second run created");

        let stopped = manager.stop_flow_session(1).expect("stop first flow");
        assert_eq!(stopped.len(), 1);
        let shutdown = manager.shutdown().expect("shutdown manager");
        assert_eq!(shutdown.len(), 1);
        manager.drain_worker_threads_for_test();

        let reader = crate::db::init::initialize_database(&path).expect("open reader");
        let first_status: String = reader
            .query_row(
                "SELECT status FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [first_run_id],
                |row| row.get(0),
            )
            .expect("read first recording status");
        let second_status: String = reader
            .query_row(
                "SELECT status FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [second_run_id],
                |row| row.get(0),
            )
            .expect("read second recording status");

        assert_eq!(first_status, "cancelled");
        assert_eq!(second_status, "cancelled");

        manager.drain_worker_threads_for_test();
        drop(reader);
        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn failure_finalization_closes_record_node_and_flow_run_failed() {
        let (mut conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::with_recording_runner_for_test(
            path.clone(),
            crate::recording_runtime::worker::RecordingRunner::from_fn(|input, output_path| {
                Ok(crate::recording_runtime::types::RecordingFinishInput {
                    account_id: input.account_id,
                    flow_id: input.flow_id,
                    flow_run_id: input.flow_run_id,
                    external_recording_id: input.external_recording_id.clone(),
                    room_id: input.room_id.clone(),
                    file_path: Some(output_path.to_string()),
                    error_message: Some("ffmpeg failed".to_string()),
                    duration_seconds: 0,
                    file_size_bytes: 0,
                    outcome: RecordingOutcome::Failed,
                })
            }),
        );

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created run");
        let external_recording_id: String = conn
            .query_row(
                "SELECT sidecar_recording_id FROM recordings WHERE flow_run_id = ?1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read recording key");

        manager
            .finalize_recording_by_key(
                &mut conn,
                &external_recording_id,
                Some("7312345"),
                false,
                Some("ffmpeg failed"),
            )
            .expect("finalize failure");

        let flow_run_status: String = conn
            .query_row(
                "SELECT status FROM flow_runs WHERE id = ?1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read flow run status");
        let record_node_status: String = conn
            .query_row(
                "SELECT status FROM flow_node_runs WHERE flow_run_id = ?1 AND node_key = 'record'",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read record node status");

        assert_eq!(flow_run_status, "failed");
        assert_eq!(record_node_status, "failed");

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn cancel_finalization_marks_recording_record_node_and_flow_run_cancelled() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::with_recording_runner_autospawn_for_test(
            path.clone(),
            crate::recording_runtime::worker::RecordingRunner::from_fn(|input, output_path| {
                std::thread::sleep(std::time::Duration::from_millis(300));
                Ok(crate::recording_runtime::types::RecordingFinishInput {
                    account_id: input.account_id,
                    flow_id: input.flow_id,
                    flow_run_id: input.flow_run_id,
                    external_recording_id: input.external_recording_id.clone(),
                    room_id: input.room_id.clone(),
                    file_path: Some(output_path.to_string()),
                    error_message: None,
                    duration_seconds: 0,
                    file_size_bytes: 0,
                    outcome: RecordingOutcome::Success,
                })
            }),
        );

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created run");

        manager.stop_flow_session(1).expect("cancel flow session");
        manager.drain_worker_threads_for_test();

        let recording_status: String = conn
            .query_row(
                "SELECT status FROM recordings WHERE flow_run_id = ?1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read recording status");
        let flow_run_status: String = conn
            .query_row(
                "SELECT status FROM flow_runs WHERE id = ?1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read flow run status");
        let record_node_status: String = conn
            .query_row(
                "SELECT status FROM flow_node_runs WHERE flow_run_id = ?1 AND node_key = 'record'",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read record node status");

        assert_eq!(recording_status, "cancelled");
        assert_eq!(flow_run_status, "cancelled");
        assert_eq!(record_node_status, "cancelled");

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn stop_and_shutdown_cancel_active_rust_worker_execution_deterministically() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::with_recording_runner_autospawn_for_test(
            path.clone(),
            crate::recording_runtime::worker::RecordingRunner::from_fn(|input, output_path| {
                std::thread::sleep(std::time::Duration::from_millis(500));
                Ok(crate::recording_runtime::types::RecordingFinishInput {
                    account_id: input.account_id,
                    flow_id: input.flow_id,
                    flow_run_id: input.flow_run_id,
                    external_recording_id: input.external_recording_id.clone(),
                    room_id: input.room_id.clone(),
                    file_path: Some(output_path.to_string()),
                    error_message: None,
                    duration_seconds: input.max_duration_seconds,
                    file_size_bytes: 0,
                    outcome: RecordingOutcome::Success,
                })
            }),
        );

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created run");

        let stopped = manager.stop_flow_session(1).expect("stop flow session");
        assert_eq!(stopped.len(), 1);
        manager.drain_worker_threads_for_test();

        let reader = crate::db::init::initialize_database(&path).expect("open reader");
        let recording_status: String = reader
            .query_row(
                "SELECT status FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read cancelled recording status");

        assert_eq!(recording_status, "cancelled");

        drop(reader);
        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn stop_flow_session_cancels_process_even_when_cancel_handle_registers_late() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let cancel_call_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let manager = LiveRuntimeManager::with_recording_process_runner_for_test(
            path.clone(),
            crate::recording_runtime::worker::RecordingProcessRunner::from_fn({
                let cancel_call_count = Arc::clone(&cancel_call_count);
                move |input, output_path| {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    let output_path = output_path.to_string();
                    let input = input.clone();
                    let cancel_call_count = Arc::clone(&cancel_call_count);
                    Ok(
                        crate::recording_runtime::worker::RecordingProcessHandle::from_parts(
                            Box::new(move || {
                                std::thread::sleep(std::time::Duration::from_millis(250));
                                Ok(crate::recording_runtime::types::RecordingFinishInput {
                                    account_id: input.account_id,
                                    flow_id: input.flow_id,
                                    flow_run_id: input.flow_run_id,
                                    external_recording_id: input.external_recording_id.clone(),
                                    room_id: input.room_id.clone(),
                                    file_path: Some(output_path.clone()),
                                    error_message: None,
                                    duration_seconds: 0,
                                    file_size_bytes: 0,
                                    outcome: RecordingOutcome::Success,
                                })
                            }),
                            Arc::new(move || {
                                cancel_call_count.fetch_add(1, Ordering::SeqCst);
                                Ok(())
                            }),
                        ),
                    )
                }
            }),
        );

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created run");

        std::thread::sleep(std::time::Duration::from_millis(50));
        manager.stop_flow_session(1).expect("stop flow session");
        manager.drain_worker_threads_for_test();

        assert_eq!(cancel_call_count.load(Ordering::SeqCst), 1);

        let reader = crate::db::init::initialize_database(&path).expect("open reader");
        let recording_status: String = reader
            .query_row(
                "SELECT status FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read recording status");
        assert_eq!(recording_status, "cancelled");

        drop(reader);
        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn handle_live_detected_creates_rust_owned_recording_row_for_active_run() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created flow run");

        let row: (i64, Option<i64>, Option<String>, String) = conn
            .query_row(
                "SELECT flow_id, flow_run_id, room_id, status FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read rust-owned recording row");

        assert_eq!(row.0, 1);
        assert_eq!(row.1, Some(flow_run_id));
        assert_eq!(row.2.as_deref(), Some("7312345"));
        assert_eq!(row.3, "recording");

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn complete_active_run_finalizes_rust_owned_recording_row_and_advances_stage() {
        let (mut conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::with_recording_runner_for_test(
            path.clone(),
            crate::recording_runtime::worker::RecordingRunner::from_fn(|input, output_path| {
                Ok(crate::recording_runtime::types::RecordingFinishInput {
                    account_id: input.account_id,
                    flow_id: input.flow_id,
                    flow_run_id: input.flow_run_id,
                    external_recording_id: input.external_recording_id.clone(),
                    room_id: input.room_id.clone(),
                    file_path: Some(output_path.to_string()),
                    error_message: None,
                    duration_seconds: input.max_duration_seconds,
                    file_size_bytes: 1,
                    outcome: RecordingOutcome::Success,
                })
            }),
        );

        manager.start_flow_session(&conn, 1).expect("start session");
        let flow_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("handle live")
            .expect("created flow run");

        manager
            .complete_active_run(&mut conn, 1, Some("7312345"))
            .expect("complete run");

        let row: (String, Option<String>) = conn
            .query_row(
                "SELECT status, ended_at FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read finalized recording row");

        assert_eq!(row.0, "done");
        assert!(row.1.is_some());
        assert!(!manager.session_is_polling_for_test(1));
        let logs = manager.list_runtime_logs_for_test(Some(1), Some(20));
        assert!(logs.iter().any(|entry| {
            entry.event == "record_completed"
                && entry.flow_run_id == Some(flow_run_id)
                && entry.stage == "record"
        }));
        assert_eq!(
            manager.session_active_flow_run_id_for_test(1),
            Some(flow_run_id)
        );

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn offline_transition_allows_same_room_id_to_start_again_after_stage_reset() {
        let (mut conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::with_recording_runner_for_test(
            path.clone(),
            crate::recording_runtime::worker::RecordingRunner::from_fn(|input, output_path| {
                Ok(crate::recording_runtime::types::RecordingFinishInput {
                    account_id: input.account_id,
                    flow_id: input.flow_id,
                    flow_run_id: input.flow_run_id,
                    external_recording_id: input.external_recording_id.clone(),
                    room_id: input.room_id.clone(),
                    file_path: Some(output_path.to_string()),
                    error_message: None,
                    duration_seconds: input.max_duration_seconds,
                    file_size_bytes: 1,
                    outcome: RecordingOutcome::Success,
                })
            }),
        );

        manager.start_flow_session(&conn, 1).expect("start session");
        let first_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(77),
                },
            )
            .expect("first live detect")
            .expect("first run created");
        manager
            .complete_active_run(&mut conn, 1, Some("7312345"))
            .expect("complete first run");
        let duplicate_before_reset = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(88),
                },
            )
            .expect("second live detect before reset");
        assert_eq!(duplicate_before_reset, None);

        manager.mark_source_offline(1).expect("mark source offline");
        let logs = manager.list_runtime_logs_for_test(Some(1), Some(20));
        assert!(logs.iter().any(|entry| {
            entry.event == "source_offline_marked"
                && entry.stage == "session"
                && entry
                    .context
                    .get("flow_id")
                    .and_then(|value| value.as_i64())
                    == Some(1)
        }));

        let second_run_id = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(99),
                },
            )
            .expect("live detect after offline")
            .expect("second run created");

        assert_ne!(first_run_id, second_run_id);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn poll_loop_autonomous_live_detect_starts_run() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start session");
        manager.with_stubbed_live_status_for_test(vec![Ok(Some(LiveStatus {
            room_id: "7312345".to_string(),
            stream_url: Some("https://example.com/live.flv".to_string()),
            viewer_count: Some(77),
        }))]);

        manager
            .run_one_poll_iteration_for_test(&conn, 1)
            .expect("run one poll iteration");

        assert!(manager.session_active_flow_run_id_for_test(1).is_some());

        let run_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM flow_runs WHERE flow_id = 1 AND status = 'running'",
                [],
                |row| row.get(0),
            )
            .expect("count running runs");
        assert_eq!(run_count, 1);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn poll_loop_live_without_stream_keeps_watching() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start session");
        manager.with_stubbed_live_status_for_test(vec![Ok(Some(LiveStatus {
            room_id: "7312345".to_string(),
            stream_url: None,
            viewer_count: Some(77),
        }))]);

        manager
            .run_one_poll_iteration_for_test(&conn, 1)
            .expect("run one poll iteration");

        assert!(manager.session_is_polling_for_test(1));
        assert_eq!(manager.session_active_flow_run_id_for_test(1), None);
        let run_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM flow_runs WHERE flow_id = 1",
                [],
                |row| row.get(0),
            )
            .expect("count flow runs");
        assert_eq!(run_count, 0);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn poll_loop_offline_resets_dedupe_and_allows_same_room_again() {
        let (mut conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start session");
        manager.with_stubbed_live_status_for_test(vec![Ok(Some(LiveStatus {
            room_id: "7312345".to_string(),
            stream_url: Some("https://example.com/live.flv".to_string()),
            viewer_count: Some(77),
        }))]);
        manager
            .run_one_poll_iteration_for_test(&conn, 1)
            .expect("first live poll iteration");
        let first_run_id = manager
            .session_active_flow_run_id_for_test(1)
            .expect("first run id");

        manager
            .fail_active_run(&mut conn, 1, Some("7312345"), Some("test failure"))
            .expect("finalize first run to watching state");

        let duplicate_before_offline = manager
            .handle_live_detected(
                &conn,
                1,
                &LiveStatus {
                    room_id: "7312345".to_string(),
                    stream_url: Some("https://example.com/live.flv".to_string()),
                    viewer_count: Some(88),
                },
            )
            .expect("duplicate detect before offline reset");
        assert_eq!(duplicate_before_offline, None);

        manager.with_stubbed_live_status_for_test(vec![
            Ok(None),
            Ok(Some(LiveStatus {
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(99),
            })),
        ]);

        manager
            .run_one_poll_iteration_for_test(&conn, 1)
            .expect("offline poll iteration");
        manager
            .run_one_poll_iteration_for_test(&conn, 1)
            .expect("live poll iteration after offline");

        let second_run_id = manager
            .session_active_flow_run_id_for_test(1)
            .expect("second run id");
        assert_ne!(first_run_id, second_run_id);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn poll_loop_stale_iteration_is_dropped_before_apply() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start session");
        manager.with_stubbed_live_status_for_test(vec![Ok(Some(LiveStatus {
            room_id: "7312345".to_string(),
            stream_url: Some("https://example.com/live.flv".to_string()),
            viewer_count: Some(77),
        }))]);
        let manager_for_hook = manager.clone();
        manager.with_before_poll_apply_hook_for_test(move || {
            manager_for_hook
                .stop_flow_session(1)
                .expect("stop session in stale hook");
        });

        manager
            .run_one_poll_iteration_for_test(&conn, 1)
            .expect("run one poll iteration");

        assert_eq!(manager.session_active_flow_run_id_for_test(1), None);
        assert!(manager.list_sessions().is_empty());
        let run_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM flow_runs WHERE flow_id = 1",
                [],
                |row| row.get(0),
            )
            .expect("count flow runs");
        assert_eq!(run_count, 0);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn start_flow_session_emits_lease_conflict_runtime_log() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        insert_flow(&conn, 2, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager
            .start_flow_session(&conn, 1)
            .expect("start first session");
        let error = manager
            .start_flow_session(&conn, 2)
            .expect_err("second session should hit username lease conflict");

        assert!(error.contains("username lease already held"));
        let logs = manager.list_runtime_logs_for_test(Some(2), Some(10));
        assert!(logs.iter().any(|entry| {
            entry.event == "lease_conflict"
                && entry.code.as_deref() == Some("start.username_conflict")
                && entry.stage == "start"
                && entry
                    .context
                    .get("lookup_key")
                    .and_then(|value| value.as_str())
                    == Some("shop_abc")
        }));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn start_flow_session_creates_single_poll_task_for_enabled_flow() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager
            .start_flow_session(&conn, 1)
            .expect("start enabled flow session");
        manager
            .start_flow_session(&conn, 1)
            .expect("second start should not duplicate task");

        assert!(manager.session_has_poll_task_for_test(1));
        assert_eq!(manager.active_poll_task_count_for_test(), 1);
        assert_eq!(manager.session_generation_for_test(1), Some(1));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn stop_flow_session_cancels_and_removes_poll_task() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start flow");
        manager.stop_flow_session(1).expect("stop flow");

        assert!(!manager.session_has_poll_task_for_test(1));
        assert_eq!(manager.active_poll_task_count_for_test(), 0);
        assert_eq!(manager.cancelled_poll_generations_for_test(1), vec![1]);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reconcile_flow_replaces_poll_task_without_duplicates() {
        let (conn, path) = open_temp_db();
        insert_flow(&conn, 1, true, "shop_abc");
        let manager = LiveRuntimeManager::new();

        manager.start_flow_session(&conn, 1).expect("start flow");
        assert_eq!(manager.session_generation_for_test(1), Some(1));
        assert_eq!(manager.active_poll_task_count_for_test(), 1);

        manager.reconcile_flow(&conn, 1).expect("reconcile flow");

        assert!(manager.session_has_poll_task_for_test(1));
        assert_eq!(manager.active_poll_task_count_for_test(), 1);
        assert_eq!(manager.session_generation_for_test(1), Some(2));
        assert_eq!(manager.cancelled_poll_generations_for_test(1), vec![1]);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }
}
