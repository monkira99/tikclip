import { RecordingList } from "@/components/recordings/recording-list";

export function RecordingsPage() {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-[var(--color-text)]">Active recordings</h2>
        <p className="text-sm text-[var(--color-text-muted)]">
          Live status from the sidecar. Stop a recording with the button below each card.
        </p>
      </div>
      <RecordingList />
    </div>
  );
}
