import { useCallback, useEffect, useMemo, useState } from "react";
import { ActivityFeed } from "@/components/dashboard/activity-feed";
import { StatCards } from "@/components/dashboard/stat-cards";
import { RecordingProgress } from "@/components/recordings/recording-progress";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { getDashboardStats, getStorageStats, type DashboardStats } from "@/lib/api";
import { useAppStore } from "@/stores/app-store";
import { useAccountStore } from "@/stores/account-store";
import { useClipStore } from "@/stores/clip-store";
import { countActiveRecordings, useRecordingStore } from "@/stores/recording-store";

export function DashboardPage() {
  const sidecarConnected = useAppStore((s) => s.sidecarConnected);
  const dashboardRevision = useAppStore((s) => s.dashboardRevision);
  const recordings = useRecordingStore((s) => s.recordings);
  const accounts = useAccountStore((s) => s.accounts);
  const fetchAccounts = useAccountStore((s) => s.fetchAccounts);
  const accountsLoading = useAccountStore((s) => s.loading);
  const clipsRevision = useClipStore((s) => s.clipsRevision);
  const [dashStats, setDashStats] = useState<DashboardStats | null>(null);
  const [sidecarUsagePct, setSidecarUsagePct] = useState<number | null>(null);
  /** Sidecar filesystem scan — matches debug log; preferred for Storage card when connected. */
  const [sidecarTotalBytes, setSidecarTotalBytes] = useState<number | null>(null);

  const loadDashboardStats = useCallback(async () => {
    try {
      const s = await getDashboardStats();
      setDashStats(s);
    } catch (e) {
      if (import.meta.env.DEV) {
        console.warn("[TikClip] getDashboardStats failed", e);
      }
      setDashStats(null);
    }
  }, []);

  const loadSidecarStorageStats = useCallback(async () => {
    if (!sidecarConnected) {
      setSidecarUsagePct(null);
      setSidecarTotalBytes(null);
      return;
    }
    try {
      const s = await getStorageStats();
      setSidecarUsagePct(s.usage_percent);
      setSidecarTotalBytes(s.total_bytes);
    } catch (e) {
      if (import.meta.env.DEV) {
        console.warn("[TikClip] getStorageStats failed", e);
      }
      setSidecarUsagePct(null);
      setSidecarTotalBytes(null);
    }
  }, [sidecarConnected]);

  /** Refetch stats when sidecar updates recording progress / finish (not only clip_revision). */
  const recordingsSnapshot = useMemo(
    () =>
      JSON.stringify(
        Object.values(recordings)
          .sort((a, b) => a.recording_id.localeCompare(b.recording_id))
          .map((r) => [
            r.recording_id,
            r.status,
            r.duration_seconds,
            r.file_size_bytes,
            r.file_path,
          ]),
      ),
    [recordings],
  );

  useEffect(() => {
    void fetchAccounts();
  }, [fetchAccounts]);

  useEffect(() => {
    const t = window.setTimeout(() => {
      void loadDashboardStats();
      void loadSidecarStorageStats();
    }, 500);
    return () => window.clearTimeout(t);
  }, [
    loadDashboardStats,
    loadSidecarStorageStats,
    clipsRevision,
    dashboardRevision,
    sidecarConnected,
    recordingsSnapshot,
  ]);

  /** Refresh storage card when user returns to the app (sidecar totals change while away). */
  useEffect(() => {
    const onVis = () => {
      if (document.visibilityState === "visible") {
        void loadDashboardStats();
        void loadSidecarStorageStats();
      }
    };
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  }, [loadDashboardStats, loadSidecarStorageStats]);

  /** Periodic refresh: DB-backed stat can lag vs disk; sidecar scan picks up new recordings/clips. */
  useEffect(() => {
    if (!sidecarConnected) {
      return;
    }
    const id = window.setInterval(() => {
      void loadDashboardStats();
      void loadSidecarStorageStats();
    }, 60_000);
    return () => window.clearInterval(id);
  }, [sidecarConnected, loadDashboardStats, loadSidecarStorageStats]);

  const activeList = Object.values(recordings).filter(
    (r) => r.status === "pending" || r.status === "recording",
  );
  const activeCount = countActiveRecordings(recordings);
  const liveAccounts = accounts.filter((a) => a.is_live);

  const storageDisplayBytes =
    sidecarConnected && sidecarTotalBytes != null
      ? sidecarTotalBytes
      : (dashStats?.storageUsedBytes ?? 0);

  return (
    <div className="flex flex-col gap-6">
      <section className="app-panel relative overflow-hidden rounded-[1.25rem] px-6 py-6 sm:px-7">
        <div className="absolute inset-y-0 right-0 hidden w-56 opacity-80 sm:block">
          <div className="absolute right-10 top-0 h-28 w-3 rotate-[28deg] rounded-full bg-[var(--color-primary)]" />
          <div className="absolute right-20 top-8 h-28 w-3 rotate-[28deg] rounded-full bg-[var(--color-primary)]/80" />
          <div className="absolute right-[7.5rem] top-16 h-28 w-3 rotate-[28deg] rounded-full bg-[var(--color-primary)]/55" />
        </div>
        <div className="relative max-w-3xl space-y-4">
          <div className="inline-flex items-center rounded-md border border-white/8 bg-white/[0.03] px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--color-text-muted)]">
            System Pulse
          </div>
          <div className="space-y-2">
            <h3 className="text-3xl font-semibold tracking-tight text-white sm:text-[2.5rem]">
              Monitor live accounts, capture streams, and turn moments into clips.
            </h3>
            <p className="max-w-2xl text-sm leading-7 text-[var(--color-text-soft)] sm:text-base">
              TikClip stays close to the sidecar, keeps live state visible, and surfaces the
              recording pipeline in a compact desktop-first control surface.
            </p>
          </div>
          <div className="flex flex-wrap gap-3 text-sm text-[var(--color-text-soft)]">
            <div className="rounded-full border border-white/8 bg-white/[0.03] px-3 py-1.5">
              {sidecarConnected ? "Realtime pipeline online" : "Realtime pipeline offline"}
            </div>
            <div className="rounded-full border border-white/8 bg-white/[0.03] px-3 py-1.5">
              {activeCount} active recording{activeCount === 1 ? "" : "s"}
            </div>
            <div className="rounded-full border border-white/8 bg-white/[0.03] px-3 py-1.5">
              {liveAccounts.length} live account{liveAccounts.length === 1 ? "" : "s"}
            </div>
          </div>
        </div>
      </section>

      <StatCards
        activeRecordings={activeCount}
        accountCount={accounts.length}
        clipsToday={dashStats?.clipsToday ?? 0}
        storageUsedBytes={storageDisplayBytes}
        storageQuotaGb={dashStats?.storageQuotaGb ?? null}
        storageSidecarUsagePercent={sidecarUsagePct}
      />

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Active recordings</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            {!sidecarConnected ? (
              <p className="text-sm text-[var(--color-text-muted)]">
                Sidecar disconnected — connect to see live recording status.
              </p>
            ) : activeList.length === 0 ? (
              <p className="text-sm text-[var(--color-text-muted)]">No active recordings.</p>
            ) : (
              <div className="flex flex-col gap-4">
                {activeList.map((r) => (
                  <RecordingProgress key={r.recording_id} recording={r} />
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Live now</CardTitle>
          </CardHeader>
          <CardContent>
            {accountsLoading && accounts.length === 0 ? (
              <p className="text-sm text-[var(--color-text-muted)]">Loading accounts…</p>
            ) : liveAccounts.length === 0 ? (
              <p className="text-sm text-[var(--color-text-muted)]">No accounts are live.</p>
            ) : (
              <ul className="space-y-2 text-sm">
                {liveAccounts.map((a) => (
                  <li
                    key={a.id}
                    className="flex items-center justify-between rounded-xl border border-white/8 bg-white/[0.03] px-4 py-3"
                  >
                    <span className="font-medium">@{a.username}</span>
                    <span className="text-[var(--color-text-muted)]">
                      {a.display_name || "—"}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </CardContent>
        </Card>
      </div>

      <ActivityFeed dashboardRevision={dashboardRevision} />
    </div>
  );
}
