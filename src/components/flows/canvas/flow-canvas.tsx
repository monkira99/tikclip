import { useEffect, useId, useMemo, useState } from "react";
import { FlowCanvasEdge } from "@/components/flows/canvas/flow-canvas-edge";
import {
  FLOW_CANVAS_NODE_H,
  FLOW_CANVAS_NODE_W,
  FLOW_CANVAS_PAD,
} from "@/components/flows/canvas/flow-canvas-layout";
import { FlowCanvasNode, type FlowCanvasNodeDetail } from "@/components/flows/canvas/flow-canvas-node";
import {
  deriveActiveRunId,
  deriveCanvasNodeStateMap,
} from "@/components/flows/canvas/flow-canvas-runtime-state";
import { formatDuration } from "@/lib/format";
import type {
  FlowEditorPayload,
  FlowNodeKey,
  FlowNodeRunRow,
  FlowRuntimeSnapshot,
  FlowRunRow,
} from "@/types";

/** Vertical center of the node row inside `sceneHeight` (top margin + NODE_H + bottom margin). */
const ROW_Y = 120;

const NODE_SCENE: Array<{ key: FlowNodeKey; x: number; y: number }> = [
  { key: "start", x: 80, y: ROW_Y },
  { key: "record", x: 400, y: ROW_Y },
  { key: "clip", x: 720, y: ROW_Y },
  { key: "caption", x: 1040, y: ROW_Y },
  { key: "upload", x: 1360, y: ROW_Y },
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

function parseRuntimeTime(value: string | null | undefined): number | null {
  if (!value) {
    return null;
  }
  const normalized = value.includes("T") ? value : `${value.replace(" ", "T")}+07:00`;
  const timestamp = Date.parse(normalized);
  return Number.isFinite(timestamp) ? timestamp : null;
}

function formatRuntimeClock(value: string | null | undefined): string {
  const timestamp = parseRuntimeTime(value);
  if (timestamp == null) {
    return "-";
  }
  return new Intl.DateTimeFormat("vi-VN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(new Date(timestamp));
}

function formatCountdown(target: string | null | undefined, nowMs: number): string {
  const timestamp = parseRuntimeTime(target);
  if (timestamp == null) {
    return "-";
  }
  const remaining = Math.max(0, Math.ceil((timestamp - nowMs) / 1000));
  const minutes = Math.floor(remaining / 60);
  const seconds = remaining % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

function formatCount(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) {
    return "-";
  }
  return new Intl.NumberFormat("vi-VN").format(value);
}

function parseJsonObject(raw: string | null | undefined): Record<string, unknown> | null {
  if (!raw) {
    return null;
  }
  try {
    const value = JSON.parse(raw) as unknown;
    return value && typeof value === "object" && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

function nodeRunTimeForSort(nodeRun: FlowNodeRunRow): string {
  return nodeRun.ended_at ?? nodeRun.started_at ?? "";
}

function nodeRunDurationSeconds(nodeRun: FlowNodeRunRow): number | null {
  const startedAt = parseRuntimeTime(nodeRun.started_at);
  const endedAt = parseRuntimeTime(nodeRun.ended_at);
  if (startedAt == null || endedAt == null || endedAt < startedAt) {
    return null;
  }
  return Math.floor((endedAt - startedAt) / 1000);
}

function nodeRunStatusLabel(status: string): string {
  if (status === "completed") {
    return "Complete";
  }
  if (status === "failed") {
    return "Failed";
  }
  if (status === "cancelled") {
    return "Cancelled";
  }
  if (status === "running") {
    return "Running";
  }
  return status || "-";
}

function nodeRunStatusTone(status: string): FlowCanvasNodeDetail["tone"] {
  if (status === "completed") {
    return "success";
  }
  if (status === "failed" || status === "cancelled") {
    return "default";
  }
  if (status === "running") {
    return "accent";
  }
  return "muted";
}

function latestNodeRun(
  nodeRuns: FlowNodeRunRow[],
  flowRunId: number | null,
  nodeKey: FlowNodeKey,
): FlowNodeRunRow | null {
  if (flowRunId == null) {
    return null;
  }
  return (
    nodeRuns
      .filter((nodeRun) => nodeRun.flow_run_id === flowRunId && nodeRun.node_key === nodeKey)
      .slice()
      .sort((a, b) => nodeRunTimeForSort(b).localeCompare(nodeRunTimeForSort(a)) || b.id - a.id)[0] ??
    null
  );
}

function findActiveRecordRun(
  runs: FlowRunRow[],
  runtimeSnapshot: FlowRuntimeSnapshot | null,
): Pick<FlowRunRow, "id" | "started_at"> | null {
  const activeRunId = runtimeSnapshot?.active_flow_run_id;
  if (activeRunId != null) {
    return (
      runs.find((run) => run.id === activeRunId) ??
      (runtimeSnapshot?.active_flow_run_started_at
        ? { id: activeRunId, started_at: runtimeSnapshot.active_flow_run_started_at }
        : null)
    );
  }

  return (
    runs
      .filter((run) => run.status === "running")
      .slice()
      .sort((a, b) => b.started_at.localeCompare(a.started_at) || b.id - a.id)[0] ?? null
  );
}

function buildStartNodeDetailLines(
  runtimeSnapshot: FlowRuntimeSnapshot | null,
  nowMs: number,
): FlowCanvasNodeDetail[] {
  if (!runtimeSnapshot) {
    return [];
  }

  const checkState =
    runtimeSnapshot.last_check_live == null
      ? runtimeSnapshot.last_checked_at
        ? "Retrying"
        : "Checking"
      : runtimeSnapshot.last_check_live
        ? "Live found"
        : "Offline";
  const stateTone: FlowCanvasNodeDetail["tone"] =
    runtimeSnapshot.last_check_live == null
      ? "muted"
      : runtimeSnapshot.last_check_live
        ? "success"
        : "default";
  const hasMovedPastStart = runtimeSnapshot.current_node != null && runtimeSnapshot.current_node !== "start";

  return [
    { label: "Status", value: checkState, tone: stateTone },
    {
      label: hasMovedPastStart ? "Next step" : "Next poll",
      value: hasMovedPastStart
        ? runtimeSnapshot.current_node === "record"
          ? "Recording"
          : "Passed"
        : runtimeSnapshot.last_checked_at
          ? formatCountdown(runtimeSnapshot.next_poll_at, nowMs)
          : "Now",
      tone: hasMovedPastStart ? "success" : "accent",
    },
    { label: "Last check", value: formatRuntimeClock(runtimeSnapshot.last_checked_at), tone: "muted" },
    { label: "Last live", value: formatRuntimeClock(runtimeSnapshot.last_live_at), tone: runtimeSnapshot.last_live_at ? "success" : "muted" },
  ];
}

function buildRecordNodeDetailLines(
  runs: FlowRunRow[],
  nodeRuns: FlowNodeRunRow[],
  runtimeSnapshot: FlowRuntimeSnapshot | null,
  nowMs: number,
): FlowCanvasNodeDetail[] {
  const activeRun = findActiveRecordRun(runs, runtimeSnapshot);
  const isRecording = runtimeSnapshot?.status === "recording" && runtimeSnapshot.current_node === "record";
  if (activeRun && isRecording) {
    const startedAtMs = parseRuntimeTime(activeRun.started_at);
    const elapsedSeconds = startedAtMs == null ? 0 : Math.max(0, Math.floor((nowMs - startedAtMs) / 1000));

    return [
      { label: "Elapsed", value: startedAtMs == null ? "-" : formatDuration(elapsedSeconds), tone: "accent" },
      { label: "Viewers", value: formatCount(runtimeSnapshot.active_viewer_count), tone: "success" },
      { label: "Started", value: formatRuntimeClock(activeRun.started_at), tone: "muted" },
      { label: "Run", value: `#${activeRun.id}`, tone: "muted" },
    ];
  }

  const activeRunId = deriveActiveRunId(runs, runtimeSnapshot);
  const latest = latestNodeRun(nodeRuns, activeRunId, "record");
  if (!latest) {
    return [];
  }

  const durationSeconds = nodeRunDurationSeconds(latest);

  return [
    { label: "Status", value: nodeRunStatusLabel(latest.status), tone: nodeRunStatusTone(latest.status) },
    { label: "Duration", value: durationSeconds == null ? "-" : formatDuration(durationSeconds), tone: "accent" },
    { label: "Ended", value: formatRuntimeClock(latest.ended_at), tone: "muted" },
    { label: "Run", value: `#${latest.flow_run_id}`, tone: "muted" },
  ];
}

function buildClipNodeDetailLines(
  runs: FlowRunRow[],
  nodeRuns: FlowNodeRunRow[],
  runtimeSnapshot: FlowRuntimeSnapshot | null,
): FlowCanvasNodeDetail[] {
  const activeRunId = deriveActiveRunId(runs, runtimeSnapshot);
  const clipRuns = activeRunId == null
    ? []
    : nodeRuns.filter((nodeRun) => nodeRun.flow_run_id === activeRunId && nodeRun.node_key === "clip");
  const latest = latestNodeRun(nodeRuns, activeRunId, "clip");
  const isProcessingClip = runtimeSnapshot?.status === "processing" && runtimeSnapshot.current_node === "clip";

  if (!latest && !isProcessingClip) {
    return [];
  }

  const completedClipRuns = clipRuns.filter((nodeRun) => nodeRun.status === "completed").length;
  const failedClipRuns = clipRuns.filter((nodeRun) => nodeRun.status === "failed").length;
  const latestOutput = parseJsonObject(latest?.output_json);
  const latestClipId = latestOutput?.clip_id;
  const status = latest?.status ?? (isProcessingClip ? "running" : "");

  return [
    { label: "Status", value: nodeRunStatusLabel(status), tone: nodeRunStatusTone(status) },
    { label: "Clips", value: formatCount(completedClipRuns), tone: completedClipRuns > 0 ? "success" : "muted" },
    {
      label: failedClipRuns > 0 ? "Failed" : "Last clip",
      value: failedClipRuns > 0
        ? formatCount(failedClipRuns)
        : typeof latestClipId === "number"
          ? `#${latestClipId}`
          : "-",
      tone: failedClipRuns > 0 ? "default" : "muted",
    },
    { label: "Updated", value: formatRuntimeClock(latest?.ended_at ?? latest?.started_at), tone: "muted" },
  ];
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
  const [nowMs, setNowMs] = useState(() => Date.now());

  useEffect(() => {
    const shouldTickForStart = runtimeSnapshot?.current_node === "start" && Boolean(runtimeSnapshot.next_poll_at);
    const shouldTickForRecord = runtimeSnapshot?.current_node === "record" && runtimeSnapshot.status === "recording";
    if (!shouldTickForStart && !shouldTickForRecord) {
      return;
    }
    const timer = window.setInterval(() => {
      setNowMs(Date.now());
    }, 1000);
    return () => {
      window.clearInterval(timer);
    };
  }, [runtimeSnapshot?.current_node, runtimeSnapshot?.next_poll_at, runtimeSnapshot?.status]);

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
    <div className="app-panel-subtle flex h-full min-h-[360px] items-center overflow-x-auto rounded-2xl">
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
          const details =
            key === "start"
              ? buildStartNodeDetailLines(runtimeSnapshot, nowMs)
              : key === "record"
                ? buildRecordNodeDetailLines(flow?.runs ?? [], flow?.nodeRuns ?? [], runtimeSnapshot, nowMs)
                : key === "clip"
                  ? buildClipNodeDetailLines(flow?.runs ?? [], flow?.nodeRuns ?? [], runtimeSnapshot)
                  : [];
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
              details={details}
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
