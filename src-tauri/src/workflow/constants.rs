pub const FLOW_NODE_KEYS: [&str; 5] = ["start", "record", "clip", "caption", "upload"];
pub const FLOW_STATUS_KEYS: [&str; 6] = [
    "idle",
    "watching",
    "recording",
    "processing",
    "error",
    "disabled",
];

pub fn is_valid_flow_node(node_key: &str) -> bool {
    FLOW_NODE_KEYS.contains(&node_key)
}

pub fn is_valid_flow_status(status: &str) -> bool {
    FLOW_STATUS_KEYS.contains(&status)
}
