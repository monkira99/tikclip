import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import type { FlowNodeKey, FlowStatus, FlowSummary } from "@/types";

const FLOW_STEPS: FlowNodeKey[] = ["start", "record", "clip", "caption", "upload"];

const STATUS_CLASS: Record<FlowStatus, string> = {
  idle: "border-white/10 bg-white/[0.05] text-[var(--color-text-muted)]",
  watching: "border-[rgba(85,179,255,0.2)] bg-[rgba(85,179,255,0.14)] text-[var(--color-accent)]",
  recording: "border-[rgba(95,201,146,0.2)] bg-[rgba(95,201,146,0.14)] text-[var(--color-success)]",
  processing: "border-[rgba(255,188,51,0.2)] bg-[rgba(255,188,51,0.14)] text-[var(--color-warning)]",
  error: "border-[rgba(255,99,99,0.22)] bg-[rgba(255,99,99,0.14)] text-[var(--color-primary)]",
  disabled: "border-white/8 bg-white/[0.03] text-[#6a6b6c]",
};

function prettifyNode(node: FlowNodeKey | null): string {
  if (node == null) {
    return "Waiting";
  }
  return node[0].toUpperCase() + node.slice(1);
}

function formatDateTime(value: string | null): string {
  if (!value) {
    return "Never";
  }
  const normalized = value.includes("T") ? value : value.replace(" ", "T");
  const date = new Date(normalized);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString("vi-VN", {
    hour12: false,
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

type FlowCardProps = {
  flow: FlowSummary;
  busy?: boolean;
  onOpen: (flowId: number) => void;
  onToggleEnabled: (flowId: number, enabled: boolean) => void;
};

export function FlowCard({ flow, busy = false, onOpen, onToggleEnabled }: FlowCardProps) {
  const currentNodeIndex = flow.current_node ? FLOW_STEPS.indexOf(flow.current_node) : -1;

  return (
    <Card className="gap-4" size="sm">
      <CardHeader className="pb-0">
        <div className="flex items-start justify-between gap-3">
          <div className="space-y-1">
            <CardTitle className="text-base">{flow.name}</CardTitle>
            <p className="text-xs text-[var(--color-text-muted)]">@{flow.account_username}</p>
          </div>
          <Badge variant="secondary" className={cn("text-[10px] capitalize", STATUS_CLASS[flow.status])}>
            {flow.status}
          </Badge>
        </div>
      </CardHeader>

      <CardContent className="space-y-3 pt-0">
        <div className="space-y-1.5">
          <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--color-text-muted)]">
            Pipeline
          </p>
          <div className="flex items-center gap-1.5">
            {FLOW_STEPS.map((step, idx) => {
              const reached = currentNodeIndex >= idx && flow.enabled;
              const active = flow.current_node === step && flow.enabled;
              return (
                <div key={step} className="flex min-w-0 flex-1 items-center gap-1.5">
                  <span
                    className={cn(
                      "h-1.5 w-full rounded-full border border-white/8 bg-white/[0.06]",
                      reached && "bg-[rgba(85,179,255,0.24)]",
                      active && "bg-[var(--color-accent)]",
                    )}
                    aria-hidden
                  />
                  {idx === FLOW_STEPS.length - 1 ? null : (
                    <span className="h-px w-2 shrink-0 bg-white/15" aria-hidden />
                  )}
                </div>
              );
            })}
          </div>
        </div>

        <div className="grid gap-1 text-xs text-[var(--color-text-muted)] sm:grid-cols-2">
          <div>
            Current node: <span className="text-[var(--color-text)]">{prettifyNode(flow.current_node)}</span>
          </div>
          <div>
            Last live: <span className="text-[var(--color-text)]">{formatDateTime(flow.last_live_at)}</span>
          </div>
          <div>
            Last run: <span className="text-[var(--color-text)]">{formatDateTime(flow.last_run_at)}</span>
          </div>
          <div className="truncate">
            Last error: <span className="text-[var(--color-text)]">{flow.last_error?.trim() || "None"}</span>
          </div>
        </div>
      </CardContent>

      <CardFooter className="flex flex-wrap items-center gap-3">
        <div className="flex flex-wrap items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--color-text-muted)]">
          <span className="rounded-md border border-white/10 bg-white/[0.03] px-2 py-1">
            Recordings {flow.recordings_count}
          </span>
          <span className="rounded-md border border-white/10 bg-white/[0.03] px-2 py-1">
            Clips {flow.clips_count}
          </span>
          <span className="rounded-md border border-white/10 bg-white/[0.03] px-2 py-1">
            Captions {flow.captions_count}
          </span>
        </div>
        <div className="ml-auto flex items-center gap-3">
          <label className="flex items-center gap-2 text-xs text-[var(--color-text-muted)]">
            Enabled
            <Switch
              checked={flow.enabled}
              disabled={busy}
              onCheckedChange={(checked) => onToggleEnabled(flow.id, checked)}
              aria-label={`Toggle flow ${flow.name}`}
            />
          </label>
          <Button variant="outline" size="sm" onClick={() => onOpen(flow.id)}>
            Open
          </Button>
        </div>
      </CardFooter>
    </Card>
  );
}
