# Flow Canvas Runtime UX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the Flow detail screen so the canvas becomes the primary runtime-monitoring surface, with node-level running/error/done states, a compact runtime strip, and diagnostics moved behind an on-demand dialog.

**Architecture:** Keep the backend unchanged and concentrate the work in the existing Flow detail presentation layer. Derive live node state by combining `runtimeSnapshots[flowId]` with `activeFlow.runs` and `activeFlow.nodeRuns`, render that state directly in `FlowCanvas` / `FlowCanvasNode`, replace the always-open bottom panels with a compact strip, and move existing log diagnostics into a dialog built from the repo's Radix-based `Dialog` primitive.

**Tech Stack:** React 19, TypeScript, Zustand, Radix Dialog, node:test, react-dom/server, Vite

---

## File Structure

**Create:**
- `src/components/flows/canvas/flow-canvas-node.test.tsx` - static markup tests for node runtime visual state labels/classes
- `src/components/flows/canvas/flow-canvas-runtime-state.ts` - focused helpers that derive per-node runtime display state from flow snapshot + run rows
- `src/components/flows/canvas/flow-canvas-runtime-state.test.ts` - unit tests for runtime aggregation rules, including `latest row wins`
- `src/components/flows/runtime/flow-runtime-diagnostics-dialog.tsx` - dialog wrapper around the existing diagnostics/log content
- `src/components/flows/runtime/flow-runtime-strip.tsx` - compact summary strip rendered under the canvas
- `src/components/flows/runtime/flow-runtime-strip.test.tsx` - static markup tests for strip truncation/priority/error presentation

**Modify:**
- `package.json` - add a dedicated frontend test script using the repo's `npm exec --package tsx -- tsx --test ...` pattern so TS/TSX tests run consistently
- `src/index.css` - define the `runtime-pulse` keyframes and reduced-motion-safe animation utility used by running nodes
- `src/components/flows/flow-detail.tsx` - replace fixed bottom runtime panels with canvas + strip + diagnostics dialog wiring
- `src/components/flows/flow-detail.test.ts` - add tests for runtime overlay helpers used by the detail screen
- `src/components/flows/canvas/flow-canvas.tsx` - pass runtime-aware node state into each canvas node and enlarge canvas shell
- `src/components/flows/canvas/flow-canvas-node.tsx` - render running/error/done/idle/selected states with fixed-height content guards
- `src/components/flows/flow-node-utils.ts` - keep only structural utilities or trim old status helper usage if superseded
- `src/components/flows/runtime/runtime-logs-panel.tsx` - adapt the outer layout only as required for dialog embedding and internal scroll containment while preserving diagnostic bundle behavior and current content structure
- `src/components/flows/runtime/runtime-logs-panel.test.tsx` - add coverage for dialog-embedded rendering changes only if the component API changes

**Reference / inspect while implementing:**
- `src/components/ui/dialog.tsx` - current Radix Dialog primitive and a11y behavior
- `src/components/flows/runtime/publish-flow-dialog.tsx` - existing flow-specific dialog pattern
- `src/components/flows/canvas/flow-canvas-layout.ts` - node dimensions that must stay fixed
- `src/types/index.ts` - `FlowRuntimeSnapshot`, `FlowEditorPayload`, `FlowRunRow`, `FlowNodeRunRow`
- `docs/superpowers/plans/2026-04-19-rust-live-ingress-with-full-python-checklive.md` - existing plan precedent for `npm exec --package tsx -- tsx --test ...`

**Test:**
- `package.json` script for frontend component/unit tests
- `src/components/flows/flow-detail.test.ts`
- `src/components/flows/canvas/flow-canvas-runtime-state.test.ts`
- `src/components/flows/canvas/flow-canvas-node.test.tsx`
- `src/components/flows/runtime/flow-runtime-strip.test.tsx`
- `src/components/flows/runtime/runtime-logs-panel.test.tsx` (only if API changes)

---

### Task 1: Derive Canonical Canvas Runtime State

**Files:**
- Modify: `package.json`
- Create: `src/components/flows/canvas/flow-canvas-runtime-state.ts`
- Create: `src/components/flows/canvas/flow-canvas-runtime-state.test.ts`
- Modify: `src/components/flows/flow-detail.test.ts`
- Reference: `src/types/index.ts`

- [ ] **Step 1: Add a dedicated frontend test runner script for TS/TSX tests**

In `package.json`, add this script alongside the existing lint/build scripts:

```json
    "test:js": "npm exec --package tsx -- tsx --test",
```

This plan uses that script for all frontend `.ts` and `.tsx` tests instead of raw `node --test`, because the repo does not currently include a direct TSX-capable node runner.

- [ ] **Step 2: Write failing unit tests for runtime-state aggregation rules**

Create `src/components/flows/canvas/flow-canvas-runtime-state.test.ts` with tests covering:

