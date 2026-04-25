import { create } from "zustand";

type ClipStore = {
  clipsRevision: number;
  bumpClipsRevision: () => void;
};

export const useClipStore = create<ClipStore>((set) => ({
  clipsRevision: 0,
  bumpClipsRevision: () => set((s) => ({ clipsRevision: s.clipsRevision + 1 })),
}));
