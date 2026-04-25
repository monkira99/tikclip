import { RecordingControls } from "@/components/recordings/recording-controls";
import { RecordingProgress } from "@/components/recordings/recording-progress";
import { useRecordingStore } from "@/stores/recording-store";
import type { ActiveRecordingStatus } from "@/types";

type RecordingListProps = {
  recordings?: ActiveRecordingStatus[];
  mode?: "active" | "all";
  emptyMessage?: string;
  showControls?: boolean;
};

export function RecordingList({
  recordings,
  mode = "active",
  emptyMessage,
  showControls = true,
}: RecordingListProps = {}) {
  const storeRecordings = useRecordingStore((s) => s.recordings);

  const source = recordings ?? Object.values(storeRecordings);

  const active = source.filter(
    (r) => r.status === "pending" || r.status === "recording",
  );
  const visible = mode === "active" ? active : source;
  const resolvedEmptyMessage =
    emptyMessage ??
    (mode === "active"
      ? "No active recordings. Rust runtime will show live flow recordings here."
      : "No recordings for this flow yet.");

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
