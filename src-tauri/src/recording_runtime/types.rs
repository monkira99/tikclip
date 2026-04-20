#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingStartInput {
    pub account_id: i64,
    pub flow_id: i64,
    pub flow_run_id: i64,
    pub room_id: String,
    pub stream_url: String,
    pub max_duration_seconds: i64,
    pub external_recording_id: String,
    pub storage_root: String,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingFinishInput {
    pub account_id: i64,
    pub flow_id: i64,
    pub flow_run_id: i64,
    pub external_recording_id: String,
    pub room_id: String,
    pub file_path: Option<String>,
    pub error_message: Option<String>,
    pub duration_seconds: i64,
    pub file_size_bytes: i64,
    pub outcome: RecordingOutcome,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingOutcome {
    Success,
    Failed,
    Cancelled,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingExecution {
    pub start: RecordingStartInput,
    pub output_path: String,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustRecordingUpsertInput {
    pub account_id: i64,
    pub flow_id: i64,
    pub flow_run_id: Option<i64>,
    pub external_recording_id: String,
    pub runtime_status: String,
    pub room_id: Option<String>,
    pub file_path: Option<String>,
    pub error_message: Option<String>,
    pub duration_seconds: i64,
    pub file_size_bytes: i64,
}
