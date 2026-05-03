#![expect(
    dead_code,
    reason = "Task 1 adds runtime log foundation that is wired into manager and commands in later tasks"
)]

use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{FixedOffset, SecondsFormat, Utc};

pub const DEFAULT_FLOW_RUNTIME_LOG_BUFFER_CAPACITY: usize = 500;

static FLOW_RUNTIME_LOG_ID_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowRuntimeLogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct FlowRuntimeLogEntry {
    pub id: String,
    pub timestamp: String,
    pub level: FlowRuntimeLogLevel,
    pub flow_id: i64,
    pub flow_run_id: Option<i64>,
    pub external_recording_id: Option<String>,
    pub stage: String,
    pub event: String,
    pub code: Option<String>,
    pub message: String,
    pub context: Value,
}

impl FlowRuntimeLogEntry {
    #[expect(
        clippy::too_many_arguments,
        reason = "Canonical runtime log shape keeps constructor inputs explicit across Rust and UI boundaries"
    )]
    pub fn new(
        flow_id: i64,
        flow_run_id: Option<i64>,
        external_recording_id: Option<&str>,
        stage: &str,
        event: &str,
        level: FlowRuntimeLogLevel,
        code: Option<&str>,
        message: &str,
        context: Value,
    ) -> Self {
        let timestamp = now_runtime_log_timestamp_hcm();
        let sequence = FLOW_RUNTIME_LOG_ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);

        Self {
            id: format!("log-{flow_id}-{timestamp}-{sequence}"),
            timestamp,
            level,
            flow_id,
            flow_run_id,
            external_recording_id: external_recording_id.map(str::to_string),
            stage: stage.to_string(),
            event: event.to_string(),
            code: code.map(str::to_string),
            message: message.to_string(),
            context: redact_sensitive_context(&context),
        }
    }
}

fn now_runtime_log_timestamp_hcm() -> String {
    let offset = FixedOffset::east_opt(7 * 3600).expect("GMT+7 offset");
    Utc::now()
        .with_timezone(&offset)
        .to_rfc3339_opts(SecondsFormat::Millis, false)
}

fn is_sensitive_context_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    normalized.contains("cookie")
        || normalized.contains("token")
        || normalized.contains("authorization")
        || normalized.contains("secret")
        || normalized.contains("password")
        || normalized == "stream_url"
        || normalized == "proxy_url"
}

fn redact_sensitive_context(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    let next_value = if is_sensitive_context_key(key) {
                        Value::String("[redacted]".to_string())
                    } else {
                        redact_sensitive_context(value)
                    };
                    (key.clone(), next_value)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(redact_sensitive_context).collect()),
        _ => value.clone(),
    }
}

pub fn format_terminal_context(context: &Value) -> String {
    redact_sensitive_context(context).to_string()
}

pub fn format_terminal_entry(entry: &FlowRuntimeLogEntry) -> String {
    format!(
        "flow_runtime {}",
        serde_json::json!({
            "timestamp": entry.timestamp,
            "level": entry.level,
            "flow_id": entry.flow_id,
            "flow_run_id": entry.flow_run_id,
            "external_recording_id": entry.external_recording_id,
            "stage": entry.stage,
            "event": entry.event,
            "code": entry.code,
            "message": entry.message,
            "context": redact_sensitive_context(&entry.context),
        })
    )
}

#[derive(Debug, Clone)]
pub struct FlowRuntimeLogBuffer {
    cap: usize,
    entries: VecDeque<FlowRuntimeLogEntry>,
}

impl Default for FlowRuntimeLogBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_FLOW_RUNTIME_LOG_BUFFER_CAPACITY)
    }
}

