import { create } from "zustand";

type ClipStore = {
  clipsRevision: number;
  bumpClipsRevision: () => void;
};

/** Bumped when the sidecar reports a new clip so ClipGrid can refetch from SQLite. */
export const useClipStore = create<ClipStore>((set) => ({
  clipsRevision: 0,
  bumpClipsRevision: () => set((s) => ({ clipsRevision: s.clipsRevision + 1 })),
}));
