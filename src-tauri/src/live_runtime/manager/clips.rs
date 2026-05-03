use super::store::load_published_node_config;
use super::LiveRuntimeManager;
use crate::live_runtime::logs::FlowRuntimeLogLevel;
use crate::workflow::clip_node::{self, ClipStageInput};
use crate::workflow::record_node::SpeechSpan;
use rusqlite::Connection;
use std::path::Path;

impl LiveRuntimeManager {
    #[expect(
        clippy::too_many_arguments,
        reason = "Runtime logging keeps clip handoff identifiers explicit at the processing boundary"
    )]
    pub(super) fn run_clip_node(
        &self,
        conn: &Connection,
        flow_id: i64,
        flow_run_id: i64,
        external_recording_id: &str,
        account_id: i64,
        username: &str,
        file_path: &str,
        speech_segments: &[SpeechSpan],
    ) -> Result<(), String> {
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
            return Err("storage root is unavailable".to_string());
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
                Ok(())
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
                    "Rust clip processing failed",
                    serde_json::json!({ "error": err }),
                );
                Err(err)
            }
        }
    }
}
