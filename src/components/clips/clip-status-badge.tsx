import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { ClipStatus } from "@/types";

const STATUS_CLASS: Record<ClipStatus, string> = {
  draft: "border-white/10 bg-white/[0.05] text-[var(--color-text-muted)]",
  ready: "border-[rgba(95,201,146,0.2)] bg-[rgba(95,201,146,0.14)] text-[var(--color-success)]",
  posted: "border-[rgba(85,179,255,0.2)] bg-[rgba(85,179,255,0.14)] text-[var(--color-accent)]",
  archived: "border-white/8 bg-white/[0.03] text-[#6a6b6c]",
};

export function ClipStatusBadge({ status }: { status: ClipStatus }) {
  return (
    <Badge
      variant="secondary"
      className={cn("text-[10px] capitalize", STATUS_CLASS[status])}
    >
      {status}
    </Badge>
  );
}
