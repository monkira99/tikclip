import { Clapperboard, GitBranch, HardDrive, Radio } from "lucide-react";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { cn } from "@/lib/utils";

type StatCardsProps = {
  activeRecordings: number;
  flowCount: number;
  clipsToday: number;
  /** Bytes used: max(DB totals, paths in DB, recursive scan of `clips/` + `records/` under storage root). */
  storageUsedBytes: number;
  /** Max storage (GB) from Settings when set; `null` if quota disabled. */
  storageQuotaGb: number | null;
  /** Optional: usage % from Rust storage scan vs quota (when quota set). Drives warn/critical styling. */
  storageUsagePercent?: number | null;
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
    return "";
  }
  if (usagePct > 95) {
    return "border-[rgba(255,99,99,0.24)] bg-[rgba(255,99,99,0.08)]";
  }
  if (usagePct >= 80) {
    return "border-[rgba(255,188,51,0.24)] bg-[rgba(255,188,51,0.08)]";
  }
  return "";
}

export function StatCards({
  activeRecordings,
  flowCount,
  clipsToday,
  storageUsedBytes,
  storageQuotaGb,
  storageUsagePercent,
}: StatCardsProps) {
  const cards = [
    {
      title: "Active recordings",
      value: String(activeRecordings),
      icon: Radio,
    },
    {
      title: "Flows",
      value: String(flowCount),
      icon: GitBranch,
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
            ? storageUsagePercent
            : null;
        return (
          <Card
            key={title}
            size="sm"
            className={cn("min-h-[152px] justify-between", isStorage ? storageCardClass(pct ?? null) : "")}
          >
            <CardHeader className="flex flex-row items-start justify-between pb-0">
              <div>
                <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--color-text-muted)]">
                  {title}
                </p>
              </div>
              <div className="flex size-10 items-center justify-center rounded-xl border border-white/8 bg-white/[0.03]">
                <Icon className="size-4 text-[var(--color-accent)]" aria-hidden />
              </div>
            </CardHeader>
            <CardContent className="space-y-2">
              <p className="font-heading text-3xl font-semibold tracking-tight tabular-nums text-[var(--color-text)]">
                {value}
              </p>
              <p className="text-sm leading-relaxed text-[var(--color-text-muted)]">
                {title === "Active recordings" && "Jobs that are still capturing stream data."}
                {title === "Flows" && "Published automations available for polling and recording."}
                {title === "Clips today" && "Short-form assets created from recent live sessions."}
                {title === "Storage" && "Current media footprint across recordings and generated clips."}
              </p>
              {isStorage &&
              pct != null &&
              Number.isFinite(pct) &&
              storageQuotaGb != null &&
              storageQuotaGb > 0 ? (
                <p className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--color-text-muted)] tabular-nums">
                  ~{pct.toFixed(1)}% quota
                </p>
              ) : null}
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}
