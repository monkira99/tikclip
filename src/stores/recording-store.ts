import { create } from "zustand";
import type { SidecarRecordingStatus } from "@/types";

type RecordingMap = Record<string, SidecarRecordingStatus>;

type RecordingStoreState = {
  recordings: RecordingMap;
  hydrateFromSidecar: (list: SidecarRecordingStatus[]) => void;
  updateRecording: (id: string, patch: Partial<SidecarRecordingStatus>) => void;
  upsertRecording: (row: SidecarRecordingStatus) => void;
  removeRecording: (id: string) => void;
};

function rowFromPayload(data: Record<string, unknown>): SidecarRecordingStatus | null {
  const recording_id = data.recording_id;
  if (typeof recording_id !== "string") {
    return null;
  }
  return {
    recording_id,
    account_id: typeof data.account_id === "number" ? data.account_id : Number(data.account_id) || 0,
    username: typeof data.username === "string" ? data.username : "",
    status: typeof data.status === "string" ? data.status : "",
    duration_seconds:
      typeof data.duration_seconds === "number" ? data.duration_seconds : Number(data.duration_seconds) || 0,
    file_size_bytes:
      typeof data.file_size_bytes === "number" ? data.file_size_bytes : Number(data.file_size_bytes) || 0,
    file_path: data.file_path != null ? String(data.file_path) : null,
    error_message: data.error_message != null ? String(data.error_message) : null,
  };
}

export const useRecordingStore = create<RecordingStoreState>((set) => ({
  recordings: {},

  hydrateFromSidecar: (list) =>
    set({
      recordings: Object.fromEntries(list.map((r) => [r.recording_id, r])),
    }),

  updateRecording: (id, patch) =>
    set((s) => {
      const cur = s.recordings[id];
      if (!cur) {
        return s;
      }
      return {
        recordings: { ...s.recordings, [id]: { ...cur, ...patch } },
      };
    }),

  upsertRecording: (row) =>
    set((s) => ({
      recordings: { ...s.recordings, [row.recording_id]: row },
    })),

  removeRecording: (id) =>
    set((s) => {
      const { [id]: _, ...rest } = s.recordings;
      return { recordings: rest };
    }),
}));

export function applyRecordingWsPayload(data: Record<string, unknown>): void {
  const row = rowFromPayload(data);
  if (!row) {
    return;
  }
  useRecordingStore.getState().upsertRecording(row);
}

export function countActiveRecordings(recordings: RecordingMap): number {
  return Object.values(recordings).filter((r) => r.status === "pending" || r.status === "recording").length;
}
