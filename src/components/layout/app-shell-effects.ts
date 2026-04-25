import { useEffect, useRef, type Dispatch, type SetStateAction } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";

import { deriveAccountLiveFlagsFromRuntime } from "@/lib/account-live-from-runtime";
import * as api from "@/lib/api";
import { hydrateNotificationsFromDb } from "@/lib/notifications-sync";
import {
  insertClipFromSidecarWsPayload,
  insertSpeechSegmentFromWsPayload,
  syncClipCaptionFromWsPayload,
} from "@/lib/sidecar-db-sync";
import {
  dispatchSidecarNotification,
  displayRuntimeNotification,
} from "@/lib/sidecar-notifications";
import { parseBooleanSetting } from "@/lib/settings-value";
import { wsClient } from "@/lib/ws";
import { useAccountStore } from "@/stores/account-store";
import { useAppStore } from "@/stores/app-store";
import { useClipStore } from "@/stores/clip-store";
import { useFlowStore } from "@/stores/flow-store";
import { countActiveRecordings, useRecordingStore } from "@/stores/recording-store";
import type { FlowRuntimeLogEntry, FlowRuntimeSnapshot } from "@/types";
import type { PageId } from "./app-shell-pages";

const CAPTION_RETRY_BASE_MS = 250;
const CAPTION_GENERATE_MAX_ATTEMPTS = 3;
const CAPTION_SYNC_NOT_FOUND_MAX_ATTEMPTS = 4;

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
    message.includes("sidecar request failed")
  );
}

function isClipCaptionNotFoundError(err: unknown): boolean {
  return errorMessage(err).toLowerCase().includes("not found");
}

function logSidecarDbSyncError(context: string, err: unknown): void {
  if (import.meta.env.DEV) {
    console.warn(`[TikClip] ${context}`, err);
  }
}

async function maybeAutoTagClipAfterInsert(
  clipId: number,
  data: Record<string, unknown>,
): Promise<void> {
  try {
    if (!api.getSidecarBaseUrl()) {
      return;
    }
    const raw = await api.getSetting("auto_tag_clip_product_enabled");
    if (!parseBooleanSetting(raw, false)) {
      return;
    }
    const videoPath = typeof data.path === "string" ? data.path : "";
    if (!videoPath) {
      return;
    }
    const thumbnailPath =
      typeof data.thumbnail_path === "string" && data.thumbnail_path.trim() !== ""
        ? data.thumbnail_path
        : null;
    const transcriptText =
      typeof data.transcript_text === "string" && data.transcript_text.trim() !== ""
        ? data.transcript_text
        : null;
    const res = await api.suggestProductForClip({
      video_path: videoPath,
      thumbnail_path: thumbnailPath,
      transcript_text: transcriptText,
    });
    if (res.product_id != null) {
      await api.tagClipProduct(clipId, res.product_id);
      useClipStore.getState().bumpClipsRevision();
    }
  } catch {
    /* sidecar/Gemini optional */
  }
}

