export type AccountType = "own" | "monitored";
export type AccountStatus = "live" | "offline" | "recording";
export type RecordingStatus = "recording" | "done" | "error" | "processing";
export type ClipStatus = "draft" | "ready" | "posted" | "archived";
export type ClipCaptionStatus = "pending" | "generating" | "completed" | "failed";
export type SceneType = "product_intro" | "highlight" | "general";
export type FlowNodeKey = "start" | "record" | "clip" | "caption" | "upload";
export type FlowStatus = "idle" | "watching" | "recording" | "processing" | "error" | "disabled";

export interface Account {
  id: number;
  username: string;
  display_name: string;
  avatar_url: string | null;
  type: AccountType;
  tiktok_uid: string | null;
  cookies_json: string | null;
  auto_record: boolean;
  auto_record_schedule: AutoRecordSchedule | null;
  priority: number;
  is_live: boolean;
  last_live_at: string | null;
  last_checked_at: string | null;
  proxy_url: string | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateAccountInput {
  username: string;
  display_name: string;
  type: AccountType;
  cookies_json?: string | null;
  proxy_url?: string | null;
  auto_record: boolean;
  priority: number;
  notes?: string | null;
}

export interface AutoRecordSchedule {
  days: number[];
  start_time: string;
  end_time: string;
}

export interface Recording {
  id: number;
  account_id: number;
  account_username?: string;
  room_id: string | null;
  status: RecordingStatus;
  started_at: string;
  ended_at: string | null;
  duration_seconds: number;
  file_path: string | null;
  file_size_bytes: number;
  stream_url: string | null;
  bitrate: string | null;
  error_message: string | null;
  auto_process: boolean;
  created_at: string;
}

export interface Clip {
  id: number;
  recording_id: number;
  account_id: number;
  account_username?: string;
  title: string | null;
  file_path: string;
  thumbnail_path: string | null;
  duration_seconds: number;
  file_size_bytes: number;
  start_time: number;
  end_time: number;
  status: ClipStatus;
  quality_score: number | null;
  scene_type: SceneType | null;
  ai_tags_json: string | null;
  notes: string | null;
  flow_id: number | null;
  transcript_text?: string | null;
  caption_text: string | null;
  caption_status: ClipCaptionStatus;
  caption_error: string | null;
  caption_generated_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface FlowNodeConfig {
  id: number;
  flow_id: number;
  node_key: FlowNodeKey;
  config_json: string;
  updated_at: string;
}

export interface FlowSummary {
  id: number;
  account_id: number;
  account_username: string;
  name: string;
  enabled: boolean;
  status: FlowStatus;
  current_node: FlowNodeKey | null;
  last_live_at: string | null;
  last_run_at: string | null;
  last_error: string | null;
  recordings_count: number;
  clips_count: number;
  captions_count: number;
  created_at: string;
  updated_at: string;
}

export interface FlowDetail {
  flow: {
    id: number;
    account_id: number;
    name: string;
    enabled: boolean;
    status: FlowStatus;
    current_node: FlowNodeKey | null;
    last_live_at: string | null;
    last_run_at: string | null;
    last_error: string | null;
    created_at: string;
    updated_at: string;
  };
  node_configs: FlowNodeConfig[];
  recordings_count: number;
  clips_count: number;
}

export interface CreateFlowInput {
  account_id: number;
  name: string;
  enabled?: boolean;
}

/** Row returned by sidecar `GET /api/recording/status` and recording WebSocket payloads. */
export interface SidecarRecordingStatus {
  recording_id: string;
  account_id: number;
  username: string;
  status: string;
  duration_seconds: number;
  file_size_bytes: number;
  file_path: string | null;
  error_message: string | null;
}

export interface SidecarStatus {
  connected: boolean;
  port: number | null;
  active_recordings: number;
}

export interface WsEvent {
  type: string;
  data: Record<string, unknown>;
  timestamp: number;
}

export interface Product {
  id: number;
  name: string;
  description: string | null;
  sku: string | null;
  image_url: string | null;
  tiktok_shop_id: string | null;
  tiktok_url: string | null;
  price: number | null;
  category: string | null;
  /** JSON array: { kind, path, source_url }[] for downloaded gallery / videos */
  media_files_json: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateProductInput {
  name: string;
  description?: string | null;
  sku?: string | null;
  image_url?: string | null;
  tiktok_shop_id?: string | null;
  tiktok_url?: string | null;
  price?: number | null;
  category?: string | null;
  media_files_json?: string | null;
}

export interface UpdateProductInput {
  name?: string;
  description?: string | null;
  sku?: string | null;
  image_url?: string | null;
  tiktok_shop_id?: string | null;
  tiktok_url?: string | null;
  price?: number | null;
  category?: string | null;
  media_files_json?: string | null;
}

export interface ClipFilters {
  status: ClipStatus | "all";
  accountId: number | null;
  sceneType: SceneType | "all";
  dateFrom: string | null;
  dateTo: string | null;
  search: string;
  sortBy: "created_at" | "duration" | "file_size" | "title";
  sortOrder: "asc" | "desc";
}

export type ViewMode = "grid" | "list";
