# Rust-Owned Account Live State Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove sidecar background live polling from `AppShell` and make Rust flow runtime snapshots the sole live-state source for `account.is_live` on accounts that are used by flows.

**Architecture:** Keep sidecar for media/clip/caption runtime work, but stop using it as a background account-live watcher in `AppShell`. Derive account live flags from the current Rust `FlowRuntimeSnapshot[]` set during `useFlowStore().refreshRuntime()`, propagate those flags into `account-store`, and remove the sidecar connect/startup polling path (`syncWatcherForAccounts`, `pollNow`, `live-overview`) from the app shell.

**Tech Stack:** React 19, TypeScript, Zustand, Tauri invoke, node:test, Vite

---

## File Structure

**Create:**
- `src/lib/account-live-from-runtime.ts` - focused helper that derives per-account live flags from `FlowRuntimeSnapshot[]`
- `src/lib/account-live-from-runtime.test.ts` - unit tests for aggregation and live-derivation rules

**Modify:**
- `src/stores/account-store.ts` - replace sidecar-oriented batch live update naming/semantics with runtime-owned batch update path
- `src/stores/flow-store.ts` - only change this file if a tiny explicit return-value/helper extraction is the cleanest way to reuse current runtime snapshot results during refresh flow
- `src/components/layout/app-shell.tsx` - remove sidecar background live polling path and wire account live updates from Rust runtime refresh results
- `src/lib/api.ts` - remove now-unused sidecar live-overview/poll-now sync calls from the frontend path only if no remaining callers exist after the AppShell change
- `src/components/layout/app-shell.test.ts` if present, or create focused tests in the nearest existing test file if the helper extraction makes behavior testable without a React DOM harness
- `package.json` only if an additional frontend test file path needs inclusion via the existing `test:js` script pattern; no new dependency needed

**Reference / inspect while implementing:**
- `src/components/layout/app-shell.tsx` - current sidecar connect/startup live sync effects
- `src/stores/account-store.ts` - current `patchAccountLive` / `applyLiveFlagsFromSidecar`
- `src/stores/flow-store.ts` - current runtime snapshot normalization and `refreshRuntime()` behavior
- `src/types/index.ts` - `FlowRuntimeSnapshot`, `FlowStatus`
- `docs/superpowers/specs/2026-04-20-rust-owned-account-live-state-design.md` - approved behavior and scope

**Test:**
- `src/lib/account-live-from-runtime.test.ts`
- `src/components/layout/app-shell.test.ts` if created, otherwise the helper-focused test file(s) covering removed sidecar polling behavior

---

### Task 1: Derive Account Live Flags From Rust Runtime Snapshots

**Files:**
- Create: `src/lib/account-live-from-runtime.ts`
- Create: `src/lib/account-live-from-runtime.test.ts`
- Reference: `src/types/index.ts`

- [ ] **Step 1: Write failing unit tests for runtime-derived account live aggregation**

Create `src/lib/account-live-from-runtime.test.ts`:

