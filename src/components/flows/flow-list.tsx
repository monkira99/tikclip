import { useEffect, useMemo, useState } from "react";
import { FlowCard } from "@/components/flows/flow-card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useAccountStore } from "@/stores/account-store";
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
  const toggleFlowEnabled = useFlowStore((s) => s.toggleFlowEnabled);
  const filters = useFlowStore((s) => s.filters);
  const accounts = useAccountStore((s) => s.accounts);
  const fetchAccounts = useAccountStore((s) => s.fetchAccounts);

  const [busyFlowIds, setBusyFlowIds] = useState<Record<number, boolean>>({});
  const [createBusy, setCreateBusy] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [newFlowName, setNewFlowName] = useState("");
  const [newFlowAccountId, setNewFlowAccountId] = useState<string>("");

  useEffect(() => {
    void fetchFlows();
  }, [fetchFlows]);

  useEffect(() => {
    if (accounts.length === 0) {
      void fetchAccounts();
    }
  }, [accounts.length, fetchAccounts]);

  useEffect(() => {
    if (!newFlowAccountId && accounts.length > 0) {
      setNewFlowAccountId(String(accounts[0].id));
    }
  }, [accounts, newFlowAccountId]);

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

  const handleCreateFlow = () => {
    const accountId = Number(newFlowAccountId);
    const name = newFlowName.trim();
    if (!Number.isFinite(accountId) || accountId <= 0) {
      setCreateError("Please select an account");
      return;
    }
    if (!name) {
      setCreateError("Flow name is required");
      return;
    }

    setCreateBusy(true);
    setCreateError(null);
    void createFlow({
      account_id: accountId,
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
          <label className="min-w-[12rem] flex-1 text-xs text-[var(--color-text-muted)]">
            <span className="mb-1 block">Account</span>
            <select
              className="h-9 w-full rounded-lg border border-input bg-transparent px-2 text-sm text-foreground outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
              value={newFlowAccountId}
              onChange={(e) => setNewFlowAccountId(e.target.value)}
              disabled={createBusy || accounts.length === 0}
              aria-label="Select account for new flow"
            >
              {accounts.length === 0 ? <option value="">No accounts available</option> : null}
              {accounts.map((account) => (
                <option key={account.id} value={account.id}>
                  @{account.username}
                </option>
              ))}
            </select>
          </label>
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
          <Button
            type="button"
            onClick={handleCreateFlow}
            disabled={createBusy || accounts.length === 0}
          >
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
              busy={busyFlowIds[flow.id] === true}
              onOpen={onOpenFlow}
              onToggleEnabled={handleToggle}
            />
          ))}
        </div>
      )}
    </div>
  );
}
