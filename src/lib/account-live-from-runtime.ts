import type { FlowRuntimeSnapshot } from "@/types";

export type AccountLiveFlag = {
  id: number;
  isLive: boolean;
};

function snapshotIndicatesLive(snapshot: FlowRuntimeSnapshot): boolean {
  if (snapshot.status === "recording") {
    return true;
  }
  if (snapshot.status === "watching") {
    return snapshot.last_live_at != null;
  }
  return false;
}

export function deriveAccountLiveFlagsFromRuntime(
  snapshots: FlowRuntimeSnapshot[],
): AccountLiveFlag[] {
  const byAccount = new Map<number, boolean>();

  for (const snapshot of snapshots) {
    if (snapshot.account_id == null) {
      continue;
    }

    const accountId = Number(snapshot.account_id);
    if (!Number.isFinite(accountId)) {
      continue;
    }

    const nextIsLive = snapshotIndicatesLive(snapshot);
    byAccount.set(accountId, (byAccount.get(accountId) ?? false) || nextIsLive);
  }

  return Array.from(byAccount.entries())
    .sort(([left], [right]) => left - right)
    .map(([id, isLive]) => ({ id, isLive }));
}
