import { Clapperboard, HardDrive, Radio, Users } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

type StatCardsProps = {
  activeRecordings: number;
  accountCount: number;
  clipsToday: number;
  /** Bytes used: max(DB totals, paths in DB, recursive scan of `clips/` + `records/` under storage root). */
  storageUsedBytes: number;
  /** Max storage (GB) from Settings when set; `null` if quota disabled. */
  storageQuotaGb: number | null;
  /** Optional: usage % from sidecar scan vs quota (when quota set). Drives warn/critical card styling. */
  storageSidecarUsagePercent?: number | null;
};

/** Human-readable used size; avoids showing "0.00 GB" for hundreds of MB. */
function formatUsedBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return "0";
  }
  const gb = bytes / (1024 * 1024 * 1024);
  if (gb >= 1) {
    if (gb >= 100) return `${gb.toFixed(0)} GB`;
    if (gb >= 10) return `${gb.toFixed(1)} GB`;
    return `${gb.toFixed(2)} GB`;
  }
  const mb = bytes / (1024 * 1024);
  if (mb >= 1) {
    return mb >= 100 ? `${mb.toFixed(0)} MB` : `${mb.toFixed(1)} MB`;
  }
  const kb = bytes / 1024;
  return kb >= 1 ? `${Math.round(kb)} KB` : `${bytes} B`;
}

function storageLabel(usedBytes: number, quotaGb: number | null): string {
  const used = formatUsedBytes(usedBytes);
  if (quotaGb != null && quotaGb > 0) {
    const q =
      quotaGb >= 100
        ? quotaGb.toFixed(0)
        : quotaGb >= 10
          ? quotaGb.toFixed(1)
          : quotaGb.toFixed(2);
    return `${used} / ${q} GB`;
  }
  return used;
}

function storageCardClass(usagePct: number | null | undefined): string {
  if (usagePct == null || !Number.isFinite(usagePct) || usagePct <= 0) {
    return "bg-[var(--color-bg-elevated)]";
  }
  if (usagePct > 95) {
    return "border border-red-500/40 bg-[var(--color-bg-elevated)]";
  }
  if (usagePct >= 80) {
    return "border border-amber-500/40 bg-[var(--color-bg-elevated)]";
  }
  return "bg-[var(--color-bg-elevated)]";
}

export function StatCards({
  activeRecordings,
  accountCount,
  clipsToday,
  storageUsedBytes,
  storageQuotaGb,
  storageSidecarUsagePercent,
}: StatCardsProps) {
  const cards = [
    {
      title: "Active recordings",
      value: String(activeRecordings),
      icon: Radio,
    },
    {
      title: "Accounts",
      value: String(accountCount),
      icon: Users,
    },
    {
      title: "Clips today",
      value: String(clipsToday),
      icon: Clapperboard,
    },
    {
      title: "Storage",
      value: storageLabel(storageUsedBytes, storageQuotaGb),
      icon: HardDrive,
    },
  ] as const;

  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
      {cards.map(({ title, value, icon: Icon }) => {
        const isStorage = title === "Storage";
        const pct =
          isStorage && storageQuotaGb != null && storageQuotaGb > 0
            ? storageSidecarUsagePercent
            : null;
        return (
          <Card
            key={title}
            size="sm"
            className={isStorage ? storageCardClass(pct ?? null) : "bg-[var(--color-bg-elevated)]"}
          >
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium text-[var(--color-text-muted)]">
                {title}
              </CardTitle>
              <Icon className="size-4 text-[var(--color-text-muted)]" aria-hidden />
            </CardHeader>
            <CardContent>
              <p className="font-heading text-2xl font-semibold tabular-nums text-[var(--color-text)]">
                {value}
              </p>
              {isStorage &&
              pct != null &&
              Number.isFinite(pct) &&
              storageQuotaGb != null &&
              storageQuotaGb > 0 ? (
                <p className="mt-1 text-xs text-[var(--color-text-muted)] tabular-nums">
                  ~{pct.toFixed(1)}% quota (sidecar)
                </p>
              ) : null}
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}
