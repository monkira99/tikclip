import { invoke } from "@tauri-apps/api/core";

import type { ActiveRecordingStatus } from "@/types";

export async function listActiveRustRecordings(): Promise<ActiveRecordingStatus[]> {
  return invoke<ActiveRecordingStatus[]>("list_active_rust_recordings");
}

export async function stopRustRecording(recordingId: string): Promise<void> {
  await invoke("stop_rust_recording", { recordingId });
}

export async function finalizeRustRecordingRuntime(input: {
  external_recording_id: string;
  worker_status: string;
  room_id?: string | null;
  error_message?: string | null;
}): Promise<void> {
  await invoke("finalize_rust_recording_runtime", {
    input: {
      external_recording_id: input.external_recording_id,
      worker_status: input.worker_status,
      room_id: input.room_id === undefined || input.room_id === null ? undefined : input.room_id,
      error_message:
        input.error_message === undefined || input.error_message === null
          ? undefined
          : input.error_message,
    },
  });
}