async function maybeGenerateCaptionAfterInsert(
  clipId: number,
  data: Record<string, unknown>,
): Promise<void> {
  try {
    if (!api.getSidecarBaseUrl()) {
      return;
    }
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

function syncAccountLiveFlagsFromRuntimeState(): void {
  const flowById = new Map(
    useFlowStore.getState().flows.map((flow) => [Number(flow.id), flow] as const),
  );
  const flowAccountIds = new Set<number>();
  for (const flow of useFlowStore.getState().flows) {
    const accountId = Number(flow.account_id);
    if (Number.isFinite(accountId)) {
      flowAccountIds.add(accountId);
    }
  }

  const liveMap = new Map<number, boolean>();
  for (const accountId of flowAccountIds) {
    liveMap.set(accountId, false);
  }

  for (const row of deriveAccountLiveFlagsFromRuntime(
    Object.values(useFlowStore.getState().runtimeSnapshots).filter((snapshot) => {
      const flow = flowById.get(Number(snapshot.flow_id));
      return flow?.enabled === true;
    }),
  )) {
    liveMap.set(row.id, row.isLive);
  }

  const rows = Array.from(liveMap.entries())
    .sort(([left], [right]) => left - right)
    .map(([id, isLive]) => ({ id, isLive }));

  if (rows.length === 0) {
    return;
  }

  useAccountStore.getState().applyLiveFlagsFromRuntime(rows);
  void api
    .syncAccountsLiveStatus(rows.map((row) => ({ account_id: row.id, is_live: row.isLive })))
    .catch((error) => {
      if (import.meta.env.DEV) {
        console.warn("[TikClip] syncAccountsLiveStatus from runtime failed", error);
      }
    });
}

async function refreshRuntimeAndSyncAccountLiveFlags(): Promise<void> {
  const [activeRecordings] = await Promise.all([
    api.listActiveRustRecordings(),
    useFlowStore.getState().refreshRuntime(),
  ]);
  useRecordingStore.getState().hydrateFromRuntime(activeRecordings);
  if (useAccountStore.getState().accounts.length === 0) {
    await useAccountStore.getState().fetchAccounts();
  }
  syncAccountLiveFlagsFromRuntimeState();
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

export function useNotificationBootstrap(sidecarConnected: boolean): void {
  const notifyWarmupDone = useRef(false);

  useEffect(() => {
    void hydrateNotificationsFromDb();
  }, []);

  /** macOS / Windows: prompt once when sidecar is up so OS alerts work before the first event. */
  useEffect(() => {
    if (!sidecarConnected || notifyWarmupDone.current || !isTauri()) {
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
  }, [sidecarConnected]);
}

export function useTauriRuntimeEvents(): void {
  useEffect(() => {
    void refreshRuntimeAndSyncAccountLiveFlags();
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
      syncAccountLiveFlagsFromRuntimeState();
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

export function useSidecarWsSync(sidecarPort: number | null): void {
  useEffect(() => {
    if (sidecarPort == null) {
      wsClient.disconnect();
      return;
    }

    const refreshRuntimeState = () => {
      void refreshRuntimeAndSyncAccountLiveFlags();
    };

    const unsubClip = wsClient.on("clip_ready", (data) => {
      dispatchSidecarNotification("clip_ready", data);
      useAppStore.getState().bumpDashboardRevision();
      void (async () => {
        try {
          const clipId = await insertClipFromSidecarWsPayload(data);
          useClipStore.getState().bumpClipsRevision();
          const accountId = parseAccountId(data.account_id);
          if (accountId != null) {
            await api.applySidecarFlowRuntimeHint({
              account_id: accountId,
              hint: "clip_ready",
              clip_id: clipId ?? undefined,
            });
          }
          if (clipId != null) {
            void maybeGenerateCaptionAfterInsert(clipId, data);
            void maybeAutoTagClipAfterInsert(clipId, data);
          }
        } catch (err) {
          logSidecarDbSyncError("clip_ready → SQLite insert failed", err);
        } finally {
          refreshRuntimeState();
        }
      })();
    });

    const unsubCaptionReady = wsClient.on("caption_ready", (data) => {
      void (async () => {
        try {
          for (let attempt = 1; attempt <= CAPTION_SYNC_NOT_FOUND_MAX_ATTEMPTS; attempt += 1) {
            try {
              const updated = await syncClipCaptionFromWsPayload(data);
              if (updated) {
                useClipStore.getState().bumpClipsRevision();
              }
              return;
            } catch (err) {
              const canRetry =
                attempt < CAPTION_SYNC_NOT_FOUND_MAX_ATTEMPTS && isClipCaptionNotFoundError(err);
              if (!canRetry) {
                logSidecarDbSyncError("caption_ready → SQLite update failed", err);
                return;
              }
            }
            await delayMs(CAPTION_RETRY_BASE_MS * attempt);
          }
        } finally {
          try {
            const clipId = parseClipId(data.clip_id);
            const accountId = parseAccountId(data.account_id);
            if (clipId != null || accountId != null) {
              await api.applySidecarFlowRuntimeHint({
                account_id: accountId ?? 0,
                hint: "caption_ready",
                clip_id: clipId ?? undefined,
              });
            }
          } catch {
            /* No matching flow is normal. */
          }
          refreshRuntimeState();
        }
      })();
    });

    const unsubSpeechSeg = wsClient.on("speech_segment_ready", (data) => {
      void (async () => {
        try {
          await insertSpeechSegmentFromWsPayload(data);
        } catch (err) {
          logSidecarDbSyncError("speech_segment_ready → SQLite insert failed", err);
        } finally {
          refreshRuntimeState();
        }
      })();
    });

    wsClient.connect(sidecarPort);

    return () => {
      unsubClip();
      unsubCaptionReady();
      unsubSpeechSeg();
      wsClient.disconnect();
    };
  }, [sidecarPort]);
}
