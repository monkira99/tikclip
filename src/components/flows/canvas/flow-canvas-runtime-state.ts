import { FLOW_NODE_LABEL, FLOW_NODE_ORDER } from "@/components/flows/flow-node-utils";
import type { FlowNodeKey, FlowNodeRunRow, FlowRuntimeSnapshot, FlowRunRow } from "@/types";

export type CanvasNodeVisualState = "idle" | "running" | "done" | "error";

export type CanvasNodeRuntimeState = {
  visualState: CanvasNodeVisualState;
  badgeLabel: "Running" | "Done" | "Error" | null;
  runtimeLabel: string;
  inlineDetail: string | null;
  activeMarker: boolean;
};

type DeriveCanvasNodeStateArgs = {
  flowEnabled: boolean;
  runs: FlowRunRow[];
  nodeRuns: FlowNodeRunRow[];
  runtimeSnapshot: FlowRuntimeSnapshot | null;
};

const ACTIVE_RUNTIME_LABEL: Partial<Record<FlowNodeKey, string>> = {
  start: "Watching for live",
  record: "Recording live",
  clip: "Creating clips",
  caption: "Generating captions",
  upload: "Waiting for upload",
};

const ACTIVE_SNAPSHOT_STATUSES = new Set(["watching", "recording", "processing"]);

function isNodeRunMoreRecent(a: FlowNodeRunRow, b: FlowNodeRunRow): number {
  const aAt = a.started_at ?? a.ended_at ?? "";
  const bAt = b.started_at ?? b.ended_at ?? "";
  return bAt.localeCompare(aAt) || b.id - a.id;
}

export function deriveActiveRunId(
  runs: FlowRunRow[],
  runtimeSnapshot: FlowRuntimeSnapshot | null,
): number | null {
  if (runtimeSnapshot?.active_flow_run_id != null) {
    return runtimeSnapshot.active_flow_run_id;
  }

  const running = runs
    .filter((run) => run.status === "running")
    .slice()
    .sort((a, b) => b.started_at.localeCompare(a.started_at) || b.id - a.id);

  if (running[0]) {
    return running[0].id;
  }

  const mostRecentRelevant = runs
    .slice()
    .sort((a, b) => b.started_at.localeCompare(a.started_at) || b.id - a.id);

  return mostRecentRelevant[0]?.id ?? null;
}

function latestNodeRunByKey(nodeRuns: FlowNodeRunRow[], flowRunId: number | null): Map<FlowNodeKey, FlowNodeRunRow> {
  const latest = new Map<FlowNodeKey, FlowNodeRunRow>();
  if (flowRunId == null) {
    return latest;
  }

  for (const nodeRun of nodeRuns.filter((row) => row.flow_run_id === flowRunId)) {
    const current = latest.get(nodeRun.node_key);
    if (!current || isNodeRunMoreRecent(current, nodeRun) > 0) {
      latest.set(nodeRun.node_key, nodeRun);
    }
  }

  return latest;
}

export function deriveCanvasNodeStateMap({
  flowEnabled,
  runs,
  nodeRuns,
  runtimeSnapshot,
}: DeriveCanvasNodeStateArgs): Record<FlowNodeKey, CanvasNodeRuntimeState> {
  if (!flowEnabled) {
    return Object.fromEntries(
      FLOW_NODE_ORDER.map((nodeKey) => [
        nodeKey,
        {
          visualState: "idle",
          badgeLabel: null,
          runtimeLabel: "Disabled",
          inlineDetail: null,
          activeMarker: false,
        },
      ]),
    ) as Record<FlowNodeKey, CanvasNodeRuntimeState>;
  }

  const snapshotStatus = runtimeSnapshot?.status ?? null;
  const snapshotIsActive = snapshotStatus != null && ACTIVE_SNAPSHOT_STATUSES.has(snapshotStatus);
  const activeRunId = snapshotIsActive && runtimeSnapshot?.active_flow_run_id == null
    ? null
    : deriveActiveRunId(runs, runtimeSnapshot);
  const latestByKey = latestNodeRunByKey(nodeRuns, activeRunId);
  const currentNode = runtimeSnapshot?.current_node ?? null;
  const snapshotIsError = snapshotStatus === "error";

  return Object.fromEntries(
    FLOW_NODE_ORDER.map((nodeKey) => {
      const latest = latestByKey.get(nodeKey);

      if (snapshotIsError && currentNode === nodeKey) {
        return [
          nodeKey,
          {
            visualState: "error",
            badgeLabel: "Error",
            runtimeLabel: `${FLOW_NODE_LABEL[nodeKey]} failed`,
            inlineDetail: latest?.error?.trim() || runtimeSnapshot?.last_error || null,
            activeMarker: false,
          },
        ];
      }

      if (snapshotIsActive && currentNode === nodeKey) {
        return [
          nodeKey,
          {
            visualState: "running",
            badgeLabel: "Running",
            runtimeLabel: ACTIVE_RUNTIME_LABEL[nodeKey] ?? `Running ${FLOW_NODE_LABEL[nodeKey]}`,
            inlineDetail: null,
            activeMarker: true,
          },
        ];
      }

      if (latest?.status === "failed") {
        return [
          nodeKey,
          {
            visualState: "error",
            badgeLabel: "Error",
            runtimeLabel: `${FLOW_NODE_LABEL[nodeKey]} failed`,
            inlineDetail: latest.error?.trim() || null,
            activeMarker: false,
          },
        ];
      }

      if (latest?.status === "completed") {
        return [
          nodeKey,
          {
            visualState: "done",
            badgeLabel: "Done",
            runtimeLabel: `${FLOW_NODE_LABEL[nodeKey]} complete`,
            inlineDetail: null,
            activeMarker: false,
          },
        ];
      }

      return [
        nodeKey,
        {
          visualState: "idle",
          badgeLabel: null,
          runtimeLabel: runtimeSnapshot?.status === "disabled" ? "Disabled" : "Not running",
          inlineDetail: null,
          activeMarker: false,
        },
      ];
    }),
  ) as Record<FlowNodeKey, CanvasNodeRuntimeState>;
}