```ts
import test from "node:test";
import assert from "node:assert/strict";

import type { FlowEditorPayload, FlowRuntimeSnapshot, FlowNodeRunRow, FlowRunRow } from "@/types";

import {
  deriveActiveRunId,
  deriveCanvasNodeStateMap,
  type CanvasNodeVisualState,
} from "./flow-canvas-runtime-state";

function createSnapshot(overrides: Partial<FlowRuntimeSnapshot> = {}): FlowRuntimeSnapshot {
  return {
    flow_id: overrides.flow_id ?? 7,
    status: overrides.status ?? "processing",
    current_node: overrides.current_node ?? "clip",
    account_id: overrides.account_id ?? 44,
    username: overrides.username ?? "shop_abc",
    last_live_at: overrides.last_live_at ?? null,
    last_error: overrides.last_error ?? null,
    active_flow_run_id: overrides.active_flow_run_id ?? 42,
  };
}

function createRun(overrides: Partial<FlowRunRow> & Pick<FlowRunRow, "id">): FlowRunRow {
  return {
    id: overrides.id,
    flow_id: overrides.flow_id ?? 7,
    definition_version: overrides.definition_version ?? 1,
    status: overrides.status ?? "running",
    started_at: overrides.started_at ?? "2026-04-20T12:00:00.000+07:00",
    ended_at: overrides.ended_at ?? null,
    trigger_reason: overrides.trigger_reason ?? "test",
    error: overrides.error ?? null,
  };
}

function createNodeRun(overrides: Partial<FlowNodeRunRow> & Pick<FlowNodeRunRow, "id" | "node_key">): FlowNodeRunRow {
  return {
    id: overrides.id,
    flow_run_id: overrides.flow_run_id ?? 42,
    flow_id: overrides.flow_id ?? 7,
    node_key: overrides.node_key,
    status: overrides.status ?? "completed",
    started_at: overrides.started_at ?? "2026-04-20T12:01:00.000+07:00",
    ended_at: overrides.ended_at ?? "2026-04-20T12:01:10.000+07:00",
    input_json: overrides.input_json ?? null,
    output_json: overrides.output_json ?? null,
    error: overrides.error ?? null,
  };
}

test("deriveActiveRunId prefers runtime snapshot active flow run id", () => {
  const runId = deriveActiveRunId(
    [createRun({ id: 41, status: "completed" }), createRun({ id: 42, status: "running" })],
    createSnapshot({ active_flow_run_id: 99 }),
  );

  assert.equal(runId, 99);
});

test("deriveActiveRunId falls back to the latest running run when snapshot run id is absent", () => {
  const runId = deriveActiveRunId(
    [
      createRun({ id: 41, status: "completed", started_at: "2026-04-20T11:00:00.000+07:00" }),
      createRun({ id: 42, status: "running", started_at: "2026-04-20T12:00:00.000+07:00" }),
    ],
    createSnapshot({ active_flow_run_id: null }),
  );

  assert.equal(runId, 42);
});

test("deriveCanvasNodeStateMap marks the snapshot current node as running", () => {
  const stateMap = deriveCanvasNodeStateMap({
    runs: [createRun({ id: 42 })],
    nodeRuns: [],
    runtimeSnapshot: createSnapshot({ current_node: "record", status: "recording" }),
  });

  assert.equal(stateMap.record.visualState, "running");
  assert.equal(stateMap.record.badgeLabel, "Running");
  assert.equal(stateMap.record.runtimeLabel, "Recording live");
});

test("deriveCanvasNodeStateMap prefers error state over running when snapshot status is error", () => {
  const stateMap = deriveCanvasNodeStateMap({
    runs: [createRun({ id: 42, status: "failed", error: "clip timeout" })],
    nodeRuns: [
      createNodeRun({
        id: 11,
        node_key: "clip",
        status: "failed",
        started_at: "2026-04-20T12:03:00.000+07:00",
        error: "clip timeout",
      }),
    ],
    runtimeSnapshot: createSnapshot({ current_node: "clip", status: "error", active_flow_run_id: 42 }),
  });

  assert.equal(stateMap.clip.visualState, "error");
  assert.equal(stateMap.clip.badgeLabel, "Error");
});

test("deriveCanvasNodeStateMap uses latest row wins for repeated node rows in the same run", () => {
  const stateMap = deriveCanvasNodeStateMap({
    runs: [createRun({ id: 42, status: "running" })],
    nodeRuns: [
      createNodeRun({ id: 10, node_key: "clip", status: "completed", started_at: "2026-04-20T12:02:00.000+07:00" }),
      createNodeRun({ id: 11, node_key: "clip", status: "failed", started_at: "2026-04-20T12:03:00.000+07:00", error: "clip timeout" }),
    ],
    runtimeSnapshot: createSnapshot({ current_node: "caption", status: "processing" }),
  });

  assert.equal(stateMap.clip.visualState, "error");
  assert.equal(stateMap.clip.badgeLabel, "Error");
  assert.equal(stateMap.clip.inlineDetail, "clip timeout");
});

test("deriveCanvasNodeStateMap omits done state when no reliable run-scoped node rows exist", () => {
  const stateMap = deriveCanvasNodeStateMap({
    runs: [],
    nodeRuns: [createNodeRun({ id: 10, flow_run_id: 77, node_key: "clip" })],
    runtimeSnapshot: createSnapshot({ active_flow_run_id: null, current_node: null, status: "processing" }),
  });

  assert.equal(stateMap.clip.visualState, "idle");
  assert.equal(stateMap.clip.badgeLabel, null);
});
```

