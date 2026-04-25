import { create } from "zustand";

import {
  createFlow,
  deleteFlow,
  getFlowDefinition,
  listLiveRuntimeLogs,
  listLiveRuntimeSessions,
  listFlows,
  publishFlowDefinition,
  restartFlowRun,
  saveFlowNodeDraft,
  setFlowEnabled,
} from "@/lib/api";
import type {
  CreateFlowInput,
  FlowEditorPayload,
  FlowNodeKey,
  FlowRuntimeLogEntry,
  FlowRuntimeSnapshot,
  FlowStatus,
  FlowSummary,
  PublishFlowResult,
} from "@/types";

type FlowView = "list" | "detail";

type FlowFilters = {
  search: string;
  status: FlowStatus | "all";
};

type FlowStore = {
  flows: FlowSummary[];
  runtimeSnapshots: Record<number, FlowRuntimeSnapshot>;
  runtimeLogs: Record<number, FlowRuntimeLogEntry[]>;
  activeFlowId: number | null;
  activeFlow: FlowEditorPayload | null;
  view: FlowView;
  selectedNode: FlowNodeKey | null;
  editorModalNode: FlowNodeKey | null;
  publishPending: boolean;
  draftDirty: boolean;
  runtimeRefreshTick: number;
  loading: boolean;
  error: string | null;
  filters: FlowFilters;

  fetchFlows: (opts?: { quiet?: boolean }) => Promise<void>;
  fetchFlowDetail: (flowId?: number, opts?: { quiet?: boolean }) => Promise<void>;
  setActiveFlowId: (flowId: number | null) => void;
  setView: (view: FlowView) => void;
  setSelectedNode: (node: FlowNodeKey | null) => void;
  setFilters: (partial: Partial<FlowFilters>) => void;
  openNodeModal: (node: FlowNodeKey) => void;
  closeNodeModal: () => void;
  saveNodeDraft: (input: {
    flow_id: number;
    node_key: FlowNodeKey;
    draft_config_json: string;
  }) => Promise<void>;
  publishFlow: (flowId: number, options: { restartCurrentRun: boolean }) => Promise<PublishFlowResult>;
  applyRuntimeSnapshots: (rows: FlowRuntimeSnapshot[]) => void;
  upsertRuntimeSnapshot: (row: FlowRuntimeSnapshot) => void;
  applyRuntimeLogs: (rows: FlowRuntimeLogEntry[], options?: { flowIds?: number[] }) => void;
  appendRuntimeLog: (row: FlowRuntimeLogEntry) => void;
  fetchRuntimeLogs: (flowId: number, limit?: number) => Promise<void>;
  refreshRuntime: () => Promise<void>;
  toggleFlowEnabled: (flowId: number, enabled: boolean) => Promise<void>;
  createFlow: (input: CreateFlowInput) => Promise<number>;
  deleteFlow: (flowId: number) => Promise<void>;
};

export const flowStoreApi = {
  listFlows,
  getFlowDefinition,
  listLiveRuntimeLogs,
  listLiveRuntimeSessions,
  saveFlowNodeDraft,
  publishFlowDefinition,
  restartFlowRun,
  setFlowEnabled,
  createFlow,
  deleteFlow,
};

const FLOW_RUNTIME_LOG_CAP = 500;

const DEFAULT_FILTERS: FlowFilters = {
  search: "",
  status: "all",
};

let pendingFetchRequests = 0;
let fetchFlowsToken = 0;
let fetchFlowDetailToken = 0;
let fetchRuntimeLogsToken = 0;
const fetchRuntimeLogsTokensByFlow: Record<number, number> = {};

function beginFetch(set: (partial: Partial<FlowStore>) => void): () => void {
  pendingFetchRequests += 1;
  set({ loading: true });
  return () => {
    pendingFetchRequests = Math.max(0, pendingFetchRequests - 1);
    set({ loading: pendingFetchRequests > 0 });
  };
}

function getErrorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback;
}

function appendRuntimeLogEntry(
  bucket: FlowRuntimeLogEntry[],
  row: FlowRuntimeLogEntry,
): FlowRuntimeLogEntry[] {
  if (bucket.some((entry) => entry.id === row.id)) {
    return bucket;
  }

  const next = [...bucket, row];
  if (next.length <= FLOW_RUNTIME_LOG_CAP) {
    return next;
  }
  return next.slice(next.length - FLOW_RUNTIME_LOG_CAP);
}

function draftJsonChanged(left: string, right: string): boolean {
  return (left ?? "").trim() !== (right ?? "").trim();
}

function editorPayloadHasDraftChanges(payload: FlowEditorPayload): boolean {
  return payload.nodes.some((node) =>
    draftJsonChanged(node.draft_config_json, node.published_config_json),
  );
}

