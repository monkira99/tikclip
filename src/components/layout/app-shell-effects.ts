import { useEffect, useRef, type Dispatch, type SetStateAction } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";

import * as api from "@/lib/api";
import { hydrateNotificationsFromDb } from "@/lib/notifications-sync";
import {
  dispatchRuntimeNotification,
  displayRuntimeNotification,
} from "@/lib/runtime-notifications";
import { useAppStore } from "@/stores/app-store";
import { useClipStore } from "@/stores/clip-store";
import { useFlowStore } from "@/stores/flow-store";
import { countActiveRecordings, useRecordingStore } from "@/stores/recording-store";
import type { FlowRuntimeLogEntry, FlowRuntimeSnapshot } from "@/types";
import type { PageId } from "./app-shell-pages";

const CAPTION_RETRY_BASE_MS = 250;
const CAPTION_GENERATE_MAX_ATTEMPTS = 3;

function delayMs(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

function errorMessage(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  return String(err ?? "");
}

function isTransientCaptionGenerationError(err: unknown): boolean {
  const message = errorMessage(err).toLowerCase();
  if (!message) {
    return false;
  }
  if (
    message.includes("400") ||
    message.includes("401") ||
    message.includes("403") ||
    message.includes("404") ||
    message.includes("422") ||
    message.includes("username is required")
  ) {
    return false;
  }
  return (
    message.includes("failed to fetch") ||
    message.includes("network") ||
    message.includes("timeout") ||
    message.includes("tempor") ||
    message.includes("429") ||
    message.includes("500") ||
    message.includes("502") ||
    message.includes("503") ||
    message.includes("504") ||
    message.includes("runtime request failed")
  );
}

async function maybeGenerateCaptionAfterInsert(
  clipId: number,
  data: Record<string, unknown>,
): Promise<void> {
  try {
    const usernameRaw = data.username;
    const username = typeof usernameRaw === "string" ? usernameRaw.trim() : "";
    if (!username) {
      return;
    }

    const transcriptTextRaw = data.transcript_text;
    const transcriptText =
      typeof transcriptTextRaw === "string" && transcriptTextRaw.trim() !== ""
        ? transcriptTextRaw
        : null;

    const clipIndexRaw = data.clip_index;
    const clipIndex =
      typeof clipIndexRaw === "number"
        ? Math.trunc(clipIndexRaw)
        : typeof clipIndexRaw === "string"
          ? Math.trunc(Number(clipIndexRaw))
          : NaN;
    const clipTitle = Number.isFinite(clipIndex) && clipIndex > 0 ? `Clip ${clipIndex}` : null;

    for (let attempt = 1; attempt <= CAPTION_GENERATE_MAX_ATTEMPTS; attempt += 1) {
      try {
        await api.generateCaptionForClip({
          clip_id: clipId,
          username,
          transcript_text: transcriptText,
          clip_title: clipTitle,
        });
        useClipStore.getState().bumpClipsRevision();
        try {
          const accountId = parseAccountId(data.account_id);
          await api.applyFlowRuntimeHint({
            account_id: accountId ?? 0,
            hint: "caption_ready",
            clip_id: clipId,
          });
        } catch {
          /* No matching flow is normal. */
        }
        void refreshRuntimeState();
        return;
      } catch (err) {
        if (attempt >= CAPTION_GENERATE_MAX_ATTEMPTS || !isTransientCaptionGenerationError(err)) {
          return;
        }
      }
      await delayMs(CAPTION_RETRY_BASE_MS * attempt);
    }
  } catch {
    /* optional runtime enhancement */
  }
}

function parseAccountId(raw: unknown): number | null {
  const id = typeof raw === "number" ? raw : Number(raw);
  if (!Number.isFinite(id)) {
    return null;
  }
  return id;
}

function parseClipId(raw: unknown): number | null {
  const id = typeof raw === "number" ? raw : Number(raw);
  if (!Number.isFinite(id) || id <= 0) {
    return null;
  }
  return Math.trunc(id);
}

async function refreshRuntimeState(): Promise<void> {
  const [activeRecordings] = await Promise.all([
    api.listActiveRustRecordings(),
    useFlowStore.getState().refreshRuntime(),
  ]);
  useRecordingStore.getState().hydrateFromRuntime(activeRecordings);
}

async function refreshDashboardRuntimeData(): Promise<void> {
  try {
    const activeRecordings = await api.listActiveRustRecordings();
    useRecordingStore.getState().hydrateFromRuntime(activeRecordings);
    useAppStore.getState().bumpDashboardRevision();
  } catch (error) {
    if (import.meta.env.DEV) {
      console.warn("[TikClip] refresh dashboard runtime data failed", error);
    }
  }
}

export function useActiveRecordingCountSync(): void {
  const activeRecordingCount = useRecordingStore((s) => countActiveRecordings(s.recordings));
  const setActiveRecordings = useAppStore((s) => s.setActiveRecordings);

  useEffect(() => {
    setActiveRecordings(activeRecordingCount);
  }, [activeRecordingCount, setActiveRecordings]);
}

export function useNavigationTargetSync(setCurrentPage: Dispatch<SetStateAction<PageId>>): void {
  const navigationTarget = useAppStore((s) => s.navigationTarget);

  useEffect(() => {
    if (!navigationTarget) {
      return;
    }
    const raw = navigationTarget.page;
    const p =
      raw === "accounts"
        ? "flows"
        : raw === "dashboard" ||
            raw === "flows" ||
            raw === "products" ||
            raw === "settings"
          ? raw
          : null;
    if (p) {
      setCurrentPage(p);
    }
    useAppStore.getState().clearNavigationTarget();
  }, [navigationTarget, setCurrentPage]);
}

export function useNotificationBootstrap(): void {
  const notifyWarmupDone = useRef(false);

  useEffect(() => {
    void hydrateNotificationsFromDb();
  }, []);

  /** Desktop OSes: prompt once so OS alerts work before the first runtime event. */
  useEffect(() => {
    if (notifyWarmupDone.current || !isTauri()) {
      return;
    }
    notifyWarmupDone.current = true;
    void (async () => {
      try {
        const granted = await isPermissionGranted();
        if (!granted) {
          await requestPermission();
        }
      } catch {
        /* ignore */
      }
    })();
  }, []);
}

export function useTauriRuntimeEvents(): void {
  useEffect(() => {
    void refreshRuntimeState();
  }, []);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    void listen<FlowRuntimeSnapshot>("flow-runtime-updated", (event) => {
      if (cancelled) {
        return;
      }
      useFlowStore.getState().upsertRuntimeSnapshot(event.payload);
      void refreshDashboardRuntimeData();
    }).then((fn) => {
      if (cancelled) {
        fn();
        return;
      }
      unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    void listen<FlowRuntimeLogEntry>("flow-runtime-log", (event) => {
      if (cancelled) {
        return;
      }
      useFlowStore.getState().appendRuntimeLog(event.payload);
    }).then((fn) => {
      if (cancelled) {
        fn();
        return;
      }
      unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    void listen<Record<string, unknown>>("rust-clip-ready", (event) => {
      if (cancelled) {
        return;
      }
      const data = event.payload;
      dispatchRuntimeNotification("clip_ready", data);
      useAppStore.getState().bumpDashboardRevision();
      useClipStore.getState().bumpClipsRevision();
      const clipId = parseClipId(data.clip_id);
      if (clipId != null) {
        void maybeGenerateCaptionAfterInsert(clipId, data);
      }
      void refreshRuntimeState();
    }).then((fn) => {
      if (cancelled) {
        fn();
        return;
      }
      unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
}

export function useStorageRuntimeEvents(): void {
  useEffect(() => {
    if (!isTauri()) {
      return;
    }
    let cancelled = false;
    let unlistenCleanup: (() => void) | null = null;
    let unlistenStorageWarn: (() => void) | null = null;

    const handleStorageEvent = (eventType: "cleanup_completed" | "storage_warning") => {
      return (event: { payload: Record<string, unknown> }) => {
        if (cancelled) {
          return;
        }
        displayRuntimeNotification(eventType, event.payload);
        useAppStore.getState().bumpDashboardRevision();
      };
    };

    void listen<Record<string, unknown>>(
      "cleanup_completed",
      handleStorageEvent("cleanup_completed"),
    ).then((fn) => {
      if (cancelled) {
        fn();
        return;
      }
      unlistenCleanup = fn;
    });
    void listen<Record<string, unknown>>(
      "storage_warning",
      handleStorageEvent("storage_warning"),
    ).then((fn) => {
      if (cancelled) {
        fn();
        return;
      }
      unlistenStorageWarn = fn;
    });

    return () => {
      cancelled = true;
      unlistenCleanup?.();
      unlistenStorageWarn?.();
    };
  }, []);
}
