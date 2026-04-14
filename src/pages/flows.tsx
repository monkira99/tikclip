import { useEffect } from "react";
import { FlowList } from "@/components/flows/flow-list";
import { Button } from "@/components/ui/button";
import { useFlowStore } from "@/stores/flow-store";

export function FlowsPage() {
  const view = useFlowStore((s) => s.view);
  const activeFlowId = useFlowStore((s) => s.activeFlowId);
  const activeFlow = useFlowStore((s) => s.activeFlow);
  const loading = useFlowStore((s) => s.loading);
  const error = useFlowStore((s) => s.error);
  const fetchFlowDetail = useFlowStore((s) => s.fetchFlowDetail);
  const setActiveFlowId = useFlowStore((s) => s.setActiveFlowId);
  const setView = useFlowStore((s) => s.setView);
  const setSelectedNode = useFlowStore((s) => s.setSelectedNode);

  useEffect(() => {
    if (view !== "detail" || activeFlowId == null) {
      return;
    }
    void fetchFlowDetail(activeFlowId);
  }, [view, activeFlowId, fetchFlowDetail]);

  const openDetail = (flowId: number) => {
    setSelectedNode(null);
    setActiveFlowId(flowId);
    setView("detail");
  };

  const backToList = () => {
    setView("list");
    setActiveFlowId(null);
    setSelectedNode(null);
  };

  if (view === "detail" && activeFlowId != null) {
    return (
      <div className="space-y-6">
        <div className="flex items-center justify-between gap-3">
          <div>
            <h2 className="text-lg font-semibold text-[var(--color-text)]">
              {activeFlow?.flow.name || `Flow #${activeFlowId}`}
            </h2>
            <p className="mt-1 text-sm text-[var(--color-text-muted)]">
              Detail editor shell is reserved for Task 5.
            </p>
          </div>
          <Button variant="outline" size="sm" onClick={backToList}>
            Back to flows
          </Button>
        </div>

        <div className="app-panel-subtle rounded-2xl px-5 py-6">
          {loading && !activeFlow ? (
            <p className="text-sm text-[var(--color-text-muted)]">Loading flow detail…</p>
          ) : error ? (
            <p className="text-sm text-[var(--color-primary)]">{error}</p>
          ) : (
            <p className="text-sm text-[var(--color-text-muted)]">
              Flow detail structure is connected. Node-level configuration UI comes in the next task.
            </p>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-[var(--color-text)]">Operation flows</h2>
        <p className="mt-1 text-sm text-[var(--color-text-muted)]">
          Monitor each account flow, status, and pipeline stage in one place.
        </p>
      </div>
      <FlowList onOpenFlow={openDetail} />
    </div>
  );
}
