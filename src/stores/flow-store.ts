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

export const useFlowStore = create<FlowStore>((set, get) => ({
  flows: [],
  activeFlowId: null,
  activeFlow: null,
  view: "list",
  selectedNode: null,
  loading: false,
  filters: { ...DEFAULT_FILTERS },

  fetchFlows: async () => {
    set({ loading: true });
    try {
      const flows = await listFlows();
      set({ flows, loading: false });
    } catch {
      set({ flows: [], loading: false });
    }
  },

  fetchFlowDetail: async (flowId) => {
    const targetId = flowId ?? get().activeFlowId;
    if (!targetId) {
      set({ activeFlow: null });
      return;
    }
    set({ loading: true });
    try {
      const detail = await getFlowDetail(targetId);
      set({ activeFlow: detail, activeFlowId: targetId, loading: false });
    } catch {
      set({ activeFlow: null, loading: false });
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
    await setFlowEnabled(flowId, enabled);
    set((state) => ({
      flows: state.flows.map((flow) => (flow.id === flowId ? { ...flow, enabled } : flow)),
      activeFlow:
        state.activeFlow && state.activeFlow.flow.id === flowId
          ? { ...state.activeFlow, flow: { ...state.activeFlow.flow, enabled } }
          : state.activeFlow,
    }));
  },

  createFlow: async (input) => {
    const id = await createFlow(input);
    await get().fetchFlows();
    set({ activeFlowId: id, view: "detail" });
    await get().fetchFlowDetail(id);
    return id;
  },
}));
