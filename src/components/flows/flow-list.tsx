import { useEffect, useMemo, useState } from "react";
import { FlowCard } from "@/components/flows/flow-card";
import { useFlowStore } from "@/stores/flow-store";

type FlowListProps = {
  onOpenFlow: (flowId: number) => void;
};

export function FlowList({ onOpenFlow }: FlowListProps) {
  const flows = useFlowStore((s) => s.flows);
  const loading = useFlowStore((s) => s.loading);
  const error = useFlowStore((s) => s.error);
  const fetchFlows = useFlowStore((s) => s.fetchFlows);
  const toggleFlowEnabled = useFlowStore((s) => s.toggleFlowEnabled);
  const filters = useFlowStore((s) => s.filters);

  const [busyFlowIds, setBusyFlowIds] = useState<Record<number, boolean>>({});

  useEffect(() => {
    void fetchFlows();
  }, [fetchFlows]);

  const visibleFlows = useMemo(() => {
    const search = filters.search.trim().toLowerCase();
    return flows.filter((flow) => {
      if (filters.status !== "all" && flow.status !== filters.status) {
        return false;
      }
      if (!search) {
        return true;
      }
      return (
        flow.name.toLowerCase().includes(search) ||
        flow.account_username.toLowerCase().includes(search) ||
        flow.status.toLowerCase().includes(search)
      );
    });
  }, [flows, filters.search, filters.status]);

  const handleToggle = (flowId: number, enabled: boolean) => {
    setBusyFlowIds((prev) => ({ ...prev, [flowId]: true }));
    void toggleFlowEnabled(flowId, enabled)
      .catch(() => {
        /* store already keeps user-facing error state */
      })
      .finally(() => {
        setBusyFlowIds((prev) => {
          const next = { ...prev };
          delete next[flowId];
          return next;
        });
      });
  };

  if (loading && flows.length === 0) {
    return <p className="text-sm text-[var(--color-text-muted)]">Loading flows…</p>;
  }

  if (error && flows.length === 0) {
    return <p className="text-sm text-[var(--color-primary)]">{error}</p>;
  }

  if (visibleFlows.length === 0) {
    return <p className="text-sm text-[var(--color-text-muted)]">No flows match current filters.</p>;
  }

  return (
    <div className="space-y-3">
      {error ? (
        <p className="rounded-lg border border-[rgba(255,99,99,0.22)] bg-[rgba(255,99,99,0.1)] px-3 py-2 text-sm text-[var(--color-primary)]">
          {error}
        </p>
      ) : null}
      <div className="grid gap-4 xl:grid-cols-2">
        {visibleFlows.map((flow) => (
          <FlowCard
            key={flow.id}
            flow={flow}
            busy={busyFlowIds[flow.id] === true}
            onOpen={onOpenFlow}
            onToggleEnabled={handleToggle}
          />
        ))}
      </div>
    </div>
  );
}
