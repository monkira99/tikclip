import test from "node:test";
import assert from "node:assert/strict";

import type { FlowEditorPayload, FlowRuntimeLogEntry, FlowRuntimeSnapshot, FlowSummary } from "@/types";
import { flowStoreApi, useFlowStore } from "./flow-store.ts";

const FRONTEND_RUNTIME_LOG_CAP = 500;

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

function flowEditorPayload(flowId: number): FlowEditorPayload {
  return {
    flow: {
      id: flowId,
      account_id: 10,
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

test("saveNodeDraft skips unchanged draft JSON without marking dirty", async (t) => {
  resetFlowStore();

  const draft = JSON.stringify({ max_duration_minutes: 5 });
  const activeFlow = flowEditorPayload(7);
  activeFlow.nodes = [
    {
      id: 1,
      flow_id: 7,
      node_key: "record",
      position: 1,
      draft_config_json: draft,
      published_config_json: draft,
      draft_updated_at: "2026-04-19T09:41:12.381+07:00",
      published_at: "2026-04-19T09:41:12.381+07:00",
    },
  ];
  useFlowStore.setState({ activeFlowId: 7, activeFlow, draftDirty: false });
  const saveDraft = t.mock.method(flowStoreApi, "saveFlowNodeDraft", async () => {});

  await useFlowStore.getState().saveNodeDraft({
    flow_id: 7,
    node_key: "record",
    draft_config_json: draft,
  });

  assert.equal(saveDraft.mock.callCount(), 0);
  assert.equal(useFlowStore.getState().draftDirty, false);
});

test("upsertRuntimeSnapshot overlays active detail immediately from runtime events", () => {
  resetFlowStore();

  useFlowStore.setState({
    flows: [flowSummary({ id: 7, status: "idle", current_node: null })],
    activeFlowId: 7,
    activeFlow: flowEditorPayload(7),
  });

  useFlowStore.getState().upsertRuntimeSnapshot({
    flow_id: 7,
    status: "watching",
    current_node: "start",
    account_id: 10,
    username: "demo-account",
    last_live_at: null,
    last_error: null,
    active_flow_run_id: null,
  });

  assert.equal(useFlowStore.getState().runtimeSnapshots[7]?.status, "watching");
  assert.equal(useFlowStore.getState().flows[0]?.status, "watching");
  assert.equal(useFlowStore.getState().flows[0]?.current_node, "start");
  assert.equal(useFlowStore.getState().activeFlow?.flow.status, "watching");
  assert.equal(useFlowStore.getState().activeFlow?.flow.current_node, "start");
});

test("applyRuntimeLogs hydrates logs by flow id", () => {
  resetFlowStore();

  useFlowStore.getState().applyRuntimeLogs([runtimeLogEntry({ id: "log-1", flow_id: 7 })]);

  assert.equal(useFlowStore.getState().runtimeLogs[7]?.length, 1);
});

test("applyRuntimeLogs replaces hydrated flow buckets and preserves other flows", () => {
  resetFlowStore();

  useFlowStore.getState().appendRuntimeLog(runtimeLogEntry({ id: "log-old-7", flow_id: 7 }));
  useFlowStore.getState().appendRuntimeLog(runtimeLogEntry({ id: "log-old-8", flow_id: 8 }));

  useFlowStore.getState().applyRuntimeLogs([
    runtimeLogEntry({ id: "log-new-7", flow_id: 7, context: ["recent", 1] }),
  ]);

  assert.deepEqual(
    useFlowStore.getState().runtimeLogs[7]?.map((entry) => entry.id),
    ["log-new-7"],
  );
  assert.deepEqual(
    useFlowStore.getState().runtimeLogs[8]?.map((entry) => entry.id),
    ["log-old-8"],
  );
});

test("applyRuntimeLogs can clear a hydrated flow bucket with an explicit flow id", () => {
  resetFlowStore();

  useFlowStore.getState().appendRuntimeLog(runtimeLogEntry({ id: "log-old-7", flow_id: 7 }));
  useFlowStore.getState().appendRuntimeLog(runtimeLogEntry({ id: "log-old-8", flow_id: 8 }));

  useFlowStore.getState().applyRuntimeLogs([], { flowIds: [7] });

  assert.deepEqual(useFlowStore.getState().runtimeLogs[7], []);
  assert.deepEqual(
    useFlowStore.getState().runtimeLogs[8]?.map((entry) => entry.id),
    ["log-old-8"],
  );
});

test("appendRuntimeLog appends into the matching flow bucket", () => {
  resetFlowStore();

  useFlowStore.getState().appendRuntimeLog(
    runtimeLogEntry({
      id: "log-2",
      flow_id: 7,
      timestamp: "2026-04-19T09:41:13.381+07:00",
      level: "warn",
      external_recording_id: "rec-42-a1b2",
      event: "sidecar_handoff_failed",
      code: "handoff.sidecar_unavailable",
      message: "Sidecar handoff failed",
      context: { reason: "port_missing" },
    }),
  );

  assert.equal(useFlowStore.getState().runtimeLogs[7]?.length, 1);
  assert.equal(useFlowStore.getState().runtimeLogs[7]?.[0]?.code, "handoff.sidecar_unavailable");
});

test("appendRuntimeLog ignores a duplicate log id after hydration already stored it", () => {
  resetFlowStore();

  useFlowStore.getState().applyRuntimeLogs([
    runtimeLogEntry({
      id: "log-dup-7",
      flow_id: 7,
      event: "record_ready",
      message: "Hydrated first",
    }),
  ]);

  useFlowStore.getState().appendRuntimeLog(
    runtimeLogEntry({
      id: "log-dup-7",
      flow_id: 7,
      event: "record_ready",
      message: "Arrived later from event bus",
    }),
  );

  assert.deepEqual(
    useFlowStore.getState().runtimeLogs[7]?.map((entry) => entry.id),
    ["log-dup-7"],
  );
});

test("appendRuntimeLog keeps only the most recent frontend runtime logs", () => {
  resetFlowStore();

  for (let index = 0; index < FRONTEND_RUNTIME_LOG_CAP + 2; index += 1) {
    useFlowStore.getState().appendRuntimeLog(
      runtimeLogEntry({
        id: `log-${index}`,
        flow_id: 7,
        timestamp: `2026-04-19T09:41:${String(index).padStart(2, "0")}.381+07:00`,
        message: `log-${index}`,
      }),
    );
  }

  const bucket = useFlowStore.getState().runtimeLogs[7] ?? [];
  assert.equal(bucket.length, FRONTEND_RUNTIME_LOG_CAP);
  assert.equal(bucket[0]?.id, "log-2");
  assert.equal(bucket[bucket.length - 1]?.id, `log-${FRONTEND_RUNTIME_LOG_CAP + 1}`);
});

test("applyRuntimeLogs keeps only the most recent hydrated runtime logs", () => {
  resetFlowStore();

  const rows = Array.from({ length: FRONTEND_RUNTIME_LOG_CAP + 3 }, (_, index) =>
    runtimeLogEntry({
      id: `hydrated-${index}`,
      flow_id: 7,
      timestamp: `2026-04-19T09:42:${String(index).padStart(2, "0")}.381+07:00`,
      message: `hydrated-${index}`,
    }),
  );

  useFlowStore.getState().applyRuntimeLogs(rows, { flowIds: [7] });

  const bucket = useFlowStore.getState().runtimeLogs[7] ?? [];
  assert.equal(bucket.length, FRONTEND_RUNTIME_LOG_CAP);
  assert.equal(bucket[0]?.id, "hydrated-3");
  assert.equal(bucket[bucket.length - 1]?.id, `hydrated-${FRONTEND_RUNTIME_LOG_CAP + 2}`);
});

test("fetchRuntimeLogs hydrates authoritative logs for a flow detail path", async (t) => {
  resetFlowStore();

  useFlowStore.getState().appendRuntimeLog(runtimeLogEntry({ id: "log-stale-7", flow_id: 7 }));
  useFlowStore.getState().appendRuntimeLog(runtimeLogEntry({ id: "log-keep-8", flow_id: 8 }));

  const listLogs = t.mock.method(flowStoreApi, "listLiveRuntimeLogs", async (flowId?: number, limit?: number) => {
    assert.equal(flowId, 7);
    assert.equal(limit, 50);
    return [
      runtimeLogEntry({
        id: "log-fresh-7",
        flow_id: 7,
        event: "record_ready",
        message: "Hydrated from Rust ring buffer",
      }),
    ];
  });

  await useFlowStore.getState().fetchRuntimeLogs(7, 50);

  assert.equal(listLogs.mock.callCount(), 1);
  assert.deepEqual(
    useFlowStore.getState().runtimeLogs[7]?.map((entry) => entry.id),
    ["log-fresh-7"],
  );
  assert.deepEqual(
    useFlowStore.getState().runtimeLogs[8]?.map((entry) => entry.id),
    ["log-keep-8"],
  );
});

test("fetchRuntimeLogs can clear an existing flow bucket when hydration returns no logs", async (t) => {
  resetFlowStore();

  useFlowStore.getState().appendRuntimeLog(runtimeLogEntry({ id: "log-old-7", flow_id: 7 }));
  useFlowStore.getState().appendRuntimeLog(runtimeLogEntry({ id: "log-old-8", flow_id: 8 }));

  t.mock.method(flowStoreApi, "listLiveRuntimeLogs", async (flowId?: number) => {
    assert.equal(flowId, 7);
    return [];
  });

  await useFlowStore.getState().fetchRuntimeLogs(7);

  assert.deepEqual(useFlowStore.getState().runtimeLogs[7], []);
  assert.deepEqual(
    useFlowStore.getState().runtimeLogs[8]?.map((entry) => entry.id),
    ["log-old-8"],
  );
});

test("fetchRuntimeLogs preserves logs appended while hydration is in flight", async (t) => {
  resetFlowStore();

  useFlowStore.getState().appendRuntimeLog(runtimeLogEntry({ id: "log-stale-7", flow_id: 7 }));

  let resolveLogs: ((rows: FlowRuntimeLogEntry[]) => void) | null = null;
  t.mock.method(flowStoreApi, "listLiveRuntimeLogs", async (flowId?: number) => {
    assert.equal(flowId, 7);
    return await new Promise<FlowRuntimeLogEntry[]>((resolve) => {
      resolveLogs = resolve;
    });
  });

  const fetchPromise = useFlowStore.getState().fetchRuntimeLogs(7);

  useFlowStore.getState().appendRuntimeLog(
    runtimeLogEntry({
      id: "log-live-7",
      flow_id: 7,
      timestamp: "2026-04-19T09:41:15.381+07:00",
      event: "record_progress",
      message: "Appended from live event while hydrating",
    }),
  );

  if (resolveLogs == null) {
    throw new Error("expected listLiveRuntimeLogs resolver to be captured");
  }
  const resolve: (rows: FlowRuntimeLogEntry[]) => void = resolveLogs;
  resolve([
    runtimeLogEntry({
      id: "log-fresh-7",
      flow_id: 7,
      event: "record_ready",
      message: "Hydrated from Rust ring buffer",
    }),
  ]);

  await fetchPromise;

  assert.deepEqual(
    useFlowStore.getState().runtimeLogs[7]?.map((entry) => entry.id),
    ["log-fresh-7", "log-live-7"],
  );
});

test("fetchRuntimeLogs keeps last known logs and does not throw when hydration fails", async (t) => {
  resetFlowStore();

  useFlowStore.getState().appendRuntimeLog(runtimeLogEntry({ id: "log-last-known-7", flow_id: 7 }));

  t.mock.method(flowStoreApi, "listLiveRuntimeLogs", async () => {
    throw new Error("ring buffer unavailable");
  });

  await assert.doesNotReject(async () => {
    await useFlowStore.getState().fetchRuntimeLogs(7);
  });

  assert.deepEqual(
    useFlowStore.getState().runtimeLogs[7]?.map((entry) => entry.id),
    ["log-last-known-7"],
  );
});

test("fetchRuntimeLogs ignores an older overlapping response for the same flow", async (t) => {
  resetFlowStore();

  let resolveFirst: ((rows: FlowRuntimeLogEntry[]) => void) | null = null;
  let resolveSecond: ((rows: FlowRuntimeLogEntry[]) => void) | null = null;
  let callCount = 0;

  t.mock.method(flowStoreApi, "listLiveRuntimeLogs", async (flowId?: number) => {
    assert.equal(flowId, 7);
    callCount += 1;
    return await new Promise<FlowRuntimeLogEntry[]>((resolve) => {
      if (callCount === 1) {
        resolveFirst = resolve;
        return;
      }
      resolveSecond = resolve;
    });
  });

  const firstFetch = useFlowStore.getState().fetchRuntimeLogs(7);
  const secondFetch = useFlowStore.getState().fetchRuntimeLogs(7);

  if (resolveSecond == null) {
    throw new Error("expected second resolver to be captured");
  }
  const resolveNewest: (rows: FlowRuntimeLogEntry[]) => void = resolveSecond;
  resolveNewest([
    runtimeLogEntry({
      id: "log-newest-7",
      flow_id: 7,
      event: "record_ready",
      message: "Newest hydration wins",
    }),
  ]);
  await secondFetch;

  if (resolveFirst == null) {
    throw new Error("expected first resolver to be captured");
  }
  const resolveStale: (rows: FlowRuntimeLogEntry[]) => void = resolveFirst;
  resolveStale([
    runtimeLogEntry({
      id: "log-stale-7",
      flow_id: 7,
      event: "record_spawned",
      message: "Older hydration must not overwrite",
    }),
  ]);
  await firstFetch;

  assert.deepEqual(
    useFlowStore.getState().runtimeLogs[7]?.map((entry) => entry.id),
    ["log-newest-7"],
  );
});

test("FlowRuntimeLogEntry context accepts non-object JSON values", () => {
  const withNullContext = runtimeLogEntry({ id: "log-null", flow_id: 7, context: null });
  const withArrayContext = runtimeLogEntry({ id: "log-array", flow_id: 7, context: [1, "two", false] });

  assert.equal(withNullContext.context, null);
  assert.deepEqual(withArrayContext.context, [1, "two", false]);
});

test("deleteFlow removes list/runtime state and resets active detail when deleting current flow", async (t) => {
  resetFlowStore();

  const deletedFlowId = 7;
  useFlowStore.setState({
    flows: [flowSummary({ id: deletedFlowId }), flowSummary({ id: 8 })],
    runtimeSnapshots: {
      [deletedFlowId]: {
        flow_id: deletedFlowId,
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
    },
    runtimeLogs: {
      [deletedFlowId]: [runtimeLogEntry({ id: "log-7", flow_id: deletedFlowId })],
      8: [runtimeLogEntry({ id: "log-8", flow_id: 8 })],
    },
    activeFlowId: deletedFlowId,
    activeFlow: flowEditorPayload(deletedFlowId),
    selectedNode: "record",
    editorModalNode: "record",
    draftDirty: true,
    view: "detail",
    error: "old error",
  });

  const deleteMock = t.mock.method(flowStoreApi, "deleteFlow", async () => {});
  await useFlowStore.getState().deleteFlow(deletedFlowId);

  const state = useFlowStore.getState();
  assert.equal(deleteMock.mock.callCount(), 1);
  assert.deepEqual(
    state.flows.map((flow) => flow.id),
    [8],
  );
  assert.equal(state.runtimeSnapshots[deletedFlowId], undefined);
  assert.equal(state.runtimeLogs[deletedFlowId], undefined);
  assert.equal(state.runtimeSnapshots[8]?.flow_id, 8);
  assert.deepEqual(
    state.runtimeLogs[8]?.map((entry) => entry.id),
    ["log-8"],
  );
  assert.equal(state.activeFlowId, null);
  assert.equal(state.activeFlow, null);
  assert.equal(state.selectedNode, null);
  assert.equal(state.editorModalNode, null);
  assert.equal(state.draftDirty, false);
  assert.equal(state.view, "list");
  assert.equal(state.error, null);
});

test("deleteFlow restores previous state and sets error when delete API fails", async (t) => {
  resetFlowStore();

  const before = {
    flows: [flowSummary({ id: 7 }), flowSummary({ id: 8 })],
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
    activeFlowId: 8,
    activeFlow: flowEditorPayload(8),
    editorModalNode: "clip" as const,
    draftDirty: true,
    view: "detail" as const,
  };

  useFlowStore.setState({ ...before, error: null });

  const expectedError = new Error("delete failed");
  t.mock.method(flowStoreApi, "deleteFlow", async () => {
    throw expectedError;
  });

  await assert.rejects(async () => {
    await useFlowStore.getState().deleteFlow(7);
  }, expectedError);

  const state = useFlowStore.getState();
  assert.deepEqual(state.flows, before.flows);
  assert.deepEqual(state.runtimeSnapshots, before.runtimeSnapshots);
  assert.deepEqual(state.runtimeLogs, before.runtimeLogs);
  assert.equal(state.activeFlowId, before.activeFlowId);
  assert.deepEqual(state.activeFlow, before.activeFlow);
  assert.equal(state.editorModalNode, before.editorModalNode);
  assert.equal(state.draftDirty, before.draftDirty);
  assert.equal(state.view, before.view);
  assert.equal(state.error, "delete failed");
});

test("deleteFlow invalidates stale fetchFlows response so deleted flow is not resurrected", async (t) => {
  resetFlowStore();

  useFlowStore.setState({
    flows: [flowSummary({ id: 7 }), flowSummary({ id: 8 })],
  });

  let resolveListFlows: ((rows: FlowSummary[]) => void) | null = null;
  t.mock.method(flowStoreApi, "listFlows", async () => {
    return await new Promise<FlowSummary[]>((resolve) => {
      resolveListFlows = resolve;
    });
  });
  t.mock.method(flowStoreApi, "deleteFlow", async () => {});

  const fetchPromise = useFlowStore.getState().fetchFlows();
  await useFlowStore.getState().deleteFlow(7);

  if (resolveListFlows == null) {
    throw new Error("expected listFlows resolver to be captured");
  }
  const resolve: (rows: FlowSummary[]) => void = resolveListFlows;
  resolve([flowSummary({ id: 7 }), flowSummary({ id: 8 })]);
  await fetchPromise;

  assert.deepEqual(
    useFlowStore.getState().flows.map((flow) => flow.id),
    [8],
  );
});

test("deleteFlow rollback keeps newer active/view state changes made while delete is pending", async (t) => {
  resetFlowStore();

  useFlowStore.setState({
    flows: [flowSummary({ id: 7 }), flowSummary({ id: 8 })],
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
    },
    runtimeLogs: {
      7: [runtimeLogEntry({ id: "log-7", flow_id: 7 })],
    },
    activeFlowId: 7,
    activeFlow: flowEditorPayload(7),
    selectedNode: "record",
    editorModalNode: "record",
    draftDirty: true,
    view: "detail",
  });

  let rejectDelete: ((error: Error) => void) | null = null;
  t.mock.method(flowStoreApi, "deleteFlow", async () => {
    return await new Promise<void>((_resolve, reject) => {
      rejectDelete = reject;
    });
  });

  const deletePromise = useFlowStore.getState().deleteFlow(7);

  useFlowStore.setState({
    activeFlowId: 8,
    activeFlow: flowEditorPayload(8),
    selectedNode: "clip",
    editorModalNode: "clip",
    draftDirty: true,
    view: "detail",
  });

  if (rejectDelete == null) {
    throw new Error("expected deleteFlow rejecter to be captured");
  }
  const reject: (error: Error) => void = rejectDelete;
  reject(new Error("delete failed"));

  await assert.rejects(async () => {
    await deletePromise;
  }, /delete failed/);

  const state = useFlowStore.getState();
  assert.deepEqual(
    state.flows.map((flow) => flow.id),
    [7, 8],
  );
  assert.equal(state.activeFlowId, 8);
  assert.equal(state.activeFlow?.flow.id, 8);
  assert.equal(state.selectedNode, "clip");
  assert.equal(state.editorModalNode, "clip");
  assert.equal(state.draftDirty, true);
  assert.equal(state.view, "detail");
  assert.equal(state.error, "delete failed");
});
