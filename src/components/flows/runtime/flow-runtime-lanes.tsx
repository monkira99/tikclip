import { useMemo } from "react";
import { FlowRuntimeLane } from "@/components/flows/runtime/flow-runtime-lane";
import { FLOW_NODE_ORDER } from "@/components/flows/flow-node-utils";
import type { FlowNodeKey, FlowNodeRunRow } from "@/types";

type FlowRuntimeLanesProps = {
  nodeRuns: FlowNodeRunRow[];
};

function entriesForNode(nodeRuns: FlowNodeRunRow[], nodeKey: FlowNodeKey): FlowNodeRunRow[] {
  return nodeRuns
    .filter((n) => n.node_key === nodeKey)
    .slice()
    .sort((a, b) => (b.started_at ?? b.ended_at ?? "").localeCompare(a.started_at ?? a.ended_at ?? ""))
    .slice(0, 6);
}

export function FlowRuntimeLanes({ nodeRuns }: FlowRuntimeLanesProps) {
  const byNode = useMemo(() => {
    const map = new Map<FlowNodeKey, FlowNodeRunRow[]>();
    for (const key of FLOW_NODE_ORDER) {
      map.set(key, entriesForNode(nodeRuns, key));
    }
    return map;
  }, [nodeRuns]);

  return (
    <section className="app-panel-subtle rounded-2xl px-4 py-4">
      <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--color-text-muted)]">
        Node lanes
      </p>
      <p className="mt-1 text-xs leading-relaxed text-[var(--color-text-muted)]">
        Latest node-run rows per pipeline stage.
      </p>
      <div className="mt-4 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5">
        {FLOW_NODE_ORDER.map((key) => (
          <FlowRuntimeLane key={key} nodeKey={key} entries={byNode.get(key) ?? []} />
        ))}
      </div>
    </section>
  );
}
