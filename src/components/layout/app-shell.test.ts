import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const sourceDir = path.dirname(fileURLToPath(import.meta.url));

const appShellSource = fs.readFileSync(
  path.resolve(sourceDir, "app-shell.tsx"),
  "utf8",
);

const appShellEffectsSource = fs.readFileSync(
  path.resolve(sourceDir, "app-shell-effects.ts"),
  "utf8",
);

const appShellRuntimeSource = `${appShellSource}\n${appShellEffectsSource}`;

test("AppShell no longer contains the sidecar account-live polling path", () => {
  assert.equal(appShellSource.includes("syncWatcherForAccounts"), false);
  assert.equal(appShellSource.includes("pollNow"), false);
  assert.equal(appShellSource.includes("live-overview"), false);
  assert.equal(appShellSource.includes("syncLiveFromSidecarHttp"), false);
  assert.equal(appShellSource.includes("LIVE_HTTP_SYNC_MS"), false);
});

test("AppShell drives account live flags from runtime-owned batch updates", () => {
  assert.equal(appShellRuntimeSource.includes("applyLiveFlagsFromRuntime"), true);
  assert.equal(appShellRuntimeSource.includes("deriveAccountLiveFlagsFromRuntime"), true);
  assert.equal(appShellRuntimeSource.includes("patchAccountLive("), false);
  assert.equal(appShellRuntimeSource.includes('wsClient.on("account_live"'), false);
  assert.equal(appShellRuntimeSource.includes('wsClient.on("account_status"'), false);
});

test("AppShell no longer subscribes to sidecar recording runtime events", () => {
  assert.equal(appShellSource.includes('wsClient.on("recording_started"'), false);
  assert.equal(appShellSource.includes('wsClient.on("recording_progress"'), false);
  assert.equal(appShellSource.includes('wsClient.on("recording_finished"'), false);
  assert.equal(appShellSource.includes("getRecordingStatus"), false);
});
