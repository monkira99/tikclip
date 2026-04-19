use crate::db::models::FlowNodeDefinition;
use serde::Deserialize;

use super::EngineNodeResult;

#[derive(Debug, Deserialize)]
struct RawRecordConfig {
    #[serde(default, alias = "maxDurationMinutes", alias = "maxDuration")]
    max_duration_minutes: Option<i64>,
    #[serde(default, alias = "maxDurationSeconds", alias = "durationSeconds")]
    max_duration_seconds: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RecordConfig {
    pub max_duration_minutes: i64,
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
    })
}

pub fn canonicalize_record_config_json(raw: &str) -> Result<String, String> {
    let cfg = parse_record_config(raw)?;
    serde_json::to_string(&serde_json::json!({
        "max_duration_minutes": cfg.max_duration_minutes,
    }))
    .map_err(|e| e.to_string())
}

fn default_max_duration_minutes() -> i64 {
    5
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