- [ ] **Step 3: Run the new runtime-state test file and watch it fail**

Run: `npm run test:js -- src/components/flows/canvas/flow-canvas-runtime-state.test.ts`
Expected: FAIL because `flow-canvas-runtime-state.ts` does not exist yet.

- [ ] **Step 4: Implement the minimal runtime-state helper module**

Create `src/components/flows/canvas/flow-canvas-runtime-state.ts` with focused helpers and exported types:

```ts
import { FLOW_NODE_ORDER, FLOW_NODE_LABEL } from "@/components/flows/flow-node-utils";
import type { FlowEditorPayload, FlowNodeKey, FlowNodeRunRow, FlowRuntimeSnapshot, FlowRunRow } from "@/types";

export type CanvasNodeVisualState = "idle" | "running" | "done" | "error";

export type CanvasNodeRuntimeState = {
  visualState: CanvasNodeVisualState;
  badgeLabel: "Running" | "Done" | "Error" | null;
  runtimeLabel: string;
  inlineDetail: string | null;
  activeMarker: boolean;
};

type DeriveCanvasNodeStateArgs = {
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

export function deriveCanvasNodeStateMap({ runs, nodeRuns, runtimeSnapshot }: DeriveCanvasNodeStateArgs): Record<FlowNodeKey, CanvasNodeRuntimeState> {
  const activeRunId = deriveActiveRunId(runs, runtimeSnapshot);
  const latestByKey = latestNodeRunByKey(nodeRuns, activeRunId);
  const currentNode = runtimeSnapshot?.current_node ?? null;
  const snapshotIsError = runtimeSnapshot?.status === "error";

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

      if (currentNode === nodeKey) {
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
```

In `src/components/flows/flow-detail.test.ts`, keep the existing `buildRuntimeLogsPanelFlow()` test and add one focused helper-level assertion that confirms snapshot overlay behavior remains unchanged after moving canvas-specific aggregation into the new helper file.

- [ ] **Step 5: Run the helper tests and verify they pass**

Run: `npm run test:js -- src/components/flows/canvas/flow-canvas-runtime-state.test.ts src/components/flows/flow-detail.test.ts`
Expected: PASS.

- [ ] **Step 6: Commit the runtime-state helper layer**

```bash
git add package.json src/components/flows/canvas/flow-canvas-runtime-state.ts src/components/flows/canvas/flow-canvas-runtime-state.test.ts src/components/flows/flow-detail.test.ts
git commit -m "feat(flows): derive canvas runtime states"
```

### Task 2: Render Runtime-Aware Canvas Nodes

**Files:**
- Modify: `src/index.css`
- Modify: `src/components/flows/canvas/flow-canvas.tsx`
- Modify: `src/components/flows/canvas/flow-canvas-node.tsx`
- Modify: `src/components/flows/flow-node-utils.ts`
- Create: `src/components/flows/canvas/flow-canvas-node.test.tsx`

- [ ] **Step 1: Write failing static-markup tests for node visual states and fixed-height content guards**

Create `src/components/flows/canvas/flow-canvas-node.test.tsx`:

```tsx
import test from "node:test";
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";

import { FlowCanvasNode } from "./flow-canvas-node";

test("FlowCanvasNode renders running badge and active marker even without motion", () => {
  const markup = renderToStaticMarkup(
    <FlowCanvasNode
      nodeKey="record"
      selected={false}
      hasDraftChanges={false}
      runtimeState="Recording live"
      summary="Max 5 min"
      visualState="running"
      badgeLabel="Running"
      inlineDetail={null}
      activeMarker
      onClick={() => {}}
    />,
  );

  assert.match(markup, /Running/);
  assert.match(markup, /Recording live/);
  assert.match(markup, /data-runtime-state="running"/);
  assert.match(markup, /data-active-marker="true"/);
});

test("FlowCanvasNode renders error detail without changing node content structure", () => {
  const markup = renderToStaticMarkup(
    <FlowCanvasNode
      nodeKey="clip"
      selected
      hasDraftChanges={false}
      runtimeState="Clip failed"
      summary="15–45s clips"
      visualState="error"
      badgeLabel="Error"
      inlineDetail="clip timeout"
      activeMarker={false}
      onClick={() => {}}
    />,
  );

  assert.match(markup, /Error/);
  assert.match(markup, /clip timeout/);
  assert.match(markup, /line-clamp-1/);
  assert.match(markup, /data-runtime-state="error"/);
});

test("FlowCanvasNode keeps summary constrained to a two-line clamp", () => {
  const markup = renderToStaticMarkup(
    <FlowCanvasNode
      nodeKey="caption"
      selected={false}
      hasDraftChanges
      runtimeState="Caption complete"
      summary="A very long detail string that should still be constrained by the node surface"
      visualState="done"
      badgeLabel="Done"
      inlineDetail={null}
      activeMarker={false}
      onClick={() => {}}
    />,
  );

  assert.match(markup, /line-clamp-2/);
  assert.match(markup, /Done/);
});
```

