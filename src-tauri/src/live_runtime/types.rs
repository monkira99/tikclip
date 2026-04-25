use serde::Serialize;

/// Summary snapshot for live runtime session state.
/// Structured runtime logs live in `live_runtime::logs` to keep the summary path separate.

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
    pub last_checked_at: Option<String>,
    pub last_check_live: Option<bool>,
    pub next_poll_at: Option<String>,
    pub poll_interval_seconds: Option<i64>,
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
            last_checked_at: Some("2026-04-25 10:00:00".to_string()),
            last_check_live: Some(false),
            next_poll_at: Some("2026-04-25 10:01:00".to_string()),
            poll_interval_seconds: Some(60),
        };

        assert_eq!(snapshot.flow_id, 11);
        assert_eq!(snapshot.username, "shop_abc");
        assert_eq!(snapshot.generation, 2);
        assert_eq!(snapshot.status, "watching");
        assert_eq!(snapshot.last_check_live, Some(false));
    }
}
