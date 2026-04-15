import type { FlowNodeKey, FlowStatus } from "@/types";

export const FLOW_NODE_ORDER: FlowNodeKey[] = ["start", "record", "clip", "caption", "upload"];

export const FLOW_NODE_LABEL: Record<FlowNodeKey, string> = {
  start: "Start",
  record: "Record",
  clip: "Clip",
  caption: "Caption",
  upload: "Upload",
};

type FlowNodeStatusSource = {
  enabled: boolean;
  current_node: FlowNodeKey | null;
  status?: FlowStatus;
} | null;

export function getFlowNodeStatus(
  flow: FlowNodeStatusSource,
  node: FlowNodeKey,
): "idle" | "done" | "current" {
  if (!flow || !flow.enabled) {
    return "idle";
  }

  const currentIndex = flow.current_node ? FLOW_NODE_ORDER.indexOf(flow.current_node) : -1;
  const nodeIndex = FLOW_NODE_ORDER.indexOf(node);

  if (currentIndex === nodeIndex) {
    return "current";
  }
  if (currentIndex > nodeIndex) {
    return "done";
  }
  return "idle";
}
