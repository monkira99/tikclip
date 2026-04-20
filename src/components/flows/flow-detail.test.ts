import test from "node:test";
import assert from "node:assert/strict";

import type { FlowContext, FlowRuntimeSnapshot } from "@/types";

import { deriveCanvasNodeStateMap } from "./canvas/flow-canvas-runtime-state";
import {
  buildRuntimeLogsPanelFlow,
  buildRuntimeMonitorMetadata,
  shouldFetchDiagnosticsLogs,
} from "./flow-detail";

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
    current_node: "current_node" in overrides ? overrides.current_node ?? null : "clip",
    account_id: overrides.account_id ?? 44,
    username: overrides.username ?? "shop_abc",
    last_live_at:
      "last_live_at" in overrides ? overrides.last_live_at ?? null : "2026-04-19T10:01:02.345+07:00",
    last_error: "last_error" in overrides ? overrides.last_error ?? null : null,
    active_flow_run_id:
      "active_flow_run_id" in overrides ? overrides.active_flow_run_id ?? null : 42,
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

test("buildRuntimeLogsPanelFlow preserves intentional null runtime fields instead of falling back to persisted flow values", () => {
  const panelFlow = buildRuntimeLogsPanelFlow(
    createFlow({
      status: "watching",
      current_node: "start",
      last_live_at: "2026-04-19T09:45:00.000+07:00",
      last_error: "stale error",
    }),
    createRuntimeSnapshot({
      status: "idle",
      current_node: null,
      last_live_at: null,
      last_error: null,
      active_flow_run_id: null,
    }),
  );

  assert.equal(panelFlow.status, "idle");
  assert.equal(panelFlow.current_node, null);
  assert.equal(panelFlow.last_live_at, null);
  assert.equal(panelFlow.last_error, null);
});

test("buildRuntimeLogsPanelFlow keeps disabled status for disabled flows even when runtime snapshot is active", () => {
  for (const runtimeStatus of ["watching", "recording", "processing"] as const) {
    const panelFlow = buildRuntimeLogsPanelFlow(
      createFlow({
        enabled: false,
        status: "disabled",
        current_node: null,
        last_live_at: null,
        last_error: null,
      }),
      createRuntimeSnapshot({
        status: runtimeStatus,
        current_node: "record",
        last_live_at: "2026-04-19T10:01:02.345+07:00",
      }),
    );

    assert.equal(panelFlow.status, "disabled");
    assert.equal(panelFlow.current_node, null);
    assert.equal(panelFlow.last_live_at, null);
    assert.equal(panelFlow.last_error, null);
  }
});

test("buildRuntimeMonitorMetadata clears stale username and run id for disabled flows", () => {
  const metadata = buildRuntimeMonitorMetadata(
    createFlow({
      enabled: false,
      status: "disabled",
      current_node: null,
      last_live_at: null,
      last_error: null,
    }),
    createRuntimeSnapshot({
      status: "recording",
      username: "stale_shop",
      active_flow_run_id: 42,
      current_node: "record",
    }),
  );

  assert.equal(metadata.username, null);
  assert.equal(metadata.activeFlowRunId, null);
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
    flowEnabled: true,
    runs: [],
    nodeRuns: [],
    runtimeSnapshot,
  });

  assert.equal(panelFlow.current_node, "clip");
  assert.equal(panelFlow.status, "processing");
  assert.equal(nodeStateMap.clip.visualState, "running");
  assert.equal(nodeStateMap.clip.runtimeLabel, "Creating clips");
});

test("shouldFetchDiagnosticsLogs is false when diagnostics is closed", () => {
  assert.equal(
    shouldFetchDiagnosticsLogs({
      diagnosticsOpen: false,
      hasFetchedInOpenCycle: false,
    }),
    false,
  );
});

test("shouldFetchDiagnosticsLogs is true on first open in a cycle when the bucket is missing", () => {
  assert.equal(
    shouldFetchDiagnosticsLogs({
      diagnosticsOpen: true,
      hasFetchedInOpenCycle: false,
    }),
    true,
  );
});

test("shouldFetchDiagnosticsLogs is true on first open in a cycle when the bucket is empty", () => {
  assert.equal(
    shouldFetchDiagnosticsLogs({
      diagnosticsOpen: true,
      hasFetchedInOpenCycle: false,
    }),
    true,
  );
});

test("shouldFetchDiagnosticsLogs is true on first open in a cycle when the bucket already has logs", () => {
  assert.equal(
    shouldFetchDiagnosticsLogs({
      diagnosticsOpen: true,
      hasFetchedInOpenCycle: false,
    }),
    true,
  );
});

test("shouldFetchDiagnosticsLogs is false once diagnostics already fetched in the same open cycle", () => {
  assert.equal(
    shouldFetchDiagnosticsLogs({
      diagnosticsOpen: true,
      hasFetchedInOpenCycle: true,
    }),
    false,
  );
});

test("shouldFetchDiagnosticsLogs allows close and reopen to fetch again", () => {
  assert.equal(
    shouldFetchDiagnosticsLogs({
      diagnosticsOpen: true,
      hasFetchedInOpenCycle: false,
    }),
    true,
  );
});
