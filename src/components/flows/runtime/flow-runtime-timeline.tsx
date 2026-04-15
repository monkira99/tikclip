import { useMemo } from "react";
import { FLOW_NODE_LABEL } from "@/components/flows/flow-node-utils";
import { cn } from "@/lib/utils";
import type { FlowNodeRunRow, FlowRunRow } from "@/types";

type FlowRuntimeTimelineProps = {
  runs: FlowRunRow[];
  nodeRuns: FlowNodeRunRow[];
};

type TimelineItem =
  | {
      kind: "run";
      id: number;
      at: string;
      title: string;
      subtitle: string;
      status: string;
    }
  | {
      kind: "node";
      id: number;
      at: string;
      title: string;
      subtitle: string;
      status: string;
    };

function buildTimelineItems(runs: FlowRunRow[], nodeRuns: FlowNodeRunRow[]): TimelineItem[] {
  const items: TimelineItem[] = [];

  for (const r of runs) {
    items.push({
      kind: "run",
      id: r.id,
      at: r.started_at,
      title: `Run #${r.id}`,
      subtitle: `Definition v${r.definition_version}`,
      status: r.status,
    });
  }

  for (const n of nodeRuns) {
    const at = n.started_at ?? n.ended_at ?? "";
    if (!at) {
      continue;
    }
    const detail =
      n.error?.trim() ||
      (n.output_json ? n.output_json.replace(/\s+/g, " ").trim().slice(0, 120) : "") ||
      "—";
    items.push({
      kind: "node",
      id: n.id,
      at,
      title: FLOW_NODE_LABEL[n.node_key],
      subtitle: detail.length > 100 ? `${detail.slice(0, 99)}…` : detail,
      status: n.status,
    });
  }

  return items.sort((a, b) => b.at.localeCompare(a.at)).slice(0, 40);
}

export function FlowRuntimeTimeline({ runs, nodeRuns }: FlowRuntimeTimelineProps) {
  const items = useMemo(() => buildTimelineItems(runs, nodeRuns), [runs, nodeRuns]);

  return (
    <section className="app-panel-subtle rounded-2xl px-4 py-4">
      <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--color-text-muted)]">
        Runtime timeline
      </p>
      <p className="mt-1 text-xs leading-relaxed text-[var(--color-text-muted)]">
        Recent flow runs and node steps, newest first.
      </p>

      {items.length === 0 ? (
        <p className="mt-4 text-sm text-[var(--color-text-muted)]">No runtime events yet.</p>
      ) : (
        <ol className="mt-4 space-y-2">
          {items.map((item) => (
            <li
              key={`${item.kind}-${item.id}`}
              className={cn(
                "rounded-xl border border-[var(--color-border)] px-3 py-2.5",
                item.kind === "run" ? "bg-white/[0.03]" : "bg-white/[0.02]",
              )}
            >
              <div className="flex flex-wrap items-baseline justify-between gap-2">
                <span className="text-sm font-medium tracking-[0.01em] text-[var(--color-text)]">
                  {item.title}
                </span>
                <span className="text-[10px] tabular-nums text-[var(--color-text-muted)]">{item.at}</span>
              </div>
              <p className="mt-0.5 text-xs leading-relaxed text-[var(--color-text-muted)]">{item.subtitle}</p>
              <p className="mt-1.5 inline-flex rounded-md border border-[var(--color-border)] bg-[rgb(16_17_17_/0.6)] px-2 py-0.5 text-[10px] font-semibold capitalize text-[var(--color-text-soft)]">
                {item.status}
              </p>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
