import { create } from "zustand";

type AppStore = {
  sidecarConnected: boolean;
  sidecarPort: number | null;
  activeRecordings: number;
  setSidecarStatus: (connected: boolean, port: number | null) => void;
};

export const useAppStore = create<AppStore>((set) => ({
  sidecarConnected: false,
  sidecarPort: null,
  activeRecordings: 0,
  setSidecarStatus: (connected, port) =>
    set({ sidecarConnected: connected, sidecarPort: port }),
}));
