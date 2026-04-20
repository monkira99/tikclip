import assert from "node:assert/strict";
import test from "node:test";

import {
  maybeTriggerRustRuntimeIngressRefresh,
  shouldTriggerRustLiveIngressFromSidecar,
} from "./app-shell";

test("AppShell seam disables Rust live ingress from sidecar account events", () => {
  assert.equal(shouldTriggerRustLiveIngressFromSidecar(), false);
  let refreshCalls = 0;
  maybeTriggerRustRuntimeIngressRefresh(() => {
    refreshCalls += 1;
  });
  assert.equal(refreshCalls, 0);
});
