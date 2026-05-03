use super::{
    store::{
        finalize_latest_recording_row, load_flow_runtime_config, load_record_duration_seconds,
        open_runtime_connection, update_flow_runtime_by_flow_id,
    },
    ActiveRecordingHandle, LiveRuntimeManager,
};
use crate::commands::flows::UpdateFlowRuntimeByAccountInput;
use crate::commands::recordings::{finalize_rust_recording_row, start_rust_recording_row};
use crate::live_runtime::account_binding::{
    resolve_or_create_account_for_username, ResolveAccountResult,
};
use crate::live_runtime::logs::FlowRuntimeLogLevel;
use crate::recording_runtime::types::{
    RecordingFinishInput, RecordingOutcome, RecordingStartInput,
};
use crate::recording_runtime::worker;
use crate::tiktok::types::LiveStatus;
use crate::time_hcm::now_timestamp_hcm;
use crate::workflow::{runtime_store, start_node};
use rusqlite::{Connection, OptionalExtension};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

impl LiveRuntimeManager {
    fn fail_downstream_processing_stage(
        &self,
        conn: &mut Connection,
        flow_id: i64,
        flow_run_id: i64,
        stage: &str,
        err: &str,
    ) -> Result<(), String> {
        log::warn!(
            "live runtime downstream stage failed flow_id={} flow_run_id={} stage={} error={}",
            flow_id,
            flow_run_id,
            stage,
            err
        );
        runtime_store::append_failed_pipeline_node_run(conn, flow_run_id, flow_id, "clip", err)?;
        update_flow_runtime_by_flow_id(
            conn,
            flow_id,
            &UpdateFlowRuntimeByAccountInput {
                status: Some("error".to_string()),
                current_node: Some(stage.to_string()),
                last_live_at: None,
                last_run_at: Some(now_timestamp_hcm()),
                last_error: Some(err.to_string()),
            },
        )?;
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        if let Some(session) = state.sessions_by_flow.get_mut(&flow_id) {
            session.mark_downstream_stage(flow_run_id, "error");
        }
        drop(state);
        self.emit_runtime_update_for_flow(conn, flow_id)
    }

