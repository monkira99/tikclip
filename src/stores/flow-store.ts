import { create } from "zustand";

import {
  createFlow,
  getFlowDetail,
  listFlows,
  saveFlowNodeConfig,
  setFlowEnabled,
} from "@/lib/api";
import type {
  CreateFlowInput,
  FlowDetail,
  FlowNodeConfig,
  FlowNodeKey,
  FlowStatus,
  FlowSummary,
} from "@/types";

type FlowView = "list" | "detail";

type FlowFilters = {
  search: string;
  status: FlowStatus | "all";
};

type FlowStore = {
  flows: FlowSummary[];
  activeFlowId: number | null;
  activeFlow: FlowDetail | null;
  view: FlowView;
  selectedNode: FlowNodeKey | null;
  loading: boolean;
  error: string | null;
  filters: FlowFilters;

  fetchFlows: () => Promise<void>;
  fetchFlowDetail: (flowId?: number) => Promise<void>;
  setActiveFlowId: (flowId: number | null) => void;
  setView: (view: FlowView) => void;
  setSelectedNode: (node: FlowNodeKey | null) => void;
  setFilters: (partial: Partial<FlowFilters>) => void;
  saveFlowConfig: (input: {
    flow_id: number;
    node_key: FlowNodeKey;
    config_json: string;
  }) => Promise<FlowNodeConfig>;
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

function normalizeFlowDetail(detail: FlowDetail): FlowDetail {
  return {
    ...detail,
    flow: {
      ...detail.flow,
      status: normalizeFlowStatus(detail.flow.status, detail.flow.enabled),
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

function applyEnabledToFlowDetail(detail: FlowDetail, enabled: boolean): FlowDetail {
  return {
    ...detail,
    flow: {
      ...detail.flow,
      enabled,
      status: normalizeFlowStatus(detail.flow.status, enabled),
    },
  };
}

export const useFlowStore = create<FlowStore>((set, get) => ({
  flows: [],
  activeFlowId: null,
  activeFlow: null,
  view: "list",
  selectedNode: null,
  loading: false,
  error: null,
  filters: { ...DEFAULT_FILTERS },

  fetchFlows: async () => {
    const token = ++fetchFlowsToken;
    const endFetch = beginFetch(set);
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

  fetchFlowDetail: async (flowId) => {
    const token = ++fetchFlowDetailToken;
    const targetId = flowId ?? get().activeFlowId;
    if (!targetId) {
      set({ activeFlow: null, error: null });
      return;
    }

    const endFetch = beginFetch(set);
    try {
      const detail = await getFlowDetail(targetId);
      if (token !== fetchFlowDetailToken) {
        return;
      }
      set({ activeFlow: normalizeFlowDetail(detail), activeFlowId: targetId, error: null });
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

  saveFlowConfig: async (input) => {
    const config = await saveFlowNodeConfig(input);
    const current = get().activeFlow;
    if (current && current.flow.id === input.flow_id) {
      const nextConfigs = current.node_configs.some((x) => x.node_key === config.node_key)
        ? current.node_configs.map((x) => (x.node_key === config.node_key ? config : x))
        : [...current.node_configs, config];
      set({
        activeFlow: {
          ...current,
          node_configs: nextConfigs,
        },
      });
    }
    return config;
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
          ? applyEnabledToFlowDetail(state.activeFlow, enabled)
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