- [ ] **Step 2: Run the node test file to verify it fails**

Run: `npm run test:js -- src/components/flows/canvas/flow-canvas-node.test.tsx`
Expected: FAIL because `FlowCanvasNode` does not yet accept the new runtime props.

- [ ] **Step 3: Define the running-node pulse animation in global styles**

In `src/index.css`, add a global keyframes definition and a utility class near the existing app-level utility layer:

```css
@keyframes runtime-pulse {
  0%,
  100% {
    box-shadow:
      0 0 0 1px rgba(255, 99, 99, 0.22),
      0 0 18px rgba(255, 99, 99, 0.12);
  }

  50% {
    box-shadow:
      0 0 0 1px rgba(255, 99, 99, 0.34),
      0 0 30px rgba(255, 99, 99, 0.2);
  }
}

.runtime-pulse {
  animation: runtime-pulse 1.8s ease-in-out infinite;
}

@media (prefers-reduced-motion: reduce) {
  .runtime-pulse {
    animation: none;
  }
}
```

This removes any ambiguity about where the pulse animation is defined.

- [ ] **Step 4: Update `FlowCanvas` and `FlowCanvasNode` to use the derived state map**

In `src/components/flows/canvas/flow-canvas.tsx`, replace the old `runtimeLabel()` usage with the new state helper:

```tsx
import { deriveCanvasNodeStateMap } from "@/components/flows/canvas/flow-canvas-runtime-state";
```

Extend props:

```tsx
export type FlowCanvasProps = {
  flow: FlowEditorPayload | null;
  selectedNode: FlowNodeKey | null;
  runtimeSnapshot: FlowRuntimeSnapshot | null;
  onSelectNode: (node: FlowNodeKey) => void;
};
```

Build node state map once:

```tsx
  const runtimeStateByNode = useMemo(
    () =>
      deriveCanvasNodeStateMap({
        runs: flow?.runs ?? [],
        nodeRuns: flow?.nodeRuns ?? [],
        runtimeSnapshot,
      }),
    [flow?.nodeRuns, flow?.runs, runtimeSnapshot],
  );
```

Pass the derived state into each node and enlarge the shell:

```tsx
  return (
    <div className="app-panel-subtle flex min-h-[360px] items-center overflow-x-auto rounded-2xl">
```

```tsx
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
```

In `src/components/flows/canvas/flow-canvas-node.tsx`, expand props and render runtime layers explicitly:

```tsx
import type { CanvasNodeVisualState } from "@/components/flows/canvas/flow-canvas-runtime-state";
```

```tsx
  visualState: CanvasNodeVisualState;
  badgeLabel: "Running" | "Done" | "Error" | null;
  inlineDetail: string | null;
  activeMarker: boolean;
```

Then change the root classes to encode runtime state without changing height:

```tsx
      data-runtime-state={visualState}
      data-active-marker={activeMarker ? "true" : "false"}
      className={cn(
        "absolute box-border flex min-h-0 flex-col gap-1 overflow-hidden rounded-2xl border px-3 py-2.5 text-left shadow-[inset_0_1px_0_rgba(255,255,255,0.04)] transition-[border-color,background-color,box-shadow]",
        visualState === "running" && "runtime-pulse border-[rgba(255,99,99,0.72)] bg-[rgba(255,99,99,0.08)] shadow-[0_0_0_1px_rgba(255,99,99,0.22),0_0_28px_rgba(255,99,99,0.12)]",
        visualState === "done" && "border-[rgba(95,201,146,0.35)] bg-[rgba(95,201,146,0.06)]",
        visualState === "error" && "border-[rgba(255,99,99,0.58)] bg-[rgba(255,99,99,0.07)] shadow-[0_0_0_1px_rgba(255,99,99,0.12)]",
        visualState === "idle" && "border-[var(--color-border)] bg-white/[0.04] hover:border-[color-mix(in_oklab,var(--color-accent)_22%,var(--color-border))] hover:bg-white/[0.06]",
        selected && "ring-1 ring-[color-mix(in_oklab,var(--color-accent)_30%,transparent)]",
      )}
```

Render the marker and constrained text:

