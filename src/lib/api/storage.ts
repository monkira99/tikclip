import { invoke } from "@tauri-apps/api/core";

export type StorageStats = {
  recordings_bytes: number;
  recordings_count: number;
  clips_bytes: number;
  clips_count: number;
  products_bytes: number;
  products_count: number;
  total_bytes: number;
  quota_bytes: number | null;
  usage_percent: number;
};

export async function getStorageStats(): Promise<StorageStats> {
  return invoke<StorageStats>("get_storage_stats");
}

export type StorageCleanupSummary = {
  deleted_recordings: number;
  deleted_clips: number;
  freed_bytes: number;
};

export async function runStorageCleanupNow(input: {
  raw_retention_days: number;
  archive_retention_days: number;
}): Promise<StorageCleanupSummary> {
  return invoke<StorageCleanupSummary>("run_storage_cleanup_now", { input });
}
