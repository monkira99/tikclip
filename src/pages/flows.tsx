import { FlowDetail } from "@/components/flows/flow-detail";
import { FlowList } from "@/components/flows/flow-list";
import { useFlowStore } from "@/stores/flow-store";

export function FlowsPage() {
  const view = useFlowStore((s) => s.view);
  const activeFlowId = useFlowStore((s) => s.activeFlowId);
  const setActiveFlowId = useFlowStore((s) => s.setActiveFlowId);
  const setView = useFlowStore((s) => s.setView);
  const setSelectedNode = useFlowStore((s) => s.setSelectedNode);

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
    return <FlowDetail flowId={activeFlowId} onBack={backToList} />;
  }

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-[var(--color-text)]">Operation flows</h2>
        <p className="mt-1 text-sm text-[var(--color-text-muted)]">
          Open a flow to edit the canvas, publish versions, and inspect recordings and clips.
        </p>
      </div>
      <FlowList onOpenFlow={openDetail} />
    </div>
  );
}
