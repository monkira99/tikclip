import { ChevronDown, Terminal } from "lucide-react";
import { useEffect, useRef } from "react";

import { FLOW_NODE_LABEL } from "@/components/flows/flow-node-utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { FlowContext, FlowNodeKey, FlowRuntimeLogEntry, FlowStatus, JsonValue } from "@/types";

type FlowRuntimeStripProps = {
  flow: FlowContext;
  activeFlowRunId: number | null;
  runtimeLogs: FlowRuntimeLogEntry[];
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
};

const STATUS_CLASS: Record<FlowStatus, string> = {
  idle: "border-white/10 bg-white/[0.05] text-[var(--color-text-muted)]",
  watching: "border-[rgba(85,179,255,0.2)] bg-[rgba(85,179,255,0.14)] text-[var(--color-accent)]",
  recording: "border-[rgba(95,201,146,0.2)] bg-[rgba(95,201,146,0.14)] text-[var(--color-success)]",
  processing: "border-[rgba(255,188,51,0.2)] bg-[rgba(255,188,51,0.14)] text-[var(--color-warning)]",
  error: "border-[rgba(255,99,99,0.22)] bg-[rgba(255,99,99,0.14)] text-[var(--color-primary)]",
  disabled: "border-white/8 bg-white/[0.03] text-[#6a6b6c]",
};

const LEVEL_CLASS: Record<FlowRuntimeLogEntry["level"], string> = {
  debug: "bg-white/25",
  info: "bg-[var(--color-accent)]",
  warn: "bg-[var(--color-warning)]",
  error: "bg-[var(--color-primary)]",
};

function formatRunLabel(activeFlowRunId: number | null | undefined): string {
  return activeFlowRunId != null ? `Run #${activeFlowRunId}` : "No active run";
}

function formatLogCountLabel(count: number): string {
  return count > 0 ? `${count} logs` : "No logs";
}

function isRecord(value: JsonValue): value is Record<string, JsonValue> {
  return value != null && typeof value === "object" && !Array.isArray(value);
}

function getContextString(context: JsonValue, key: string): string | null {
  if (!isRecord(context)) {
    return null;
  }

  const value = context[key];
  if (typeof value === "string") {
    return value.trim() || null;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }

  return null;
}

function formatRuntimeTime(timestamp: string | null | undefined): string {
  if (!timestamp) {
    return "-";
  }

  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) {
    return timestamp;
  }

  return new Intl.DateTimeFormat("vi-VN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);
}

