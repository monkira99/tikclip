import { useEffect, useState } from "react";
import { FlowNodeInspector } from "@/components/flows/flow-node-inspector";
import { FlowPipeline } from "@/components/flows/flow-pipeline";
import { Button } from "@/components/ui/button";
import { useFlowStore } from "@/stores/flow-store";
import type { FlowNodeKey } from "@/types";

type FlowDetailProps = {
  flowId: number;
  onBack: () => void;
};

export function FlowDetail({ flowId, onBack }: FlowDetailProps) {
  const activeFlow = useFlowStore((s) => s.activeFlow);
  const selectedNode = useFlowStore((s) => s.selectedNode);
  const loading = useFlowStore((s) => s.loading);
  const error = useFlowStore((s) => s.error);
  const fetchFlowDetail = useFlowStore((s) => s.fetchFlowDetail);
  const setSelectedNode = useFlowStore((s) => s.setSelectedNode);
  const saveFlowConfig = useFlowStore((s) => s.saveFlowConfig);

  const [savingNodeConfig, setSavingNodeConfig] = useState(false);
  const [nodeConfigDirty, setNodeConfigDirty] = useState(false);

  useEffect(() => {
    void fetchFlowDetail(flowId);
  }, [flowId, fetchFlowDetail]);

  const flow = activeFlow && activeFlow.flow.id === flowId ? activeFlow : null;

  const handleSelectNode = (node: FlowNodeKey) => {
    if (selectedNode === node) {
      return;
    }
    if (nodeConfigDirty && !window.confirm("Discard unsaved config changes for current node?")) {
      return;
    }
    setSelectedNode(node);
  };

  const handleSaveNodeConfig = async (input: { nodeKey: FlowNodeKey; configJson: string }) => {
    if (!flow) {
      return;
    }

    setSavingNodeConfig(true);
    try {
      await saveFlowConfig({
        flow_id: flow.flow.id,
        node_key: input.nodeKey,
        config_json: input.configJson,
      });
    } finally {
      setSavingNodeConfig(false);
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold text-[var(--color-text)]">
            {flow?.flow.name || `Flow #${flowId}`}
          </h2>
          <p className="mt-1 text-sm text-[var(--color-text-muted)]">
            Manage pipeline state and quick node configuration.
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={onBack}>
          Back to flows
        </Button>
      </div>

      <FlowPipeline flow={flow?.flow ?? null} selectedNode={selectedNode} onSelectNode={handleSelectNode} />

      {loading && !flow ? (
        <p className="text-sm text-[var(--color-text-muted)]">Loading flow detail...</p>
      ) : null}
      {error ? (
        <div className="rounded-xl border border-[color-mix(in_oklab,var(--color-primary)_35%,transparent)] bg-[color-mix(in_oklab,var(--color-primary)_12%,transparent)] px-3 py-2">
          <p className="text-sm text-[var(--color-primary)]">{error}</p>
        </div>
      ) : null}

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_320px]">
        <section className="app-panel-subtle min-h-[320px] rounded-2xl px-5 py-5">
          <p className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--color-text-muted)]">
            Workspace
          </p>
          <p className="mt-2 text-sm text-[var(--color-text-muted)]">
            Task 5 shell only. Node data panels and operational controls will be added in Task 6.
          </p>
        </section>

        <FlowNodeInspector
          flow={flow}
          selectedNode={selectedNode}
          saving={savingNodeConfig}
          onSaveConfig={handleSaveNodeConfig}
          onDirtyChange={setNodeConfigDirty}
        />
      </div>
    </div>
  );
}
