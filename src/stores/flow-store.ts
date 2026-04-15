import { create } from "zustand";

import {
  createFlow,
  getFlowDefinition,
  publishFlowDefinition,
  restartFlowRun,
  saveFlowNodeDraft,
  setFlowEnabled,
  listFlows,
} from "@/lib/api";
import type {
  CreateFlowInput,
  FlowEditorPayload,
  FlowNodeKey,
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
  refreshRuntime: () => Promise<void>;
  toggleFlowEnabled: (flowId: number, enabled: boolean) => Promise<void>;
  createFlow: (input: CreateFlowInput) => Promise<number>;
};

const DEFAULT_FILTERS: FlowFilters = {
  search: "",
  status: "all",
};

let pendingFetchRequests = 0;
let fetchFlowsToken = 0;
let fetchFlowDetailToken = 0;

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

function normalizeFlowStatus(status: FlowStatus, enabled: boolean): FlowStatus {
  if (!enabled) {
    return "disabled";
  }
  if (status === "disabled") {
    return "idle";
  }
  return status;
}

function normalizeFlowSummary(flow: FlowSummary): FlowSummary {
  return {
    ...flow,
    status: normalizeFlowStatus(flow.status, flow.enabled),
  };
}

function normalizeFlowEditorPayload(payload: FlowEditorPayload): FlowEditorPayload {
  return {
    ...payload,
    flow: {
      ...payload.flow,
      status: normalizeFlowStatus(payload.flow.status, payload.flow.enabled),
    },
  };
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
      const flows = await listFlows();
      if (token !== fetchFlowsToken) {
        return;
      }
      set({ flows: flows.map(normalizeFlowSummary), error: null });
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
      const payload = await getFlowDefinition(targetId);
      if (token !== fetchFlowDetailToken) {
        return;
      }
      set({
        activeFlow: normalizeFlowEditorPayload(payload),
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
    await saveFlowNodeDraft(input);
    const current = get().activeFlow;
    if (current && current.flow.id === input.flow_id) {
      const now = new Date().toISOString();
      const nextNodes = current.nodes.some((n) => n.node_key === input.node_key)
        ? current.nodes.map((n) =>
            n.node_key === input.node_key
              ? { ...n, draft_config_json: input.draft_config_json, draft_updated_at: now }
              : n,
          )
        : current.nodes;
      set({
        activeFlow: { ...current, nodes: nextNodes },
        draftDirty: true,
      });
    }
  },

  publishFlow: async (flowId, options) => {
    set({ publishPending: true, error: null });
    try {
      const result = await publishFlowDefinition(flowId);
      if (options.restartCurrentRun && result.isRunning) {
        await restartFlowRun(flowId);
      }
      await get().fetchFlowDetail(flowId);
      set({ draftDirty: false });
      return result;
    } finally {
      set({ publishPending: false });
    }
  },

  refreshRuntime: async () => {
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
      await setFlowEnabled(flowId, enabled);
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
    const id = await createFlow(input);
    await get().fetchFlows();
    set({ activeFlowId: id, view: "detail" });
    await get().fetchFlowDetail(id);
    return id;
  },
}));
