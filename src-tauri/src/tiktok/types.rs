#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveStatus {
    pub room_id: String,
    pub stream_url: Option<String>,
    pub viewer_count: Option<i64>,
}