function normalizeFlowStatus(status: FlowStatus, enabled: boolean): FlowStatus {
  if (!enabled) {
    return "disabled";
  }
  if (status === "disabled") {
    return "idle";
  }
  return status;
}

function applyRuntimeSnapshotToFlowSummary(
  flow: FlowSummary,
  runtimeSnapshot?: FlowRuntimeSnapshot,
): FlowSummary {
  if (!runtimeSnapshot) {
    return flow;
  }

  return {
    ...flow,
    status: normalizeFlowStatus(runtimeSnapshot.status, flow.enabled),
    current_node: runtimeSnapshot.current_node,
    last_live_at: runtimeSnapshot.last_live_at ?? flow.last_live_at,
    last_error: runtimeSnapshot.last_error ?? flow.last_error,
  };
}

function applyRuntimeSnapshotToEditor(
  payload: FlowEditorPayload,
  runtimeSnapshot?: FlowRuntimeSnapshot,
): FlowEditorPayload {
  if (!runtimeSnapshot) {
    return payload;
  }

  return {
    ...payload,
    flow: {
      ...payload.flow,
      status: normalizeFlowStatus(runtimeSnapshot.status, payload.flow.enabled),
      current_node: runtimeSnapshot.current_node,
      last_live_at: runtimeSnapshot.last_live_at ?? payload.flow.last_live_at,
      last_error: runtimeSnapshot.last_error ?? payload.flow.last_error,
    },
  };
}

function normalizeFlowSummary(
  flow: FlowSummary,
  runtimeSnapshots: Record<number, FlowRuntimeSnapshot>,
): FlowSummary {
  return applyRuntimeSnapshotToFlowSummary(
    {
      ...flow,
      status: normalizeFlowStatus(flow.status, flow.enabled),
    },
    runtimeSnapshots[flow.id],
  );
}

function normalizeFlowEditorPayload(
  payload: FlowEditorPayload,
  runtimeSnapshots: Record<number, FlowRuntimeSnapshot>,
): FlowEditorPayload {
  return applyRuntimeSnapshotToEditor(
    {
      ...payload,
      flow: {
        ...payload.flow,
        status: normalizeFlowStatus(payload.flow.status, payload.flow.enabled),
      },
    },
    runtimeSnapshots[payload.flow.id],
  );
}

function applyEnabledToFlowSummary(flow: FlowSummary, enabled: boolean): FlowSummary {
  return {
    ...flow,
    enabled,
    status: normalizeFlowStatus(flow.status, enabled),
  };
}

function applyEnabledToFlowEditor(payload: FlowEditorPayload, enabled: boolean): FlowEditorPayload {
  return {
    ...payload,
    flow: {
      ...payload.flow,
      enabled,
      status: normalizeFlowStatus(payload.flow.status, enabled),
    },
  };
}

