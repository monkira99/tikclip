import { useEffect, useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { listClipsByFlow } from "@/lib/api";
import { cn } from "@/lib/utils";
import type { Clip, ClipCaptionStatus, FlowNodeKey } from "@/types";

type FlowCaptionsPanelProps = {
  flowId: number;
  selectedNode: FlowNodeKey | null;
  clips?: Clip[];
  loading?: boolean;
  error?: string | null;
};

type CaptionGroup = {
  status: ClipCaptionStatus;
  count: number;
};

const CAPTION_ORDER: ClipCaptionStatus[] = ["pending", "generating", "completed", "failed"];

const CAPTION_STATUS_CLASS: Record<ClipCaptionStatus, string> = {
  pending: "border-white/10 bg-white/[0.03] text-[var(--color-text-muted)]",
  generating: "border-[rgba(85,179,255,0.2)] bg-[rgba(85,179,255,0.14)] text-[var(--color-accent)]",
  completed: "border-[rgba(95,201,146,0.2)] bg-[rgba(95,201,146,0.14)] text-[var(--color-success)]",
  failed: "border-[rgba(255,99,99,0.22)] bg-[rgba(255,99,99,0.14)] text-[var(--color-primary)]",
};

function summarizeCaptionStatuses(clips: Clip[]): CaptionGroup[] {
  const map = new Map<ClipCaptionStatus, number>();
  for (const status of CAPTION_ORDER) {
    map.set(status, 0);
  }

  for (const clip of clips) {
    const status = CAPTION_ORDER.includes(clip.caption_status) ? clip.caption_status : "pending";
    map.set(status, (map.get(status) ?? 0) + 1);
  }

  return CAPTION_ORDER.map((status) => ({ status, count: map.get(status) ?? 0 }));
}

function captionPreview(clips: Clip[]): Clip[] {
  return clips
    .filter((clip) => clip.caption_text?.trim())
    .sort((a, b) => b.updated_at.localeCompare(a.updated_at))
    .slice(0, 6);
}

export function FlowCaptionsPanel({
  flowId,
  selectedNode,
  clips: controlledClips,
  loading: controlledLoading,
  error: controlledError,
}: FlowCaptionsPanelProps) {
  const [localClips, setLocalClips] = useState<Clip[]>([]);
  const [localLoading, setLocalLoading] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);

  const controlled = controlledClips != null;
  const rows = controlledClips ?? localClips;
  const loadingState = controlledLoading ?? localLoading;
  const errorState = controlledError ?? localError;

  useEffect(() => {
    if (controlled) {
      return;
    }

    let cancelled = false;
    setLocalLoading(true);
    setLocalError(null);

    void listClipsByFlow(flowId)
      .then((rows) => {
        if (!cancelled) {
          setLocalClips(rows);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setLocalClips([]);
          setLocalError(e instanceof Error ? e.message : String(e));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLocalLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [flowId, controlled]);

  const groups = useMemo(() => summarizeCaptionStatuses(rows), [rows]);
  const preview = useMemo(() => captionPreview(rows), [rows]);
  const focused = selectedNode === "caption";

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
            Captions
          </p>
          <p className="mt-1 text-sm text-[var(--color-text-muted)]">Caption progress and latest caption text.</p>
        </div>
        {focused ? <Badge variant="secondary">Selected node</Badge> : null}
      </div>

      {loadingState ? <p className="text-sm text-[var(--color-text-muted)]">Loading captions...</p> : null}
      {errorState ? <p className="text-sm text-[var(--color-primary)]">{errorState}</p> : null}

      {!loadingState && !errorState ? (
        <>
          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
            {groups.map((group) => (
              <div
                key={group.status}
                className={cn(
                  "rounded-xl border px-3 py-2",
                  CAPTION_STATUS_CLASS[group.status],
                )}
              >
                <p className="text-[10px] font-semibold uppercase tracking-[0.1em]">{group.status}</p>
                <p className="mt-1 text-xl font-semibold">{group.count}</p>
              </div>
            ))}
          </div>

          {preview.length === 0 ? (
            <p className="text-sm text-[var(--color-text-muted)]">No generated captions yet for this flow.</p>
          ) : (
            <div className="space-y-2">
              <p className="text-xs font-semibold uppercase tracking-[0.1em] text-[var(--color-text-muted)]">
                Latest generated
              </p>
              <div className="space-y-2">
                {preview.map((clip) => (
                  <article
                    key={clip.id}
                    className="rounded-xl border border-white/8 bg-white/[0.02] px-3 py-2"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <p className="truncate text-sm font-medium text-[var(--color-text)]">
                        {clip.title?.trim() || `Clip #${clip.id}`}
                      </p>
                      <Badge variant="outline" className="text-[10px] capitalize">
                        {clip.caption_status}
                      </Badge>
                    </div>
                    <p className="mt-1 line-clamp-2 text-xs text-[var(--color-text-muted)]">
                      {clip.caption_text?.trim()}
                    </p>
                  </article>
                ))}
              </div>
            </div>
          )}
        </>
      ) : null}
    </section>
  );
}