```ts
import test from "node:test";
import assert from "node:assert/strict";

import type { FlowRuntimeSnapshot } from "@/types";

import { deriveAccountLiveFlagsFromRuntime } from "./account-live-from-runtime";

function createSnapshot(overrides: Partial<FlowRuntimeSnapshot> = {}): FlowRuntimeSnapshot {
  return {
    flow_id: overrides.flow_id ?? 7,
    status: overrides.status ?? "watching",
    current_node: "current_node" in overrides ? overrides.current_node ?? null : "start",
    account_id: overrides.account_id ?? 44,
    username: overrides.username ?? "shop_abc",
    last_live_at: "last_live_at" in overrides ? overrides.last_live_at ?? null : null,
    last_error: "last_error" in overrides ? overrides.last_error ?? null : null,
    active_flow_run_id: "active_flow_run_id" in overrides ? overrides.active_flow_run_id ?? null : null,
  };
}

test("deriveAccountLiveFlagsFromRuntime marks recording flows as live", () => {
  const rows = deriveAccountLiveFlagsFromRuntime([
    createSnapshot({ account_id: 44, status: "recording", current_node: "record" }),
  ]);

  assert.deepEqual(rows, [{ id: 44, isLive: true }]);
});

test("deriveAccountLiveFlagsFromRuntime marks watching flows as live only when a current live signal is present", () => {
  const rows = deriveAccountLiveFlagsFromRuntime([
    createSnapshot({ account_id: 44, status: "watching", current_node: "start", last_live_at: "2026-04-20T18:00:00.000+07:00" }),
    createSnapshot({ account_id: 45, status: "watching", current_node: "start", last_live_at: null }),
  ]);

  assert.deepEqual(rows, [
    { id: 44, isLive: true },
    { id: 45, isLive: false },
  ]);
});

test("deriveAccountLiveFlagsFromRuntime does not treat processing as live", () => {
  const rows = deriveAccountLiveFlagsFromRuntime([
    createSnapshot({ account_id: 44, status: "processing", current_node: "clip", last_live_at: "2026-04-20T18:00:00.000+07:00" }),
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
    createSnapshot({ account_id: null, status: "recording" }),
  ]);

  assert.deepEqual(rows, []);
});
```

- [ ] **Step 2: Run the new helper test file and verify it fails**

Run: `npm run test:js -- src/lib/account-live-from-runtime.test.ts`
Expected: FAIL because the helper file does not exist yet.

- [ ] **Step 3: Implement the minimal runtime-live derivation helper**

Create `src/lib/account-live-from-runtime.ts`:

```ts
import type { FlowRuntimeSnapshot } from "@/types";

export type AccountLiveFlag = {
  id: number;
  isLive: boolean;
};

function snapshotIndicatesLive(snapshot: FlowRuntimeSnapshot): boolean {
  if (snapshot.status === "recording") {
    return true;
  }
  if (snapshot.status === "watching") {
    return snapshot.last_live_at != null;
  }
  return false;
}

export function deriveAccountLiveFlagsFromRuntime(
  snapshots: FlowRuntimeSnapshot[],
): AccountLiveFlag[] {
  const byAccount = new Map<number, boolean>();

  for (const snapshot of snapshots) {
    if (snapshot.account_id == null) {
      continue;
    }
    const accountId = Number(snapshot.account_id);
    if (!Number.isFinite(accountId)) {
      continue;
    }
    const nextIsLive = snapshotIndicatesLive(snapshot);
    byAccount.set(accountId, (byAccount.get(accountId) ?? false) || nextIsLive);
  }

  return Array.from(byAccount.entries())
    .sort((a, b) => a[0] - b[0])
    .map(([id, isLive]) => ({ id, isLive }));
}
```

- [ ] **Step 4: Run the helper tests and verify they pass**

Run: `npm run test:js -- src/lib/account-live-from-runtime.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit the runtime live derivation helper**

```bash
git add src/lib/account-live-from-runtime.ts src/lib/account-live-from-runtime.test.ts
git commit -m "feat(runtime): derive account live flags from flow snapshots"
```

### Task 2: Replace Sidecar-Oriented Account Live Updates In The Store Layer

**Files:**
- Modify: `src/stores/account-store.ts`
- Reference: `src/lib/account-live-from-runtime.ts`

- [ ] **Step 1: Write failing store-level tests for the new runtime batch update behavior**

Add these tests to a new file `src/stores/account-store.test.ts` if the repo does not already have one; if it already exists, use it. The new file should contain:

```ts
import test from "node:test";
import assert from "node:assert/strict";

import { useAccountStore } from "./account-store";

function resetAccountStore() {
  useAccountStore.setState({
    accounts: [],
    loading: false,
    error: null,
  });
}

