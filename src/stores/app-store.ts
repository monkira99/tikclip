import { create } from "zustand";

export type NavigationTarget = { page: string; clipId?: number; flowId?: number };

type AppStore = {
  sidecarConnected: boolean;
  sidecarPort: number | null;
  activeRecordings: number;
  dashboardRevision: number;
  /** Consumed by AppShell to switch page (e.g. activity feed → clip). */
  navigationTarget: NavigationTarget | null;
  setSidecarStatus: (connected: boolean, port: number | null) => void;
  setActiveRecordings: (n: number) => void;
  bumpDashboardRevision: () => void;
  requestNavigation: (target: NavigationTarget) => void;
  clearNavigationTarget: () => void;
};

export const useAppStore = create<AppStore>((set) => ({
  sidecarConnected: false,
  sidecarPort: null,
  activeRecordings: 0,
  dashboardRevision: 0,
  navigationTarget: null,
  setSidecarStatus: (connected, port) =>
    set({ sidecarConnected: connected, sidecarPort: port }),
  setActiveRecordings: (n) => set({ activeRecordings: n }),
  bumpDashboardRevision: () => set((s) => ({ dashboardRevision: s.dashboardRevision + 1 })),
  requestNavigation: (target) => set({ navigationTarget: target }),
  clearNavigationTarget: () => set({ navigationTarget: null }),
}));
