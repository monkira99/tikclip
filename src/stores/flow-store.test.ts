import test from "node:test";
import assert from "node:assert/strict";

import type { FlowRuntimeLogEntry } from "@/types";

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
