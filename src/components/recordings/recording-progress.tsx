import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import type { SidecarRecordingStatus } from "@/types";

function formatDuration(totalSeconds: number): string {
  const m = Math.floor(totalSeconds / 60);
  const s = totalSeconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function formatBytes(n: number): string {
  if (n < 1024) {
    return `${n} B`;
  }
  if (n < 1024 * 1024) {
    return `${(n / 1024).toFixed(1)} KB`;
  }
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

interface RecordingProgressProps {
  recording: SidecarRecordingStatus;
}

export function RecordingProgress({ recording }: RecordingProgressProps) {
  const isActive = recording.status === "pending" || recording.status === "recording";

  return (
    <Card className="border-[var(--color-border)] bg-[var(--color-surface)]">
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <div>
          <div className="text-sm font-semibold text-[var(--color-text)]">{recording.username}</div>
          <div className="font-mono text-[10px] text-[var(--color-text-muted)]">{recording.recording_id}</div>
        </div>
        <Badge
          variant="outline"
          className={
            isActive
              ? "border-red-500/40 bg-red-500/10 text-red-400"
              : "border-[var(--color-border)] text-[var(--color-text-muted)]"
          }
        >
          {recording.status}
        </Badge>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex justify-between text-xs text-[var(--color-text-muted)]">
          <span>Duration</span>
          <span className="font-mono text-[var(--color-text)]">
            {formatDuration(recording.duration_seconds)}
          </span>
        </div>
        <div className="flex justify-between text-xs text-[var(--color-text-muted)]">
          <span>File size</span>
          <span className="font-mono text-[var(--color-text)]">{formatBytes(recording.file_size_bytes)}</span>
        </div>
        {isActive && (
          <div className="h-1.5 overflow-hidden rounded-full bg-white/10">
            <div
              className="h-full animate-pulse rounded-full bg-[var(--color-primary)]"
              style={{ width: "100%" }}
            />
          </div>
        )}
        {recording.error_message && (
          <p className="text-xs text-red-400">{recording.error_message}</p>
        )}
      </CardContent>
    </Card>
  );
}
