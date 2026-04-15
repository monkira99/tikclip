import { RecordingControls } from "@/components/recordings/recording-controls";
import { RecordingProgress } from "@/components/recordings/recording-progress";
import { useAppStore } from "@/stores/app-store";
import { useRecordingStore } from "@/stores/recording-store";
import type { SidecarRecordingStatus } from "@/types";

type RecordingListProps = {
  recordings?: SidecarRecordingStatus[];
  sidecarConnected?: boolean;
  mode?: "active" | "all";
  emptyMessage?: string;
  showControls?: boolean;
};

export function RecordingList({
  recordings,
  sidecarConnected: sidecarConnectedProp,
  mode = "active",
  emptyMessage,
  showControls = true,
}: RecordingListProps = {}) {
  const sidecarConnectedFromStore = useAppStore((s) => s.sidecarConnected);
  const storeRecordings = useRecordingStore((s) => s.recordings);

  const resolvedSidecarConnected = sidecarConnectedProp ?? sidecarConnectedFromStore;
  const source = recordings ?? Object.values(storeRecordings);

  const active = source.filter(
    (r) => r.status === "pending" || r.status === "recording",
  );
  const visible = mode === "active" ? active : source;
  const resolvedEmptyMessage =
    emptyMessage ??
    (mode === "active"
      ? "No active recordings. Start one from the sidecar API or account watcher."
      : "No recordings for this flow yet.");

  if (!recordings && !resolvedSidecarConnected) {
    return (
      <p className="text-sm text-[var(--color-text-muted)]">
        Connect to the sidecar to see live recordings. Ensure the Python sidecar is running and reachable.
      </p>
    );
  }

  if (visible.length === 0) {
    return (
      <p className="text-sm text-[var(--color-text-muted)]">
        {resolvedEmptyMessage}
      </p>
    );
  }

  return (
    <div className="grid gap-4 md:grid-cols-2">
      {visible.map((r) => (
        <div key={r.recording_id} className="flex flex-col gap-2 sm:flex-row sm:items-start">
          <div className="min-w-0 flex-1">
            <RecordingProgress recording={r} />
          </div>
          {showControls ? <RecordingControls recordingId={r.recording_id} /> : null}
        </div>
      ))}
    </div>
  );
}
