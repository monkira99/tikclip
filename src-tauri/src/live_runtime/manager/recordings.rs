use super::{
    store::{load_published_node_config, open_runtime_connection},
    LiveRuntimeManager,
};
use crate::live_runtime::logs::FlowRuntimeLogLevel;
use crate::recording_runtime::types::{RecordingOutcome, RecordingStartInput};
use crate::recording_runtime::worker;
use crate::workflow::record_node::{self, RecordPostProcessInput, SpeechSpan};
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub(super) enum RecordNodePostProcessHandoff {
    Completed { speech_segments: Vec<SpeechSpan> },
    Disabled,
    Failed,
}

impl RecordNodePostProcessHandoff {
    pub(super) fn completed_or_disabled(&self) -> bool {
        matches!(
            self,
            RecordNodePostProcessHandoff::Completed { .. } | RecordNodePostProcessHandoff::Disabled
        )
    }

    pub(super) fn speech_segments_or_empty(&self) -> &[SpeechSpan] {
        match self {
            RecordNodePostProcessHandoff::Completed { speech_segments } => speech_segments,
            RecordNodePostProcessHandoff::Disabled | RecordNodePostProcessHandoff::Failed => &[],
        }
    }

    pub(super) fn speech_segments(&self) -> Option<&[SpeechSpan]> {
        match self {
            RecordNodePostProcessHandoff::Completed { speech_segments } => Some(speech_segments),
            RecordNodePostProcessHandoff::Disabled | RecordNodePostProcessHandoff::Failed => None,
        }
    }
}

impl LiveRuntimeManager {
    pub(super) fn run_record_node_post_processing(
        &self,
        conn: &Connection,
        recording_id: i64,
        flow_id: i64,
        flow_run_id: i64,
        external_recording_id: &str,
        file_path: &str,
    ) -> RecordNodePostProcessHandoff {
        let Some(storage_root) = self.storage_root.as_deref() else {
            let _ = self.log_runtime_event(
                flow_id,
                Some(flow_run_id),
                Some(external_recording_id),
                "record",
                "rust_audio_processing_failed",
                FlowRuntimeLogLevel::Warn,
                Some("audio.storage_root_missing"),
                "Skipped record-node audio processing because storage root is unavailable",
                serde_json::json!({ "file_path_present": true }),
            );
            return RecordNodePostProcessHandoff::Failed;
        };

        let _ = self.log_runtime_event(
            flow_id,
            Some(flow_run_id),
            Some(external_recording_id),
            "record",
            "rust_audio_processing_started",
            FlowRuntimeLogLevel::Info,
            None,
            "Started record-node Rust sherpa-onnx audio processing",
            serde_json::json!({ "file_path_present": true }),
        );

        match record_node::run_post_record_audio(
            conn,
            storage_root,
            load_published_node_config(conn, flow_id, "record")
                .unwrap_or_else(|_| "{}".to_string())
                .as_str(),
            &RecordPostProcessInput {
                recording_id,
                file_path: Path::new(file_path).to_path_buf(),
            },
        ) {
            Ok(output) => {
                let segment_count = output.speech_segments.len();
                let _ = self.log_runtime_event(
                    flow_id,
                    Some(flow_run_id),
                    Some(external_recording_id),
                    "record",
                    "rust_audio_processing_completed",
                    FlowRuntimeLogLevel::Info,
                    None,
                    "Completed record-node Rust sherpa-onnx audio processing",
                    serde_json::json!({ "speech_segments": segment_count }),
                );
                if output.audio_enabled {
                    RecordNodePostProcessHandoff::Completed {
                        speech_segments: output.speech_segments,
                    }
                } else {
                    RecordNodePostProcessHandoff::Disabled
                }
            }
            Err(err) => {
                let _ = self.log_runtime_event(
                    flow_id,
                    Some(flow_run_id),
                    Some(external_recording_id),
                    "record",
                    "rust_audio_processing_failed",
                    FlowRuntimeLogLevel::Warn,
                    Some("audio.processing_failed"),
                    "Record-node Rust audio processing failed; sidecar audio fallback remains enabled",
                    serde_json::json!({ "error": err }),
                );
                RecordNodePostProcessHandoff::Failed
            }
        }
    }

    pub fn list_active_rust_recordings(
        &self,
        conn: &Connection,
    ) -> Result<Vec<crate::commands::recordings::ActiveRustRecordingStatus>, String> {
        let active_keys: Vec<String> = self
            .state
            .lock()
            .map_err(|e| e.to_string())?
            .active_recordings_by_flow
            .values()
            .map(|handle| handle.external_recording_id.clone())
            .collect();
        if active_keys.is_empty() {
            return Ok(Vec::new());
        }

        let mut rows = Vec::with_capacity(active_keys.len());
        for key in active_keys {
            let row = conn
                .query_row(
                    "SELECT r.sidecar_recording_id, r.account_id, a.username, r.status, \
                            r.duration_seconds, r.file_size_bytes, r.file_path, r.error_message \
                     FROM recordings r \
                     JOIN accounts a ON a.id = r.account_id \
                     WHERE r.sidecar_recording_id = ?1",
                    [key.as_str()],
                    |row| {
                        Ok(crate::commands::recordings::ActiveRustRecordingStatus {
                            recording_id: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                            account_id: row.get(1)?,
                            username: row.get(2)?,
                            status: row.get(3)?,
                            duration_seconds: row.get(4)?,
                            file_size_bytes: row.get(5)?,
                            file_path: row.get(6)?,
                            error_message: row.get(7)?,
                        })
                    },
                )
                .optional()
                .map_err(|e| e.to_string())?;
            if let Some(row) = row {
                rows.push(row);
            }
        }
        rows.sort_by(|a, b| a.recording_id.cmp(&b.recording_id));
        Ok(rows)
    }

    pub(super) fn spawn_recording_execution(
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
