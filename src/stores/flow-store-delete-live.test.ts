import test from "node:test";
import assert from "node:assert/strict";

import type { FlowEditorPayload, FlowRuntimeLogEntry, FlowRuntimeSnapshot, FlowSummary } from "@/types";

import { flowStoreApi, useFlowStore } from "./flow-store.ts";

function resetFlowStore(): void {
  useFlowStore.setState({
    flows: [],
    runtimeSnapshots: {},
    runtimeLogs: {},
    activeFlowId: null,
    activeFlow: null,
    view: "list",
    selectedNode: null,
    editorModalNode: null,
    publishPending: false,
    draftDirty: false,
    runtimeRefreshTick: 0,
    loading: false,
    error: null,
    filters: {
      search: "",
      status: "all",
    },
  });
}

function runtimeLogEntry(
  overrides: Partial<FlowRuntimeLogEntry> & Pick<FlowRuntimeLogEntry, "id" | "flow_id">,
): FlowRuntimeLogEntry {
  return {
    id: overrides.id,
    timestamp: overrides.timestamp ?? "2026-04-19T09:41:12.381+07:00",
    level: overrides.level ?? "info",
    flow_id: overrides.flow_id,
    flow_run_id: overrides.flow_run_id ?? 42,
    external_recording_id:
      overrides.external_recording_id === undefined ? null : overrides.external_recording_id,
    stage: overrides.stage ?? "record",
    event: overrides.event ?? "record_spawned",
    code: overrides.code === undefined ? null : overrides.code,
    message: overrides.message ?? "Spawned worker",
    context: overrides.context === undefined ? {} : overrides.context,
  };
}

function flowSummary(overrides: Partial<FlowSummary> & Pick<FlowSummary, "id">): FlowSummary {
  return {
    id: overrides.id,
    account_id: overrides.account_id ?? 10,
    account_username: overrides.account_username ?? "demo-account",
    name: overrides.name ?? `Flow ${overrides.id}`,
    enabled: overrides.enabled ?? true,
    status: overrides.status ?? "idle",
    current_node: overrides.current_node ?? "start",
    last_live_at: overrides.last_live_at ?? null,
    last_run_at: overrides.last_run_at ?? null,
    last_error: overrides.last_error ?? null,
    published_version: overrides.published_version ?? 1,
    draft_version: overrides.draft_version ?? 1,
    recordings_count: overrides.recordings_count ?? 0,
    clips_count: overrides.clips_count ?? 0,
    captions_count: overrides.captions_count ?? 0,
    created_at: overrides.created_at ?? "2026-04-19T09:41:12.381+07:00",
    updated_at: overrides.updated_at ?? "2026-04-19T09:41:12.381+07:00",
  };
}

function flowEditorPayload(flowId: number, accountId = 10): FlowEditorPayload {
  return {
    flow: {
      id: flowId,
      account_id: accountId,
      name: `Flow ${flowId}`,
      enabled: true,
      status: "idle",
      current_node: "start",
      last_live_at: null,
      last_run_at: null,
      last_error: null,
      published_version: 1,
      draft_version: 1,
      created_at: "2026-04-19T09:41:12.381+07:00",
      updated_at: "2026-04-19T09:41:12.381+07:00",
    },
    nodes: [],
    runs: [],
    nodeRuns: [],
    recordings_count: 0,
    clips_count: 0,
  };
}

test("deleteFlow removes runtime state for the deleted flow only", async (t) => {
  resetFlowStore();

  useFlowStore.setState({
    flows: [flowSummary({ id: 7, account_id: 10 }), flowSummary({ id: 8, account_id: 11 })],
    runtimeSnapshots: {
      7: {
        flow_id: 7,
        status: "watching",
        current_node: "record",
        account_id: 10,
        username: "demo-account",
        last_live_at: "2026-04-19T09:41:12.381+07:00",
        last_error: null,
        active_flow_run_id: 101,
      },
      8: {
        flow_id: 8,
        status: "idle",
        current_node: "start",
        account_id: 11,
        username: "other-account",
        last_live_at: null,
        last_error: null,
        active_flow_run_id: null,
      },
    } satisfies Record<number, FlowRuntimeSnapshot>,
    runtimeLogs: {
      7: [runtimeLogEntry({ id: "log-7", flow_id: 7 })],
      8: [runtimeLogEntry({ id: "log-8", flow_id: 8 })],
    },
    activeFlowId: 7,
    activeFlow: flowEditorPayload(7, 10),
    selectedNode: "record",
    editorModalNode: "record",
    draftDirty: true,
    view: "detail",
    error: null,
  });

  const previousDeleteFlow = flowStoreApi.deleteFlow;
  flowStoreApi.deleteFlow = async () => {};
  t.after(() => {
    flowStoreApi.deleteFlow = previousDeleteFlow;
  });

  await useFlowStore.getState().deleteFlow(7);

  assert.equal(useFlowStore.getState().flows.some((flow) => flow.id === 7), false);
  assert.equal(useFlowStore.getState().flows.some((flow) => flow.id === 8), true);
  assert.equal(useFlowStore.getState().runtimeSnapshots[7], undefined);
  assert.ok(useFlowStore.getState().runtimeSnapshots[8]);
  assert.equal(useFlowStore.getState().runtimeLogs[7], undefined);
  assert.ok(useFlowStore.getState().runtimeLogs[8]);
});

test("deleteFlow restores flow state when delete fails", async (t) => {
  resetFlowStore();

  useFlowStore.setState({
    flows: [flowSummary({ id: 7, account_id: 20 }), flowSummary({ id: 8, account_id: 21 })],
    runtimeSnapshots: {},
    runtimeLogs: {},
    activeFlowId: 7,
    activeFlow: flowEditorPayload(7, 20),
    selectedNode: "record",
    editorModalNode: "record",
    draftDirty: true,
    view: "detail",
    error: null,
  });

  const previousDeleteFlow = flowStoreApi.deleteFlow;
  flowStoreApi.deleteFlow = async () => {
    throw new Error("delete failed");
  };
  t.after(() => {
    flowStoreApi.deleteFlow = previousDeleteFlow;
  });

  await assert.rejects(async () => {
    await useFlowStore.getState().deleteFlow(7);
  }, /delete failed/);

  assert.equal(useFlowStore.getState().flows.some((flow) => flow.id === 7), true);
  assert.equal(useFlowStore.getState().activeFlowId, 7);
  assert.equal(useFlowStore.getState().selectedNode, "record");
});
