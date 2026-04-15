import type { CSSProperties } from "react";
import { FLOW_NODE_LABEL } from "@/components/flows/flow-node-utils";
import { cn } from "@/lib/utils";
import type { FlowNodeKey } from "@/types";

export type FlowCanvasNodeProps = {
  nodeKey: FlowNodeKey;
  selected: boolean;
  hasDraftChanges: boolean;
  runtimeState: string;
  summary: string;
  onClick: () => void;
  style?: CSSProperties;
};

export function FlowCanvasNode({
  nodeKey,
  selected,
  hasDraftChanges,
  runtimeState,
  summary,
  onClick,
  style,
}: FlowCanvasNodeProps) {
  const label = FLOW_NODE_LABEL[nodeKey];

  return (
    <button
      type="button"
      onClick={onClick}
      style={style}
      className={cn(
        "absolute box-border flex min-h-0 flex-col gap-1 overflow-hidden rounded-2xl border border-[var(--color-border)] px-3 py-2.5 text-left shadow-[inset_0_1px_0_rgba(255,255,255,0.04)] transition-[border-color,background-color,box-shadow]",
        "bg-white/[0.04] hover:border-[color-mix(in_oklab,var(--color-accent)_22%,var(--color-border))] hover:bg-white/[0.06]",
        selected &&
          "border-[color-mix(in_oklab,var(--color-accent)_40%,var(--color-border))] bg-[color-mix(in_oklab,var(--color-accent)_10%,transparent)] shadow-[0_0_0_1px_color-mix(in_oklab,var(--color-accent)_25%,transparent)]",
      )}
    >
      <div className="flex items-start justify-between gap-2">
        <span className="text-[11px] font-semibold uppercase tracking-[0.1em] text-[var(--color-text-muted)]">
          {label}
        </span>
        {hasDraftChanges ? (
          <span className="shrink-0 rounded-md border border-[rgba(255,188,51,0.25)] bg-[rgba(255,188,51,0.1)] px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-[var(--color-text-soft)]">
            Draft
          </span>
        ) : (
          <span className="shrink-0 rounded-md border border-white/8 bg-white/[0.03] px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-[var(--color-text-muted)]">
            Live
          </span>
        )}
      </div>
      <p className="text-[10px] font-medium capitalize leading-snug text-[var(--color-text-soft)]">
        {runtimeState}
      </p>
      <p className="line-clamp-2 font-mono text-[10px] leading-relaxed text-[var(--color-text-muted)]">
        {summary || "—"}
      </p>
    </button>
  );
}
