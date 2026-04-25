import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";

import type { FlowEditorPayload, FlowNodeDefinition, FlowNodeRunRow } from "@/types";

import { FlowCanvas } from "./flow-canvas";

const nodeKeys = ["start", "record", "clip", "caption", "upload"] as const;

function createNodeDefinition(node_key: (typeof nodeKeys)[number], position: number): FlowNodeDefinition {
  return {
    id: position,
    flow_id: 7,
    node_key,
    position,
    draft_config_json: "{}",
    published_config_json: "{}",
    draft_updated_at: "2026-04-20T12:00:00.000+07:00",
    published_at: "2026-04-20T12:00:00.000+07:00",
  };
}

function createNodeRun(overrides: Partial<FlowNodeRunRow> & Pick<FlowNodeRunRow, "id" | "node_key">): FlowNodeRunRow {
  return {
    id: overrides.id,
    flow_run_id: overrides.flow_run_id ?? 42,
    flow_id: overrides.flow_id ?? 7,
    node_key: overrides.node_key,
    status: overrides.status ?? "completed",
    started_at: overrides.started_at ?? "2026-04-20T12:01:00.000+07:00",
    ended_at: overrides.ended_at ?? "2026-04-20T12:01:10.000+07:00",
    input_json: overrides.input_json ?? null,
    output_json: overrides.output_json ?? null,
    error: overrides.error ?? null,
  };
}

function createFlowPayload(): FlowEditorPayload {
  return {
    flow: {
      id: 7,
      account_id: 44,
      name: "Night Shift Recorder",
      enabled: true,
      status: "processing",
      current_node: "caption",
      last_live_at: "2026-04-20T12:00:00.000+07:00",
      last_run_at: "2026-04-20T12:04:00.000+07:00",
      last_error: null,
      published_version: 1,
      draft_version: 1,
      created_at: "2026-04-20T11:00:00.000+07:00",
      updated_at: "2026-04-20T12:04:00.000+07:00",
    },
    nodes: nodeKeys.map((nodeKey, index) => createNodeDefinition(nodeKey, index + 1)),
    runs: [
      {
        id: 42,
        flow_id: 7,
        definition_version: 1,
        status: "running",
        started_at: "2026-04-20T12:00:00.000+07:00",
        ended_at: null,
        trigger_reason: "start_live_detected",
        error: null,
      },
    ],
    nodeRuns: [
      createNodeRun({
        id: 10,
        node_key: "record",
        started_at: "2026-04-20T12:01:00.000+07:00",
        ended_at: "2026-04-20T12:03:00.000+07:00",
      }),
      createNodeRun({
        id: 11,
        node_key: "clip",
        started_at: "2026-04-20T12:03:10.000+07:00",
        ended_at: "2026-04-20T12:03:30.000+07:00",
        output_json: JSON.stringify({ clip_id: 101 }),
      }),
      createNodeRun({
        id: 12,
        node_key: "clip",
        started_at: "2026-04-20T12:03:31.000+07:00",
        ended_at: "2026-04-20T12:03:50.000+07:00",
        output_json: JSON.stringify({ clip_id: 102 }),
      }),
    ],
    recordings_count: 1,
    clips_count: 2,
  };
}

test("FlowCanvas keeps record and clip status detail cells visible after those nodes complete", () => {
  const markup = renderToStaticMarkup(
    <FlowCanvas
      flow={createFlowPayload()}
      selectedNode={null}
      runtimeSnapshot={{
        flow_id: 7,
        status: "processing",
        current_node: "caption",
        account_id: 44,
        username: "shop_abc",
        last_live_at: "2026-04-20T12:00:00.000+07:00",
        last_error: null,
        active_flow_run_id: 42,
      }}
      onSelectNode={() => {}}
    />,
  );

  assert.match(markup, /Record complete/);
  assert.match(markup, /Duration/);
  assert.match(markup, /Clip complete/);
  assert.match(markup, /Clips/);
  assert.match(markup, /#102/);
});
