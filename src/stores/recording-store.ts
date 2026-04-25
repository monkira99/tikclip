import { create } from "zustand";
import type { ActiveRecordingStatus } from "@/types";

type RecordingMap = Record<string, ActiveRecordingStatus>;

type RecordingStoreState = {
  recordings: RecordingMap;
  hydrateFromRuntime: (list: ActiveRecordingStatus[]) => void;
};

export const useRecordingStore = create<RecordingStoreState>((set) => ({
  recordings: {},

  hydrateFromRuntime: (list) =>
    set({
      recordings: Object.fromEntries(list.map((r) => [r.recording_id, r])),
    }),
}));

export function countActiveRecordings(recordings: RecordingMap): number {
  return Object.values(recordings).filter((r) => r.status === "pending" || r.status === "recording").length;
}
