import { Badge } from "@/components/ui/badge";
import type { AccountStatus } from "@/types";

const statusConfig: Record<
  AccountStatus,
  { label: string; className: string }
> = {
  live: {
    label: "Live",
    className:
      "border-[rgba(95,201,146,0.2)] bg-[rgba(95,201,146,0.14)] text-[var(--color-success)]",
  },
  offline: {
    label: "Offline",
    className:
      "border-white/8 bg-white/[0.03] text-[var(--color-text-muted)]",
  },
  recording: {
    label: "Recording",
    className:
      "border-[rgba(255,99,99,0.2)] bg-[rgba(255,99,99,0.14)] text-[var(--color-primary)]",
  },
};

export function AccountBadge({ status }: { status: AccountStatus }) {
  const config = statusConfig[status];
  return (
    <Badge variant="outline" className={config.className}>
      {status === "live" && (
        <span className="mr-1.5 size-1.5 animate-pulse rounded-full bg-[var(--color-success)]" />
      )}
      {status === "recording" && (
        <span className="mr-1.5 size-1.5 animate-pulse rounded-full bg-[var(--color-primary)]" />
      )}
      {config.label}
    </Badge>
  );
}
