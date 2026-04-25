import type { CSSProperties } from "react";
import type { CanvasNodeVisualState } from "@/components/flows/canvas/flow-canvas-runtime-state";
import { FLOW_NODE_LABEL } from "@/components/flows/flow-node-utils";
import { cn } from "@/lib/utils";
import type { FlowNodeKey } from "@/types";

export type FlowCanvasNodeDetail = {
  label: string;
  value: string;
  tone?: "default" | "accent" | "success" | "muted";
};

export type FlowCanvasNodeProps = {
  nodeKey: FlowNodeKey;
  selected: boolean;
  hasDraftChanges: boolean;
  runtimeState: string;
  summary: string;
  visualState: CanvasNodeVisualState;
  badgeLabel: "Running" | "Done" | "Error" | null;
  inlineDetail: string | null;
  details?: FlowCanvasNodeDetail[];
  activeMarker: boolean;
  onClick: () => void;
  style?: CSSProperties;
};

const DETAIL_VALUE_CLASS: Record<NonNullable<FlowCanvasNodeDetail["tone"]>, string> = {
  default: "text-[var(--color-text-soft)]",
  accent: "text-[var(--color-accent)]",
  success: "text-[var(--color-success)]",
  muted: "text-[var(--color-text-muted)]",
};

export function FlowCanvasNode({
  nodeKey,
  selected,
  hasDraftChanges,
  runtimeState,
  summary,
  visualState,
  badgeLabel,
  inlineDetail,
  details = [],
  activeMarker,
  onClick,
  style,
}: FlowCanvasNodeProps) {
  const label = FLOW_NODE_LABEL[nodeKey];

  return (
    <button
      type="button"
      onClick={onClick}
      style={style}
      aria-pressed={selected}
      data-runtime-state={visualState}
      data-active-marker={activeMarker ? "true" : "false"}
      className={cn(
        "absolute box-border flex min-h-0 flex-col gap-2 overflow-hidden rounded-2xl border px-4 py-3.5 text-left shadow-[inset_0_1px_0_rgba(255,255,255,0.04)] transition-[border-color,background-color,box-shadow]",
        visualState === "running" &&
          "border-[rgba(255,99,99,0.72)] bg-[rgba(255,99,99,0.08)] shadow-[0_0_0_1px_rgba(255,99,99,0.22)]",
        visualState === "done" && "border-[rgba(95,201,146,0.35)] bg-[rgba(95,201,146,0.06)]",
        visualState === "error" &&
          "border-[rgba(255,99,99,0.58)] bg-[rgba(255,99,99,0.07)] shadow-[0_0_0_1px_rgba(255,99,99,0.12)]",
        visualState === "idle" &&
          "border-[var(--color-border)] bg-white/[0.04] hover:border-[color-mix(in_oklab,var(--color-accent)_22%,var(--color-border))] hover:bg-white/[0.06]",
        selected && "ring-1 ring-[color-mix(in_oklab,var(--color-accent)_30%,transparent)]",
      )}
    >
      {visualState === "running" ? (
        <span
          aria-hidden
          className="runtime-pulse runtime-pulse-glow pointer-events-none absolute inset-[-8px] rounded-[inherit] border border-[rgba(255,99,99,0.24)] bg-[radial-gradient(circle_at_center,rgba(255,99,99,0.18),rgba(255,99,99,0.06)_48%,transparent_72%)]"
        />
      ) : null}
      <div className="flex items-start justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <span
            aria-hidden
            className={cn(
              "size-2 shrink-0 rounded-full border border-white/10 bg-white/12",
              activeMarker && "bg-[var(--color-primary)] shadow-[0_0_0_4px_rgba(255,99,99,0.12)]",
            )}
          />
          <span className="truncate text-[11px] font-semibold uppercase tracking-[0.1em] text-[var(--color-text-muted)]">
            {label}
          </span>
        </div>
        {badgeLabel ? (
          <span className="shrink-0 rounded-md border border-white/10 bg-white/[0.04] px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-[var(--color-text-soft)]">
            {badgeLabel}
          </span>
        ) : hasDraftChanges ? (
          <span className="shrink-0 rounded-md border border-[rgba(255,188,51,0.25)] bg-[rgba(255,188,51,0.1)] px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-[var(--color-text-soft)]">
            Draft
          </span>
        ) : (
          null
        )}
      </div>
      <p className="line-clamp-1 text-[10px] font-medium leading-snug text-[var(--color-text-soft)]">
        {runtimeState}
      </p>
      <p className="line-clamp-5 font-mono text-[10px] leading-relaxed text-[var(--color-text-muted)]">
        {summary || "—"}
      </p>
      {details.length > 0 ? (
        <div className="mt-3 grid grid-cols-[repeat(2,minmax(0,1fr))] gap-2 border-t border-white/[0.06] pb-3 pt-3">
          {details.map((detail) => (
            <div
              key={detail.label}
              className="flex h-[50px] min-w-0 flex-col items-center justify-center rounded-lg border border-white/[0.06] bg-black/15 px-2.5 py-1.5 text-center"
            >
              <p className="w-full truncate text-[8px] font-semibold leading-none uppercase tracking-[0.12em] text-[var(--color-text-muted)]">
                {detail.label}
              </p>
              <p className={cn("mt-2 w-full truncate text-[11px] font-semibold leading-none tabular-nums", DETAIL_VALUE_CLASS[detail.tone ?? "default"])}>
                {detail.value}
              </p>
            </div>
          ))}
        </div>
      ) : null}
      {inlineDetail ? (
        <p className="line-clamp-1 text-[10px] leading-snug text-[var(--color-primary)]">{inlineDetail}</p>
      ) : null}
    </button>
  );
}
