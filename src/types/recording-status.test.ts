import test from "node:test";
import assert from "node:assert/strict";

import type { RecordingStatus } from "@/types";

const RECORDING_STATUSES = [
  "recording",
  "processing",
  "done",
  "error",
  "cancelled",
] as const satisfies readonly RecordingStatus[];

test("RecordingStatus mirrors durable recording statuses", () => {
  assert.deepEqual([...RECORDING_STATUSES], [
    "recording",
    "processing",
    "done",
    "error",
    "cancelled",
  ]);
});
