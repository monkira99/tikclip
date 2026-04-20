import test from "node:test";
import assert from "node:assert/strict";

import type { FlowContext, FlowRuntimeSnapshot } from "@/types";

import { deriveCanvasNodeStateMap } from "./canvas/flow-canvas-runtime-state";
import { buildRuntimeLogsPanelFlow, shouldFetchDiagnosticsLogs } from "./flow-detail";

function createFlow(overrides: Partial<FlowContext> = {}): FlowContext {
  return {
    id: overrides.id ?? 7,
    account_id: overrides.account_id ?? 44,
    name: overrides.name ?? "Night Shift Recorder",
    enabled: overrides.enabled ?? true,
    status: overrides.status ?? "watching",
    current_node: overrides.current_node ?? "start",
    last_live_at: overrides.last_live_at ?? null,
    last_run_at: overrides.last_run_at ?? "2026-04-19T09:40:00.000+07:00",
    last_error: overrides.last_error ?? "stale error",
    published_version: overrides.published_version ?? 3,
    draft_version: overrides.draft_version ?? 4,
    created_at: overrides.created_at ?? "2026-04-18T22:10:00.000+07:00",
    updated_at: overrides.updated_at ?? "2026-04-19T09:45:12.000+07:00",
  };
}

function createRuntimeSnapshot(
  overrides: Partial<FlowRuntimeSnapshot> = {},
): FlowRuntimeSnapshot {
  return {
    flow_id: overrides.flow_id ?? 7,
    status: overrides.status ?? "processing",
    current_node: overrides.current_node ?? "clip",
    account_id: overrides.account_id ?? 44,
    username: overrides.username ?? "shop_abc",
    last_live_at: overrides.last_live_at ?? "2026-04-19T10:01:02.345+07:00",
    last_error: overrides.last_error ?? null,
    active_flow_run_id: overrides.active_flow_run_id ?? 42,
  };
}

test("buildRuntimeLogsPanelFlow overlays runtime snapshot summary fields onto the panel flow", () => {
  const panelFlow = buildRuntimeLogsPanelFlow(
    createFlow({
      status: "watching",
      current_node: "start",
      last_live_at: null,
      last_error: "stale error",
    }),
    createRuntimeSnapshot({
      status: "processing",
      current_node: "clip",
      last_live_at: "2026-04-19T10:01:02.345+07:00",
      last_error: null,
    }),
  );

  assert.equal(panelFlow.status, "processing");
  assert.equal(panelFlow.current_node, "clip");
  assert.equal(panelFlow.last_live_at, "2026-04-19T10:01:02.345+07:00");
  assert.equal(panelFlow.last_error, null);
});

test("runtime snapshot overlay keeps canvas helper focused on node-level state only", () => {
  const runtimeSnapshot = createRuntimeSnapshot({
    status: "processing",
    current_node: "clip",
    last_live_at: "2026-04-19T10:01:02.345+07:00",
    last_error: null,
  });

  const panelFlow = buildRuntimeLogsPanelFlow(
    createFlow({
      status: "watching",
      current_node: "start",
      last_live_at: null,
      last_error: "stale error",
    }),
    runtimeSnapshot,
  );
  const nodeStateMap = deriveCanvasNodeStateMap({
    runs: [],
    nodeRuns: [],
    runtimeSnapshot,
  });

  assert.equal(panelFlow.current_node, "clip");
  assert.equal(panelFlow.status, "processing");
  assert.equal(nodeStateMap.clip.visualState, "running");
  assert.equal(nodeStateMap.clip.runtimeLabel, "Creating clips");
});

test("shouldFetchDiagnosticsLogs is false before diagnostics open", () => {
  assert.equal(
    shouldFetchDiagnosticsLogs({
      diagnosticsOpen: false,
      runtimeLogs: {},
      flowId: 7,
    }),
    false,
  );
});

test("shouldFetchDiagnosticsLogs is true when diagnostics opens and no logs exist yet", () => {
  assert.equal(
    shouldFetchDiagnosticsLogs({
      diagnosticsOpen: true,
      runtimeLogs: {},
      flowId: 7,
    }),
    true,
  );
});

test("shouldFetchDiagnosticsLogs is false when logs bucket already exists", () => {
  assert.equal(
    shouldFetchDiagnosticsLogs({
      diagnosticsOpen: true,
      runtimeLogs: { 7: [] },
      flowId: 7,
    }),
    false,
  );
});
