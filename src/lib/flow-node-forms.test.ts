import test from "node:test";
import assert from "node:assert/strict";

import { parseRecordNodeDraft, parseStartNodeDraft } from "./flow-node-forms.ts";

test("parseStartNodeDraft accepts legacy camelCase start config", () => {
  const parsed = parseStartNodeDraft(
    JSON.stringify({
      username: " @shop_abc ",
      cookiesJson: "{}",
      proxyUrl: "http://127.0.0.1:9000",
      wafBypassEnabled: false,
      pollIntervalSeconds: 25,
      retryLimit: 4,
    }),
  );

  assert.deepEqual(parsed, {
    username: "shop_abc",
    cookies_json: "{}",
    proxy_url: "http://127.0.0.1:9000",
    waf_bypass_enabled: false,
    poll_interval_seconds: 25,
    retry_limit: 4,
  });
});

test("parseRecordNodeDraft accepts legacy duration keys", () => {
  const secondsBased = parseRecordNodeDraft(JSON.stringify({ maxDurationSeconds: 600 }));
  const legacyMinutes = parseRecordNodeDraft(JSON.stringify({ maxDuration: 7 }));
  const nonMinuteSeconds = parseRecordNodeDraft(JSON.stringify({ durationSeconds: 61 }));

  assert.equal(secondsBased.max_duration_minutes, 10);
  assert.equal(legacyMinutes.max_duration_minutes, 7);
  assert.equal(nonMinuteSeconds.max_duration_minutes, 2);
});
