#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveStatus {
    pub room_id: String,
    pub stream_url: Option<String>,
    pub viewer_count: Option<i64>,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveCheckConfig<'a> {
    pub username: &'a str,
    pub cookies_json: &'a str,
    pub proxy_url: Option<&'a str>,
    pub waf_bypass_enabled: bool,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedLiveStatus {
    pub is_live: bool,
    pub room_id: Option<String>,
    pub title: Option<String>,
    pub stream_url: Option<String>,
    pub viewer_count: Option<i64>,
}
