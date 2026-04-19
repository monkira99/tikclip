import test from "node:test";
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";

import type { FlowContext, FlowRuntimeLogEntry } from "@/types";

import {
  RuntimeLogsPanel,
  buildDiagnosticBundle,
  buildLogHeaderLine,
} from "./runtime-logs-panel";

function createFlow(overrides: Partial<FlowContext> = {}): FlowContext {
  return {
    id: overrides.id ?? 7,
    account_id: overrides.account_id ?? 44,
    name: overrides.name ?? "Night Shift Recorder",
    enabled: overrides.enabled ?? true,
    status: overrides.status ?? "recording",
    current_node: overrides.current_node ?? "record",
    last_live_at: overrides.last_live_at ?? "2026-04-19T09:45:00.000+07:00",
    last_run_at: overrides.last_run_at ?? "2026-04-19T09:40:00.000+07:00",
    last_error: overrides.last_error ?? null,
    published_version: overrides.published_version ?? 3,
    draft_version: overrides.draft_version ?? 4,
    created_at: overrides.created_at ?? "2026-04-18T22:10:00.000+07:00",
    updated_at: overrides.updated_at ?? "2026-04-19T09:45:12.000+07:00",
  };
}

function createLog(overrides: Partial<FlowRuntimeLogEntry> & Pick<FlowRuntimeLogEntry, "id">): FlowRuntimeLogEntry {
  return {
    id: overrides.id,
    timestamp: overrides.timestamp ?? "2026-04-19T09:41:12.381+07:00",
    level: overrides.level ?? "info",
    flow_id: overrides.flow_id ?? 7,
    flow_run_id: overrides.flow_run_id ?? 42,
    external_recording_id:
      overrides.external_recording_id === undefined ? null : overrides.external_recording_id,
    stage: overrides.stage ?? "record",
    event: overrides.event ?? "record_spawned",
    code: overrides.code === undefined ? null : overrides.code,
    message: overrides.message ?? "Spawned Rust-owned recording worker",
    context: overrides.context === undefined ? { room_id: "7312345" } : overrides.context,
  };
}

test("buildDiagnosticBundle includes summary fields and recent logs", () => {
  const flow = createFlow({ last_error: "Sidecar timeout" });
  const logs = [
    createLog({ id: "log-1" }),
    createLog({
      id: "log-2",
      timestamp: "2026-04-19T09:42:16.381+07:00",
      level: "error",
      event: "handoff_failed",
      code: "handoff.sidecar_unavailable",
      message: "Sidecar handoff failed",
      context: { reason: "port_missing", retryable: false },
    }),
  ];

  const bundle = buildDiagnosticBundle(flow, logs);
  const firstHeader = buildLogHeaderLine(logs[0]);
  const secondHeader = buildLogHeaderLine(logs[1]);

  assert.match(bundle, /^flow_id: 7/m);
  assert.match(bundle, /^flow_name: Night Shift Recorder/m);
  assert.match(bundle, /^current_status: recording/m);
  assert.match(bundle, /^current_node: record/m);
  assert.match(bundle, /^last_live_at: 2026-04-19T09:45:00.000\+07:00/m);
  assert.match(bundle, /^last_error: Sidecar timeout/m);
  assert.match(bundle, /recent_logs:/m);
  assert.match(bundle, new RegExp(`header: ${firstHeader.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`));
  assert.match(bundle, new RegExp(`header: ${secondHeader.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`));
  assert.match(bundle, /context: \{"reason":"port_missing","retryable":false\}/m);
});

test("buildDiagnosticBundle includes runtime correlation fields for support handoff", () => {
  const bundle = buildDiagnosticBundle(
    createFlow(),
    [createLog({ id: "log-1" })],
    {
      username: "shop_abc",
      active_flow_run_id: 42,
    },
  );

  assert.match(bundle, /^username: shop_abc/m);
  assert.match(bundle, /^active_flow_run_id: 42/m);
});

test("buildLogHeaderLine preserves canonical compact debug fields", () => {
  const line = buildLogHeaderLine(
    createLog({
      id: "log-9",
      timestamp: "2026-04-19T09:42:16.381+07:00",
      level: "error",
      flow_id: 7,
      flow_run_id: 42,
      stage: "record",
      event: "handoff_failed",
      code: "handoff.sidecar_unavailable",
      external_recording_id: "rec-42-a1b2",
      message: "Sidecar handoff failed",
    }),
  );

  assert.equal(
    line,
    "[2026-04-19T09:42:16.381+07:00] ERROR flow=7 run=42 stage=record event=handoff_failed code=handoff.sidecar_unavailable recording=rec-42-a1b2",
  );
});

test("RuntimeLogsPanel renders summary, message, and context for runtime logs", () => {
  const markup = renderToStaticMarkup(
    <RuntimeLogsPanel
      flow={createFlow()}
      logs={[
        createLog({
          id: "log-1",
          timestamp: "2026-04-19T09:41:12.381+07:00",
          message: "Spawned Rust-owned recording worker",
          context: { room_id: "7312345", poll_attempt: 2 },
        }),
      ]}
    />,
  );

  assert.match(markup, /Runtime Logs/);
  assert.match(markup, /Copy diagnostic bundle/);
  assert.match(
    markup,
    /\[2026-04-19T09:41:12.381\+07:00\] INFO flow=7 run=42 stage=record event=record_spawned code=-/,
  );
  assert.match(markup, /Spawned Rust-owned recording worker/);
  assert.match(markup, /\{&quot;room_id&quot;:&quot;7312345&quot;,&quot;poll_attempt&quot;:2\}/);
});

test("RuntimeLogsPanel shows empty state when no logs are available", () => {
  const markup = renderToStaticMarkup(
    <RuntimeLogsPanel flow={createFlow()} logs={[]} />,
  );

  assert.match(markup, /No runtime logs yet\./);
});

test("RuntimeLogsPanel renders runtime correlation fields in the diagnostic bundle", () => {
  const markup = renderToStaticMarkup(
    <RuntimeLogsPanel
      flow={createFlow()}
      logs={[createLog({ id: "log-1" })]}
      username="shop_abc"
      activeFlowRunId={42}
    />,
  );

  assert.match(markup, /username: shop_abc/);
  assert.match(markup, /active_flow_run_id: 42/);
});

test("RuntimeLogsPanel bundle reflects the explicitly provided current runtime view", () => {
  const markup = renderToStaticMarkup(
    <RuntimeLogsPanel
      flow={createFlow({
        status: "processing",
        current_node: "clip",
        last_live_at: "2026-04-19T10:01:02.345+07:00",
        last_error: null,
      })}
      logs={[createLog({ id: "log-1" })]}
      username="shop_abc"
      activeFlowRunId={42}
    />,
  );

  assert.match(markup, /current_status: processing/);
  assert.match(markup, /current_node: clip/);
  assert.match(markup, /last_live_at: 2026-04-19T10:01:02.345\+07:00/);
  assert.match(markup, /last_error: -/);
  assert.match(markup, /username: shop_abc/);
  assert.match(markup, /active_flow_run_id: 42/);
});
