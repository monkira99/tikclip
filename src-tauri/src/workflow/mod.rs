//! Workflow engine (fixed-node progression). Sidecar dispatch lands in later tasks.

pub mod types;

pub mod caption_node;
pub mod clip_node;
pub mod constants;
pub mod node_runner;
pub mod record_node;
pub mod runtime_store;
pub mod start_node;

#[derive(Debug, Clone)]
pub struct EngineNodeResult {
    pub status: String,
    pub output_json: Option<String>,
    pub error: Option<String>,
    pub next_node: Option<String>,
}
