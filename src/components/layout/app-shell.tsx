import { useEffect, useState, type ComponentType } from "react";
import { useSidecar } from "@/hooks/use-sidecar";
import * as api from "@/lib/api";
import { wsClient } from "@/lib/ws";
import { AccountsPage } from "@/pages/accounts";
import { ClipsPage } from "@/pages/clips";
import { DashboardPage } from "@/pages/dashboard";
import { RecordingsPage } from "@/pages/recordings";
import { SettingsPage } from "@/pages/settings";
import { dispatchSidecarNotification } from "@/lib/sidecar-notifications";
import { useAccountStore } from "@/stores/account-store";
import { useAppStore } from "@/stores/app-store";
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
  | "recordings"
  | "clips"
  | "statistics"
  | "settings";

const pageMeta: Record<PageId, { title: string; subtitle: string }> = {
  dashboard: { title: "Dashboard", subtitle: "Overview of all activities" },
  accounts: { title: "Accounts", subtitle: "Manage TikTok accounts" },
  recordings: { title: "Recordings", subtitle: "Active and completed recordings" },
  clips: { title: "Clips", subtitle: "Generated video clips" },
  statistics: { title: "Statistics", subtitle: "Analytics and reports" },
  settings: { title: "Settings", subtitle: "App configuration" },
};

const pageComponents: Record<PageId, ComponentType> = {
  dashboard: DashboardPage,
  accounts: AccountsPage,
  recordings: RecordingsPage,
  clips: ClipsPage,
  statistics: () => (
    <p className="text-[var(--color-text-muted)]">Statistics coming in Phase 3.</p>
  ),
  settings: SettingsPage,
};

const FINISHED_CLEANUP_MS = 8000;

export function AppShell() {
  useSidecar();
  const [currentPage, setCurrentPage] = useState<PageId>("dashboard");
  const sidecarPort = useAppStore((s) => s.sidecarPort);
  const sidecarConnected = useAppStore((s) => s.sidecarConnected);
  const activeRecordings = useAppStore((s) => s.activeRecordings);
  const setActiveRecordings = useAppStore((s) => s.setActiveRecordings);
  const activeRecordingCount = useRecordingStore((s) => countActiveRecordings(s.recordings));

  useEffect(() => {
    setActiveRecordings(activeRecordingCount);
  }, [activeRecordingCount, setActiveRecordings]);

  useEffect(() => {
    if (sidecarPort == null) {
      wsClient.disconnect();
      useRecordingStore.getState().hydrateFromSidecar([]);
      return;
    }
    wsClient.connect(sidecarPort);

    const onRecordingEvent = (data: Record<string, unknown>) => {
      applyRecordingWsPayload(data);
    };

    const onFinished = (data: Record<string, unknown>) => {
      applyRecordingWsPayload(data);
      dispatchSidecarNotification("recording_finished", data);
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
    });
    const unsubClip = wsClient.on("clip_ready", (data) => {
      dispatchSidecarNotification("clip_ready", data);
    });

    return () => {
      unsubStart();
      unsubProgress();
      unsubFinished();
      unsubLive();
      unsubAccountStatus();
      unsubClip();
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
    })();
    return () => {
      cancelled = true;
    };
  }, [sidecarPort, sidecarConnected]);

  const meta = pageMeta[currentPage];
  const PageComponent = pageComponents[currentPage];

  return (
    <div className="flex h-screen bg-[var(--color-bg)] text-[var(--color-text)]">
      <Sidebar
        currentPage={currentPage}
        onNavigate={setCurrentPage}
        sidecarConnected={sidecarConnected}
        activeRecordings={activeRecordings}
      />
      <div className="flex flex-1 flex-col overflow-hidden">
        <TopBar title={meta.title} subtitle={meta.subtitle} />
        <main className="flex-1 overflow-y-auto p-6">
          <PageComponent />
        </main>
      </div>
    </div>
  );
}
