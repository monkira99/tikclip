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
const projectSrcDir = path.resolve(sourceDir, "../..");
const clipsApiSource = fs.readFileSync(
  path.resolve(projectSrcDir, "lib/api/clips.ts"),
  "utf8",
);
const productsApiSource = fs.readFileSync(
  path.resolve(projectSrcDir, "lib/api/products.ts"),
  "utf8",
);

test("AppShell no longer contains the legacy account-live polling path", () => {
  assert.equal(appShellSource.includes("syncWatcherForAccounts"), false);
  assert.equal(appShellSource.includes("pollNow"), false);
  assert.equal(appShellSource.includes("live-overview"), false);
  assert.equal(appShellSource.includes("syncLiveFromSidecarHttp"), false);
  assert.equal(appShellSource.includes("LIVE_HTTP_SYNC_MS"), false);
});

test("AppShell keeps live state on flow runtime instead of syncing account flags", () => {
  assert.equal(appShellRuntimeSource.includes("applyLiveFlagsFromRuntime"), false);
  assert.equal(appShellRuntimeSource.includes("deriveAccountLiveFlagsFromRuntime"), false);
  assert.equal(appShellRuntimeSource.includes("syncAccountsLiveStatus"), false);
  assert.equal(appShellRuntimeSource.includes("patchAccountLive("), false);
  assert.equal(appShellRuntimeSource.includes('wsClient.on("account_live"'), false);
  assert.equal(appShellRuntimeSource.includes('wsClient.on("account_status"'), false);
});

test("AppShell no longer subscribes to legacy recording runtime events", () => {
  assert.equal(appShellSource.includes('wsClient.on("recording_started"'), false);
  assert.equal(appShellSource.includes('wsClient.on("recording_progress"'), false);
  assert.equal(appShellSource.includes('wsClient.on("recording_finished"'), false);
  assert.equal(appShellSource.includes("getRecordingStatus"), false);
});

test("AppShell leaves clip auto-tag orchestration to Rust", () => {
  assert.equal(appShellEffectsSource.includes("maybeAutoTagClipAfterInsert"), false);
  assert.equal(appShellEffectsSource.includes("suggestProductForClip"), false);
  assert.equal(appShellEffectsSource.includes("tagClipProduct"), false);
});

test("Frontend API no longer calls removed suggest/product embedding HTTP routes directly", () => {
  assert.equal(clipsApiSource.includes("/api/clips/suggest-product"), false);
  assert.equal(productsApiSource.includes("/api/products/embeddings/index"), false);
  assert.equal(productsApiSource.includes("/api/products/embeddings/delete"), false);
  assert.equal(clipsApiSource.includes("sidecarJson"), false);
  assert.equal(productsApiSource.includes("sidecarJson"), false);
});
