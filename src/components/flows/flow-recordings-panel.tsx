import { useEffect, useMemo, useState } from "react";
import { RecordingList } from "@/components/recordings/recording-list";
import { Badge } from "@/components/ui/badge";
import { listRecordingsByFlow, type FlowRecording } from "@/lib/api";
import { cn } from "@/lib/utils";
import type { FlowNodeKey, SidecarRecordingStatus } from "@/types";

type FlowRecordingsPanelProps = {
  flowId: number;
  selectedNode: FlowNodeKey | null;
  recordingsCountHint?: number;
};

function mapFlowRecordingToLiveRow(row: FlowRecording): SidecarRecordingStatus {
  return {
    recording_id: row.sidecar_recording_id ?? `flow-${row.id}`,
    account_id: row.account_id,
    username: row.account_username,
    status: row.status,
    duration_seconds: row.duration_seconds,
    file_size_bytes: row.file_size_bytes,
    file_path: row.file_path,
    error_message: row.error_message,
  };
}

export function FlowRecordingsPanel({
  flowId,
  selectedNode,
  recordingsCountHint,
}: FlowRecordingsPanelProps) {
  const [rows, setRows] = useState<FlowRecording[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    void listRecordingsByFlow(flowId)
      .then((next) => {
        if (!cancelled) {
          setRows(next);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setRows([]);
          setError(e instanceof Error ? e.message : String(e));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [flowId]);

  const recordings = useMemo(() => rows.map(mapFlowRecordingToLiveRow), [rows]);
  const count = recordingsCountHint ?? rows.length;
  const focused = selectedNode === "record";

  return (
    <section
      className={cn(
        "app-panel-subtle space-y-3 rounded-2xl px-4 py-4",
        focused && "ring-1 ring-[rgba(85,179,255,0.35)]",
      )}
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--color-text-muted)]">
            Recordings
          </p>
          <p className="mt-1 text-sm text-[var(--color-text-muted)]">Flow-scoped recording history.</p>
        </div>
        <div className="flex items-center gap-2">
          {focused ? <Badge variant="secondary">Selected node</Badge> : null}
          <Badge variant="outline">{count} total</Badge>
        </div>
      </div>

      {loading ? <p className="text-sm text-[var(--color-text-muted)]">Loading recordings...</p> : null}
      {error ? <p className="text-sm text-[var(--color-primary)]">{error}</p> : null}
      {!loading && !error ? (
        <RecordingList
          recordings={recordings}
          mode="all"
          showControls={false}
          emptyMessage="No recordings for this flow yet."
        />
      ) : null}
    </section>
  );
}
