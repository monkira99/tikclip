import { create } from "zustand";
import { listClipsFiltered, batchUpdateClipStatus, batchDeleteClips } from "@/lib/api";
import type { Clip, ClipFilters, ClipStatus, ViewMode } from "@/types";

const DEFAULT_FILTERS: ClipFilters = {
  status: "all",
  accountId: null,
  sceneType: "all",
  dateFrom: null,
  dateTo: null,
  search: "",
  sortBy: "created_at",
  sortOrder: "desc",
};

type ClipStore = {
  clips: Clip[];
  filters: ClipFilters;
  viewMode: ViewMode;
  selectedClipIds: Set<number>;
  loading: boolean;
  activeClipId: number | null;
  clipsRevision: number;

  fetchClips: () => Promise<void>;
  setFilter: (partial: Partial<ClipFilters>) => void;
  resetFilters: () => void;
  setViewMode: (mode: ViewMode) => void;
  toggleSelect: (clipId: number) => void;
  selectAll: () => void;
  clearSelection: () => void;
  batchUpdateStatus: (status: ClipStatus) => Promise<void>;
  batchDelete: () => Promise<void>;
  setActiveClipId: (id: number | null) => void;
  bumpClipsRevision: () => void;
};

export const useClipStore = create<ClipStore>((set, get) => ({
  clips: [],
  filters: { ...DEFAULT_FILTERS },
  viewMode: "grid",
  selectedClipIds: new Set(),
  loading: false,
  activeClipId: null,
  clipsRevision: 0,

  fetchClips: async () => {
    set({ loading: true });
    try {
      const clips = await listClipsFiltered(get().filters);
      set({ clips, loading: false });
    } catch {
      set({ clips: [], loading: false });
    }
  },

  setFilter: (partial) => {
    set((s) => ({ filters: { ...s.filters, ...partial } }));
    void get().fetchClips();
  },

  resetFilters: () => {
    set({ filters: { ...DEFAULT_FILTERS } });
    void get().fetchClips();
  },

  setViewMode: (mode) => set({ viewMode: mode }),

  toggleSelect: (clipId) =>
    set((s) => {
      const next = new Set(s.selectedClipIds);
      if (next.has(clipId)) {
        next.delete(clipId);
      } else {
        next.add(clipId);
      }
      return { selectedClipIds: next };
    }),

  selectAll: () =>
    set((s) => ({
      selectedClipIds: new Set(s.clips.map((c) => c.id)),
    })),

  clearSelection: () => set({ selectedClipIds: new Set() }),

  batchUpdateStatus: async (status) => {
    const ids = Array.from(get().selectedClipIds);
    if (ids.length === 0) return;
    await batchUpdateClipStatus(ids, status);
    set({ selectedClipIds: new Set() });
    void get().fetchClips();
  },

  batchDelete: async () => {
    const ids = Array.from(get().selectedClipIds);
    if (ids.length === 0) return;
    await batchDeleteClips(ids);
    set({ selectedClipIds: new Set() });
    void get().fetchClips();
  },

  setActiveClipId: (id) => set({ activeClipId: id }),

  bumpClipsRevision: () => set((s) => ({ clipsRevision: s.clipsRevision + 1 })),
}));
