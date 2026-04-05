import { RecordingControls } from "@/components/recordings/recording-controls";
import { RecordingProgress } from "@/components/recordings/recording-progress";
import { useAppStore } from "@/stores/app-store";
import { useRecordingStore } from "@/stores/recording-store";

export function RecordingList() {
  const sidecarConnected = useAppStore((s) => s.sidecarConnected);
  const recordings = useRecordingStore((s) => s.recordings);

  const active = Object.values(recordings).filter(
    (r) => r.status === "pending" || r.status === "recording",
  );

  if (!sidecarConnected) {
    return (
      <p className="text-sm text-[var(--color-text-muted)]">
        Connect to the sidecar to see live recordings. Ensure the Python sidecar is running and reachable.
      </p>
    );
  }

  if (active.length === 0) {
    return (
      <p className="text-sm text-[var(--color-text-muted)]">
        No active recordings. Start one from the sidecar API or account watcher.
      </p>
    );
  }

  return (
    <div className="grid gap-4 md:grid-cols-2">
      {active.map((r) => (
        <div key={r.recording_id} className="flex flex-col gap-2 sm:flex-row sm:items-start">
          <div className="min-w-0 flex-1">
            <RecordingProgress recording={r} />
          </div>
          <RecordingControls recordingId={r.recording_id} />
        </div>
      ))}
    </div>
  );
}