function humanizeStage(stage: string): string {
  if (stage in FLOW_NODE_LABEL) {
    return FLOW_NODE_LABEL[stage as FlowNodeKey];
  }

  return stage
    .split(/[_-]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function humanizeEvent(event: string): string {
  return event
    .split(/[._-]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function formatLogRunLabel(flowRunId: number | null): string {
  return flowRunId != null ? `Run #${flowRunId}` : "Watcher";
}

function formatUsername(username: string | null): string {
  return username ? `@${username}` : "Unknown";
}

function deriveLogCopy(log: FlowRuntimeLogEntry): { title: string; detail: string; meta: string } {
  const lookupKey = getContextString(log.context, "lookup_key");
  const clipId = getContextString(log.context, "clip_id");
  const roomId = getContextString(log.context, "room_id");
  const stageLabel = humanizeStage(log.stage);
  const meta = `${stageLabel} · ${formatLogRunLabel(log.flow_run_id)}`;

  if (log.level === "error") {
    return {
      title: `${stageLabel} needs attention`,
      detail: log.message || "An error occurred while running this flow.",
      meta,
    };
  }

  if (log.level === "warn") {
    return {
      title: `${stageLabel} warning`,
      detail: log.message || "This step reported a warning.",
      meta,
    };
  }

  switch (log.event) {
    case "session_bootstrap_started":
      return {
        title: "Starting watcher",
        detail: "Preparing this flow to monitor the account.",
        meta,
      };
    case "lease_acquired":
      return {
        title: "Account is reserved",
        detail: `Only this flow is watching ${formatUsername(lookupKey)} right now.`,
        meta,
      };
    case "session_started":
      return {
        title: "Watcher is running",
        detail: "The flow is waiting for a live signal.",
        meta,
      };
    case "source_offline_marked":
      return {
        title: "Account is offline",
        detail: "No live signal was found. The next poll will check again.",
        meta,
      };
    case "live_detected":
      return {
        title: "Live detected",
        detail: roomId
          ? `Live room ${roomId} was found; recording will start next.`
          : "A live room was found; recording will start next.",
        meta,
      };
    case "run_creation_skipped_dedupe":
      return {
        title: "Live already handled",
        detail: "This live room was already completed, so the flow skipped creating a duplicate run.",
        meta,
      };
    case "recording_started":
    case "record_spawned":
      return {
        title: "Recording started",
        detail: "The live stream is being recorded.",
        meta,
      };
    case "recording_finished":
      return {
        title: "Recording finished",
        detail: "Recording completed and is ready for the next processing step.",
        meta,
      };
    case "clip.created":
    case "clip_ready":
      return {
        title: "Clip created",
        detail: clipId ? `Clip #${clipId} is ready for review.` : log.message || "A clip is ready for review.",
        meta,
      };
    case "caption_ready":
      return {
        title: "Caption ready",
        detail: "Caption generation completed for the clip.",
        meta,
      };
    default:
      return {
        title: humanizeEvent(log.event) || "Runtime update",
        detail: log.message || "The flow reported a runtime update.",
        meta,
      };
  }
}

function derivePrimaryRuntimeCopy(flow: FlowContext): {
  title: string;
  detail: string | null;
} {
  if (flow.status === "error") {
    return {
      title: flow.current_node ? `${FLOW_NODE_LABEL[flow.current_node]} failed` : "Runtime failed",
      detail: flow.last_error,
    };
  }

  if (flow.current_node != null) {
    return {
      title:
        flow.current_node === "start"
          ? "Watching for live"
          : flow.current_node === "record"
            ? "Recording live"
            : flow.current_node === "clip"
              ? "Creating clips"
              : flow.current_node === "caption"
                ? "Generating captions"
                : "Waiting for upload",
      detail: null,
    };
  }

  return {
    title: "Not running",
    detail: null,
  };
}

export function FlowRuntimeStrip({
  flow,
  activeFlowRunId,
  runtimeLogs,
  expanded,
  onExpandedChange,
}: FlowRuntimeStripProps) {
  const primaryCopy = derivePrimaryRuntimeCopy(flow);
  const summaryCopy = primaryCopy.detail ? `${primaryCopy.title} · ${primaryCopy.detail}` : primaryCopy.title;
  const logListRef = useRef<HTMLOListElement | null>(null);

  useEffect(() => {
    if (!expanded || runtimeLogs.length === 0) {
      return;
    }

    const frameId = window.requestAnimationFrame(() => {
      const logList = logListRef.current;
      if (logList) {
        logList.scrollTop = logList.scrollHeight;
      }
    });

    return () => window.cancelAnimationFrame(frameId);
  }, [expanded, runtimeLogs.length]);

  return (
    <section
      className={cn(
        "app-panel-subtle overflow-hidden rounded-2xl transition-[box-shadow,opacity] duration-300 ease-out",
        expanded ? "shadow-[0_18px_60px_rgba(0,0,0,0.34),inset_0_1px_0_rgba(255,255,255,0.04)]" : "",
      )}
      aria-label="Runtime event terminal"
    >
      <div
        className={cn(
          "flex items-center justify-between gap-3 border-b border-white/[0.06] px-4",
          expanded ? "min-h-14 py-3" : "min-h-16 py-2",
        )}
      >
        <button
          type="button"
          className="flex min-w-0 flex-1 items-center gap-3 text-left"
          onClick={() => onExpandedChange(!expanded)}
          aria-expanded={expanded}
        >
          <span className="flex size-8 shrink-0 items-center justify-center rounded-xl border border-white/10 bg-black/25 text-[var(--color-text-soft)] shadow-[inset_0_1px_0_rgba(255,255,255,0.06)]">
            <Terminal className="size-4" />
          </span>
          <span className="min-w-0">
            <span className="flex flex-wrap items-center gap-2">
              <span className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--color-text-muted)]">
                Event Terminal
              </span>
              <Badge variant="secondary" className={cn("text-[10px] capitalize", STATUS_CLASS[flow.status])}>
                {flow.status}
              </Badge>
              <span className="rounded-md border border-[var(--color-border)] bg-white/[0.03] px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--color-text-soft)]">
                {formatLogCountLabel(runtimeLogs.length)}
              </span>
            </span>
            <span className="mt-0.5 block truncate font-mono text-[11px] leading-4 tracking-[0.02em] text-[var(--color-text-muted)]">
              {summaryCopy} · {formatRunLabel(activeFlowRunId)}
            </span>
          </span>
        </button>

        <div className="flex shrink-0 items-center gap-2">
          <Button size="icon-xs" variant="ghost" onClick={() => onExpandedChange(!expanded)} aria-label={expanded ? "Collapse terminal" : "Expand terminal"}>
            <ChevronDown className={cn("size-3.5 transition-transform duration-300 ease-out", expanded ? "rotate-0" : "rotate-180")} />
          </Button>
        </div>
      </div>

      <div
        className={cn(
          "grid transition-[grid-template-rows,opacity] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]",
          expanded ? "grid-rows-[1fr] opacity-100" : "grid-rows-[0fr] opacity-0",
        )}
      >
        <div className="min-h-0 overflow-hidden">
          <div
            className={cn(
              "transition-transform duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]",
              expanded ? "translate-y-0" : "-translate-y-2",
            )}
          >
          {runtimeLogs.length === 0 ? (
            <div className="flex h-52 items-center px-4 text-sm font-medium text-[var(--color-text-muted)]">
              Waiting for readable activity…
            </div>
          ) : (
            <ol ref={logListRef} className="h-52 space-y-2 overflow-auto bg-[rgb(7_8_10_/0.54)] px-4 py-3">
              {runtimeLogs.map((log) => {
                const copy = deriveLogCopy(log);

                return (
                  <li
                    key={log.id}
                    className="group rounded-xl border border-white/[0.055] bg-white/[0.025] px-3 py-2.5 shadow-[inset_0_1px_0_rgba(255,255,255,0.025)] transition-colors hover:bg-white/[0.04]"
                  >
                    <div className="flex min-w-0 items-start gap-2.5">
                      <span
                        className={cn(
                          "mt-1.5 size-2 shrink-0 rounded-full shadow-[0_0_12px_currentColor]",
                          LEVEL_CLASS[log.level],
                        )}
                      />
                      <div className="min-w-0 flex-1">
                        <div className="flex min-w-0 items-center justify-between gap-3">
                          <p className="truncate text-sm font-semibold tracking-[0.01em] text-[var(--color-text)]">
                            {copy.title}
                          </p>
                          <span className="shrink-0 font-mono text-[10px] text-[var(--color-text-muted)]">
                            {formatRuntimeTime(log.timestamp)}
                          </span>
                        </div>
                        <p className="mt-0.5 text-xs font-medium leading-5 text-[var(--color-text-muted)]">
                          {copy.detail}
                        </p>
                        <p className="mt-1 truncate font-mono text-[10px] leading-4 text-[var(--color-text-muted)]">
                          {copy.meta}
                        </p>
                      </div>
                    </div>
                  </li>
                );
              })}
            </ol>
          )}
          </div>
        </div>
      </div>
    </section>
  );
}
