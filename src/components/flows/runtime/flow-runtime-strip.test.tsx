import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";

import type { FlowContext } from "@/types";

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

test("FlowRuntimeStrip renders live runtime summary and diagnostics affordance", () => {
  const markup = renderToStaticMarkup(
    <FlowRuntimeStrip
      flow={createFlow({
        status: "processing",
        current_node: "clip",
        last_live_at: "2026-04-19T10:01:02.345+07:00",
      })}
      username="shop_abc"
      activeFlowRunId={42}
      runtimeLogsCount={3}
      onOpenDiagnostics={() => {}}
    />,
  );

  assert.match(markup, /Runtime Monitor/);
  assert.match(markup, /Creating clips/);
  assert.match(markup, /shop_abc/);
  assert.match(markup, /Run #42/);
  assert.match(markup, /3 logs/);
  assert.match(markup, /Open diagnostics/);
});

test("FlowRuntimeStrip surfaces last error and empty log state when runtime is degraded", () => {
  const markup = renderToStaticMarkup(
    <FlowRuntimeStrip
      flow={createFlow({
        status: "error",
        current_node: "caption",
        last_live_at: null,
        last_error: "Caption worker crashed",
      })}
      username="shop_abc"
      activeFlowRunId={null}
      runtimeLogsCount={0}
      onOpenDiagnostics={() => {}}
    />,
  );

  assert.match(markup, /Caption failed/);
  assert.match(markup, /Caption worker crashed/);
  assert.match(markup, /No logs loaded/);
  assert.match(markup, /line-clamp-1/);
});

test("FlowRuntimeStrip falls back to non-running copy without a runtime snapshot", () => {
  const markup = renderToStaticMarkup(
    <FlowRuntimeStrip
      flow={createFlow({
        status: "idle",
        current_node: null,
        last_live_at: null,
        last_error: null,
      })}
      username={null}
      activeFlowRunId={null}
      runtimeLogsCount={0}
      onOpenDiagnostics={() => {}}
    />,
  );

  assert.match(markup, /Not running/);
  assert.match(markup, /No recent live signal/);
});

test("FlowRuntimeStrip respects canonical overlaid null runtime fields without falling back to persisted values", () => {
  const markup = renderToStaticMarkup(
    <FlowRuntimeStrip
      flow={createFlow({
        status: "idle",
        current_node: null,
        last_live_at: null,
        last_error: null,
      })}
      username={null}
      activeFlowRunId={null}
      runtimeLogsCount={0}
      onOpenDiagnostics={() => {}}
    />,
  );

  assert.match(markup, /Not running/);
  assert.match(markup, /Waiting/);
  assert.match(markup, /No recent live signal/);
  assert.doesNotMatch(markup, />Clip</);
  assert.doesNotMatch(markup, /2026-04-19T10:01:02.345\+07:00/);
});
