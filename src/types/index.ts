export type AccountType = "own" | "monitored";
export type AccountStatus = "live" | "offline" | "recording";
export type RecordingStatus = "recording" | "done" | "error" | "processing";
export type ClipStatus = "draft" | "ready" | "posted" | "archived";
export type SceneType = "product_intro" | "highlight" | "general";

export interface Account {
  id: number;
  username: string;
  display_name: string;
  avatar_url: string | null;
  type: AccountType;
  tiktok_uid: string | null;
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
  notes: string | null;
  created_at: string;
  updated_at: string;
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
