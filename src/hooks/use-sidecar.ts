import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";
import { setSidecarPort } from "@/lib/api";
import { useAppStore } from "@/stores/app-store";

type SidecarStatusPayload = {
  connected: boolean;
  port?: number | null;
};

export function useSidecar(pollMs = 2000) {
  const setSidecarStatus = useAppStore((s) => s.setSidecarStatus);

  useEffect(() => {
    let cancelled = false;

    const tick = async () => {
      try {
        const status = await invoke<SidecarStatusPayload>("get_sidecar_status");
        if (!cancelled) {
          const port =
            status.port !== undefined && status.port !== null ? status.port : null;
          setSidecarStatus(status.connected, port);
          setSidecarPort(port);
        }
      } catch {
        if (!cancelled) {
          setSidecarStatus(false, null);
          setSidecarPort(null);
        }
      }
    };

    void tick();
    const id = window.setInterval(() => void tick(), pollMs);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [setSidecarStatus, pollMs]);
}
