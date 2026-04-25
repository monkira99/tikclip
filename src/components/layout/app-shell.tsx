import { useEffect, useRef, useState, type ComponentType } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";
import { useSidecar } from "@/hooks/use-sidecar";
import * as api from "@/lib/api";
import { wsClient } from "@/lib/ws";
import { DashboardPage } from "@/pages/dashboard";
import { FlowsPage } from "@/pages/flows";
import { ProductsPage } from "@/pages/products";
import { SettingsPage } from "@/pages/settings";
import {
  insertClipFromSidecarWsPayload,
  insertSpeechSegmentFromWsPayload,
  syncClipCaptionFromWsPayload,
} from "@/lib/sidecar-db-sync";
import { deriveAccountLiveFlagsFromRuntime } from "@/lib/account-live-from-runtime";
import { hydrateNotificationsFromDb } from "@/lib/notifications-sync";
import {
  dispatchSidecarNotification,
  displayRuntimeNotification,
} from "@/lib/sidecar-notifications";
import { cn } from "@/lib/utils";
import { useAccountStore } from "@/stores/account-store";
import { useAppStore } from "@/stores/app-store";
import { useClipStore } from "@/stores/clip-store";
import { useFlowStore } from "@/stores/flow-store";
import { countActiveRecordings, useRecordingStore } from "@/stores/recording-store";
import type { FlowRuntimeLogEntry, FlowRuntimeSnapshot } from "@/types";
import { Sidebar } from "./sidebar";
import { TopBar } from "./top-bar";

type PageId = "dashboard" | "flows" | "products" | "statistics" | "settings";

const pageMeta: Record<PageId, { title: string; subtitle: string }> = {
  dashboard: { title: "Dashboard", subtitle: "Overview of all activities" },
  flows: { title: "Flows", subtitle: "Monitor and control account automation flows" },
  products: { title: "Products", subtitle: "Product catalog and tagging" },
  statistics: { title: "Statistics", subtitle: "Analytics and reports" },
  settings: { title: "Settings", subtitle: "App configuration" },
};

const pageComponents: Record<PageId, ComponentType> = {
  dashboard: DashboardPage,
  flows: FlowsPage,
  products: ProductsPage,
  statistics: () => (
    <p className="text-[var(--color-text-muted)]">Statistics coming in Phase 3.</p>
  ),
  settings: SettingsPage,
};

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

function parseAutoTagClipProductEnabled(raw: string | null): boolean {
  if (raw === null || raw.trim() === "") {
    return false;
  }
  const t = raw.trim().toLowerCase();
  return t === "1" || t === "true" || t === "yes" || t === "on";
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
    if (!parseAutoTagClipProductEnabled(raw)) {
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

export function AppShell() {
  useSidecar();
  const [currentPage, setCurrentPage] = useState<PageId>("dashboard");
  const sidecarPort = useAppStore((s) => s.sidecarPort);
  const sidecarConnected = useAppStore((s) => s.sidecarConnected);
  const navigationTarget = useAppStore((s) => s.navigationTarget);
  const activeRecordings = useAppStore((s) => s.activeRecordings);
  const setActiveRecordings = useAppStore((s) => s.setActiveRecordings);
  const activeRecordingCount = useRecordingStore((s) => countActiveRecordings(s.recordings));
  const notifyWarmupDone = useRef(false);
  const refreshRuntimeState = () => {
    void refreshRuntimeAndSyncAccountLiveFlags();
  };

  useEffect(() => {
    setActiveRecordings(activeRecordingCount);
  }, [activeRecordingCount, setActiveRecordings]);

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
            raw === "statistics" ||
            raw === "settings"
          ? raw
          : null;
    if (p) {
      setCurrentPage(p);
    }
    if (navigationTarget.clipId != null) {
      useClipStore.getState().setActiveClipId(navigationTarget.clipId);
    }
    useAppStore.getState().clearNavigationTarget();
  }, [navigationTarget]);

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

  useEffect(() => {
    refreshRuntimeState();
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
    if (sidecarPort == null) {
      wsClient.disconnect();
      return;
    }

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


  const meta = pageMeta[currentPage];
  const PageComponent = pageComponents[currentPage];

  return (
    <div className="relative flex h-screen bg-[var(--color-bg)] text-[var(--color-text)]">
      <div className="pointer-events-none absolute inset-0 overflow-hidden">
        <div className="absolute left-[-10%] top-[-8%] h-72 w-72 rounded-full bg-[rgba(85,179,255,0.08)] blur-3xl" />
        <div className="absolute right-[-6%] top-[8%] h-60 w-60 rounded-full bg-[rgba(255,99,99,0.08)] blur-3xl" />
        <div className="absolute bottom-[-12%] left-[28%] h-80 w-80 rounded-full bg-[var(--color-warm-glow)] blur-3xl" />
      </div>
      <Sidebar
        currentPage={currentPage}
        onNavigate={setCurrentPage}
        sidecarConnected={sidecarConnected}
        activeRecordings={activeRecordings}
      />
      <div className="relative flex flex-1 flex-col overflow-hidden">
        <TopBar
          title={meta.title}
          subtitle={meta.subtitle}
        />
        <main
          className={cn(
            "flex-1 overflow-y-auto px-6 pt-6 sm:px-8",
            currentPage === "flows" ? "pb-3" : "pb-8",
          )}
        >
          <div className="mx-auto flex w-full max-w-[1280px] flex-col gap-8">
            <PageComponent />
          </div>
        </main>
      </div>
    </div>
  );
}
