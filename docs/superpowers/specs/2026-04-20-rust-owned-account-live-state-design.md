# Rust-Owned Account Live State Design

**Date:** 2026-04-20
**Status:** Draft for review

---

## 1. Overview

The desktop app currently uses the Python sidecar watcher as a background source of truth for whether an account is live.

That happens through `AppShell` in three ways:

- re-registering accounts into the sidecar watcher on connect
- calling `poll-now` on startup/connect
- calling `live-overview` on an interval as an HTTP fallback

At the same time, the Rust/Tauri flow runtime already owns live-aware runtime state for flow execution. This leaves the app with two competing live-state systems:

- sidecar watcher state for accounts
- Rust runtime state for flows

This design removes sidecar live polling from `AppShell` and makes Rust the sole live-state source for accounts that are used by flows.

---

## 2. Goals And Non-Goals

### Goals

- Remove sidecar background live polling from `AppShell`
- Stop calling sidecar `syncWatcherForAccounts`, `pollNow`, and periodic `live-overview` for account live sync
- Make Rust/Tauri runtime the only live-state source for accounts that are used by flows
- Keep `account-store` live flags updated from Rust-derived runtime snapshots instead of sidecar watcher snapshots
- Keep behavior explicit and scoped to accounts with flows

### Non-Goals

- Replacing sidecar media-processing responsibilities
- Designing a separate Rust watch system for accounts with no flows
- Changing manual `checkAccountStatus()` behavior used when adding/testing an account unless needed later
- Introducing a new cross-process account polling API beyond what Rust runtime snapshots already provide unless the current runtime data proves insufficient

---

## 3. Problem Statement

Today the frontend still triggers sidecar account polling through `AppShell`.

Key current behaviors:

- `api.syncWatcherForAccounts(...)` re-registers all DB accounts into the sidecar watcher after connect
- `api.pollNow()` forces an immediate sidecar poll on connect/startup
- `syncLiveFromSidecarHttp()` calls `api.getLiveOverview()` immediately and every 5 seconds
- `account-store` receives `is_live` updates from sidecar HTTP snapshots and sidecar WebSocket account events

This means the UI still treats sidecar watcher state as a primary live source even after the Rust runtime was introduced.

That creates architectural drift and can produce contradictory states:

- Rust runtime says one thing about flow live state
- sidecar watcher says another thing about account live state

---

## 4. Chosen Direction

The chosen direction is:

- Rust runtime owns live state for accounts that have flows
- `AppShell` stops background account live polling through sidecar
- `account-store` derives `is_live` from Rust runtime snapshots for flow-linked accounts

This keeps a single runtime truth for flow-driven automation while avoiding the cost and confusion of sidecar polling in parallel.

---

## 5. Scope Boundary

This change applies only to accounts that are actively represented by flows.

### Included

- accounts referenced by one or more flows in SQLite
- account live flags shown in the desktop UI for those flow-linked accounts
- `AppShell` startup/connect logic that currently triggers sidecar live polling

### Excluded

- accounts with no flows
- any future standalone desktop watch registry not tied to flows
- sidecar manual status checks initiated by direct user actions unless explicitly removed later

This is intentionally narrower than “Rust owns live state for every account in the app.”

---

## 6. Source Of Truth

### 6.1 Flow Runtime Source

Rust runtime snapshots are the source of truth.

The existing `FlowRuntimeSnapshot` already includes the fields needed to reason about runtime activity, including:

- `flow_id`
- `account_id`
- `status`
- `current_node`
- `last_live_at`
- `active_flow_run_id`

### 6.2 Account Live Source

For flow-linked accounts, `account.is_live` in the frontend should no longer come from sidecar watcher snapshots.

Instead, it is derived from the current set of Rust runtime snapshots.

---

## 7. Live Derivation Rule

The user selected this rule:

- treat an account as live when Rust runtime indicates a live-capable state equivalent to “watching with current live signal” or “recording”
- do not treat `processing` as still live

### Canonical Rule

For a given flow-linked account:

- `is_live = true` if any current Rust runtime snapshot for that account satisfies one of these:
  - `status === "recording"`
  - `status === "watching"` and the runtime snapshot indicates a current live signal according to the runtime data already available
- `is_live = false` otherwise

### Important Constraint

`processing` must not keep the account live.

That means once the live room has ended and the flow has moved into post-live processing, the account live flag should fall back to `false`.

### Data Sufficiency Requirement

This design assumes the current Rust runtime snapshot data is sufficient to distinguish:

- “watching but not live yet”
- “watching and live is currently detected”

If the current snapshot fields are not sufficient for that distinction, the implementation must add the smallest explicit runtime signal needed at the Rust/Tauri boundary instead of guessing from ambiguous fields.

---

## 8. Aggregation Across Multiple Flows

