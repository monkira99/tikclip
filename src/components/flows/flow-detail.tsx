import { useCallback, useEffect, useState } from "react";
import { FlowCanvas } from "@/components/flows/canvas/flow-canvas";
import { CaptionNodeModal } from "@/components/flows/modals/caption-node-modal";
import { ClipNodeModal } from "@/components/flows/modals/clip-node-modal";
import { RecordNodeModal } from "@/components/flows/modals/record-node-modal";
import { StartNodeModal } from "@/components/flows/modals/start-node-modal";
import { UploadNodeModal } from "@/components/flows/modals/upload-node-modal";
import { FlowRuntimeLanes } from "@/components/flows/runtime/flow-runtime-lanes";
import { RuntimeLogsPanel } from "@/components/flows/runtime/runtime-logs-panel";
import { FlowRuntimeTimeline } from "@/components/flows/runtime/flow-runtime-timeline";
import { PublishFlowDialog } from "@/components/flows/runtime/publish-flow-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useFlowStore } from "@/stores/flow-store";
import type { FlowContext, FlowEditorPayload, FlowNodeKey, FlowRuntimeSnapshot } from "@/types";

type FlowDetailProps = {
  flowId: number;
  onBack: () => void;
};

export function buildRuntimeLogsPanelFlow(
  flow: FlowContext,
  runtimeSnapshot: FlowRuntimeSnapshot | null,
): FlowContext {
  if (!runtimeSnapshot) {
    return flow;
  }

  return {
    ...flow,
    status: runtimeSnapshot.status,
    current_node: runtimeSnapshot.current_node,
    last_live_at: runtimeSnapshot.last_live_at,
    last_error: runtimeSnapshot.last_error,
  };
}