    pub fn handle_live_detected(
        &self,
        conn: &Connection,
        flow_id: i64,
        live_status: &LiveStatus,
    ) -> Result<Option<i64>, String> {
        log::info!(
            "live runtime handle live detected flow_id={} room_id={} has_stream_url={} viewer_count={:?}",
            flow_id,
            live_status.room_id,
            live_status.stream_url.is_some(),
            live_status.viewer_count
        );
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
            log::info!(
                "live runtime skipped live detected flow_id={} reason=session_active active_flow_run_id={:?}",
                flow_id,
                session_active
            );
            return Ok(None);
        }
        if live_status.room_id.trim().is_empty() {
            log::warn!(
                "live runtime skipped live detected flow_id={} reason=empty_room_id",
                flow_id
            );
            return Ok(None);
        }
        let detected_at = now_timestamp_hcm();
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
        update_flow_runtime_by_flow_id(
            conn,
            flow_id,
            &UpdateFlowRuntimeByAccountInput {
                status: None,
                current_node: None,
                last_live_at: Some(detected_at.clone()),
                last_run_at: None,
                last_error: Some(String::new()),
            },
        )?;
        if !crate::live_runtime::session::should_start_new_run(
            Some(live_status.room_id.as_str()),
            last_completed_room_id.as_deref(),
            true,
        ) {
            log::info!(
                "live runtime skipped run creation flow_id={} room_id={} reason=dedupe",
                flow_id,
                live_status.room_id
            );
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
            log::warn!(
                "live runtime skipped run creation flow_id={} room_id={} reason=missing_stream_url",
                flow_id,
                live_status.room_id
            );
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

        let account_id = Self::resolve_account_id(resolve_or_create_account_for_username(
            conn,
            config.username.as_str(),
        )?);
        let output_json = serde_json::to_string(&start_node::StartOutput {
            account_id,
            username: config.username,
            room_id: live_status.room_id.clone(),
            stream_url: stream_url.to_string(),
            viewer_count: live_status.viewer_count,
            detected_at: detected_at.clone(),
        })
        .map_err(|e| e.to_string())?;
        let flow_run_id = runtime_store::create_run_with_completed_start_node(
            conn,
            flow_id,
            config.definition_version,
            output_json.as_str(),
        )?;
        log::info!(
            "live runtime flow run created flow_id={} flow_run_id={} account_id={} room_id={}",
            flow_id,
            flow_run_id,
            account_id,
            live_status.room_id
        );
        runtime_store::upsert_running_record_node_run(conn, flow_run_id, flow_id)?;
        let start_input = RecordingStartInput {
            account_id,
            flow_id,
            flow_run_id,
            room_id: live_status.room_id.clone(),
            stream_url: stream_url.to_string(),
            max_duration_seconds: load_record_duration_seconds(conn, flow_id)?,
            external_recording_id: worker::generate_external_recording_id(flow_run_id),
            storage_root: self
                .storage_root
                .as_ref()
                .ok_or_else(|| "missing storage_root for recording runtime".to_string())?
                .to_string_lossy()
                .into_owned(),
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
        log::info!(
            "live runtime recording row started flow_id={} flow_run_id={} external_recording_id={}",
            flow_id,
            flow_run_id,
            start_input.external_recording_id
        );
        drop(state);
        if self.auto_spawn_recording_execution {
            log::info!(
                "live runtime spawning recording execution flow_id={} flow_run_id={} external_recording_id={}",
                flow_id,
                flow_run_id,
                start_input.external_recording_id
            );
            self.spawn_recording_execution(start_input.clone(), cancel_flag)?;
        }
        update_flow_runtime_by_flow_id(
            conn,
            flow_id,
            &UpdateFlowRuntimeByAccountInput {
                status: Some("recording".to_string()),
                current_node: Some("record".to_string()),
                last_live_at: Some(detected_at),
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
        log::info!(
            "live runtime live detected handling completed flow_id={} flow_run_id={}",
            flow_id,
            flow_run_id
        );
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
        type RecordingFinalizeRow = (i64, i64, i64, i64, Option<String>, Option<String>);
        let row: Option<RecordingFinalizeRow> = conn
            .query_row(
                "SELECT id, account_id, flow_id, flow_run_id, room_id, file_path FROM recordings WHERE external_recording_id = ?1",
                [external_recording_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let Some((recording_id, account_id, flow_id, flow_run_id, existing_room_id, file_path)) =
            row
        else {
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
                let username = load_flow_runtime_config(conn, flow_id)?.username;
                let audio_handoff = self.run_record_node_post_processing(
                    conn,
                    recording_id,
                    flow_id,
                    flow_run_id,
                    external_recording_id,
                    file_path,
                );
                if !audio_handoff.completed_or_disabled() {
                    self.fail_downstream_processing_stage(
                        conn,
                        flow_id,
                        flow_run_id,
                        "record",
                        "Rust audio processing failed",
                    )?;
                    return Ok(());
                }
                if let Err(err) = self.run_clip_node(
                    conn,
                    flow_id,
                    flow_run_id,
                    external_recording_id,
                    account_id,
                    username.as_str(),
                    file_path,
                    audio_handoff.speech_segments_or_empty(),
                ) {
                    self.fail_downstream_processing_stage(
                        conn,
                        flow_id,
                        flow_run_id,
                        "clip",
                        err.as_str(),
                    )?;
                    return Ok(());
                }
                self.emit_runtime_update_for_flow(conn, flow_id)?;
                return Ok(());
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

    pub fn restart_active_run(&self, conn: &mut Connection, flow_id: i64) -> Result<(), String> {
        let poll_interval_seconds = load_flow_runtime_config(conn, flow_id)?.poll_interval_seconds;
        let active_recording = {
            let state = self.state.lock().map_err(|e| e.to_string())?;
            if !state.sessions_by_flow.contains_key(&flow_id) {
                return Err(format!("missing live runtime session for flow {flow_id}"));
            }
            state.active_recordings_by_flow.get(&flow_id).cloned()
        };
        let running_run_id = runtime_store::load_latest_running_flow_run_id(conn, flow_id)?;

        if let Some(handle) = &active_recording {
            handle.cancelled.store(true, Ordering::SeqCst);
            if let Some(cancel_process) = &handle.cancel_process {
                cancel_process()?;
            }
            self.finalize_recording_by_key(
                conn,
                &handle.external_recording_id,
                None,
                false,
                Some("Recording cancelled"),
            )?;
        }

        if let Some(run_id) = running_run_id {
            runtime_store::cancel_flow_run_by_id(conn, run_id, Some("Publish restart"))?;
        } else {
            runtime_store::cancel_latest_running_flow_run(conn, flow_id, Some("Publish restart"))?;
        }
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
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        let mut session = state
            .sessions_by_flow
            .remove(&flow_id)
            .ok_or_else(|| format!("missing live runtime session for flow {flow_id}"))?;
        session.mark_flow_run_stopped();
        let poll_task = Self::replace_poll_task_for_session(&mut state, flow_id, &mut session);
        state.sessions_by_flow.insert(flow_id, session);
        drop(state);
        let _ = self.log_runtime_event(
            flow_id,
            None,
            None,
            "session",
            "publish_restart_completed",
            FlowRuntimeLogLevel::Info,
            None,
            "Stopped current run after publishing changes",
            serde_json::json!({
                "flow_id": flow_id,
                "active_recording_cancelled": active_recording.is_some(),
            }),
        );
        self.spawn_poll_loop_worker(flow_id, poll_interval_seconds, poll_task);
        self.emit_runtime_update_for_flow(conn, flow_id)?;
        Ok(())
    }

    fn resolve_account_id(result: ResolveAccountResult) -> i64 {
        match result {
            ResolveAccountResult::Existing { account_id }
            | ResolveAccountResult::Created { account_id } => account_id,
        }
    }
}
