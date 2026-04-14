import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { ClipStatus } from "@/types";

const STATUS_CLASS: Record<ClipStatus, string> = {
  draft: "border-transparent bg-zinc-700 text-zinc-300",
  ready: "border-transparent bg-emerald-900 text-emerald-300",
  posted: "border-transparent bg-blue-900 text-blue-300",
  archived: "border-transparent bg-zinc-800 text-zinc-500",
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
