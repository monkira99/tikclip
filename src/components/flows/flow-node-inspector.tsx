import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import type { FlowDetail, FlowNodeConfig, FlowNodeKey } from "@/types";

const NODE_LABEL: Record<FlowNodeKey, string> = {
  start: "Start",
  record: "Record",
  clip: "Clip",
  caption: "Caption",
  upload: "Upload",
};

type FlowNodeInspectorProps = {
  flow: FlowDetail | null;
  selectedNode: FlowNodeKey | null;
  saving?: boolean;
  onSaveConfig: (input: { nodeKey: FlowNodeKey; configJson: string }) => Promise<void>;
};

function getNodeStatus(flow: FlowDetail | null, node: FlowNodeKey): "idle" | "done" | "current" {
  if (!flow || !flow.flow.enabled) {
    return "idle";
  }

  const order: FlowNodeKey[] = ["start", "record", "clip", "caption", "upload"];
  const currentIndex = flow.flow.current_node ? order.indexOf(flow.flow.current_node) : -1;
  const nodeIndex = order.indexOf(node);

  if (currentIndex === nodeIndex) {
    return "current";
  }
  if (currentIndex > nodeIndex) {
    return "done";
  }
  return "idle";
}

function findNodeConfig(nodeConfigs: FlowNodeConfig[], nodeKey: FlowNodeKey): string {
  const existing = nodeConfigs.find((item) => item.node_key === nodeKey);
  return existing?.config_json ?? "{}";
}

export function FlowNodeInspector({
  flow,
  selectedNode,
  saving = false,
  onSaveConfig,
}: FlowNodeInspectorProps) {
  const initialConfig = useMemo(() => {
    if (!flow || !selectedNode) {
      return "";
    }
    return findNodeConfig(flow.node_configs, selectedNode);
  }, [flow, selectedNode]);

  const [draftConfig, setDraftConfig] = useState(initialConfig);
  const [localError, setLocalError] = useState<string | null>(null);

  useEffect(() => {
    setDraftConfig(initialConfig);
    setLocalError(null);
  }, [initialConfig]);

  if (!flow) {
    return (
      <aside className="app-panel-subtle min-h-[280px] rounded-2xl px-4 py-4">
        <p className="text-sm text-[var(--color-text-muted)]">Flow detail is not loaded.</p>
      </aside>
    );
  }

  if (!selectedNode) {
    return (
      <aside className="app-panel-subtle min-h-[280px] rounded-2xl px-4 py-4">
        <p className="text-sm text-[var(--color-text-muted)]">
          Select a node in pipeline to inspect and quick-edit its config.
        </p>
      </aside>
    );
  }

  const nodeStatus = getNodeStatus(flow, selectedNode);

  const handleSave = () => {
    setLocalError(null);
    try {
      JSON.parse(draftConfig || "{}");
    } catch {
      setLocalError("Config must be valid JSON before saving.");
      return;
    }

    void onSaveConfig({ nodeKey: selectedNode, configJson: draftConfig || "{}" }).catch((error) => {
      const message = error instanceof Error && error.message ? error.message : "Failed to save node config";
      setLocalError(message);
    });
  };

  return (
    <aside className="app-panel-subtle min-h-[280px] space-y-4 rounded-2xl px-4 py-4">
      <div className="space-y-1">
        <p className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--color-text-muted)]">
          Node inspector
        </p>
        <h3 className="text-base font-semibold text-[var(--color-text)]">{NODE_LABEL[selectedNode]}</h3>
      </div>

      <div className="rounded-xl border border-white/8 bg-white/[0.03] px-3 py-2 text-xs text-[var(--color-text-muted)]">
        Status:
        <span
          className={cn(
            "ml-1.5 font-semibold capitalize",
            nodeStatus === "current" && "text-[var(--color-success)]",
            nodeStatus === "done" && "text-[var(--color-accent)]",
            nodeStatus === "idle" && "text-[var(--color-text)]",
          )}
        >
          {nodeStatus}
        </span>
      </div>

      <div className="space-y-2">
        <Label htmlFor="flow-node-config" className="text-xs uppercase tracking-[0.08em] text-[var(--color-text-muted)]">
          Config JSON
        </Label>
        <Input
          id="flow-node-config"
          value={draftConfig}
          onChange={(event) => setDraftConfig(event.target.value)}
          placeholder='{"enabled": true}'
          disabled={saving}
        />
      </div>

      {localError ? <p className="text-xs text-[var(--color-primary)]">{localError}</p> : null}

      <Button variant="outline" size="sm" onClick={handleSave} disabled={saving}>
        {saving ? "Saving..." : "Save config"}
      </Button>
    </aside>
  );
}
