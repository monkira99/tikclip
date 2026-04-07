import { RecordingList } from "@/components/recordings/recording-list";

export function RecordingsPage() {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-[var(--color-text)]">Active recordings</h2>
      </div>
      <RecordingList />
    </div>
  );
}
