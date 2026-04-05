import { Badge } from "@/components/ui/badge";
import type { AccountStatus } from "@/types";

const statusConfig: Record<
  AccountStatus,
  { label: string; className: string }
> = {
  live: {
    label: "Live",
    className:
      "border-green-500/30 bg-green-500/20 text-green-400 dark:text-green-300",
  },
  offline: {
    label: "Offline",
    className:
      "border-muted-foreground/30 bg-muted/50 text-muted-foreground",
  },
  recording: {
    label: "Recording",
    className: "border-red-500/30 bg-red-500/20 text-red-400 dark:text-red-300",
  },
};

export function AccountBadge({ status }: { status: AccountStatus }) {
  const config = statusConfig[status];
  return (
    <Badge variant="outline" className={config.className}>
      {status === "live" && (
        <span className="mr-1.5 size-1.5 animate-pulse rounded-full bg-green-400" />
      )}
      {status === "recording" && (
        <span className="mr-1.5 size-1.5 animate-pulse rounded-full bg-red-400" />
      )}
      {config.label}
    </Badge>
  );
}