```tsx
        <div className="flex items-start justify-between gap-2">
          <div className="flex min-w-0 items-center gap-2">
            <span
              aria-hidden
              className={cn(
                "size-2 shrink-0 rounded-full border border-white/10 bg-white/12",
                activeMarker && "bg-[var(--color-primary)] shadow-[0_0_0_4px_rgba(255,99,99,0.12)]",
              )}
            />
            <span className="truncate text-[11px] font-semibold uppercase tracking-[0.1em] text-[var(--color-text-muted)]">
              {label}
            </span>
          </div>
          {badgeLabel ? (
            <span className="shrink-0 rounded-md border border-white/10 bg-white/[0.04] px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-[var(--color-text-soft)]">
              {badgeLabel}
            </span>
          ) : hasDraftChanges ? (
            <span className="shrink-0 rounded-md border border-[rgba(255,188,51,0.25)] bg-[rgba(255,188,51,0.1)] px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-[var(--color-text-soft)]">
              Draft
            </span>
          ) : null}
        </div>
        <p className="line-clamp-1 text-[10px] font-medium leading-snug text-[var(--color-text-soft)]">{runtimeState}</p>
        <p className="line-clamp-2 font-mono text-[10px] leading-relaxed text-[var(--color-text-muted)]">{summary || "—"}</p>
        {inlineDetail ? (
          <p className="line-clamp-1 text-[10px] leading-snug text-[var(--color-primary)]">{inlineDetail}</p>
        ) : null}
```

In `src/components/flows/flow-node-utils.ts`, remove `getFlowNodeStatus()` only if no longer used anywhere after the canvas change. If another component still imports it, keep it.

- [ ] **Step 5: Run the canvas tests and confirm they pass**

Run: `npm run test:js -- src/components/flows/canvas/flow-canvas-runtime-state.test.ts src/components/flows/canvas/flow-canvas-node.test.tsx`
Expected: PASS.

- [ ] **Step 6: Commit the canvas runtime-state UI changes**

```bash
git add src/index.css src/components/flows/canvas/flow-canvas.tsx src/components/flows/canvas/flow-canvas-node.tsx src/components/flows/canvas/flow-canvas-node.test.tsx src/components/flows/canvas/flow-canvas-runtime-state.ts src/components/flows/canvas/flow-canvas-runtime-state.test.ts src/components/flows/flow-node-utils.ts
git commit -m "feat(flows): surface runtime state on canvas nodes"
```

### Task 3: Replace Bottom Panels With Runtime Strip And Diagnostics Dialog

**Files:**
- Create: `src/components/flows/runtime/flow-runtime-strip.tsx`
- Create: `src/components/flows/runtime/flow-runtime-strip.test.tsx`
- Create: `src/components/flows/runtime/flow-runtime-diagnostics-dialog.tsx`
- Modify: `src/components/flows/flow-detail.tsx`
- Modify: `src/components/flows/flow-detail.test.ts`
- Modify: `src/components/flows/runtime/runtime-logs-panel.tsx` (only if dialog embedding requires API/layout tweaks)
- Modify: `src/components/flows/runtime/runtime-logs-panel.test.tsx` (only if API changes)
- Reference: `src/components/ui/dialog.tsx`
- Reference: `src/components/flows/runtime/publish-flow-dialog.tsx`

- [ ] **Step 1: Write failing static-markup tests for the compact strip**

Create `src/components/flows/runtime/flow-runtime-strip.test.tsx`:

```tsx
import test from "node:test";
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";

import type { FlowContext } from "@/types";

import { FlowRuntimeStrip } from "./flow-runtime-strip";

function createFlow(overrides: Partial<FlowContext> = {}): FlowContext {
  return {
    id: overrides.id ?? 7,
    account_id: overrides.account_id ?? 44,
    name: overrides.name ?? "Night Shift Recorder",
    enabled: overrides.enabled ?? true,
    status: overrides.status ?? "processing",
    current_node: overrides.current_node ?? "caption",
    last_live_at: overrides.last_live_at ?? null,
    last_run_at: overrides.last_run_at ?? null,
    last_error: overrides.last_error ?? null,
    published_version: overrides.published_version ?? 1,
    draft_version: overrides.draft_version ?? 1,
    created_at: overrides.created_at ?? "2026-04-20T12:00:00.000+07:00",
    updated_at: overrides.updated_at ?? "2026-04-20T12:00:00.000+07:00",
  };
}

test("FlowRuntimeStrip prioritizes status, current node, and diagnostics trigger", () => {
  const markup = renderToStaticMarkup(
    <FlowRuntimeStrip
      flow={createFlow({ current_node: "record", status: "recording" })}
      username="shop_abc"
      activeFlowRunId={42}
      onOpenDiagnostics={() => {}}
    />,
  );

  assert.match(markup, /recording/i);
  assert.match(markup, /record/i);
  assert.match(markup, /Diagnostics/);
});

test("FlowRuntimeStrip line-clamps the last error and keeps diagnostics visible", () => {
  const markup = renderToStaticMarkup(
    <FlowRuntimeStrip
      flow={createFlow({
        status: "error",
        current_node: null,
        last_error: "A very long orchestration error that should not turn the strip into a second dashboard",
      })}
      username="shop_abc"
      activeFlowRunId={42}
      onOpenDiagnostics={() => {}}
    />,
  );

  assert.match(markup, /line-clamp-1/);
  assert.match(markup, /Diagnostics/);
});
```

