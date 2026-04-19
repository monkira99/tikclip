import test from "node:test";
import assert from "node:assert/strict";

import { syncRecordingListFromSidecar } from "./recording-sidecar-sync.ts";

test("syncRecordingListFromSidecar upserts every sidecar recording payload into SQLite bridge", async () => {
  const calls: Array<Record<string, unknown>> = [];

  await syncRecordingListFromSidecar(
    [
      {
        recording_id: "rec-1",
        account_id: 11,
        status: "recording",
        duration_seconds: 12,
      },
      {
        recording_id: "rec-2",
        account_id: 22,
        status: "done",
        file_path: "/tmp/out.mp4",
      },
    ],
    async (payload) => {
      calls.push(payload);
    },
  );

  assert.equal(calls.length, 2);
  assert.deepEqual(calls.map((row) => row.recording_id), ["rec-1", "rec-2"]);
});

test("syncRecordingListFromSidecar ignores malformed entries and keeps valid ones", async () => {
  const calls: Array<Record<string, unknown>> = [];

  await syncRecordingListFromSidecar(
    [
      {
        recording_id: "rec-1",
        account_id: 11,
        status: "recording",
      },
      {
        account_id: 22,
        status: "recording",
      },
      {
        recording_id: "rec-3",
        account_id: 0,
        status: "recording",
      },
    ],
    async (payload) => {
      calls.push(payload);
    },
  );

  assert.equal(calls.length, 1);
  assert.equal(calls[0]?.recording_id, "rec-1");
});
