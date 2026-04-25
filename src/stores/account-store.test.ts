import test from "node:test";
import assert from "node:assert/strict";

import { useAccountStore } from "./account-store";

function resetAccountStore(): void {
  useAccountStore.setState({
    accounts: [],
    loading: false,
    error: null,
  });
}

function account(
  id: number,
  is_live: boolean,
): {
  id: number;
  username: string;
  display_name: string;
  avatar_url: string | null;
  type: "own" | "monitored";
  tiktok_uid: string | null;
  cookies_json: string | null;
  auto_record: boolean;
  auto_record_schedule: null;
  priority: number;
  is_live: boolean;
  last_live_at: string | null;
  last_checked_at: string | null;
  proxy_url: string | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
} {
  return {
    id,
    username: `account-${id}`,
    display_name: `Account ${id}`,
    avatar_url: null,
    type: "monitored",
    tiktok_uid: null,
    cookies_json: null,
    auto_record: false,
    auto_record_schedule: null,
    priority: 0,
    is_live,
    last_live_at: null,
    last_checked_at: null,
    proxy_url: null,
    notes: null,
    created_at: "2026-04-20T00:00:00.000+07:00",
    updated_at: "2026-04-20T00:00:00.000+07:00",
  };
}

test("applyLiveFlagsFromRuntime updates only explicitly listed accounts", () => {
  resetAccountStore();
  useAccountStore.setState({
    accounts: [account(1, false), account(2, true)],
  });

  useAccountStore.getState().applyLiveFlagsFromRuntime([{ id: 1, isLive: true }]);

  const accounts = useAccountStore.getState().accounts;
  assert.equal(accounts[0]?.is_live, true);
  assert.equal(accounts[1]?.is_live, true);
});

test("applyLiveFlagsFromRuntime updates in-memory rows immediately", () => {
  resetAccountStore();
  useAccountStore.setState({
    accounts: [account(1, false)],
  });

  useAccountStore.getState().applyLiveFlagsFromRuntime([{ id: 1, isLive: true }]);

  assert.equal(useAccountStore.getState().accounts[0]?.is_live, true);
});