In `src/components/flows/flow-detail.test.ts`, add lazy-fetch behavior coverage for diagnostics logs with extracted effect helpers from `flow-detail.tsx`:

```ts
import {
  buildRuntimeLogsPanelFlow,
  shouldFetchDiagnosticsLogs,
} from "./flow-detail";

test("shouldFetchDiagnosticsLogs returns false before diagnostics open", () => {
  assert.equal(
    shouldFetchDiagnosticsLogs({
      diagnosticsOpen: false,
      flowId: 7,
      runtimeLogs: {},
    }),
    false,
  );
});

test("shouldFetchDiagnosticsLogs returns true when diagnostics opens and no logs exist yet", () => {
  assert.equal(
    shouldFetchDiagnosticsLogs({
      diagnosticsOpen: true,
      flowId: 7,
      runtimeLogs: {},
    }),
    true,
  );
});

test("shouldFetchDiagnosticsLogs reuses existing log bucket when logs already exist", () => {
  assert.equal(
    shouldFetchDiagnosticsLogs({
      diagnosticsOpen: true,
      flowId: 7,
      runtimeLogs: {
        7: [
          {
            id: "log-1",
            timestamp: "2026-04-20T12:00:00.000+07:00",
            level: "info",
            flow_id: 7,
            flow_run_id: 42,
            external_recording_id: null,
            stage: "record",
            event: "record_spawned",
            code: null,
            message: "Spawned Rust-owned recording worker",
            context: { room_id: "7312345" },
          },
        ],
      },
    }),
    false,
  );
});
```

- [ ] **Step 2: Run the strip test file to verify it fails**

Run: `npm run test:js -- src/components/flows/runtime/flow-runtime-strip.test.tsx src/components/flows/flow-detail.test.ts`
Expected: FAIL because `flow-runtime-strip.tsx` does not exist yet, while `flow-detail.test.ts` captures the diagnostics lazy-fetch helper coverage added in this task.

- [ ] **Step 3: Implement the strip and diagnostics dialog, then rewire `FlowDetail`**

Create `src/components/flows/runtime/flow-runtime-strip.tsx`:

```tsx
import { Button } from "@/components/ui/button";
import { FLOW_NODE_LABEL } from "@/components/flows/flow-node-utils";
import type { FlowContext } from "@/types";

type FlowRuntimeStripProps = {
  flow: FlowContext;
  username: string | null;
  activeFlowRunId: number | null;
  onOpenDiagnostics: () => void;
};

export function FlowRuntimeStrip({ flow, username, activeFlowRunId, onOpenDiagnostics }: FlowRuntimeStripProps) {
  const currentNodeLabel = flow.current_node ? FLOW_NODE_LABEL[flow.current_node] : "No active node";

  return (
    <section className="app-panel-subtle rounded-2xl px-4 py-3">
      <div className="flex min-w-0 flex-wrap items-start justify-between gap-3">
        <div className="flex min-w-0 flex-1 flex-wrap items-center gap-2 text-xs text-[var(--color-text-soft)]">
          <span className="rounded-md border border-[var(--color-border)] bg-white/[0.03] px-2 py-1 font-semibold capitalize text-[var(--color-text)]">
            {flow.status}
          </span>
          <span className="rounded-md border border-[var(--color-border)] bg-white/[0.02] px-2 py-1">
            Node: {currentNodeLabel}
          </span>
          {flow.last_error ? (
            <span className="min-w-0 max-w-full rounded-md border border-[rgba(255,99,99,0.28)] bg-[rgba(255,99,99,0.08)] px-2 py-1 text-[var(--color-primary)]">
              <span className="line-clamp-1 block min-w-0">{flow.last_error}</span>
            </span>
          ) : (
            <>
              <span className="min-w-0 truncate rounded-md border border-[var(--color-border)] bg-white/[0.02] px-2 py-1">
                User: {username ?? "-"}
              </span>
              <span className="min-w-0 truncate rounded-md border border-[var(--color-border)] bg-white/[0.02] px-2 py-1">
                Run: {activeFlowRunId ?? "-"}
              </span>
            </>
          )}
        </div>
        <Button type="button" size="sm" variant="outline" onClick={onOpenDiagnostics}>
          Diagnostics
        </Button>
      </div>
    </section>
  );
}
```

Create `src/components/flows/runtime/flow-runtime-diagnostics-dialog.tsx`:

