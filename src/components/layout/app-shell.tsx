import { useEffect, useRef, useState, type ComponentType } from "react";
import { isTauri } from "@tauri-apps/api/core";
import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";
import { useSidecar } from "@/hooks/use-sidecar";
import * as api from "@/lib/api";
import { wsClient } from "@/lib/ws";
import { AccountsPage } from "@/pages/accounts";
import { DashboardPage } from "@/pages/dashboard";
import { FlowsPage } from "@/pages/flows";
import { ProductsPage } from "@/pages/products";
import { SettingsPage } from "@/pages/settings";
import {
  insertClipFromSidecarWsPayload,
  insertSpeechSegmentFromWsPayload,
  syncClipCaptionFromWsPayload,
  syncRecordingFromSidecarWsPayload,
} from "@/lib/sidecar-db-sync";
import { hydrateNotificationsFromDb } from "@/lib/notifications-sync";
import { dispatchSidecarNotification } from "@/lib/sidecar-notifications";
import { useAccountStore } from "@/stores/account-store";
import { useAppStore } from "@/stores/app-store";
import { useClipStore } from "@/stores/clip-store";
import {
  applyRecordingWsPayload,
  countActiveRecordings,
  useRecordingStore,
} from "@/stores/recording-store";
import { Sidebar } from "./sidebar";
import { TopBar } from "./top-bar";

type PageId =
  | "dashboard"
  | "accounts"
  | "flows"
  | "products"
  | "statistics"
  | "settings";

const pageMeta: Record<PageId, { title: string; subtitle: string }> = {
  dashboard: { title: "Dashboard", subtitle: "Overview of all activities" },
  accounts: { title: "Accounts", subtitle: "Manage TikTok accounts" },
  flows: { title: "Flows", subtitle: "Monitor and control account automation flows" },
  products: { title: "Products", subtitle: "Product catalog and tagging" },
  statistics: { title: "Statistics", subtitle: "Analytics and reports" },
  settings: { title: "Settings", subtitle: "App configuration" },
};

const pageComponents: Record<PageId, ComponentType> = {
  dashboard: DashboardPage,
  accounts: AccountsPage,
  flows: FlowsPage,
  products: ProductsPage,
  statistics: () => (
    <p className="text-[var(--color-text-muted)]">Statistics coming in Phase 3.</p>
  ),
  settings: SettingsPage,
};

const FINISHED_CLEANUP_MS = 8000;
/** HTTP backup for live flags; sidecar poll is ~30s, sync often enough for UI without hammering. */
const LIVE_HTTP_SYNC_MS = 5000;
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

