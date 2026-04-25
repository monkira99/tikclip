mod audio_processing;

pub use self::audio_processing::SpeechSpan;
use self::audio_processing::{
    process_recording_audio, replace_recording_speech_segments, AudioProcessingConfig,
};
use crate::db::models::FlowNodeDefinition;
use rusqlite::Connection;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use super::EngineNodeResult;

#[derive(Debug, Deserialize)]
struct RawRecordConfig {
    #[serde(default, alias = "maxDurationMinutes", alias = "maxDuration")]
    max_duration_minutes: Option<i64>,
    #[serde(default, alias = "maxDurationSeconds", alias = "durationSeconds")]
    max_duration_seconds: Option<i64>,
    #[serde(default)]
    speech_merge_gap_sec: Option<f32>,
    #[serde(default)]
    stt_num_threads: Option<i32>,
    #[serde(default)]
    stt_quantize: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RecordConfig {
    pub max_duration_minutes: i64,
    pub speech_merge_gap_sec: f32,
    pub stt_num_threads: i32,
    pub stt_quantize: String,
}

#[derive(Debug, Clone)]
pub struct RecordPostProcessInput {
    pub recording_id: i64,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RecordPostProcessOutput {
    pub audio_enabled: bool,
    pub speech_segments: Vec<SpeechSpan>,
}

impl RecordConfig {
    #[allow(dead_code)]
    pub fn max_duration_seconds(&self) -> i64 {
        self.max_duration_minutes * 60
    }
}

fn seconds_to_rounded_up_minutes(seconds: i64) -> i64 {
    (seconds.max(1) + 59) / 60
}

pub fn parse_record_config(raw: &str) -> Result<RecordConfig, String> {
    let cfg: RawRecordConfig = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let max_duration_minutes = if let Some(minutes) = cfg.max_duration_minutes {
        minutes
    } else if let Some(seconds) = cfg.max_duration_seconds {
        seconds_to_rounded_up_minutes(seconds)
    } else {
        default_max_duration_minutes()
    };

    Ok(RecordConfig {
        max_duration_minutes: max_duration_minutes.max(1),
        speech_merge_gap_sec: cfg.speech_merge_gap_sec.unwrap_or(0.5).max(0.0),
        stt_num_threads: cfg.stt_num_threads.unwrap_or(4).max(1),
        stt_quantize: normalize_stt_quantize(cfg.stt_quantize.as_deref()),
    })
}

pub fn canonicalize_record_config_json(raw: &str) -> Result<String, String> {
    let cfg = parse_record_config(raw)?;
    serde_json::to_string(&serde_json::json!({
        "max_duration_minutes": cfg.max_duration_minutes,
        "speech_merge_gap_sec": cfg.speech_merge_gap_sec,
        "stt_num_threads": cfg.stt_num_threads,
        "stt_quantize": cfg.stt_quantize,
    }))
    .map_err(|e| e.to_string())
}

fn default_max_duration_minutes() -> i64 {
    5
}

fn normalize_stt_quantize(value: Option<&str>) -> String {
    match value.unwrap_or("auto").trim().to_ascii_lowercase().as_str() {
        "fp32" | "float32" => "fp32".to_string(),
        "int8" => "int8".to_string(),
        _ => "auto".to_string(),
    }
}

pub fn run(def: &FlowNodeDefinition, input_json: Option<&str>) -> Result<EngineNodeResult, String> {
    let _ = parse_record_config(def.published_config_json.as_str())?;
    Ok(EngineNodeResult {
        status: "completed".to_string(),
        output_json: input_json.map(|x| x.to_string()),
        error: None,
        next_node: Some("clip".to_string()),
    })
}

pub fn run_post_record_audio(
    conn: &Connection,
    storage_root: &Path,
    config_json: &str,
    input: &RecordPostProcessInput,
) -> Result<RecordPostProcessOutput, String> {
    let record_config = parse_record_config(config_json)?;
    let config = AudioProcessingConfig::from_record_config(conn, storage_root, &record_config)?;
    if !config.enabled {
        return Ok(RecordPostProcessOutput {
            audio_enabled: false,
            speech_segments: Vec::new(),
        });
    }

    let spans = process_recording_audio(input.file_path.as_path(), &config)?;
    replace_recording_speech_segments(conn, input.recording_id, &spans)?;
    Ok(RecordPostProcessOutput {
        audio_enabled: true,
        speech_segments: spans,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_record_config;

    #[test]
    fn parse_record_config_converts_minutes_to_runtime_seconds() {
        let cfg = parse_record_config(r#"{"max_duration_minutes":5}"#).unwrap();

        assert_eq!(cfg.max_duration_minutes, 5);
        assert_eq!(cfg.max_duration_seconds(), 300);
    }

    #[test]
    fn parse_record_config_accepts_legacy_duration_keys() {
        let seconds_cfg = parse_record_config(r#"{"maxDurationSeconds":600}"#).unwrap();
        let minutes_cfg = parse_record_config(r#"{"maxDuration":7}"#).unwrap();
        let rounded_cfg = parse_record_config(r#"{"durationSeconds":61}"#).unwrap();

        assert_eq!(seconds_cfg.max_duration_minutes, 10);
        assert_eq!(minutes_cfg.max_duration_minutes, 7);
        assert_eq!(rounded_cfg.max_duration_minutes, 2);
    }
}
