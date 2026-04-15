import { FLOW_NODE_LABEL } from "@/components/flows/flow-node-utils";
import { cn } from "@/lib/utils";
import type { FlowNodeKey, FlowNodeRunRow } from "@/types";

function truncate(s: string | null, max: number): string {
  if (s == null || s === "") {
    return "—";
  }
  const t = s.replace(/\s+/g, " ").trim();
  if (t.length <= max) {
    return t;
  }
  return `${t.slice(0, Math.max(0, max - 1))}…`;
}

function summarizeNodeRun(n: FlowNodeRunRow): string {
  if (n.error) {
    return truncate(n.error, 96);
  }
  if (n.output_json) {
    return truncate(n.output_json, 96);
  }
  return "—";
}

type FlowRuntimeLaneProps = {
  nodeKey: FlowNodeKey;
  entries: FlowNodeRunRow[];
};

export function FlowRuntimeLane({ nodeKey, entries }: FlowRuntimeLaneProps) {
  const label = FLOW_NODE_LABEL[nodeKey];

  return (
    <div
      className={cn(
        "flex min-h-[120px] flex-col rounded-xl border border-[var(--color-border)] bg-[rgb(16_17_17_/0.45)] px-3 py-3",
        entries.length > 0 && "border-[color-mix(in_oklab,var(--color-accent)_22%,var(--color-border))]",
      )}
    >
      <p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--color-text-muted)]">
        {label}
      </p>
      {entries.length === 0 ? (
        <p className="mt-3 text-xs text-[var(--color-text-muted)]">No recent steps.</p>
      ) : (
        <ul className="mt-2 flex flex-col gap-2">
          {entries.map((n) => (
            <li
              key={n.id}
              className="rounded-lg border border-[var(--color-border)] bg-white/[0.02] px-2 py-1.5"
            >
              <div className="flex flex-wrap items-center justify-between gap-1 text-[10px] text-[var(--color-text-muted)]">
                <span className="font-mono text-[var(--color-text-soft)]">#{n.id}</span>
                <span className="capitalize text-[var(--color-text)]">{n.status}</span>
              </div>
              <p className="mt-1 font-mono text-[10px] leading-snug text-[var(--color-text-muted)]">
                {summarizeNodeRun(n)}
              </p>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
