import { useState } from "react";
import { AlertTriangle, Loader2, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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

function formatCount(value: number): string {
  return value.toLocaleString("vi-VN");
}

type FlowCardProps = {
  flow: FlowSummary;
  busy?: boolean;
  deleting?: boolean;
  onOpen: (flowId: number) => void;
  onToggleEnabled: (flowId: number, enabled: boolean) => void;
  onDelete: (flowId: number) => Promise<boolean>;
};

export function FlowCard({
  flow,
  busy = false,
  deleting = false,
  onOpen,
  onToggleEnabled,
  onDelete,
}: FlowCardProps) {
  const [confirmOpen, setConfirmOpen] = useState(false);
  const currentNodeIndex = flow.current_node ? FLOW_STEPS.indexOf(flow.current_node) : -1;
  const affectedItems = [
    { label: "Recordings", value: formatCount(flow.recordings_count) },
    { label: "Clips", value: formatCount(flow.clips_count) },
    { label: "Captions", value: formatCount(flow.captions_count) },
  ];

  const handleConfirmDelete = async () => {
    const deleted = await onDelete(flow.id);
    if (deleted) {
      setConfirmOpen(false);
    }
  };

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
            Recordings {formatCount(flow.recordings_count)}
          </span>
          <span className="rounded-md border border-white/10 bg-white/[0.03] px-2 py-1">
            Clips {formatCount(flow.clips_count)}
          </span>
          <span className="rounded-md border border-white/10 bg-white/[0.03] px-2 py-1">
            Captions {formatCount(flow.captions_count)}
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
          <Button variant="outline" size="sm" onClick={() => onOpen(flow.id)} disabled={busy}>
            Open
          </Button>
          <Button variant="destructive" size="sm" onClick={() => setConfirmOpen(true)} disabled={busy}>
            Delete
          </Button>
        </div>
      </CardFooter>

      <Dialog
        open={confirmOpen}
        onOpenChange={(nextOpen) => {
          if (!deleting) {
            setConfirmOpen(nextOpen);
          }
        }}
      >
        <DialogContent showCloseButton={!deleting} className="gap-5 overflow-hidden p-0 sm:max-w-[480px]">
          <DialogHeader className="gap-3 border-b border-white/10 px-5 pb-4 pt-5">
            <div className="flex items-start gap-3 pr-8">
              <span
                className="grid size-10 shrink-0 place-items-center rounded-xl border border-[rgba(255,99,99,0.24)] bg-[rgba(255,99,99,0.12)] text-[var(--color-primary)]"
                aria-hidden
              >
                <AlertTriangle className="size-5" />
              </span>
              <div className="min-w-0 space-y-1.5">
                <DialogTitle>Delete flow?</DialogTitle>
                <DialogDescription>
                  This permanently removes the flow and its workflow state. Recorded media files are not deleted here.
                </DialogDescription>
              </div>
            </div>
          </DialogHeader>

          <div className="space-y-4 px-5">
            <div className="rounded-xl border border-white/10 bg-white/[0.035] p-4">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <p className="truncate text-sm font-semibold text-[var(--color-text)]">{flow.name}</p>
                  <p className="mt-1 truncate text-xs text-[var(--color-text-muted)]">@{flow.account_username}</p>
                </div>
                <Badge variant="secondary" className={cn("shrink-0 text-[10px] capitalize", STATUS_CLASS[flow.status])}>
                  {flow.status}
                </Badge>
              </div>
              <div className="mt-4 grid grid-cols-3 gap-2">
                {affectedItems.map((item) => (
                  <div
                    key={item.label}
                    className="flex min-h-[68px] flex-col items-center justify-center rounded-lg border border-white/8 bg-black/15 px-2 py-2 text-center"
                  >
                    <p className="text-[11px] font-semibold uppercase tracking-[0.1em] text-[var(--color-text-muted)]">
                      {item.label}
                    </p>
                    <p className="mt-1 text-sm font-semibold text-[var(--color-text)]">{item.value}</p>
                  </div>
                ))}
              </div>
            </div>

            <p className="text-xs leading-5 text-[var(--color-text-muted)]">
              After deletion, this flow cannot be recovered from the app. Create a new flow if you need to watch the same
              account again.
            </p>
          </div>

          <DialogFooter className="mx-0 mb-0 mt-0 min-h-[88px] flex-col items-center gap-2 rounded-b-xl border-white/10 bg-white/[0.035] px-6 py-5 sm:flex-row sm:justify-end sm:gap-3">
            <Button
              type="button"
              variant="outline"
              disabled={deleting}
              onClick={() => setConfirmOpen(false)}
              className="h-11 w-full rounded-full border-white/12 bg-white/[0.045] px-5 text-sm shadow-none sm:w-auto sm:min-w-[142px]"
            >
              Keep flow
            </Button>
            <Button
              type="button"
              variant="destructive"
              disabled={deleting}
              onClick={() => void handleConfirmDelete()}
              className="h-11 w-full rounded-full border-[rgba(255,99,99,0.32)] bg-[rgba(255,99,99,0.18)] px-5 text-sm text-[#ff6b6b] shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_10px_24px_rgba(255,99,99,0.1)] sm:w-auto sm:min-w-[170px]"
            >
              {deleting ? <Loader2 className="animate-spin" /> : <Trash2 />}
              {deleting ? "Deleting..." : "Delete flow"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
