import { useEffect } from "react";
import { StatCards } from "@/components/dashboard/stat-cards";
import { RecordingProgress } from "@/components/recordings/recording-progress";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useAppStore } from "@/stores/app-store";
import { useAccountStore } from "@/stores/account-store";
import { countActiveRecordings, useRecordingStore } from "@/stores/recording-store";

export function DashboardPage() {
  const sidecarConnected = useAppStore((s) => s.sidecarConnected);
  const recordings = useRecordingStore((s) => s.recordings);
  const accounts = useAccountStore((s) => s.accounts);
  const fetchAccounts = useAccountStore((s) => s.fetchAccounts);
  const accountsLoading = useAccountStore((s) => s.loading);

  useEffect(() => {
    void fetchAccounts();
  }, [fetchAccounts]);

  const activeList = Object.values(recordings).filter(
    (r) => r.status === "pending" || r.status === "recording",
  );
  const activeCount = countActiveRecordings(recordings);
  const liveAccounts = accounts.filter((a) => a.is_live);

  return (
    <div className="flex flex-col gap-8">
      <StatCards
        activeRecordings={activeCount}
        accountCount={accounts.length}
        clipsToday={0}
        storageUsedGb={0}
        storageTotalGb={100}
      />

      <div className="grid gap-6 lg:grid-cols-2">
        <Card className="bg-[var(--color-bg-elevated)]">
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

        <Card className="bg-[var(--color-bg-elevated)]">
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
                    className="flex items-center justify-between rounded-lg border border-foreground/10 px-3 py-2"
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
    </div>
  );
}
