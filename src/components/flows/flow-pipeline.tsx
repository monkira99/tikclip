import { Badge } from "@/components/ui/badge";
import {
  FLOW_NODE_LABEL,
  FLOW_NODE_ORDER,
  getFlowNodeStatus,
} from "@/components/flows/flow-node-utils";
import { cn } from "@/lib/utils";
import type { FlowDetail, FlowNodeKey } from "@/types";

type FlowPipelineProps = {
  flow: FlowDetail["flow"] | null;
  selectedNode: FlowNodeKey | null;
  onSelectNode: (node: FlowNodeKey) => void;
};

export function FlowPipeline({ flow, selectedNode, onSelectNode }: FlowPipelineProps) {
  return (
    <div className="app-panel-subtle space-y-4 rounded-2xl px-4 py-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--color-text-muted)]">
          Pipeline
        </p>
        <div className="flex items-center gap-2">
          {flow ? (
            <Badge
              variant="secondary"
              className={cn(
                "text-[10px] capitalize",
                flow.enabled
                  ? "border-[rgba(85,179,255,0.2)] bg-[rgba(85,179,255,0.14)] text-[var(--color-accent)]"
                  : "border-white/8 bg-white/[0.03] text-[#6a6b6c]",
              )}
            >
              {flow.status}
            </Badge>
          ) : null}
          {selectedNode ? (
            <Badge variant="secondary" className="text-[10px] capitalize">
              selected: {FLOW_NODE_LABEL[selectedNode]}
            </Badge>
          ) : null}
        </div>
      </div>

      <div className="flex items-center gap-1.5">
        {FLOW_NODE_ORDER.map((node, index) => {
          const nodeState = getFlowNodeStatus(flow, node);
          const selected = selectedNode === node;

          return (
            <div key={node} className="flex min-w-0 flex-1 items-center gap-1.5">
              <button
                type="button"
                onClick={() => onSelectNode(node)}
                className={cn(
                  "w-full rounded-xl border px-2.5 py-2 text-center text-[11px] font-semibold uppercase tracking-[0.08em] transition-colors",
                  "border-white/10 bg-white/[0.03] text-[var(--color-text-muted)] hover:bg-white/[0.06]",
                  nodeState === "done" && "border-[rgba(85,179,255,0.2)] bg-[rgba(85,179,255,0.1)] text-[var(--color-text)]",
                  nodeState === "current" &&
                    "border-[rgba(95,201,146,0.35)] bg-[rgba(95,201,146,0.15)] text-[var(--color-success)]",
                  selected && "ring-2 ring-[color-mix(in_oklab,var(--color-accent)_35%,transparent)]",
                )}
                aria-pressed={selected}
              >
                {FLOW_NODE_LABEL[node]}
              </button>
              {index === FLOW_NODE_ORDER.length - 1 ? null : (
                <span
                  className={cn(
                    "h-px w-3 shrink-0 bg-white/15",
                    (nodeState === "done" || nodeState === "current") && "bg-[rgba(85,179,255,0.45)]",
                  )}
                  aria-hidden
                />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