```tsx
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { FlowContext, FlowRuntimeLogEntry } from "@/types";
import { RuntimeLogsPanel } from "./runtime-logs-panel";

type FlowRuntimeDiagnosticsDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  flow: FlowContext;
  logs: FlowRuntimeLogEntry[];
  username: string | null;
  activeFlowRunId: number | null;
};

export function FlowRuntimeDiagnosticsDialog({
  open,
  onOpenChange,
  flow,
  logs,
  username,
  activeFlowRunId,
}: FlowRuntimeDiagnosticsDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-full max-w-[min(72rem,calc(100vw-2rem))] gap-0 p-0 sm:max-w-[min(72rem,calc(100vw-2rem))]">
        <div className="p-4">
          <DialogHeader className="space-y-2 text-left">
            <DialogTitle className="text-base font-semibold tracking-[0.01em] text-[var(--color-text)]">
              Flow diagnostics
            </DialogTitle>
            <DialogDescription className="text-left text-sm leading-relaxed text-[var(--color-text-muted)]">
              Runtime logs and diagnostic bundle for the current flow.
            </DialogDescription>
          </DialogHeader>
        </div>
        <div className="max-h-[min(78vh,56rem)] overflow-auto px-4 pb-4">
          <RuntimeLogsPanel flow={flow} logs={logs} username={username} activeFlowRunId={activeFlowRunId} />
        </div>
      </DialogContent>
    </Dialog>
  );
}
```

Then update `src/components/flows/flow-detail.tsx`:

```tsx
import { FlowRuntimeDiagnosticsDialog } from "@/components/flows/runtime/flow-runtime-diagnostics-dialog";
import { FlowRuntimeStrip } from "@/components/flows/runtime/flow-runtime-strip";
```

Remove imports for `FlowRuntimeLanes`, `FlowRuntimeTimeline`, and direct `RuntimeLogsPanel` usage from the default layout.

Add local state:

```tsx
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);
```

Replace the eager runtime log effect with an on-demand fetch effect:

```tsx
  useEffect(() => {
    if (!shouldFetchDiagnosticsLogs({
      diagnosticsOpen,
      flowId,
      runtimeLogs: useFlowStore.getState().runtimeLogs,
    })) {
      return;
    }
    void fetchRuntimeLogs(flowId);
  }, [diagnosticsOpen, flowId, fetchRuntimeLogs]);
```

Add the extracted helper near `buildRuntimeLogsPanelFlow()` in `src/components/flows/flow-detail.tsx`:

```ts
export function shouldFetchDiagnosticsLogs(args: {
  diagnosticsOpen: boolean;
  flowId: number;
  runtimeLogs: Record<number, FlowRuntimeLogEntry[]>;
}): boolean {
  if (!args.diagnosticsOpen) {
    return false;
  }

  return (args.runtimeLogs[args.flowId] ?? []).length === 0;
}
```

Delete the old mount-time effect:

```tsx
  useEffect(() => {
    void fetchRuntimeLogs(flowId);
  }, [flowId, fetchRuntimeLogs]);
```

This makes diagnostics logs truly lazy-loaded while reusing any already-fetched bucket.

Pass `runtimeSnapshot` into the canvas:

```tsx
      <FlowCanvas
        flow={flow}
        selectedNode={selectedNode}
        runtimeSnapshot={runtimeSnapshot}
        onSelectNode={handleCanvasSelect}
      />
```

Replace the old bottom panels with:

```tsx
      {flow ? (
        <FlowRuntimeStrip
          flow={runtimePanelFlow ?? flow.flow}
          username={runtimeSnapshot?.username ?? null}
          activeFlowRunId={runtimeSnapshot?.active_flow_run_id ?? null}
          onOpenDiagnostics={() => setDiagnosticsOpen(true)}
        />
      ) : null}
```

```tsx
      {flow ? (
        <FlowRuntimeDiagnosticsDialog
          open={diagnosticsOpen}
          onOpenChange={setDiagnosticsOpen}
          flow={runtimePanelFlow ?? flow.flow}
          logs={flowLogs}
          username={runtimeSnapshot?.username ?? null}
          activeFlowRunId={runtimeSnapshot?.active_flow_run_id ?? null}
        />
      ) : null}
```

If `RuntimeLogsPanel` already scrolls cleanly when embedded, leave its API unchanged. Only make minimal layout changes if the dialog embedding shows nested overflow problems.

- [ ] **Step 4: Run the strip/log/detail tests and frontend verification**

Run: `npm run test:js -- src/components/flows/runtime/flow-runtime-strip.test.tsx src/components/flows/runtime/runtime-logs-panel.test.tsx src/components/flows/flow-detail.test.ts`
Expected: PASS.

Run: `npm run lint:js`
Expected: PASS.

- [ ] **Step 5: Commit the detail-layout and diagnostics changes**

