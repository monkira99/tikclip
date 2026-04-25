import { invoke } from "@tauri-apps/api/core";

import type { ActiveRecordingStatus } from "@/types";

export async function listActiveRustRecordings(): Promise<ActiveRecordingStatus[]> {
  return invoke<ActiveRecordingStatus[]>("list_active_rust_recordings");
}
