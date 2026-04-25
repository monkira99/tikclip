import { invoke } from "@tauri-apps/api/core";

import type {
  CreateFlowInput,
  FlowEditorPayload,
  FlowNodeKey,
  FlowRuntimeLogEntry,
  FlowRuntimeSnapshot,
  FlowSummary,
  PublishFlowResult,
} from "@/types";

export async function listFlows(): Promise<FlowSummary[]> {
  return invoke<FlowSummary[]>("list_flows");
}

export async function getFlowDefinition(flowId: number): Promise<FlowEditorPayload> {
  return invoke<FlowEditorPayload>("get_flow_definition", { flowId });
}

export async function listLiveRuntimeSessions(): Promise<FlowRuntimeSnapshot[]> {
  return invoke<FlowRuntimeSnapshot[]>("list_live_runtime_sessions");
}

export async function listLiveRuntimeLogs(
  flowId?: number,
  limit?: number,
): Promise<FlowRuntimeLogEntry[]> {
  return invoke<FlowRuntimeLogEntry[]>("list_live_runtime_logs", { flowId, limit });
}

export async function createFlow(input: CreateFlowInput): Promise<number> {
  return invoke<number>("create_flow", { input });
}

export async function deleteFlow(flowId: number): Promise<void> {
  await invoke("delete_flow", { flowId });
}

export async function saveFlowNodeDraft(input: {
  flow_id: number;
  node_key: FlowNodeKey;
  draft_config_json: string;
}): Promise<void> {
  await invoke("save_flow_node_draft", {
    flowId: input.flow_id,
    nodeKey: input.node_key,
    draftConfigJson: input.draft_config_json,
  });
}

export async function publishFlowDefinition(flowId: number): Promise<PublishFlowResult> {
  return invoke<PublishFlowResult>("publish_flow_definition", { flowId });
}

export type RestartFlowRunResult = {
  flowId: number;
  restarted: boolean;
};

export async function restartFlowRun(flowId: number): Promise<RestartFlowRunResult> {
  return invoke<RestartFlowRunResult>("restart_flow_run", { flowId });
}

export type SidecarFlowRuntimeHint =
  | "clip_ready"
  | "caption_ready";

/** Rust maps `hint` to `flows` / Start-node telemetry (no transition logic in JS). */
export async function applySidecarFlowRuntimeHint(input: {
  account_id: number;
  hint: SidecarFlowRuntimeHint;
  /** Desktop SQLite `clips.id` for `clip_ready` / `caption_ready` pipeline node runs. */
  clip_id?: number | null;
}): Promise<void> {
  await invoke("apply_sidecar_flow_runtime_hint", {
    input: {
      account_id: input.account_id,
      hint: input.hint,
      clip_id:
        input.clip_id === undefined || input.clip_id === null ? undefined : input.clip_id,
    },
  });
}

export async function setFlowEnabled(flowId: number, enabled: boolean): Promise<void> {
  await invoke("set_flow_enabled", { flowId, enabled });
}
