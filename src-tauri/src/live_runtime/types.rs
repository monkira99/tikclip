use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRuntimeSessionSnapshot {
    pub flow_id: i64,
    pub flow_name: String,
    pub username: String,
    pub lookup_key: String,
    pub generation: u64,
    pub status: String,
    pub last_error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::LiveRuntimeSessionSnapshot;

    #[test]
    fn live_runtime_session_snapshot_exposes_minimal_debug_fields() {
        let snapshot = LiveRuntimeSessionSnapshot {
            flow_id: 11,
            flow_name: "Flow".to_string(),
            username: "shop_abc".to_string(),
            lookup_key: "shop_abc".to_string(),
            generation: 2,
            status: "watching".to_string(),
            last_error: None,
        };

        assert_eq!(snapshot.flow_id, 11);
        assert_eq!(snapshot.username, "shop_abc");
        assert_eq!(snapshot.generation, 2);
        assert_eq!(snapshot.status, "watching");
    }
}
