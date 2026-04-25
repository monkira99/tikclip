import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";

import type { FlowContext, FlowRuntimeLogEntry } from "@/types";

import { FlowRuntimeStrip } from "./flow-runtime-strip";

function createFlow(overrides: Partial<FlowContext> = {}): FlowContext {
  return {
    id: overrides.id ?? 7,
    account_id: overrides.account_id ?? 44,
    name: overrides.name ?? "Night Shift Recorder",
    enabled: overrides.enabled ?? true,
    status: overrides.status ?? "watching",
    current_node: "current_node" in overrides ? overrides.current_node ?? null : "start",
    last_live_at: "last_live_at" in overrides ? overrides.last_live_at ?? null : null,
    last_run_at: overrides.last_run_at ?? "2026-04-19T09:40:00.000+07:00",
    last_error: "last_error" in overrides ? overrides.last_error ?? null : null,
    published_version: overrides.published_version ?? 3,
    draft_version: overrides.draft_version ?? 4,
    created_at: overrides.created_at ?? "2026-04-18T22:10:00.000+07:00",
    updated_at: overrides.updated_at ?? "2026-04-19T09:45:12.000+07:00",
  };
}

function createLog(overrides: Partial<FlowRuntimeLogEntry> = {}): FlowRuntimeLogEntry {
  return {
    id: overrides.id ?? "log-1",
    timestamp: overrides.timestamp ?? "2026-04-19T10:01:02.345+07:00",
    level: overrides.level ?? "info",
    flow_id: overrides.flow_id ?? 7,
    flow_run_id: "flow_run_id" in overrides ? overrides.flow_run_id ?? null : 42,
    external_recording_id:
      "external_recording_id" in overrides ? overrides.external_recording_id ?? null : null,
    stage: overrides.stage ?? "clip",
    event: overrides.event ?? "clip.created",
    code: "code" in overrides ? overrides.code ?? null : null,
    message: overrides.message ?? "Clip generated",
    context: overrides.context ?? { clip_id: 99 },
  };
}

function renderStrip(options: {
  flow?: FlowContext;
  runtimeLogs?: FlowRuntimeLogEntry[];
  expanded?: boolean;
  activeFlowRunId?: number | null;
} = {}): string {
  return renderToStaticMarkup(
    <FlowRuntimeStrip
      flow={options.flow ?? createFlow()}
      activeFlowRunId={"activeFlowRunId" in options ? options.activeFlowRunId ?? null : 42}
      runtimeLogs={options.runtimeLogs ?? []}
      expanded={options.expanded ?? false}
      onExpandedChange={() => {}}
    />,
  );
}

test("FlowRuntimeStrip renders collapsed terminal summary by default", () => {
  const markup = renderStrip({
    flow: createFlow({
      status: "processing",
      current_node: "clip",
      last_live_at: "2026-04-19T10:01:02.345+07:00",
    }),
    runtimeLogs: [createLog()],
    expanded: false,
  });

  assert.match(markup, /Event Terminal/);
  assert.match(markup, /processing/);
  assert.match(markup, /1 logs/);
  assert.match(markup, /Creating clips/);
  assert.doesNotMatch(markup, /Open diagnostics/);
  assert.doesNotMatch(markup, /event=clip.created/);
  assert.doesNotMatch(markup, /Clip generated/);
});

test("FlowRuntimeStrip renders expanded terminal logs and runtime metadata", () => {
  const markup = renderStrip({
    flow: createFlow({
      status: "processing",
      current_node: "clip",
      last_live_at: "2026-04-19T10:01:02.345+07:00",
    }),
    runtimeLogs: [createLog()],
    expanded: true,
  });

  assert.doesNotMatch(markup, /Open diagnostics/);
  assert.match(markup, /Creating clips/);
  assert.match(markup, /Clip created/);
  assert.match(markup, /Clip #99 is ready for review/);
  assert.match(markup, /Run #42/);
  assert.doesNotMatch(markup, /Current step/);
  assert.doesNotMatch(markup, /Last live/);
  assert.doesNotMatch(markup, /account=shop_abc/);
  assert.doesNotMatch(markup, /event=clip.created/);
});

test("FlowRuntimeStrip surfaces last error in expanded runtime metadata", () => {
  const markup = renderStrip({
    flow: createFlow({
      status: "error",
      current_node: "caption",
      last_live_at: null,
      last_error: "Caption worker crashed",
    }),
    activeFlowRunId: null,
    expanded: true,
  });

  assert.match(markup, /Caption failed/);
  assert.match(markup, /Caption worker crashed/);
  assert.match(markup, /No logs/);
  assert.match(markup, /Waiting for readable activity/);
});

test("FlowRuntimeStrip respects canonical overlaid null runtime fields", () => {
  const markup = renderStrip({
    flow: createFlow({
      status: "idle",
      current_node: null,
      last_live_at: null,
      last_error: null,
    }),
    activeFlowRunId: null,
    expanded: true,
  });

  assert.match(markup, /Not running/);
  assert.doesNotMatch(markup, /Current step/);
  assert.doesNotMatch(markup, /Account/);
  assert.doesNotMatch(markup, /Last live/);
  assert.doesNotMatch(markup, /node=Waiting/);
  assert.doesNotMatch(markup, /account=unknown/);
  assert.doesNotMatch(markup, />Clip</);
  assert.doesNotMatch(markup, /2026-04-19T10:01:02.345\+07:00/);
});