impl FlowRuntimeLogBuffer {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            entries: VecDeque::with_capacity(cap),
        }
    }

    pub fn push(&mut self, entry: FlowRuntimeLogEntry) {
        self.entries.push_back(entry);
        while self.entries.len() > self.cap {
            self.entries.pop_front();
        }
    }

    pub fn entries(&self) -> Vec<FlowRuntimeLogEntry> {
        self.entries.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{FlowRuntimeLogBuffer, FlowRuntimeLogEntry, FlowRuntimeLogLevel};
    use serde_json::json;
    #[test]
    fn flow_runtime_log_entry_keeps_required_fields() {
        let entry = FlowRuntimeLogEntry::new(
            7,
            Some(42),
            Some("rec-42-a1b2"),
            "record",
            "record_spawned",
            FlowRuntimeLogLevel::Info,
            None,
            "Spawned Rust-owned recording worker",
            json!({"room_id":"7312345"}),
        );

        assert_eq!(entry.flow_id, 7);
        assert_eq!(entry.flow_run_id, Some(42));
        assert_eq!(entry.external_recording_id.as_deref(), Some("rec-42-a1b2"));
        assert_eq!(entry.stage, "record");
        assert_eq!(entry.event, "record_spawned");
    }

    #[test]
    fn flow_runtime_log_entry_keeps_external_recording_id_when_present() {
        let entry = FlowRuntimeLogEntry::new(
            7,
            Some(42),
            Some("rec-42-a1b2"),
            "record",
            "record_spawned",
            FlowRuntimeLogLevel::Info,
            None,
            "Spawned Rust-owned recording worker",
            json!({"room_id":"7312345"}),
        );

        assert_eq!(entry.external_recording_id.as_deref(), Some("rec-42-a1b2"));
    }

    #[test]
    fn flow_runtime_log_entry_serializes_with_snake_case_fields() {
        let entry = test_entry("serialized");
        let value = serde_json::to_value(&entry).expect("entry should serialize");

        assert!(value.get("flow_id").is_some());
        assert!(value.get("flow_run_id").is_some());
        assert!(value.get("external_recording_id").is_some());
        assert!(value.get("flowId").is_none());
        assert!(value.get("flowRunId").is_none());
    }

    #[test]
    fn ring_buffer_keeps_only_latest_entries_with_cap() {
        let mut buffer = FlowRuntimeLogBuffer::new(2);
        buffer.push(test_entry("first"));
        buffer.push(test_entry("second"));
        buffer.push(test_entry("third"));

        let entries = buffer.entries();
        let messages = entries
            .iter()
            .map(|entry| entry.message.as_str())
            .collect::<Vec<_>>();
        assert_eq!(messages, vec!["second", "third"]);
    }

    #[test]
    fn flow_runtime_log_entry_generates_unique_ids_for_back_to_back_same_flow_entries() {
        let first = test_entry("first");
        let second = test_entry("second");

        assert_ne!(first.id, second.id);
    }

    #[test]
    fn flow_runtime_log_entry_timestamp_includes_millisecond_precision() {
        let entry = test_entry("millis");

        assert!(entry.timestamp.contains('.'));
        assert!(entry.timestamp.contains('+'));
    }

    #[test]
    fn format_terminal_context_redacts_sensitive_fields() {
        let formatted = super::format_terminal_context(&json!({
            "username": "shop_abc",
            "stream_url": "https://example.com/live.flv",
            "has_stream_url": true,
            "cookies_json": "very-secret",
            "token": "abc123",
            "nested": {
                "authorization": "Bearer secret",
                "keep": true
            }
        }));

        assert!(formatted.contains("shop_abc"));
        assert!(formatted.contains("\"stream_url\":\"[redacted]\""));
        assert!(formatted.contains("\"has_stream_url\":true"));
        assert!(formatted.contains("\"cookies_json\":\"[redacted]\""));
        assert!(formatted.contains("\"token\":\"[redacted]\""));
        assert!(formatted.contains("\"authorization\":\"[redacted]\""));
        assert!(!formatted.contains("very-secret"));
        assert!(!formatted.contains("abc123"));
        assert!(!formatted.contains("Bearer secret"));
    }

    #[test]
    fn flow_runtime_log_entry_stores_redacted_context_for_all_sinks() {
        let entry = FlowRuntimeLogEntry::new(
            7,
            Some(42),
            Some("rec-42-a1b2"),
            "record",
            "handoff_failed",
            FlowRuntimeLogLevel::Warn,
            Some("runtime.handoff_failed"),
            "Runtime handoff failed",
            json!({
                "username": "shop_abc",
                "stream_url": "https://example.com/live.flv",
                "has_stream_url": true,
                "cookies_json": "very-secret",
                "nested": {
                    "authorization": "Bearer secret",
                    "reason": "port_missing"
                }
            }),
        );

        assert_eq!(entry.context["username"], json!("shop_abc"));
        assert_eq!(entry.context["stream_url"], json!("[redacted]"));
        assert_eq!(entry.context["has_stream_url"], json!(true));
        assert_eq!(entry.context["cookies_json"], json!("[redacted]"));
        assert_eq!(
            entry.context["nested"]["authorization"],
            json!("[redacted]")
        );
        assert_eq!(entry.context["nested"]["reason"], json!("port_missing"));
        assert_ne!(
            entry.context["stream_url"],
            json!("https://example.com/live.flv")
        );
        assert_ne!(entry.context["cookies_json"], json!("very-secret"));
        assert_ne!(
            entry.context["nested"]["authorization"],
            json!("Bearer secret")
        );
    }

    #[test]
    fn format_terminal_entry_includes_structured_runtime_context() {
        let entry = FlowRuntimeLogEntry::new(
            7,
            Some(42),
            Some("rec-42-a1b2"),
            "record",
            "handoff_failed",
            FlowRuntimeLogLevel::Warn,
            Some("runtime.handoff_failed"),
            "Runtime handoff failed",
            json!({
                "username": "shop_abc",
                "active_flow_run_id": 42,
                "reason": "port_missing",
                "stream_url": "https://example.com/live.flv"
            }),
        );

        let formatted = super::format_terminal_entry(&entry);

        assert!(formatted.contains("\"flow_id\":7"));
        assert!(formatted.contains("\"flow_run_id\":42"));
        assert!(formatted.contains("\"stage\":\"record\""));
        assert!(formatted.contains("\"event\":\"handoff_failed\""));
        assert!(formatted.contains("\"external_recording_id\":\"rec-42-a1b2\""));
        assert!(formatted.contains("\"reason\":\"port_missing\""));
        assert!(formatted.contains("\"stream_url\":\"[redacted]\""));
    }

    fn test_entry(message: &str) -> FlowRuntimeLogEntry {
        FlowRuntimeLogEntry::new(
            1,
            None,
            None,
            "runtime",
            "session_started",
            FlowRuntimeLogLevel::Info,
            None,
            message,
            json!({}),
        )
    }
}
