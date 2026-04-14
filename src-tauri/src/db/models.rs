use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Account {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    #[serde(rename = "type")]
    pub account_type: String, // "own" | "monitored"
    pub tiktok_uid: Option<String>,
    pub cookies_json: Option<String>,
    pub proxy_url: Option<String>,
    pub auto_record: bool,
    pub auto_record_schedule: Option<String>, // JSON string
    pub priority: i32,
    pub is_live: bool,
    pub last_live_at: Option<String>,
    pub last_checked_at: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Recording {
    pub id: i64,
    pub account_id: i64,
    pub account_username: Option<String>,
    pub room_id: Option<String>,
    pub status: String, // "recording" | "done" | "error" | "processing"
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: i64,
    pub file_path: Option<String>,
    pub file_size_bytes: i64,
    pub stream_url: Option<String>,
    pub bitrate: Option<String>,
    pub error_message: Option<String>,
    pub auto_process: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Clip {
    pub id: i64,
    pub recording_id: i64,
    pub account_id: i64,
    pub account_username: Option<String>,
    pub title: Option<String>,
    pub file_path: String,
    pub thumbnail_path: Option<String>,
    pub duration_seconds: i64,
    pub file_size_bytes: i64,
    pub start_time: f64,
    pub end_time: f64,
    pub status: String, // "draft" | "ready" | "posted" | "archived"
    pub quality_score: Option<f64>,
    pub scene_type: Option<String>,
    pub ai_tags_json: Option<String>,
    pub notes: Option<String>,
    pub flow_id: Option<i64>,
    pub transcript_text: Option<String>,
    pub caption_text: Option<String>,
    pub caption_status: String,
    pub caption_error: Option<String>,
    pub caption_generated_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Flow {
    pub id: i64,
    pub account_id: i64,
    pub name: String,
    pub enabled: bool,
    pub status: String,
    pub current_node: Option<String>,
    pub last_live_at: Option<String>,
    pub last_run_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FlowNodeConfig {
    pub id: i64,
    pub flow_id: i64,
    pub node_key: String,
    pub config_json: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpeechSegment {
    pub id: i64,
    pub recording_id: i64,
    pub start_time: f64,
    pub end_time: f64,
    pub text: String,
    pub confidence: Option<f64>,
    pub created_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct Notification {
    pub id: i64,
    pub notification_type: String,
    pub title: String,
    pub message: String,
    pub account_id: Option<i64>,
    pub recording_id: Option<i64>,
    pub clip_id: Option<i64>,
    pub is_read: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Product {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub sku: Option<String>,
    pub image_url: Option<String>,
    pub tiktok_shop_id: Option<String>,
    pub tiktok_url: Option<String>,
    pub price: Option<f64>,
    pub category: Option<String>,
    pub media_files_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