test("applyLiveFlagsFromRuntime updates only explicitly listed accounts", () => {
  resetAccountStore();
  useAccountStore.setState({
    accounts: [
      { id: 1, username: "a", display_name: null, cookies_json: null, proxy_url: null, auto_record: false, auto_record_schedule: null, is_live: false, type: "monitoring", created_at: "", updated_at: "" },
      { id: 2, username: "b", display_name: null, cookies_json: null, proxy_url: null, auto_record: false, auto_record_schedule: null, is_live: true, type: "monitoring", created_at: "", updated_at: "" },
    ],
  });

  useAccountStore.getState().applyLiveFlagsFromRuntime([{ id: 1, isLive: true }]);

  const accounts = useAccountStore.getState().accounts;
  assert.equal(accounts[0]?.is_live, true);
  assert.equal(accounts[1]?.is_live, true);
});

test("applyLiveFlagsFromRuntime bumps generation semantics by updating in-memory rows immediately", () => {
  resetAccountStore();
  useAccountStore.setState({
    accounts: [
      { id: 1, username: "a", display_name: null, cookies_json: null, proxy_url: null, auto_record: false, auto_record_schedule: null, is_live: false, type: "monitoring", created_at: "", updated_at: "" },
    ],
  });

  useAccountStore.getState().applyLiveFlagsFromRuntime([{ id: 1, isLive: true }]);

  assert.equal(useAccountStore.getState().accounts[0]?.is_live, true);
});
```

- [ ] **Step 2: Run the account-store tests and verify they fail**

Run: `npm run test:js -- src/stores/account-store.test.ts`
Expected: FAIL because `applyLiveFlagsFromRuntime` does not exist yet.

- [ ] **Step 3: Rename or replace the sidecar batch update action with a runtime-owned one**

In `src/stores/account-store.ts`, change the interface and implementation:

```ts
  applyLiveFlagsFromRuntime: (rows: { id: number; isLive: boolean }[]) => void;
```

Replace the old action implementation:

```ts
  applyLiveFlagsFromRuntime: (rows) => {
    if (rows.length === 0) {
      return;
    }
    listAccountsGeneration += 1;
    const map = new Map(rows.map((r) => [Number(r.id), r.isLive]));
    set((s) => ({
      loading: false,
      accounts: s.accounts.map((a) => {
        const live = map.get(Number(a.id));
        return live === undefined ? a : { ...a, is_live: live };
      }),
    }));
  },
```

Keep `patchAccountLive(...)` unchanged for targeted single-row updates if still used elsewhere.

- [ ] **Step 4: Run the account-store tests and verify they pass**

Run: `npm run test:js -- src/stores/account-store.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit the account-store live-state rename/update**

```bash
git add src/stores/account-store.ts src/stores/account-store.test.ts
git commit -m "refactor(accounts): accept runtime-owned live flag updates"
```

### Task 3: Remove Sidecar Background Live Polling From AppShell

**Files:**
- Modify: `src/components/layout/app-shell.tsx`
- Modify: `src/lib/api.ts` only if a sidecar live sync helper becomes unused after the AppShell changes

- [ ] **Step 1: Add failing tests for AppShell-side live sync decisions via extracted helpers**

If `src/components/layout/app-shell.test.ts` does not exist, create it with helper-focused tests instead of a full rendered shell harness:

```ts
import test from "node:test";
import assert from "node:assert/strict";

import {
  shouldStartSidecarAccountLiveSync,
} from "./app-shell";

test("shouldStartSidecarAccountLiveSync is false when sidecar is connected", () => {
  assert.equal(shouldStartSidecarAccountLiveSync(), false);
});
```

This helper exists only to lock the architectural decision that AppShell should no longer start the sidecar account-live sync path.

- [ ] **Step 2: Run the AppShell helper test and verify it fails**

Run: `npm run test:js -- src/components/layout/app-shell.test.ts`
Expected: FAIL because the helper does not exist yet.

- [ ] **Step 3: Extract a small helper and remove sidecar background live sync effects from AppShell**

In `src/components/layout/app-shell.tsx`, add a small exported helper near the existing runtime ingress helper section:

```ts
export function shouldStartSidecarAccountLiveSync(): boolean {
  return false;
}
```

Then remove the background sidecar account-live sync path:

- delete the effect that calls `api.syncWatcherForAccounts(...)`, `api.pollNow()`, and `api.syncAccountsLiveStatus(...)`
- delete the effect that calls `syncLiveFromSidecarHttp()` immediately and on `LIVE_HTTP_SYNC_MS`
- delete `syncLiveFromSidecarHttp()` and `LIVE_HTTP_SYNC_MS` if they become unused

Do **not** remove sidecar recording-status sync or sidecar media-event WebSocket handling.

In `src/lib/api.ts`, remove `getLiveOverview`, `pollNow`, and `syncWatcherForAccounts` only if `grep` confirms they have no remaining callers after the AppShell refactor.

- [ ] **Step 4: Run the AppShell helper test and frontend lint/build verification**

Run: `npm run test:js -- src/components/layout/app-shell.test.ts`
Expected: PASS.

Run: `npm run lint:js`
Expected: PASS.

- [ ] **Step 5: Commit the AppShell polling removal**

```bash
git add src/components/layout/app-shell.tsx src/components/layout/app-shell.test.ts src/lib/api.ts
git commit -m "refactor(app-shell): remove sidecar account live polling"
```

If `src/lib/api.ts` ended up unchanged because the helpers are still used elsewhere, leave it out of the commit.

### Task 4: Drive Account Live Flags From Rust Runtime Refreshes

**Files:**
- Modify: `src/components/layout/app-shell.tsx`
- Modify: `src/lib/account-live-from-runtime.ts` only if a tiny additional helper is needed
- Modify: `src/stores/flow-store.ts` only if the current refresh path needs a small return-value/helper extraction to avoid duplicating runtime fetches

- [ ] **Step 1: Write failing tests for the runtime-to-account update path**

Extend `src/components/layout/app-shell.test.ts` with helper tests for a small extracted function such as `deriveAccountLiveFlagsForShell(...)` or direct use of `deriveAccountLiveFlagsFromRuntime(...)` if no shell-specific wrapper is needed:

```ts
import { deriveAccountLiveFlagsFromRuntime } from "@/lib/account-live-from-runtime";

test("runtime-derived account flags use recording and watching-with-live-signal only", () => {
  const rows = deriveAccountLiveFlagsFromRuntime([
    {
      flow_id: 7,
      status: "watching",
      current_node: "start",
      account_id: 44,
      username: "shop_abc",
      last_live_at: "2026-04-20T18:00:00.000+07:00",
      last_error: null,
      active_flow_run_id: null,
    },
    {
      flow_id: 8,
      status: "processing",
      current_node: "clip",
      account_id: 45,
      username: "shop_xyz",
      last_live_at: "2026-04-20T18:00:00.000+07:00",
      last_error: null,
      active_flow_run_id: 50,
    },
  ]);

  assert.deepEqual(rows, [
    { id: 44, isLive: true },
    { id: 45, isLive: false },
  ]);
});
```

- [ ] **Step 2: Run the helper tests and verify they fail or expose missing wiring**

Run: `npm run test:js -- src/components/layout/app-shell.test.ts src/lib/account-live-from-runtime.test.ts src/stores/account-store.test.ts`
Expected: if helper tests were added against missing shell wiring, FAIL before implementation; after helper exists but before AppShell wiring, the behavior gap should still be explicit in code.

- [ ] **Step 3: Wire account-store updates from the Rust runtime refresh path**

In `src/components/layout/app-shell.tsx`, after runtime refresh points that already call `useFlowStore.getState().refreshRuntime()`, ensure account live flags are derived from the latest runtime snapshots and applied to `account-store`.

Use a small local helper pattern such as:

```ts
async function refreshRuntimeAndSyncAccountLiveFlags(): Promise<void> {
  await useFlowStore.getState().refreshRuntime();
  const snapshots = Object.values(useFlowStore.getState().runtimeSnapshots);
  const rows = deriveAccountLiveFlagsFromRuntime(snapshots);
  useAccountStore.getState().applyLiveFlagsFromRuntime(rows);
}
```

Then replace existing `refreshRuntime()` calls in `AppShell` that are meant to keep UI state current with this wrapper where appropriate.

Keep the change tight:

- only replace callsites that matter for account live synchronization
- do not introduce a second polling loop
- do not change unrelated sidecar event handling semantics

If `flow-store.refreshRuntime()` can be extended to return snapshot rows cleanly with less duplication, that is acceptable only if the change stays small and explicit.

- [ ] **Step 4: Run the relevant tests and frontend verification**

Run: `npm run test:js -- src/components/layout/app-shell.test.ts src/lib/account-live-from-runtime.test.ts src/stores/account-store.test.ts`
Expected: PASS.

Run: `npm run lint:js`
Expected: PASS.

- [ ] **Step 5: Commit the Rust-owned account live sync path**

```bash
git add src/components/layout/app-shell.tsx src/components/layout/app-shell.test.ts src/lib/account-live-from-runtime.ts src/lib/account-live-from-runtime.test.ts src/stores/account-store.ts src/stores/account-store.test.ts src/stores/flow-store.ts
git commit -m "feat(runtime): drive account live state from rust flow snapshots"
```

If `src/stores/flow-store.ts` did not need changes, leave it out of the commit.

### Task 5: Final Verification And Regression Pass

**Files:**
- Modify: none, unless verification reveals a small follow-up issue

- [ ] **Step 1: Run the complete frontend verification for the touched live-state files**

Run: `npm run test:js -- src/lib/account-live-from-runtime.test.ts src/stores/account-store.test.ts src/components/layout/app-shell.test.ts`
Expected: PASS.

Run: `npm run lint:js`
Expected: PASS.

- [ ] **Step 2: Manually verify the sidecar account-polling path is gone**

Open the app in the normal local workflow and verify:

```text
1. Start the desktop app with sidecar available.
2. Confirm the app no longer calls `/api/accounts/poll-now` on startup/connect.
3. Confirm the app no longer calls `/api/accounts/live-overview` immediately on connect.
4. Confirm the app no longer calls `/api/accounts/live-overview` on a 5-second interval.
5. Confirm sidecar logs no longer show periodic account watcher polling triggered by AppShell background sync.
6. Confirm flow-linked account live flags still update in the UI when Rust runtime snapshots change.
7. Confirm a `processing` flow does not keep the account shown as live.
8. Confirm multiple flows on one account still aggregate to `is_live = true` if at least one flow is live.
```

Expected: all checks pass.

- [ ] **Step 3: Review final diff for scope discipline**

Run: `git diff -- src/components/layout/app-shell.tsx src/components/layout/app-shell.test.ts src/stores/account-store.ts src/stores/account-store.test.ts src/lib/account-live-from-runtime.ts src/lib/account-live-from-runtime.test.ts src/lib/api.ts src/stores/flow-store.ts`
Expected: only the planned sidecar-poll removal and Rust-owned account-live changes appear.

- [ ] **Step 4: Commit any verification-driven polish fix when manual verification reveals one**

```bash
git add src/components/layout/app-shell.tsx src/components/layout/app-shell.test.ts src/stores/account-store.ts src/stores/account-store.test.ts src/lib/account-live-from-runtime.ts src/lib/account-live-from-runtime.test.ts src/lib/api.ts src/stores/flow-store.ts
git commit -m "fix(runtime): polish rust-owned account live state"
```

Skip this step if no follow-up fix is needed.

---

## Self-Review

- Spec coverage check: the plan covers removing sidecar background polling from `AppShell`, deriving account live flags from Rust runtime snapshots, renaming the sidecar-oriented account batch update path, enforcing the chosen live rule (`recording` or `watching + current live signal`, but not `processing`), and keeping the scope limited to flow-linked accounts.
- Placeholder scan: no `TODO`, `TBD`, or vague “handle appropriately” steps remain; each task points to exact files and concrete actions.
- Type consistency: the plan consistently uses `deriveAccountLiveFlagsFromRuntime(...)`, `applyLiveFlagsFromRuntime(...)`, and `shouldStartSidecarAccountLiveSync()` to separate the new Rust-owned flow from the removed sidecar polling path.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-20-rust-owned-account-live-state.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
