use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowRun {
    pub id: i64,
    pub flow_id: i64,
    pub definition_version: i64,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub trigger_reason: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowNodeRun {
    pub id: i64,
    pub flow_run_id: i64,
    pub flow_id: i64,
    pub node_key: String,
    pub status: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub input_json: Option<String>,
    pub output_json: Option<String>,
    pub error: Option<String>,
}
