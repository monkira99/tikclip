import { useId, useMemo } from "react";
import { FlowCanvasEdge } from "@/components/flows/canvas/flow-canvas-edge";
import {
  FLOW_CANVAS_NODE_H,
  FLOW_CANVAS_NODE_W,
  FLOW_CANVAS_PAD,
} from "@/components/flows/canvas/flow-canvas-layout";
import { FlowCanvasNode } from "@/components/flows/canvas/flow-canvas-node";
import { deriveCanvasNodeStateMap } from "@/components/flows/canvas/flow-canvas-runtime-state";
import type { FlowEditorPayload, FlowNodeKey, FlowRuntimeSnapshot } from "@/types";

/** Vertical center of the node row inside `sceneHeight` (top margin + NODE_H + bottom margin). */
const ROW_Y = 120;

const NODE_SCENE: Array<{ key: FlowNodeKey; x: number; y: number }> = [
  { key: "start", x: 80, y: ROW_Y },
  { key: "record", x: 360, y: ROW_Y },
  { key: "clip", x: 660, y: ROW_Y },
  { key: "caption", x: 940, y: ROW_Y },
  { key: "upload", x: 1240, y: ROW_Y },
];

function toSvgSpace(x: number, y: number): { x: number; y: number } {
  return { x: x - FLOW_CANVAS_PAD, y: y - FLOW_CANVAS_PAD };
}

function nodeHasDraftChanges(draft: string, published: string): boolean {
  return (draft ?? "").trim() !== (published ?? "").trim();
}

function summarizeDraft(nodeKey: FlowNodeKey, draft: string): string {
  try {
    const value = JSON.parse(draft || "{}") as Record<string, unknown>;
    if (nodeKey === "start") {
      const u = value.username;
      return typeof u === "string" && u.trim() ? `@${u.trim()}` : "No username";
    }
    if (nodeKey === "record") {
      const m = value.max_duration_minutes;
      return typeof m === "number" && Number.isFinite(m) ? `Max ${m} min` : "Recording";
    }
    if (nodeKey === "clip") {
      const a = value.clip_min_duration;
      const b = value.clip_max_duration;
      if (typeof a === "number" && typeof b === "number" && Number.isFinite(a) && Number.isFinite(b)) {
        return `${a}–${b}s clips`;
      }
      return "Clip rules";
    }
    if (nodeKey === "caption") {
      const m = value.model;
      return typeof m === "string" && m ? m : "Caption model";
    }
    return "Upload target";
  } catch {
    return "Invalid JSON";
  }
}

export type FlowCanvasProps = {
  flow: FlowEditorPayload | null;
  selectedNode: FlowNodeKey | null;
  runtimeSnapshot?: FlowRuntimeSnapshot | null;
  onSelectNode: (node: FlowNodeKey) => void;
};

export function FlowCanvas({ flow, selectedNode, runtimeSnapshot = null, onSelectNode }: FlowCanvasProps) {
  const arrowMarkerId = useId().replace(/:/g, "");
  const markerEndUrl = `url(#${arrowMarkerId})`;

  const nodeMap = useMemo(() => {
    const map = new Map<FlowNodeKey, FlowEditorPayload["nodes"][number]>();
    for (const n of flow?.nodes ?? []) {
      map.set(n.node_key, n);
    }
    return map;
  }, [flow?.nodes]);

  const runtimeStateByNode = useMemo(
    () =>
      deriveCanvasNodeStateMap({
        flowEnabled: flow?.flow.enabled ?? true,
        runs: flow?.runs ?? [],
        nodeRuns: flow?.nodeRuns ?? [],
        runtimeSnapshot,
      }),
    [flow?.flow.enabled, flow?.nodeRuns, flow?.runs, runtimeSnapshot],
  );

  const sceneWidth = Math.max(...NODE_SCENE.map((n) => n.x)) + FLOW_CANVAS_NODE_W + 80;
  const sceneHeight = Math.max(...NODE_SCENE.map((n) => n.y)) + FLOW_CANVAS_NODE_H + 80;
  /** Border-box height of the padded scene — locks layout so nodes do not sit low when a parent stretches `min-height`. */
  const shellHeight = sceneHeight + FLOW_CANVAS_PAD * 2;

  const edges = NODE_SCENE.slice(0, -1).map((from, i) => {
    const to = NODE_SCENE[i + 1]!;
    const x1n = from.x + FLOW_CANVAS_NODE_W;
    const y1n = from.y + FLOW_CANVAS_NODE_H / 2;
    const x2n = to.x;
    const y2n = to.y + FLOW_CANVAS_NODE_H / 2;
    const p1 = toSvgSpace(x1n, y1n);
    const p2 = toSvgSpace(x2n, y2n);
    return (
      <FlowCanvasEdge
        key={`${from.key}-${to.key}`}
        x1={p1.x}
        y1={p1.y}
        x2={p2.x}
        y2={p2.y}
        markerEnd={markerEndUrl}
      />
    );
  });

  return (
    <div className="app-panel-subtle flex min-h-[360px] items-center overflow-x-auto rounded-2xl">
      <div
        className="relative shrink-0 p-6"
        style={{ width: sceneWidth, minWidth: "100%", height: shellHeight }}
      >
        <svg
          className="pointer-events-none absolute left-6 top-6 text-[var(--color-border)]"
          width={sceneWidth - 48}
          height={sceneHeight - 48}
          aria-hidden
        >
          <defs>
            <marker
              id={arrowMarkerId}
              markerUnits="userSpaceOnUse"
              markerWidth="10"
              markerHeight="9"
              refX="8"
              refY="4.5"
              orient="auto"
            >
              <path d="M0 0 L8 4.5 L0 9 Z" fill="rgba(255,255,255,0.32)" />
            </marker>
          </defs>
          {edges}
        </svg>
        {NODE_SCENE.map(({ key, x, y }) => {
          const def = nodeMap.get(key);
          const draft = def?.draft_config_json ?? "{}";
          const published = def?.published_config_json ?? "{}";
          const hasDraft = def ? nodeHasDraftChanges(draft, published) : false;
          const summary = summarizeDraft(key, draft);
          const runtimeState = runtimeStateByNode[key];
          return (
            <FlowCanvasNode
              key={key}
              nodeKey={key}
              selected={selectedNode === key}
              hasDraftChanges={hasDraft}
              runtimeState={runtimeState.runtimeLabel}
              summary={summary}
              visualState={runtimeState.visualState}
              badgeLabel={runtimeState.badgeLabel}
              inlineDetail={runtimeState.inlineDetail}
              activeMarker={runtimeState.activeMarker}
              onClick={() => onSelectNode(key)}
              style={{
                left: x,
                top: y,
                width: FLOW_CANVAS_NODE_W,
                height: FLOW_CANVAS_NODE_H,
              }}
            />
          );
        })}
      </div>
    </div>
  );
}
