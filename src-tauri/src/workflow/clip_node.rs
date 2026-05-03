mod clip_processing;
mod common;
pub mod product_suggest;
pub mod product_vectors;

use self::clip_processing::{
    process_recording_clips, ClipProcessingConfig, ClipProcessingInput, ClipProcessingResult,
};
use crate::db::models::FlowNodeDefinition;
use rusqlite::Connection;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

use super::{record_node::SpeechSpan, EngineNodeResult};

#[derive(Debug, Deserialize)]
struct RawClipConfig {
    #[serde(default)]
    clip_min_duration: Option<i64>,
    #[serde(default)]
    clip_max_duration: Option<i64>,
    #[serde(default)]
    scene_threshold: Option<f64>,
    #[serde(default)]
    speech_cut_tolerance_sec: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ClipConfig {
    pub clip_min_duration: i64,
    pub clip_max_duration: i64,
    pub scene_threshold: f64,
    pub speech_cut_tolerance_sec: f64,
}

#[derive(Debug, Clone)]
pub struct ClipStageInput {
    pub external_recording_id: String,
    pub account_id: i64,
    pub username: String,
    pub file_path: PathBuf,
    pub speech_segments: Vec<SpeechSpan>,
}

pub fn parse_clip_config(raw: &str) -> Result<ClipConfig, String> {
    let cfg: RawClipConfig = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let min = cfg.clip_min_duration.unwrap_or(15).max(1);
    Ok(ClipConfig {
        clip_min_duration: min,
        clip_max_duration: cfg.clip_max_duration.unwrap_or(90).max(min),
        scene_threshold: cfg.scene_threshold.unwrap_or(30.0),
        speech_cut_tolerance_sec: cfg.speech_cut_tolerance_sec.unwrap_or(1.5).max(0.0),
    })
}

pub fn canonicalize_clip_config_json(raw: &str) -> Result<String, String> {
    let cfg = parse_clip_config(raw)?;
    serde_json::to_string(&serde_json::json!({
        "clip_min_duration": cfg.clip_min_duration,
        "clip_max_duration": cfg.clip_max_duration,
        "scene_threshold": cfg.scene_threshold,
        "speech_cut_tolerance_sec": cfg.speech_cut_tolerance_sec,
    }))
    .map_err(|e| e.to_string())
}

pub fn run(def: &FlowNodeDefinition, input_json: Option<&str>) -> Result<EngineNodeResult, String> {
    let _ = parse_clip_config(def.published_config_json.as_str())?;
    Ok(EngineNodeResult {
        status: "completed".to_string(),
        output_json: input_json.map(|x| x.to_string()),
        error: None,
        next_node: Some("caption".to_string()),
    })
}

pub fn run_clip_stage(
    conn: &Connection,
    app_handle: Option<&AppHandle>,
    storage_root: &Path,
    config_json: &str,
    input: ClipStageInput,
) -> Result<ClipProcessingResult, String> {
    let clip_config = parse_clip_config(config_json)?;
    let config = ClipProcessingConfig::from_clip_config(storage_root, &clip_config);
    process_recording_clips(
        conn,
        app_handle,
        &ClipProcessingInput {
            external_recording_id: input.external_recording_id,
            account_id: input.account_id,
            username: input.username,
            source_path: input.file_path,
            speech_segments: input.speech_segments,
        },
        &config,
    )
}
