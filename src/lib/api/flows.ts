import { invoke } from "@tauri-apps/api/core";

import type {
  Clip,
  CreateFlowInput,
  FlowEditorPayload,
  FlowNodeConfig,
  FlowNodeKey,
  FlowRuntimeLogEntry,
  FlowRuntimeSnapshot,
  FlowStatus,
  FlowSummary,
  PublishFlowResult,
} from "@/types";

export type FlowRecording = {
  id: number;
  account_id: number;
  account_username: string;
  room_id: string | null;
  status: string;
  started_at: string;
  ended_at: string | null;
  duration_seconds: number;
  file_path: string | null;
  file_size_bytes: number;
  sidecar_recording_id: string | null;
  error_message: string | null;
  flow_id: number | null;
  created_at: string;
};

export type UpdateFlowInput = {
  name?: string;
  status?: FlowStatus;
  current_node?: FlowNodeKey | null;
  last_live_at?: string | null;
  last_run_at?: string | null;
  last_error?: string | null;
};

export type UpdateFlowRuntimeByAccountInput = {
  status?: FlowStatus;
  current_node?: FlowNodeKey | null;
  last_live_at?: string | null;
  last_run_at?: string | null;
  last_error?: string | null;
};

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

function optionalBlank(value: string | null | undefined): string | undefined {
  return value === undefined ? undefined : value === null ? "" : value;
}

export async function updateFlow(flowId: number, input: UpdateFlowInput): Promise<void> {
  await invoke("update_flow", {
    flowId,
    input: {
      name: input.name,
      status: input.status,
      current_node: optionalBlank(input.current_node),
      last_live_at: optionalBlank(input.last_live_at),
      last_run_at: optionalBlank(input.last_run_at),
      last_error: optionalBlank(input.last_error),
    },
  });
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

export async function updateFlowRuntimeByAccount(
  accountId: number,
  input: UpdateFlowRuntimeByAccountInput,
): Promise<void> {
  await invoke("update_flow_runtime_by_account", {
    accountId,
    input: {
      status: input.status,
      current_node: optionalBlank(input.current_node),
      last_live_at: optionalBlank(input.last_live_at),
      last_run_at: optionalBlank(input.last_run_at),
      last_error: optionalBlank(input.last_error),
    },
  });
}

export async function setFlowEnabled(flowId: number, enabled: boolean): Promise<void> {
  await invoke("set_flow_enabled", { flowId, enabled });
}

export async function saveFlowNodeConfig(input: {
  flow_id: number;
  node_key: FlowNodeKey;
  config_json: string;
}): Promise<FlowNodeConfig> {
  return invoke<FlowNodeConfig>("save_flow_node_config", { input });
}

export async function listRecordingsByFlow(flowId: number): Promise<FlowRecording[]> {
  return invoke<FlowRecording[]>("list_recordings_by_flow", { flowId });
}

export async function listClipsByFlow(flowId: number): Promise<Clip[]> {
  return invoke<Clip[]>("list_clips_by_flow", { flowId });
}
