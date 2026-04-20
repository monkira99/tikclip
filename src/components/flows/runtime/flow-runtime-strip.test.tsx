import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";

import type { FlowContext, FlowRuntimeSnapshot } from "@/types";

import { FlowRuntimeStrip } from "./flow-runtime-strip";

function createFlow(overrides: Partial<FlowContext> = {}): FlowContext {
  return {
    id: overrides.id ?? 7,
    account_id: overrides.account_id ?? 44,
    name: overrides.name ?? "Night Shift Recorder",
    enabled: overrides.enabled ?? true,
    status: overrides.status ?? "watching",
    current_node: overrides.current_node ?? "start",
    last_live_at: overrides.last_live_at ?? null,
    last_run_at: overrides.last_run_at ?? "2026-04-19T09:40:00.000+07:00",
    last_error: overrides.last_error ?? null,
    published_version: overrides.published_version ?? 3,
    draft_version: overrides.draft_version ?? 4,
    created_at: overrides.created_at ?? "2026-04-18T22:10:00.000+07:00",
    updated_at: overrides.updated_at ?? "2026-04-19T09:45:12.000+07:00",
  };
}

function createRuntimeSnapshot(
  overrides: Partial<FlowRuntimeSnapshot> = {},
): FlowRuntimeSnapshot {
  return {
    flow_id: overrides.flow_id ?? 7,
    status: overrides.status ?? "processing",
    current_node: overrides.current_node ?? "clip",
    account_id: overrides.account_id ?? 44,
    username: overrides.username ?? "shop_abc",
    last_live_at: overrides.last_live_at ?? "2026-04-19T10:01:02.345+07:00",
    last_error: overrides.last_error ?? null,
    active_flow_run_id: overrides.active_flow_run_id ?? 42,
  };
}

test("FlowRuntimeStrip renders live runtime summary and diagnostics affordance", () => {
  const markup = renderToStaticMarkup(
    <FlowRuntimeStrip
      flow={createFlow({
        status: "watching",
        current_node: "start",
        last_live_at: null,
      })}
      runtimeSnapshot={createRuntimeSnapshot({
        status: "processing",
        current_node: "clip",
        username: "shop_abc",
        active_flow_run_id: 42,
      })}
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
        status: "watching",
        current_node: "start",
        last_error: "stale error",
      })}
      runtimeSnapshot={createRuntimeSnapshot({
        status: "error",
        current_node: "caption",
        last_error: "Caption worker crashed",
        active_flow_run_id: null,
      })}
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
      runtimeSnapshot={null}
      runtimeLogsCount={0}
      onOpenDiagnostics={() => {}}
    />,
  );

  assert.match(markup, /Not running/);
  assert.match(markup, /No recent live signal/);
});
