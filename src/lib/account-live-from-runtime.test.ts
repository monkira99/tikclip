import test from "node:test";
import assert from "node:assert/strict";

import type { FlowRuntimeSnapshot } from "@/types";

import { deriveAccountLiveFlagsFromRuntime } from "./account-live-from-runtime";

function createSnapshot(
  overrides: Partial<FlowRuntimeSnapshot> & Pick<FlowRuntimeSnapshot, "flow_id" | "account_id">,
): FlowRuntimeSnapshot {
  return {
    flow_id: overrides.flow_id,
    status: overrides.status ?? "watching",
    current_node:
      "current_node" in overrides ? overrides.current_node ?? null : "start",
    account_id: overrides.account_id,
    username: overrides.username ?? "shop_abc",
    last_live_at:
      "last_live_at" in overrides ? overrides.last_live_at ?? null : null,
    last_error:
      "last_error" in overrides ? overrides.last_error ?? null : null,
    active_flow_run_id:
      "active_flow_run_id" in overrides ? overrides.active_flow_run_id ?? null : null,
  };
}

test("deriveAccountLiveFlagsFromRuntime marks recording flows as live", () => {
  const rows = deriveAccountLiveFlagsFromRuntime([
    createSnapshot({ flow_id: 7, account_id: 44, status: "recording", current_node: "record" }),
  ]);

  assert.deepEqual(rows, [{ id: 44, isLive: true }]);
});

test("deriveAccountLiveFlagsFromRuntime marks watching flows as live only when a current live signal is present", () => {
  const rows = deriveAccountLiveFlagsFromRuntime([
    createSnapshot({
      flow_id: 7,
      account_id: 44,
      status: "watching",
      current_node: "start",
      last_live_at: "2026-04-20T18:00:00.000+07:00",
    }),
    createSnapshot({
      flow_id: 8,
      account_id: 45,
      status: "watching",
      current_node: "start",
      last_live_at: null,
    }),
  ]);

  assert.deepEqual(rows, [
    { id: 44, isLive: true },
    { id: 45, isLive: false },
  ]);
});

test("deriveAccountLiveFlagsFromRuntime does not treat processing as live", () => {
  const rows = deriveAccountLiveFlagsFromRuntime([
    createSnapshot({
      flow_id: 7,
      account_id: 44,
      status: "processing",
      current_node: "clip",
      last_live_at: "2026-04-20T18:00:00.000+07:00",
    }),
  ]);

  assert.deepEqual(rows, [{ id: 44, isLive: false }]);
});

test("deriveAccountLiveFlagsFromRuntime aggregates multiple flows for one account with OR semantics", () => {
  const rows = deriveAccountLiveFlagsFromRuntime([
    createSnapshot({ flow_id: 7, account_id: 44, status: "processing", current_node: "clip" }),
    createSnapshot({ flow_id: 8, account_id: 44, status: "recording", current_node: "record" }),
  ]);

  assert.deepEqual(rows, [{ id: 44, isLive: true }]);
});

test("deriveAccountLiveFlagsFromRuntime ignores snapshots without account_id", () => {
  const rows = deriveAccountLiveFlagsFromRuntime([
    createSnapshot({ flow_id: 7, account_id: null, status: "recording" }),
  ]);

  assert.deepEqual(rows, []);
});