export function FlowDetail({ flowId, onBack }: FlowDetailProps) {
  const activeFlow = useFlowStore((s) => s.activeFlow);
  const selectedNode = useFlowStore((s) => s.selectedNode);
  const loading = useFlowStore((s) => s.loading);
  const error = useFlowStore((s) => s.error);
  const publishPending = useFlowStore((s) => s.publishPending);
  const draftDirty = useFlowStore((s) => s.draftDirty);
  const runtimeLogs = useFlowStore((s) => s.runtimeLogs);
  const runtimeSnapshot = useFlowStore((s) => s.runtimeSnapshots[flowId] ?? null);
  const fetchFlowDetail = useFlowStore((s) => s.fetchFlowDetail);
  const fetchRuntimeLogs = useFlowStore((s) => s.fetchRuntimeLogs);
  const editorModalNode = useFlowStore((s) => s.editorModalNode);
  const openNodeModal = useFlowStore((s) => s.openNodeModal);
  const closeNodeModal = useFlowStore((s) => s.closeNodeModal);
  const saveNodeDraft = useFlowStore((s) => s.saveNodeDraft);
  const publishFlow = useFlowStore((s) => s.publishFlow);

  const [publishError, setPublishError] = useState<string | null>(null);
  const [publishOpen, setPublishOpen] = useState(false);

  useEffect(() => {
    void fetchFlowDetail(flowId);
  }, [flowId, fetchFlowDetail]);

  useEffect(() => {
    void fetchRuntimeLogs(flowId);
  }, [flowId, fetchRuntimeLogs]);

  const flow = activeFlow && activeFlow.flow.id === flowId ? activeFlow : null;
  const flowLogs = runtimeLogs[flowId] ?? [];
  const runtimePanelFlow = flow ? buildRuntimeLogsPanelFlow(flow.flow, runtimeSnapshot) : null;

  const handleCanvasSelect = (node: FlowNodeKey) => {
    openNodeModal(node);
  };

  const draftFor = useCallback(
    (key: FlowNodeKey, payload: FlowEditorPayload | null) =>
      payload?.nodes.find((n) => n.node_key === key)?.draft_config_json ?? "{}",
    [],
  );

  const handleNodeAutosave = useCallback(
    async (nodeKey: FlowNodeKey, draftJson: string) => {
      await saveNodeDraft({
        flow_id: flowId,
        node_key: nodeKey,
        draft_config_json: draftJson,
      });
    },
    [flowId, saveNodeDraft],
  );

  const dismissUnlessSwapped = useCallback(
    (nodeKey: FlowNodeKey) => (next: boolean) => {
      if (
        !next &&
        useFlowStore.getState().editorModalNode === nodeKey
      ) {
        closeNodeModal();
      }
    },
    [closeNodeModal],
  );

  const hasActiveRun = Boolean(flow?.runs?.some((r) => r.status === "running"));

  const runPublish = async (restartCurrentRun: boolean) => {
    if (!flow) {
      return;
    }
    setPublishError(null);
    try {
      await publishFlow(flow.flow.id, { restartCurrentRun });
      setPublishOpen(false);
    } catch (e) {
      setPublishError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="min-w-0 space-y-2">
          <h2 className="text-lg font-semibold text-[var(--color-text)]">
            {flow?.flow.name || `Flow #${flowId}`}
          </h2>
          <div className="flex flex-wrap items-center gap-2">
            {flow ? (
              <>
                <Badge variant="secondary" className="text-[10px] font-medium">
                  Published v{flow.flow.published_version}
                </Badge>
                <Badge variant="outline" className="text-[10px] font-medium">
                  Draft v{flow.flow.draft_version}
                </Badge>
                {draftDirty ? (
                  <Badge variant="outline" className="border-[rgba(255,188,51,0.3)] text-[10px] text-[var(--color-text-soft)]">
                    Unpublished draft changes
                  </Badge>
                ) : null}
              </>
            ) : null}
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Button variant="outline" size="sm" onClick={onBack}>
            Back
          </Button>
          <Button
            size="sm"
            variant="secondary"
            disabled={!flow || publishPending}
            onClick={() => {
              setPublishError(null);
              setPublishOpen(true);
            }}
          >
            Publish
          </Button>
        </div>
      </div>

      {loading && !flow ? (
        <p className="text-sm text-[var(--color-text-muted)]">Loading flow…</p>
      ) : null}
      {error ? (
        <div className="rounded-xl border border-[color-mix(in_oklab,var(--color-primary)_35%,transparent)] bg-[color-mix(in_oklab,var(--color-primary)_12%,transparent)] px-3 py-2">
          <p className="text-sm text-[var(--color-primary)]">{error}</p>
        </div>
      ) : null}
      {publishError ? (
        <div className="rounded-xl border border-[color-mix(in_oklab,var(--color-primary)_35%,transparent)] bg-[color-mix(in_oklab,var(--color-primary)_12%,transparent)] px-3 py-2">
          <p className="text-sm text-[var(--color-primary)]">{publishError}</p>
        </div>
      ) : null}

      <FlowCanvas
        flow={flow}
        selectedNode={selectedNode}
        runtimeSnapshot={runtimeSnapshot}
        onSelectNode={handleCanvasSelect}
      />

      <div className="grid gap-4 lg:grid-cols-2">
        <FlowRuntimeTimeline runs={flow?.runs ?? []} nodeRuns={flow?.nodeRuns ?? []} />
        <FlowRuntimeLanes nodeRuns={flow?.nodeRuns ?? []} />
      </div>

      {flow ? (
        <RuntimeLogsPanel
          flow={runtimePanelFlow ?? flow.flow}
          logs={flowLogs}
          username={runtimeSnapshot?.username ?? null}
          activeFlowRunId={runtimeSnapshot?.active_flow_run_id ?? null}
        />
      ) : null}

      <PublishFlowDialog
        open={publishOpen}
        onOpenChange={setPublishOpen}
        hasActiveRun={hasActiveRun}
        pending={publishPending}
        onPublishKeepRun={() => runPublish(false)}
        onPublishRestart={() => runPublish(true)}
      />

      {flow ? (
        <>
          <StartNodeModal
            flowId={flowId}
            rawDraft={draftFor("start", flow)}
            open={editorModalNode === "start"}
            onOpenChange={dismissUnlessSwapped("start")}
            onAutoSave={(json) => handleNodeAutosave("start", json)}
          />
          <RecordNodeModal
            flowId={flowId}
            rawDraft={draftFor("record", flow)}
            open={editorModalNode === "record"}
            onOpenChange={dismissUnlessSwapped("record")}
            onAutoSave={(json) => handleNodeAutosave("record", json)}
          />
          <ClipNodeModal
            flowId={flowId}
            rawDraft={draftFor("clip", flow)}
            open={editorModalNode === "clip"}
            onOpenChange={dismissUnlessSwapped("clip")}
            onAutoSave={(json) => handleNodeAutosave("clip", json)}
          />
          <CaptionNodeModal
            flowId={flowId}
            rawDraft={draftFor("caption", flow)}
            open={editorModalNode === "caption"}
            onOpenChange={dismissUnlessSwapped("caption")}
            onAutoSave={(json) => handleNodeAutosave("caption", json)}
          />
          <UploadNodeModal
            flowId={flowId}
            open={editorModalNode === "upload"}
            onOpenChange={dismissUnlessSwapped("upload")}
          />
        </>
      ) : null}
    </div>
  );
}
