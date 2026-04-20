import { FLOW_NODE_LABEL } from "@/components/flows/flow-node-utils";
import { deriveCanvasNodeStateMap } from "@/components/flows/canvas/flow-canvas-runtime-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { FlowContext, FlowNodeKey, FlowRuntimeSnapshot, FlowStatus } from "@/types";

type FlowRuntimeStripProps = {
  flow: FlowContext;
  runtimeSnapshot: FlowRuntimeSnapshot | null;
  runtimeLogsCount: number;
  onOpenDiagnostics: () => void;
};

const STATUS_CLASS: Record<FlowStatus, string> = {
  idle: "border-white/10 bg-white/[0.05] text-[var(--color-text-muted)]",
  watching: "border-[rgba(85,179,255,0.2)] bg-[rgba(85,179,255,0.14)] text-[var(--color-accent)]",
  recording: "border-[rgba(95,201,146,0.2)] bg-[rgba(95,201,146,0.14)] text-[var(--color-success)]",
  processing: "border-[rgba(255,188,51,0.2)] bg-[rgba(255,188,51,0.14)] text-[var(--color-warning)]",
  error: "border-[rgba(255,99,99,0.22)] bg-[rgba(255,99,99,0.14)] text-[var(--color-primary)]",
  disabled: "border-white/8 bg-white/[0.03] text-[#6a6b6c]",
};

function formatNodeLabel(nodeKey: FlowNodeKey | null): string {
  if (nodeKey == null) {
    return "Waiting";
  }
  return FLOW_NODE_LABEL[nodeKey];
}

function formatLastLiveLabel(value: string | null): string {
  return value ? value : "No recent live signal";
}

function formatRunLabel(activeFlowRunId: number | null | undefined): string {
  return activeFlowRunId != null ? `Run #${activeFlowRunId}` : "No active run";
}

function formatLogCountLabel(count: number): string {
  return count > 0 ? `${count} logs` : "No logs loaded";
}

function derivePrimaryRuntimeCopy(runtimeSnapshot: FlowRuntimeSnapshot | null): {
  title: string;
  detail: string | null;
} {
  if (!runtimeSnapshot) {
    return {
      title: "Not running",
      detail: null,
    };
  }

  const stateMap = deriveCanvasNodeStateMap({
    runs: [],
    nodeRuns: [],
    runtimeSnapshot,
  });
  const currentNode = runtimeSnapshot.current_node;

  if (currentNode != null) {
    const nodeState = stateMap[currentNode];
    return {
      title: nodeState.runtimeLabel,
      detail: nodeState.inlineDetail,
    };
  }

  if (runtimeSnapshot.status === "error") {
    return {
      title: "Runtime failed",
      detail: runtimeSnapshot.last_error,
    };
  }

  return {
    title: "Not running",
    detail: null,
  };
}

export function FlowRuntimeStrip({
  flow,
  runtimeSnapshot,
  runtimeLogsCount,
  onOpenDiagnostics,
}: FlowRuntimeStripProps) {
  const currentStatus = runtimeSnapshot?.status ?? flow.status;
  const primaryCopy = derivePrimaryRuntimeCopy(runtimeSnapshot);

  return (
    <section className="app-panel-subtle rounded-2xl px-4 py-4">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0 space-y-3">
          <div className="flex flex-wrap items-center gap-2">
            <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--color-text-muted)]">
              Runtime Monitor
            </p>
            <Badge variant="secondary" className={cn("text-[10px] capitalize", STATUS_CLASS[currentStatus])}>
              {currentStatus}
            </Badge>
          </div>

          <div className="space-y-1">
            <p className="text-sm font-semibold tracking-[0.01em] text-[var(--color-text)]">
              {primaryCopy.title}
            </p>
            {primaryCopy.detail ? (
              <p className="line-clamp-1 text-sm leading-relaxed text-[var(--color-primary)]">{primaryCopy.detail}</p>
            ) : null}
          </div>

          <div className="grid gap-2 text-xs text-[var(--color-text-muted)] sm:grid-cols-2 xl:grid-cols-4">
            <div className="rounded-xl border border-[var(--color-border)] bg-white/[0.02] px-3 py-2">
              <p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--color-text-soft)]">
                Node
              </p>
              <p className="mt-1 text-sm text-[var(--color-text)]">{formatNodeLabel(runtimeSnapshot?.current_node ?? flow.current_node)}</p>
            </div>
            <div className="rounded-xl border border-[var(--color-border)] bg-white/[0.02] px-3 py-2">
              <p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--color-text-soft)]">
                Account
              </p>
              <p className="mt-1 text-sm text-[var(--color-text)]">{runtimeSnapshot?.username ?? "Unknown"}</p>
            </div>
            <div className="rounded-xl border border-[var(--color-border)] bg-white/[0.02] px-3 py-2">
              <p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--color-text-soft)]">
                Run
              </p>
              <p className="mt-1 text-sm text-[var(--color-text)]">{formatRunLabel(runtimeSnapshot?.active_flow_run_id)}</p>
            </div>
            <div className="rounded-xl border border-[var(--color-border)] bg-white/[0.02] px-3 py-2">
              <p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--color-text-soft)]">
                Last live
              </p>
              <p className="mt-1 text-sm text-[var(--color-text)]">
                {formatLastLiveLabel(runtimeSnapshot?.last_live_at ?? flow.last_live_at)}
              </p>
            </div>
          </div>
        </div>

        <div className="flex shrink-0 flex-col items-stretch gap-2 lg:min-w-48 lg:items-end">
          <span className="rounded-md border border-[var(--color-border)] bg-white/[0.03] px-2 py-1 text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--color-text-soft)]">
            {formatLogCountLabel(runtimeLogsCount)}
          </span>
          <Button size="sm" variant="outline" onClick={onOpenDiagnostics}>
            Open diagnostics
          </Button>
        </div>
      </div>
    </section>
  );
}