export const useFlowStore = create<FlowStore>((set, get) => ({
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
  filters: { ...DEFAULT_FILTERS },

  fetchFlows: async (opts) => {
    const quiet = opts?.quiet ?? false;
    const token = ++fetchFlowsToken;
    const endFetch = quiet ? () => {} : beginFetch(set);
    try {
      const flows = await flowStoreApi.listFlows();
      if (token !== fetchFlowsToken) {
        return;
      }
      const runtimeSnapshots = get().runtimeSnapshots;
      set({ flows: flows.map((flow) => normalizeFlowSummary(flow, runtimeSnapshots)), error: null });
    } catch (error) {
      if (token !== fetchFlowsToken) {
        return;
      }
      set({ error: getErrorMessage(error, "Failed to load flows") });
    } finally {
      endFetch();
    }
  },

  fetchFlowDetail: async (flowId, opts) => {
    const quiet = opts?.quiet ?? false;
    const token = ++fetchFlowDetailToken;
    const targetId = flowId ?? get().activeFlowId;
    if (!targetId) {
      set({ activeFlow: null, error: null });
      return;
    }

    const endFetch = quiet ? () => {} : beginFetch(set);
    try {
      const payload = await flowStoreApi.getFlowDefinition(targetId);
      if (token !== fetchFlowDetailToken) {
        return;
      }
      const runtimeSnapshots = get().runtimeSnapshots;
      set({
        activeFlow: normalizeFlowEditorPayload(payload, runtimeSnapshots),
        activeFlowId: targetId,
        error: null,
        draftDirty: quiet ? get().draftDirty : false,
      });
    } catch (error) {
      if (token !== fetchFlowDetailToken) {
        return;
      }
      set({ error: getErrorMessage(error, "Failed to load flow detail") });
    } finally {
      endFetch();
    }
  },

  setActiveFlowId: (flowId) => set({ activeFlowId: flowId }),

  setView: (view) => set({ view }),

  setSelectedNode: (node) => set({ selectedNode: node }),

  setFilters: (partial) => {
    set((s) => ({ filters: { ...s.filters, ...partial } }));
  },

  openNodeModal: (node) => set({ editorModalNode: node, selectedNode: node }),

  closeNodeModal: () => set({ editorModalNode: null }),

  saveNodeDraft: async (input) => {
    const current = get().activeFlow;
    const existingNode =
      current && current.flow.id === input.flow_id
        ? current.nodes.find((node) => node.node_key === input.node_key)
        : null;
    if (existingNode && !draftJsonChanged(existingNode.draft_config_json, input.draft_config_json)) {
      return;
    }

    await flowStoreApi.saveFlowNodeDraft(input);
    const latest = get().activeFlow;
    if (latest && latest.flow.id === input.flow_id) {
      const now = new Date().toISOString();
      const nextNodes = latest.nodes.some((n) => n.node_key === input.node_key)
        ? latest.nodes.map((n) =>
            n.node_key === input.node_key
              ? { ...n, draft_config_json: input.draft_config_json, draft_updated_at: now }
              : n,
          )
        : latest.nodes;
      const nextActiveFlow = { ...latest, nodes: nextNodes };
      set({
        activeFlow: nextActiveFlow,
        draftDirty: editorPayloadHasDraftChanges(nextActiveFlow),
      });
    }
  },

  publishFlow: async (flowId, options) => {
    set({ publishPending: true, error: null });
    try {
      const result = await flowStoreApi.publishFlowDefinition(flowId);
      if (options.restartCurrentRun && result.isRunning) {
        await flowStoreApi.restartFlowRun(flowId);
        await get().refreshRuntime();
      }
      await get().fetchFlowDetail(flowId);
      set({ draftDirty: false });
      return result;
    } finally {
      set({ publishPending: false });
    }
  },

  applyRuntimeSnapshots: (rows) => {
    set({
      runtimeSnapshots: Object.fromEntries(rows.map((row) => [row.flow_id, row])),
    });
  },

  upsertRuntimeSnapshot: (row) => {
    set((state) => {
      const runtimeSnapshots = {
        ...state.runtimeSnapshots,
        [row.flow_id]: row,
      };
      return {
        runtimeSnapshots,
        flows: state.flows.map((flow) => normalizeFlowSummary(flow, runtimeSnapshots)),
        activeFlow:
          state.activeFlow && state.activeFlow.flow.id === row.flow_id
            ? normalizeFlowEditorPayload(state.activeFlow, runtimeSnapshots)
            : state.activeFlow,
      };
    });
  },

  applyRuntimeLogs: (rows, options) => {
    set((state) => {
      const next = { ...state.runtimeLogs };
      const flowIds = new Set(options?.flowIds ?? rows.map((row) => row.flow_id));
      for (const flowId of flowIds) {
        next[flowId] = [];
      }
      for (const row of rows) {
        next[row.flow_id] = appendRuntimeLogEntry(next[row.flow_id] ?? [], row);
      }
      return { runtimeLogs: next };
    });
  },

  appendRuntimeLog: (row) => {
    set((state) => ({
      runtimeLogs: {
        ...state.runtimeLogs,
        [row.flow_id]: appendRuntimeLogEntry(state.runtimeLogs[row.flow_id] ?? [], row),
      },
    }));
  },

  fetchRuntimeLogs: async (flowId, limit = 50) => {
    const token = ++fetchRuntimeLogsToken;
    fetchRuntimeLogsTokensByFlow[flowId] = token;
    const initialBucketIds = new Set((get().runtimeLogs[flowId] ?? []).map((row) => row.id));

    try {
      const rows = await flowStoreApi.listLiveRuntimeLogs(flowId, limit);
      if (fetchRuntimeLogsTokensByFlow[flowId] !== token) {
        return;
      }
      const appendedDuringFetch = (get().runtimeLogs[flowId] ?? []).filter(
        (row) => !initialBucketIds.has(row.id),
      );
      const mergedRows = [
        ...rows,
        ...appendedDuringFetch.filter((row) => !rows.some((hydrated) => hydrated.id === row.id)),
      ];
      get().applyRuntimeLogs(mergedRows, { flowIds: [flowId] });
    } catch {
      /* keep last known logs when runtime log hydration races startup or sidecar reconnect */
    }
  },

  refreshRuntime: async () => {
    try {
      const snapshots = await flowStoreApi.listLiveRuntimeSessions();
      get().applyRuntimeSnapshots(snapshots);
    } catch {
      /* keep last known runtime snapshots when refresh races startup */
    }
    set((s) => ({ runtimeRefreshTick: s.runtimeRefreshTick + 1 }));
    await get().fetchFlows({ quiet: true });
    const id = get().activeFlowId;
    if (id != null) {
      await get().fetchFlowDetail(id, { quiet: true });
    }
  },

  toggleFlowEnabled: async (flowId, enabled) => {
    const previousFlows = get().flows;
    const previousActiveFlow = get().activeFlow;

    set((state) => ({
      flows: state.flows.map((flow) =>
        flow.id === flowId ? applyEnabledToFlowSummary(flow, enabled) : flow,
      ),
      activeFlow:
        state.activeFlow && state.activeFlow.flow.id === flowId
          ? applyEnabledToFlowEditor(state.activeFlow, enabled)
          : state.activeFlow,
      error: null,
    }));

    try {
      await flowStoreApi.setFlowEnabled(flowId, enabled);
    } catch (error) {
      set({
        flows: previousFlows,
        activeFlow: previousActiveFlow,
        error: getErrorMessage(error, "Failed to update flow state"),
      });
      throw error;
    }
  },

  createFlow: async (input) => {
    const id = await flowStoreApi.createFlow(input);
    await get().fetchFlows();
    set({ activeFlowId: id, view: "detail" });
    await get().fetchFlowDetail(id);
    return id;
  },

  deleteFlow: async (flowId) => {
    fetchFlowsToken += 1;
    fetchFlowDetailToken += 1;
    fetchRuntimeLogsToken += 1;
    fetchRuntimeLogsTokensByFlow[flowId] = fetchRuntimeLogsToken;

    const previousFlows = get().flows;
    const previousFlow = previousFlows.find((flow) => flow.id === flowId);
    const previousFlowIndex = previousFlows.findIndex((flow) => flow.id === flowId);
    const previousRuntimeSnapshots = get().runtimeSnapshots;
    const previousRuntimeSnapshot = previousRuntimeSnapshots[flowId];
    const previousRuntimeLogs = get().runtimeLogs;
    const previousRuntimeLogBucket = previousRuntimeLogs[flowId];
    const previousActiveFlowId = get().activeFlowId;
    const previousActiveFlow = get().activeFlow;
    const previousSelectedNode = get().selectedNode;
    const previousEditorModalNode = get().editorModalNode;
    const previousDraftDirty = get().draftDirty;
    const previousView = get().view;
    const deletingActive =
      previousActiveFlowId === flowId || previousActiveFlow?.flow.id === flowId;

    set((state) => {
      const nextRuntimeSnapshots = { ...state.runtimeSnapshots };
      delete nextRuntimeSnapshots[flowId];

      const nextRuntimeLogs = { ...state.runtimeLogs };
      delete nextRuntimeLogs[flowId];

      return {
        flows: state.flows.filter((flow) => flow.id !== flowId),
        runtimeSnapshots: nextRuntimeSnapshots,
        runtimeLogs: nextRuntimeLogs,
        activeFlowId: deletingActive ? null : state.activeFlowId,
        activeFlow: deletingActive ? null : state.activeFlow,
        selectedNode: deletingActive ? null : state.selectedNode,
        editorModalNode: deletingActive ? null : state.editorModalNode,
        draftDirty: deletingActive ? false : state.draftDirty,
        view: deletingActive ? "list" : state.view,
        error: null,
      };
    });
    try {
      await flowStoreApi.deleteFlow(flowId);
    } catch (error) {
      set((state) => {
        const next: Partial<FlowStore> = {
          error: getErrorMessage(error, "Failed to delete flow"),
        };

        if (previousFlow && !state.flows.some((flow) => flow.id === flowId)) {
          const insertAt =
            previousFlowIndex >= 0 && previousFlowIndex <= state.flows.length
              ? previousFlowIndex
              : state.flows.length;
          next.flows = [
            ...state.flows.slice(0, insertAt),
            previousFlow,
            ...state.flows.slice(insertAt),
          ];
        }

        if (previousRuntimeSnapshot && state.runtimeSnapshots[flowId] == null) {
          next.runtimeSnapshots = {
            ...state.runtimeSnapshots,
            [flowId]: previousRuntimeSnapshot,
          };
        }

        if (previousRuntimeLogBucket && state.runtimeLogs[flowId] == null) {
          next.runtimeLogs = {
            ...state.runtimeLogs,
            [flowId]: previousRuntimeLogBucket,
          };
        }

        const matchesOptimisticClearedShape =
          deletingActive &&
          state.activeFlowId === null &&
          state.activeFlow === null &&
          state.selectedNode === null &&
          state.editorModalNode === null &&
          state.draftDirty === false &&
          state.view === "list";

        if (matchesOptimisticClearedShape) {
          next.activeFlowId = previousActiveFlowId;
          next.activeFlow = previousActiveFlow;
          next.selectedNode = previousSelectedNode;
          next.editorModalNode = previousEditorModalNode;
          next.draftDirty = previousDraftDirty;
          next.view = previousView;
        }

        return next;
      });
      throw error;
    }
  },
}));
