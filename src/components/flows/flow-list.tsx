import { useEffect, useMemo, useState } from "react";
import { FlowCard } from "@/components/flows/flow-card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useFlowStore } from "@/stores/flow-store";

type FlowListProps = {
  onOpenFlow: (flowId: number) => void;
};

export function FlowList({ onOpenFlow }: FlowListProps) {
  const flows = useFlowStore((s) => s.flows);
  const loading = useFlowStore((s) => s.loading);
  const error = useFlowStore((s) => s.error);
  const fetchFlows = useFlowStore((s) => s.fetchFlows);
  const createFlow = useFlowStore((s) => s.createFlow);
  const deleteFlow = useFlowStore((s) => s.deleteFlow);
  const toggleFlowEnabled = useFlowStore((s) => s.toggleFlowEnabled);
  const filters = useFlowStore((s) => s.filters);

  const [toggleBusyCounts, setToggleBusyCounts] = useState<Record<number, number>>({});
  const [deleteBusyCounts, setDeleteBusyCounts] = useState<Record<number, number>>({});
  const [createBusy, setCreateBusy] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [newFlowName, setNewFlowName] = useState("");

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
    setToggleBusyCounts((prev) => ({ ...prev, [flowId]: (prev[flowId] ?? 0) + 1 }));
    void toggleFlowEnabled(flowId, enabled)
      .catch(() => {
        /* store already keeps user-facing error state */
      })
      .finally(() => {
        setToggleBusyCounts((prev) => {
          const next = { ...prev };
          const count = (next[flowId] ?? 1) - 1;
          if (count > 0) {
            next[flowId] = count;
          } else {
            delete next[flowId];
          }
          return next;
        });
      });
  };

  const handleDelete = async (flowId: number): Promise<boolean> => {
    setDeleteBusyCounts((prev) => ({ ...prev, [flowId]: (prev[flowId] ?? 0) + 1 }));

    try {
      await deleteFlow(flowId);
      return true;
    } catch {
      /* store already keeps user-facing error state */
      return false;
    } finally {
      setDeleteBusyCounts((prev) => {
        const next = { ...prev };
        const count = (next[flowId] ?? 1) - 1;
        if (count > 0) {
          next[flowId] = count;
        } else {
          delete next[flowId];
        }
        return next;
      });
    }
  };

  const handleCreateFlow = () => {
    const name = newFlowName.trim();
    if (!name) {
      setCreateError("Flow name is required");
      return;
    }

    setCreateBusy(true);
    setCreateError(null);
    void createFlow({
      name,
      enabled: true,
    })
      .then(() => {
        setNewFlowName("");
      })
      .catch((err: unknown) => {
        const message = err instanceof Error && err.message ? err.message : "Failed to create flow";
        setCreateError(message);
      })
      .finally(() => {
        setCreateBusy(false);
      });
  };

  if (loading && flows.length === 0) {
    return <p className="text-sm text-[var(--color-text-muted)]">Loading flows…</p>;
  }

  if (error && flows.length === 0) {
    return <p className="text-sm text-[var(--color-primary)]">{error}</p>;
  }

  return (
    <div className="space-y-3">
      <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-3">
        <div className="flex flex-wrap items-end gap-2">
          <label className="min-w-[14rem] flex-1 text-xs text-[var(--color-text-muted)]">
            <span className="mb-1 block">Flow name</span>
            <Input
              value={newFlowName}
              onChange={(e) => setNewFlowName(e.target.value)}
              placeholder="e.g. Main live automation"
              disabled={createBusy}
              aria-label="New flow name"
            />
          </label>
          <Button type="button" onClick={handleCreateFlow} disabled={createBusy}>
            {createBusy ? "Creating..." : "Create flow"}
          </Button>
        </div>
        {createError ? (
          <p className="mt-2 text-sm text-[var(--color-primary)]" role="alert">
            {createError}
          </p>
        ) : null}
      </div>
      {error ? (
        <p className="rounded-lg border border-[rgba(255,99,99,0.22)] bg-[rgba(255,99,99,0.1)] px-3 py-2 text-sm text-[var(--color-primary)]">
          {error}
        </p>
      ) : null}
      {visibleFlows.length === 0 ? (
        <p className="text-sm text-[var(--color-text-muted)]">No flows match current filters.</p>
      ) : (
        <div className="grid gap-4 xl:grid-cols-2">
          {visibleFlows.map((flow) => (
            <FlowCard
              key={flow.id}
              flow={flow}
              busy={(toggleBusyCounts[flow.id] ?? 0) > 0 || (deleteBusyCounts[flow.id] ?? 0) > 0}
              deleting={(deleteBusyCounts[flow.id] ?? 0) > 0}
              onOpen={onOpenFlow}
              onToggleEnabled={handleToggle}
              onDelete={handleDelete}
            />
          ))}
        </div>
      )}
    </div>
  );
}