```bash
git add src/components/flows/flow-detail.tsx src/components/flows/flow-detail.test.ts src/components/flows/runtime/flow-runtime-strip.tsx src/components/flows/runtime/flow-runtime-strip.test.tsx src/components/flows/runtime/flow-runtime-diagnostics-dialog.tsx src/components/flows/runtime/runtime-logs-panel.tsx src/components/flows/runtime/runtime-logs-panel.test.tsx
git commit -m "feat(flows): make canvas the runtime monitoring surface"
```

### Task 4: Final Verification And UX Regression Pass

**Files:**
- Modify: none, unless verification reveals a small follow-up issue
- Verify: flow detail rendering, diagnostics dialog, canvas node state semantics

- [ ] **Step 1: Run the complete frontend verification for touched files**

Run: `npm run test:js -- src/components/flows/canvas/flow-canvas-runtime-state.test.ts src/components/flows/canvas/flow-canvas-node.test.tsx src/components/flows/runtime/flow-runtime-strip.test.tsx src/components/flows/runtime/runtime-logs-panel.test.tsx src/components/flows/flow-detail.test.ts`
Expected: PASS.

Run: `npm run lint:js`
Expected: PASS, including the production build.

- [ ] **Step 2: Manually verify the redesigned Flow detail experience**

Open the app in the normal local workflow and check this exact list:

```text
1. Open a flow detail screen and confirm the canvas occupies more visual space than before.
2. Confirm the old Runtime timeline / Node lanes / always-open Runtime Logs blocks are no longer visible below the canvas.
3. Confirm the current active node is immediately identifiable on the canvas.
4. Confirm the running node shows a red pulsing treatment plus a persistent Running label/marker.
5. Confirm selected nodes still look selected without erasing running/error meaning.
6. Confirm completed nodes show a done state only when the latest node-run row for that node in the selected run context is completed.
7. Confirm a node with `runtimeSnapshot.status === "error"` and matching `current_node` renders as Error, not Running.
8. Confirm node text stays within fixed-height cards and does not resize the canvas layout.
9. Confirm logs are not fetched until Diagnostics is opened, and reopening Diagnostics reuses the existing log bucket.
10. Confirm the runtime strip remains compact and the Diagnostics button is always visible.
11. Open Diagnostics and confirm focus moves into the dialog, Escape closes it, and the log area scrolls internally.
12. Reopen Diagnostics from keyboard focus and confirm focus returns to the trigger after close.
```

Expected: all items pass.

- [ ] **Step 3: Review the final diff for scope discipline**

Run: `git diff -- src/components/flows/flow-detail.tsx src/components/flows/flow-detail.test.ts src/components/flows/canvas/flow-canvas.tsx src/components/flows/canvas/flow-canvas-node.tsx src/components/flows/canvas/flow-canvas-node.test.tsx src/components/flows/canvas/flow-canvas-runtime-state.ts src/components/flows/canvas/flow-canvas-runtime-state.test.ts src/components/flows/runtime/flow-runtime-strip.tsx src/components/flows/runtime/flow-runtime-strip.test.tsx src/components/flows/runtime/flow-runtime-diagnostics-dialog.tsx src/components/flows/runtime/runtime-logs-panel.tsx src/components/flows/runtime/runtime-logs-panel.test.tsx src/components/flows/flow-node-utils.ts`
Expected: only the planned Flow UX redesign changes appear.

- [ ] **Step 4: Commit any verification-driven polish fix when manual verification reveals one**

```bash
git add src/components/flows/flow-detail.tsx src/components/flows/flow-detail.test.ts src/components/flows/canvas/flow-canvas.tsx src/components/flows/canvas/flow-canvas-node.tsx src/components/flows/canvas/flow-canvas-node.test.tsx src/components/flows/canvas/flow-canvas-runtime-state.ts src/components/flows/canvas/flow-canvas-runtime-state.test.ts src/components/flows/runtime/flow-runtime-strip.tsx src/components/flows/runtime/flow-runtime-strip.test.tsx src/components/flows/runtime/flow-runtime-diagnostics-dialog.tsx src/components/flows/runtime/runtime-logs-panel.tsx src/components/flows/runtime/runtime-logs-panel.test.tsx src/components/flows/flow-node-utils.ts
git commit -m "fix(flows): polish canvas runtime monitoring UX"
```

Skip this step if verification does not require any further changes.

---

## Self-Review

- Spec coverage check: the plan covers canvas-first layout, red pulsing running node, diagnostics moved behind a dialog, runtime strip rules, reduced reliance on permanent bottom panels, `latest row wins` node-run aggregation, and fixed-height node content guards.
- Placeholder scan: no `TODO`, `TBD`, or vague “implement appropriately” steps remain; each task points to exact files and concrete code.
- Type consistency: the plan uses one consistent vocabulary for the new UI state layer: `CanvasNodeRuntimeState`, `visualState`, `badgeLabel`, `inlineDetail`, `activeMarker`, and `FlowRuntimeDiagnosticsDialog`.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-20-flow-canvas-runtime-ux.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
