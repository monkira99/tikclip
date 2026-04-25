#[cfg(test)]
use crate::commands::live_runtime::FlowRuntimeSnapshot;
use crate::live_runtime::logs::FlowRuntimeLogBuffer;
use crate::live_runtime::session::LiveRuntimeSession;
use crate::live_runtime::types::LiveRuntimeSessionSnapshot;
#[cfg(test)]
use crate::recording_runtime::types::RecordingStartInput;
#[cfg(test)]
use crate::recording_runtime::worker::RecordingProcessHandle;
use crate::recording_runtime::worker::{RecordingProcessRunner, RecordingRunner};
#[cfg(test)]
use crate::tiktok::types::LiveStatus;
use std::collections::HashMap;
#[cfg(test)]
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use tauri::AppHandle;

mod events;
mod polling;
mod recordings;
mod runs;
mod sessions;
mod store;

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
    storage_root: Option<PathBuf>,
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
            storage_root: self.storage_root.clone(),
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

fn infer_storage_root_from_db_path(db_path: &Path) -> Option<PathBuf> {
    let parent = db_path.parent()?;
    if db_path.file_name().and_then(|name| name.to_str()) == Some("app.db")
        && parent.file_name().and_then(|name| name.to_str()) == Some("data")
    {
        return parent.parent().map(Path::to_path_buf);
    }
    Some(parent.to_path_buf())
}

impl LiveRuntimeManager {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(LiveRuntimeState::default())),
            recording_runner: RecordingRunner::ffmpeg(),
            recording_process_runner: RecordingProcessRunner::ffmpeg(),
            runtime_db_path: None,
            storage_root: Some(std::env::temp_dir()),
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
        let storage_root = infer_storage_root_from_db_path(&db_path);
        Self {
            state: Arc::new(Mutex::new(LiveRuntimeState::default())),
            recording_runner: RecordingRunner::ffmpeg(),
            recording_process_runner: RecordingProcessRunner::ffmpeg(),
            runtime_db_path: Some(db_path),
            storage_root,
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
        let storage_root = infer_storage_root_from_db_path(&db_path);
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
            storage_root,
            auto_spawn_recording_execution: false,
            app_handle: None,
            runtime_log_emit_error: Arc::new(Mutex::new(None)),
            latest_runtime_event: Arc::new(Mutex::new(None)),
            worker_threads: Arc::new(Mutex::new(Vec::new())),
            stubbed_live_statuses: Arc::new(Mutex::new(VecDeque::new())),
            before_poll_apply_hook: Arc::new(Mutex::new(None)),
        }
    }

    #[cfg(test)]
    pub fn with_recording_runner_autospawn_for_test(
        db_path: PathBuf,
        recording_runner: RecordingRunner,
    ) -> Self {
        let storage_root = infer_storage_root_from_db_path(&db_path);
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
            storage_root,
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
        let storage_root = infer_storage_root_from_db_path(&db_path);
        Self {
            state: Arc::new(Mutex::new(LiveRuntimeState::default())),
            recording_runner: RecordingRunner::from_fn(|_, _| {
                Err("recording_runner not used in process-runner test".to_string())
            }),
            recording_process_runner,
            runtime_db_path: Some(db_path),
            storage_root,
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

    pub fn attach_storage_root(&mut self, storage_root: PathBuf) {
        self.storage_root = Some(storage_root);
    }
}

#[cfg(test)]
mod tests;
