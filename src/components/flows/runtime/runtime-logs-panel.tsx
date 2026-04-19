import { useEffect, useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { FlowContext, FlowRuntimeLogEntry, JsonValue } from "@/types";

type RuntimeLogsPanelProps = {
  flow: FlowContext;
  logs: FlowRuntimeLogEntry[];
  username?: string | null;
  activeFlowRunId?: number | null;
};

function formatContext(context: JsonValue): string {
  return JSON.stringify(context);
}

function formatLogLevel(level: FlowRuntimeLogEntry["level"]): string {
  return level.toUpperCase();
}

export function buildLogHeaderLine(log: FlowRuntimeLogEntry): string {
  return [
    `[${log.timestamp}]`,
    formatLogLevel(log.level),
    `flow=${log.flow_id}`,
    `run=${log.flow_run_id ?? "-"}`,
    `stage=${log.stage}`,
    `event=${log.event}`,
    `code=${log.code ?? "-"}`,
    ...(log.external_recording_id ? [`recording=${log.external_recording_id}`] : []),
  ].join(" ");
}

export function buildDiagnosticBundle(
  flow: FlowContext,
  logs: FlowRuntimeLogEntry[],
  options?: {
    username?: string | null;
    active_flow_run_id?: number | null;
  },
): string {
  const recentLogs = logs.map((log) => {
    const lines = [
      `- header: ${buildLogHeaderLine(log)}`,
      `  message: ${log.message}`,
      `  context: ${formatContext(log.context)}`,
    ];

    if (log.flow_run_id != null) {
      lines.push(`  flow_run_id: ${log.flow_run_id}`);
    }
    if (log.external_recording_id) {
      lines.push(`  external_recording_id: ${log.external_recording_id}`);
    }
    if (log.code) {
      lines.push(`  code: ${log.code}`);
    }

    return lines.join("\n");
  });

  return [
    `flow_id: ${flow.id}`,
    `flow_name: ${flow.name}`,
    `current_status: ${flow.status}`,
    `current_node: ${flow.current_node ?? "-"}`,
    `username: ${options?.username ?? "-"}`,
    `active_flow_run_id: ${options?.active_flow_run_id ?? "-"}`,
    `last_live_at: ${flow.last_live_at ?? "-"}`,
    `last_error: ${flow.last_error ?? "-"}`,
    "recent_logs:",
    ...(recentLogs.length > 0 ? recentLogs : ["- none"]),
  ].join("\n");
}

export function RuntimeLogsPanel({ flow, logs, username, activeFlowRunId }: RuntimeLogsPanelProps) {
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle");
  const diagnosticBundle = useMemo(
    () =>
      buildDiagnosticBundle(flow, logs, {
        username,
        active_flow_run_id: activeFlowRunId,
      }),
    [activeFlowRunId, flow, logs, username],
  );

  useEffect(() => {
    if (copyState === "idle") {
      return;
    }

    const timer = window.setTimeout(() => {
      setCopyState("idle");
    }, 1800);

    return () => window.clearTimeout(timer);
  }, [copyState]);

  const copyDiagnosticBundle = async () => {
    try {
      await navigator.clipboard.writeText(diagnosticBundle);
      setCopyState("copied");
    } catch {
      setCopyState("failed");
    }
  };

  return (
    <section className="app-panel-subtle rounded-2xl px-4 py-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--color-text-muted)]">
            Runtime Logs
          </p>
          <p className="mt-1 text-xs leading-relaxed text-[var(--color-text-muted)]">
            Recent Rust runtime entries and a support-ready diagnostic bundle for this flow.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <span className="rounded-md border border-[var(--color-border)] bg-white/[0.03] px-2 py-1 text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--color-text-soft)]">
            {logs.length} recent
          </span>
          <Button size="sm" variant="outline" onClick={() => void copyDiagnosticBundle()}>
            {copyState === "copied"
              ? "Copied"
              : copyState === "failed"
                ? "Copy failed"
                : "Copy diagnostic bundle"}
          </Button>
        </div>
      </div>

      <div className="mt-4 grid gap-4 xl:grid-cols-[minmax(0,1.35fr)_minmax(18rem,0.95fr)]">
        <div>
          {logs.length === 0 ? (
            <p className="rounded-xl border border-[var(--color-border)] bg-white/[0.02] px-3 py-3 text-sm text-[var(--color-text-muted)]">
              No runtime logs yet.
            </p>
          ) : (
            <ol className="space-y-2">
              {logs.map((log) => (
                <li
                  key={log.id}
                  className={cn(
                    "rounded-xl border px-3 py-3",
                    log.level === "error"
                      ? "border-[rgba(255,99,99,0.18)] bg-[rgba(255,99,99,0.05)]"
                      : log.level === "warn"
                        ? "border-[rgba(255,188,51,0.18)] bg-[rgba(255,188,51,0.04)]"
                        : "border-[var(--color-border)] bg-white/[0.02]",
                  )}
                >
                  <div className="flex flex-wrap items-baseline justify-between gap-2">
                    <p className="text-xs font-semibold tracking-[0.02em] text-[var(--color-text)]">
                      {buildLogHeaderLine(log)}
                    </p>
                    <span className="text-[10px] text-[var(--color-text-muted)]">
                      {formatLogLevel(log.level)}
                    </span>
                  </div>
                  <p className="mt-2 text-sm leading-relaxed text-[var(--color-text-soft)]">{log.message}</p>
                  <pre className="mt-2 overflow-x-auto rounded-lg border border-[var(--color-border)] bg-[rgb(7_8_10_/0.72)] px-3 py-2 font-mono text-[11px] leading-5 tracking-[0.02em] whitespace-pre-wrap text-[var(--color-text-muted)]">
                    {formatContext(log.context)}
                  </pre>
                </li>
              ))}
            </ol>
          )}
        </div>

        <div className="rounded-xl border border-[var(--color-border)] bg-[rgb(7_8_10_/0.72)] p-3">
          <div className="flex items-center justify-between gap-2">
            <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--color-text-muted)]">
              Diagnostic bundle
            </p>
            <span className="text-[10px] text-[var(--color-text-muted)]">Summary + recent logs</span>
          </div>
          <pre className="mt-3 max-h-96 overflow-auto rounded-lg border border-[var(--color-border)] bg-black/20 px-3 py-2 font-mono text-[11px] leading-5 tracking-[0.02em] whitespace-pre-wrap text-[var(--color-text-soft)]">
            {diagnosticBundle}
          </pre>
        </div>
      </div>
    </section>
  );
}
