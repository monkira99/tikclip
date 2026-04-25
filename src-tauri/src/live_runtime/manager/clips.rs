use super::store::{load_published_node_config, load_sidecar_base_url};
use super::LiveRuntimeManager;
use crate::live_runtime::logs::FlowRuntimeLogLevel;
use crate::workflow::clip_node::{self, ClipStageInput};
use crate::workflow::record_node::SpeechSpan;
use rusqlite::Connection;
use std::path::Path;

pub(super) enum ClipNodeExecutionHandoff {
    Completed,
    FallbackToSidecar,
}

impl LiveRuntimeManager {
    #[expect(
        clippy::too_many_arguments,
        reason = "Structured handoff logging keeps runtime identifiers explicit at the sidecar boundary"
    )]
    pub(super) fn handoff_recording_to_sidecar_processing(
        &self,
        conn: &Connection,
        flow_id: i64,
        flow_run_id: i64,
        external_recording_id: &str,
        account_id: i64,
        username: &str,
        file_path: &str,
        speech_segments: Option<&[SpeechSpan]>,
    ) -> Result<(), String> {
        let Some(sidecar_base_url) = load_sidecar_base_url(conn)? else {
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
        let mut body = serde_json::json!({
            "recording_id": external_recording_id,
            "username": username,
            "file_path": file_path,
            "account_id": account_id,
        });
        if let Ok(raw_clip_config) = load_published_node_config(conn, flow_id, "clip") {
            if let Ok(clip_config) = clip_node::parse_clip_config(raw_clip_config.as_str()) {
                body["clip_min_duration"] = serde_json::json!(clip_config.clip_min_duration);
                body["clip_max_duration"] = serde_json::json!(clip_config.clip_max_duration);
                body["scene_threshold"] = serde_json::json!(clip_config.scene_threshold);
                body["speech_cut_tolerance_sec"] =
                    serde_json::json!(clip_config.speech_cut_tolerance_sec);
            }
        }
        if let Some(segments) = speech_segments {
            body["speech_segments"] = serde_json::to_value(segments).map_err(|e| e.to_string())?;
        }
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

    #[expect(
        clippy::too_many_arguments,
        reason = "Runtime logging keeps clip handoff identifiers explicit at the processing boundary"
    )]
    pub(super) fn run_clip_node_or_fallback(
        &self,
        conn: &Connection,
        flow_id: i64,
        flow_run_id: i64,
        external_recording_id: &str,
        account_id: i64,
        username: &str,
        file_path: &str,
        speech_segments: &[SpeechSpan],
    ) -> ClipNodeExecutionHandoff {
        let Some(storage_root) = self.storage_root.as_deref() else {
            let _ = self.log_runtime_event(
                flow_id,
                Some(flow_run_id),
                Some(external_recording_id),
                "clip",
                "rust_clip_processing_failed",
                FlowRuntimeLogLevel::Warn,
                Some("clip.storage_root_missing"),
                "Skipped Rust clip processing because storage root is unavailable",
                serde_json::json!({ "file_path_present": true }),
            );
            return ClipNodeExecutionHandoff::FallbackToSidecar;
        };
        let _ = self.log_runtime_event(
            flow_id,
            Some(flow_run_id),
            Some(external_recording_id),
            "clip",
            "rust_clip_processing_started",
            FlowRuntimeLogLevel::Info,
            None,
            "Started Rust clip processing",
            serde_json::json!({
                "file_path_present": true,
                "speech_segments": speech_segments.len(),
            }),
        );

        match clip_node::run_clip_stage(
            conn,
            self.app_handle.as_ref(),
            storage_root,
            load_published_node_config(conn, flow_id, "clip")
                .unwrap_or_else(|_| "{}".to_string())
                .as_str(),
            ClipStageInput {
                external_recording_id: external_recording_id.to_string(),
                account_id,
                username: username.to_string(),
                file_path: Path::new(file_path).to_path_buf(),
                speech_segments: speech_segments.to_vec(),
            },
        ) {
            Ok(result) => {
                let _ = self.log_runtime_event(
                    flow_id,
                    Some(flow_run_id),
                    Some(external_recording_id),
                    "clip",
                    "rust_clip_processing_completed",
                    FlowRuntimeLogLevel::Info,
                    None,
                    "Completed Rust clip processing",
                    serde_json::json!({ "clips": result.clips.len() }),
                );
                ClipNodeExecutionHandoff::Completed
            }
            Err(err) => {
                let _ = self.log_runtime_event(
                    flow_id,
                    Some(flow_run_id),
                    Some(external_recording_id),
                    "clip",
                    "rust_clip_processing_failed",
                    FlowRuntimeLogLevel::Warn,
                    Some("clip.processing_failed"),
                    "Rust clip processing failed; sidecar clip fallback remains enabled",
                    serde_json::json!({ "error": err }),
                );
                ClipNodeExecutionHandoff::FallbackToSidecar
            }
        }
    }
}
