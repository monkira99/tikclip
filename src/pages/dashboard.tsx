import { useCallback, useEffect, useMemo, useState } from "react";
import { ActivityFeed } from "@/components/dashboard/activity-feed";
import { StatCards } from "@/components/dashboard/stat-cards";
import { RecordingProgress } from "@/components/recordings/recording-progress";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { getDashboardStats, getStorageStats, type DashboardStats } from "@/lib/api";
import { useAppStore } from "@/stores/app-store";
import { useClipStore } from "@/stores/clip-store";
import { useFlowStore } from "@/stores/flow-store";
import { countActiveRecordings, useRecordingStore } from "@/stores/recording-store";

export function DashboardPage() {
  const dashboardRevision = useAppStore((s) => s.dashboardRevision);
  const recordings = useRecordingStore((s) => s.recordings);
  const flows = useFlowStore((s) => s.flows);
  const flowRuntimeSnapshots = useFlowStore((s) => s.runtimeSnapshots);
  const fetchFlows = useFlowStore((s) => s.fetchFlows);
  const flowsLoading = useFlowStore((s) => s.loading);
  const clipsRevision = useClipStore((s) => s.clipsRevision);
  const [dashStats, setDashStats] = useState<DashboardStats | null>(null);
  const [storageUsagePct, setStorageUsagePct] = useState<number | null>(null);
  /** Rust filesystem scan; preferred for Storage card because it reflects local files directly. */
  const [storageTotalBytes, setStorageTotalBytes] = useState<number | null>(null);

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

  const loadStorageStats = useCallback(async () => {
    try {
      const s = await getStorageStats();
      setStorageUsagePct(s.usage_percent);
      setStorageTotalBytes(s.total_bytes);
    } catch (e) {
      if (import.meta.env.DEV) {
        console.warn("[TikClip] getStorageStats failed", e);
      }
      setStorageUsagePct(null);
      setStorageTotalBytes(null);
    }
  }, []);

  /** Refetch stats when recording progress / finish changes (not only clip_revision). */
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
    void fetchFlows({ quiet: true });
  }, [fetchFlows, dashboardRevision]);

  useEffect(() => {
    const t = window.setTimeout(() => {
      void loadDashboardStats();
      void loadStorageStats();
    }, 500);
    return () => window.clearTimeout(t);
  }, [
    loadDashboardStats,
    loadStorageStats,
    clipsRevision,
    dashboardRevision,
    recordingsSnapshot,
  ]);

  /** Refresh storage card when user returns to the app; disk totals can change while away. */
  useEffect(() => {
    const onVis = () => {
      if (document.visibilityState === "visible") {
        void loadDashboardStats();
        void loadStorageStats();
      }
    };
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  }, [loadDashboardStats, loadStorageStats]);

  /** Periodic refresh: DB-backed stat can lag vs disk; Rust scan picks up new recordings/clips. */
  useEffect(() => {
    const id = window.setInterval(() => {
      void loadDashboardStats();
      void loadStorageStats();
    }, 60_000);
    return () => window.clearInterval(id);
  }, [loadDashboardStats, loadStorageStats]);

  const activeList = Object.values(recordings).filter(
    (r) => r.status === "pending" || r.status === "recording",
  );
  const activeCount = countActiveRecordings(recordings);
  const liveFlows = flows.filter((flow) => {
    const snapshot = flowRuntimeSnapshots[flow.id];
    return (
      snapshot?.last_check_live === true ||
      flow.status === "recording" ||
      flow.status === "processing"
    );
  });

  const storageDisplayBytes =
    storageTotalBytes != null
      ? storageTotalBytes
      : (dashStats?.storageUsedBytes ?? 0);

  return (
    <div className="flex flex-col gap-6">
      <StatCards
        activeRecordings={activeCount}
        flowCount={flows.length}
        clipsToday={dashStats?.clipsToday ?? 0}
        storageUsedBytes={storageDisplayBytes}
        storageQuotaGb={dashStats?.storageQuotaGb ?? null}
        storageUsagePercent={storageUsagePct}
      />

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Active recordings</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            {activeList.length === 0 ? (
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
            {flowsLoading && flows.length === 0 ? (
              <p className="text-sm text-[var(--color-text-muted)]">Loading flows…</p>
            ) : liveFlows.length === 0 ? (
              <p className="text-sm text-[var(--color-text-muted)]">No flows are live.</p>
            ) : (
              <ul className="space-y-2 text-sm">
                {liveFlows.map((flow) => (
                  <li
                    key={flow.id}
                    className="flex items-center justify-between rounded-xl border border-white/8 bg-white/[0.03] px-4 py-3"
                  >
                    <span className="font-medium">{flow.name}</span>
                    <span className="text-[var(--color-text-muted)]">
                      {flow.account_username ? `@${flow.account_username}` : flow.status}
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