An account may have multiple flows.

The account-level rule is:

- `account.is_live = true` if any flow snapshot for that account satisfies the canonical live derivation rule
- `account.is_live = false` only if none of that account’s flow snapshots satisfy the rule

This is an OR aggregation across flows belonging to the same account.

---

## 9. Frontend Data Flow

### Current Flow

Current `AppShell` behavior:

1. sidecar connects
2. app re-registers accounts into the sidecar watcher
3. app triggers sidecar polling
4. app polls sidecar `live-overview`
5. app writes sidecar results into SQLite and `account-store`

### New Flow

New behavior:

1. `useFlowStore().refreshRuntime()` gets Rust runtime snapshots
2. `flow-store` updates `runtimeSnapshots`
3. frontend derives account live flags from those runtime snapshots for flow-linked accounts
4. `account-store` receives a batch update from the Rust-derived live flags

The `account-store` update should happen from the same runtime refresh path rather than from a separate sidecar HTTP poll loop.

---

## 10. `AppShell` Changes

The following background behaviors should be removed from `AppShell`:

- re-registering all accounts into the sidecar watcher via `api.syncWatcherForAccounts(...)`
- immediate sidecar `api.pollNow()` on connect/startup
- immediate HTTP `syncLiveFromSidecarHttp()` on connect
- periodic `syncLiveFromSidecarHttp()` interval using `LIVE_HTTP_SYNC_MS`

The following behavior remains valid:

- Rust runtime refreshes through `useFlowStore().refreshRuntime()`
- sidecar WebSocket/event handling for recordings, clips, captions, and other media workflow events

The result is that `AppShell` stops using sidecar as a background live-checking service.

---

## 11. `account-store` Changes

`account-store` currently has sidecar-oriented batch update naming and semantics.

The design should replace that conceptual model with a Rust-owned live-state update path.

Recommended shape:

- keep `patchAccountLive(...)` for direct single-row updates when still useful
- replace or rename `applyLiveFlagsFromSidecar(...)` with a runtime-owned batch update such as `applyLiveFlagsFromRuntime(...)`

Behavior of the runtime batch update:

- only update accounts whose ids are explicitly present in the derived Rust live-flag batch
- avoid resetting unrelated accounts with no flow-linked runtime context unless the implementation intentionally defines that as part of the derived batch
- keep the same stale-fetch protection pattern already used in the store

---

## 12. Persistence Boundary

The frontend currently uses both in-memory store updates and SQLite writes for live flags.

This design keeps the persistence boundary explicit:

- if the app still needs account live flags persisted in SQLite for other screens or reload behavior, those writes should come from the Rust-derived state path
- sidecar-originated background writes for live flags should be removed from this app-shell path

If persistence is not actually needed beyond in-memory UI freshness, that should be verified during implementation and the simplest safe behavior should win.

---

## 13. Disabled / Missing Flow Cases

The Rust-owned account live model only applies to accounts with flow context.

Rules:

- accounts with no flows are outside the scope of this live-derivation system
- accounts with flows but no current runtime snapshot derive `is_live = false` unless another snapshot for the same account satisfies the live rule
- disabled flows must not produce active live cues for the account

This keeps the account flag aligned with the runtime system instead of stale historical values.

---

## 14. Failure Handling

If Rust runtime refresh fails:

- do not silently fall back to sidecar HTTP live polling in `AppShell`
- keep the failure explicit through the existing flow/runtime error surfaces
- keep the last known account live state until a successful Rust refresh replaces it, unless the implementation has a stronger correctness guarantee available

This avoids reintroducing the old dual-source architecture as an implicit fallback.

---

## 15. Verification

Implementation verification should confirm all of the following:

- `AppShell` no longer calls sidecar `syncWatcherForAccounts`, `pollNow`, or `live-overview` for background live sync
- sidecar logs no longer show periodic `live-overview` hits triggered by desktop background sync
- `account-store` live flags update from Rust runtime refreshes for flow-linked accounts
- a flow-linked account becomes live when runtime indicates a live-capable state under the chosen rule
- `processing` does not keep the account flagged as live
- multiple flows on the same account aggregate with OR semantics
- disabled flows do not keep account live flags stuck on stale runtime state

Code verification for frontend work should include:

- `npm run lint:js`

If Rust/Tauri runtime surface changes are required to expose an explicit watching-with-live signal, the Rust verification for touched files should also run.

---

## 16. Success Criteria

This change is successful when:

- opening the app no longer triggers background sidecar live polling from `AppShell`
- live flags for flow-linked accounts come from Rust runtime state only
- the app no longer has two competing background sources of truth for account live state
- Accounts UI and Flow runtime state agree for accounts with flows

The architecture should become simpler: sidecar handles media/runtime auxiliaries, while Rust owns flow-linked live state.
