import assert from "node:assert/strict";
import test from "node:test";

import type { FlowNodeRunRow, FlowRuntimeSnapshot, FlowRunRow } from "@/types";

import { deriveActiveRunId, deriveCanvasNodeStateMap } from "./flow-canvas-runtime-state";

function createSnapshot(overrides: Partial<FlowRuntimeSnapshot> = {}): FlowRuntimeSnapshot {
  return {
    flow_id: overrides.flow_id ?? 7,
    status: overrides.status ?? "processing",
    current_node: "current_node" in overrides ? overrides.current_node ?? null : "clip",
    account_id: overrides.account_id ?? 44,
    username: overrides.username ?? "shop_abc",
    last_live_at: overrides.last_live_at ?? null,
    last_error: overrides.last_error ?? null,
    active_flow_run_id: "active_flow_run_id" in overrides ? overrides.active_flow_run_id ?? null : 42,
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

function createNodeRun(
  overrides: Partial<FlowNodeRunRow> & Pick<FlowNodeRunRow, "id" | "node_key">,
): FlowNodeRunRow {
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

test("deriveActiveRunId falls back to latest running run", () => {
  const runId = deriveActiveRunId(
    [
      createRun({ id: 41, status: "completed", started_at: "2026-04-20T11:00:00.000+07:00" }),
      createRun({ id: 42, status: "running", started_at: "2026-04-20T12:00:00.000+07:00" }),
    ],
    createSnapshot({ active_flow_run_id: null }),
  );

  assert.equal(runId, 42);
});

test("deriveCanvasNodeStateMap marks snapshot current node as running", () => {
  const stateMap = deriveCanvasNodeStateMap({
    flowEnabled: true,
    runs: [createRun({ id: 42 })],
    nodeRuns: [],
    runtimeSnapshot: createSnapshot({ current_node: "record", status: "recording" }),
  });

  assert.equal(stateMap.record.visualState, "running");
  assert.equal(stateMap.record.badgeLabel, "Running");
  assert.equal(stateMap.record.runtimeLabel, "Recording live");
  assert.equal(stateMap.start.visualState, "done");
  assert.equal(stateMap.start.badgeLabel, "Done");
});

test("deriveCanvasNodeStateMap prefers error over running when snapshot status is error", () => {
  const stateMap = deriveCanvasNodeStateMap({
    flowEnabled: true,
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

test("deriveCanvasNodeStateMap uses latest row wins for repeated node rows in same run", () => {
  const stateMap = deriveCanvasNodeStateMap({
    flowEnabled: true,
    runs: [createRun({ id: 42, status: "running" })],
    nodeRuns: [
      createNodeRun({
        id: 10,
        node_key: "clip",
        status: "completed",
        started_at: "2026-04-20T12:02:00.000+07:00",
      }),
      createNodeRun({
        id: 11,
        node_key: "clip",
        status: "failed",
        started_at: "2026-04-20T12:03:00.000+07:00",
        error: "clip timeout",
      }),
    ],
    runtimeSnapshot: createSnapshot({ current_node: "caption", status: "processing" }),
  });

  assert.equal(stateMap.clip.visualState, "error");
  assert.equal(stateMap.clip.badgeLabel, "Error");
  assert.equal(stateMap.clip.inlineDetail, "clip timeout");
});

test("deriveCanvasNodeStateMap omits done state when no reliable run-scoped node rows exist", () => {
  const stateMap = deriveCanvasNodeStateMap({
    flowEnabled: true,
    runs: [],
    nodeRuns: [createNodeRun({ id: 10, flow_run_id: 77, node_key: "clip" })],
    runtimeSnapshot: createSnapshot({ active_flow_run_id: null, current_node: null, status: "processing" }),
  });

  assert.equal(stateMap.clip.visualState, "idle");
  assert.equal(stateMap.clip.badgeLabel, null);
});

test("deriveCanvasNodeStateMap does not render running for idle or disabled snapshots", () => {
  const idleStateMap = deriveCanvasNodeStateMap({
    flowEnabled: true,
    runs: [],
    nodeRuns: [],
    runtimeSnapshot: createSnapshot({ status: "idle", current_node: "clip", active_flow_run_id: null }),
  });
  const disabledStateMap = deriveCanvasNodeStateMap({
    flowEnabled: true,
    runs: [],
    nodeRuns: [],
    runtimeSnapshot: createSnapshot({ status: "disabled", current_node: "record", active_flow_run_id: null }),
  });

  assert.equal(idleStateMap.clip.visualState, "idle");
  assert.equal(idleStateMap.clip.badgeLabel, null);
  assert.equal(disabledStateMap.record.visualState, "idle");
  assert.equal(disabledStateMap.record.badgeLabel, null);
});

test("deriveCanvasNodeStateMap ignores stale active runtime snapshot cues when the flow is disabled", () => {
  const stateMap = deriveCanvasNodeStateMap({
    flowEnabled: false,
    runs: [createRun({ id: 42, status: "running" })],
    nodeRuns: [
      createNodeRun({
        id: 10,
        node_key: "record",
        status: "failed",
        error: "stale recorder error",
      }),
    ],
    runtimeSnapshot: createSnapshot({
      status: "recording",
      current_node: "record",
      last_error: "stale recorder error",
      active_flow_run_id: 42,
    }),
  });

  assert.equal(stateMap.record.visualState, "idle");
  assert.equal(stateMap.record.badgeLabel, null);
  assert.equal(stateMap.record.activeMarker, false);
});

test("deriveCanvasNodeStateMap does not leak historical node rows into active snapshots without confirmed run id", () => {
  const stateMap = deriveCanvasNodeStateMap({
    flowEnabled: true,
    runs: [createRun({ id: 41, status: "completed", started_at: "2026-04-20T11:00:00.000+07:00" })],
    nodeRuns: [
      createNodeRun({
        id: 10,
        flow_run_id: 41,
        node_key: "clip",
        status: "completed",
        started_at: "2026-04-20T11:01:00.000+07:00",
      }),
      createNodeRun({
        id: 11,
        flow_run_id: 41,
        node_key: "caption",
        status: "failed",
        started_at: "2026-04-20T11:02:00.000+07:00",
        error: "old caption error",
      }),
    ],
    runtimeSnapshot: createSnapshot({
      status: "processing",
      current_node: "record",
      active_flow_run_id: null,
    }),
  });

  assert.equal(stateMap.record.visualState, "running");
  assert.equal(stateMap.start.visualState, "done");
  assert.equal(stateMap.clip.visualState, "idle");
  assert.equal(stateMap.clip.badgeLabel, null);
  assert.equal(stateMap.caption.visualState, "idle");
  assert.equal(stateMap.caption.badgeLabel, null);
});