async function syncLiveFromSidecarHttp(): Promise<void> {
  try {
    const rows = await api.getLiveOverview();
    await api.syncAccountsLiveStatus(
      rows.map((r) => ({ account_id: r.account_id, is_live: r.is_live })),
    );
    useAccountStore.getState().applyLiveFlagsFromSidecar(
      rows.map((r) => ({ id: r.account_id, isLive: r.is_live })),
    );
  } catch (e) {
    if (import.meta.env.DEV) {
      console.warn("[TikClip] syncLiveFromSidecarHttp failed", e);
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

  useEffect(() => {
    setActiveRecordings(activeRecordingCount);
  }, [activeRecordingCount, setActiveRecordings]);

  useEffect(() => {
    if (!navigationTarget) {
      return;
    }
    const p = navigationTarget.page;
    if (
      p === "dashboard" ||
      p === "accounts" ||
      p === "flows" ||
      p === "products" ||
      p === "statistics" ||
      p === "settings"
    ) {
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
    if (sidecarPort == null) {
      wsClient.disconnect();
      useRecordingStore.getState().hydrateFromSidecar([]);
      return;
    }

    const onRecordingEvent = (data: Record<string, unknown>) => {
      applyRecordingWsPayload(data);
      void syncRecordingFromSidecarWsPayload(data).catch((err) =>
        logSidecarDbSyncError("recording → SQLite sync failed", err),
      );
    };

    const onFinished = (data: Record<string, unknown>) => {
      applyRecordingWsPayload(data);
      void syncRecordingFromSidecarWsPayload(data).catch((err) =>
        logSidecarDbSyncError("recording_finished → SQLite sync failed", err),
      );
      dispatchSidecarNotification("recording_finished", data);
      useAppStore.getState().bumpDashboardRevision();
      const id = data.recording_id;
      if (typeof id === "string") {
        window.setTimeout(() => {
          useRecordingStore.getState().removeRecording(id);
        }, FINISHED_CLEANUP_MS);
      }
    };

    const unsubStart = wsClient.on("recording_started", onRecordingEvent);
    const unsubProgress = wsClient.on("recording_progress", onRecordingEvent);
    const unsubFinished = wsClient.on("recording_finished", onFinished);
    const persistLive = (id: number, isLive: boolean, source: string) => {
      void (async () => {
        try {
          await api.updateAccountLiveStatus(id, isLive);
          const ok = useAccountStore.getState().patchAccountLive(id, isLive);
          if (!ok) {
            void useAccountStore.getState().fetchAccounts();
          }
        } catch (err) {
          console.warn(`[TikClip] ${source} → SQLite failed, refetching`, err);
          void useAccountStore.getState().fetchAccounts();
        }
      })();
    };

    const unsubLive = wsClient.on("account_live", (data) => {
      dispatchSidecarNotification("account_live", data);
      useAppStore.getState().bumpDashboardRevision();
      const rawId = data.account_id;
      const id = typeof rawId === "number" ? rawId : Number(rawId);
      if (Number.isFinite(id)) {
        persistLive(id, true, "account_live");
      }
    });
    const unsubAccountStatus = wsClient.on("account_status", (data) => {
      const rawId = data.account_id;
      const rawLive = data.is_live;
      const id = typeof rawId === "number" ? rawId : Number(rawId);
      const isLive =
        typeof rawLive === "boolean"
          ? rawLive
          : rawLive === 1 || rawLive === "1" || rawLive === "true";
      if (!Number.isFinite(id)) {
        return;
      }
      persistLive(id, isLive, "account_status");
      useAppStore.getState().bumpDashboardRevision();
    });
    const unsubClip = wsClient.on("clip_ready", (data) => {
      dispatchSidecarNotification("clip_ready", data);
      useAppStore.getState().bumpDashboardRevision();
      void (async () => {
        try {
          const clipId = await insertClipFromSidecarWsPayload(data);
          useClipStore.getState().bumpClipsRevision();
          if (clipId != null) {
            void maybeGenerateCaptionAfterInsert(clipId, data);
            void maybeAutoTagClipAfterInsert(clipId, data);
          }
        } catch (err) {
          logSidecarDbSyncError("clip_ready → SQLite insert failed", err);
        }
      })();
    });

    const unsubCaptionReady = wsClient.on("caption_ready", (data) => {
      void (async () => {
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
      })();
    });

    const unsubSpeechSeg = wsClient.on("speech_segment_ready", (data) => {
      void (async () => {
        try {
          await insertSpeechSegmentFromWsPayload(data);
        } catch (err) {
          logSidecarDbSyncError("speech_segment_ready → SQLite insert failed", err);
        }
      })();
    });

    const unsubCleanup = wsClient.on("cleanup_completed", (data) => {
      dispatchSidecarNotification("cleanup_completed", data);
      useAppStore.getState().bumpDashboardRevision();
    });
    const unsubStorageWarn = wsClient.on("storage_warning", (data) => {
      dispatchSidecarNotification("storage_warning", data);
      useAppStore.getState().bumpDashboardRevision();
    });

    wsClient.connect(sidecarPort);

    return () => {
      unsubStart();
      unsubProgress();
      unsubFinished();
      unsubLive();
      unsubAccountStatus();
      unsubClip();
      unsubCaptionReady();
      unsubSpeechSeg();
      unsubCleanup();
      unsubStorageWarn();
      wsClient.disconnect();
    };
  }, [sidecarPort]);

  useEffect(() => {
    if (sidecarPort == null || !sidecarConnected) {
      return;
    }
    let cancelled = false;
    void api.getRecordingStatus()
      .then((list) => {
        if (!cancelled) {
          useRecordingStore.getState().hydrateFromSidecar(list);
          for (const r of list) {
            void syncRecordingFromSidecarWsPayload({
              recording_id: r.recording_id,
              account_id: r.account_id,
              status: r.status,
              duration_seconds: r.duration_seconds,
              file_size_bytes: r.file_size_bytes,
              file_path: r.file_path,
              error_message: r.error_message,
            }).catch((err) =>
              logSidecarDbSyncError("recording status poll → SQLite sync failed", err),
            );
          }
        }
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [sidecarPort, sidecarConnected]);

  useEffect(() => {
    if (sidecarPort == null || !sidecarConnected) {
      return;
    }
    let cancelled = false;
    void (async () => {
      await useAccountStore.getState().fetchAccounts();
      if (cancelled) {
        return;
      }
      const accounts = useAccountStore.getState().accounts;
      await api.syncWatcherForAccounts(
        accounts.map((a) => ({
          id: a.id,
          username: a.username,
          auto_record: a.auto_record,
          cookies_json: a.cookies_json,
          proxy_url: a.proxy_url,
        })),
      );
      if (!cancelled) {
        try {
          const fresh = await api.pollNow();
          await api.syncAccountsLiveStatus(
            fresh.map((r) => ({ account_id: r.account_id, is_live: r.is_live })),
          );
          useAccountStore.getState().applyLiveFlagsFromSidecar(
            fresh.map((r) => ({ id: r.account_id, isLive: r.is_live })),
          );
        } catch {
          await syncLiveFromSidecarHttp();
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [sidecarPort, sidecarConnected]);

  useEffect(() => {
    if (sidecarPort == null || !sidecarConnected) {
      return;
    }
    void syncLiveFromSidecarHttp();
    const timer = window.setInterval(() => {
      void syncLiveFromSidecarHttp();
    }, LIVE_HTTP_SYNC_MS);
    return () => {
      window.clearInterval(timer);
    };
  }, [sidecarPort, sidecarConnected]);

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
          sidecarConnected={sidecarConnected}
        />
        <main className="flex-1 overflow-y-auto px-6 pb-8 pt-6 sm:px-8">
          <div className="mx-auto flex w-full max-w-[1280px] flex-col gap-8">
            <PageComponent />
          </div>
        </main>
      </div>
    </div>
  );
}
