# Rust TikTok Start And Record Flow Design

**Date:** 2026-04-18
**Status:** Draft for review

---

## 1. Overview

This design ports the TikTok live detection and recording slice from the Python sidecar into `Rust/Tauri`, but aligns it to the V2 flow model instead of copying the old global watcher shape.

The chosen execution loop for each enabled flow is:

`Start(check live) -> Record(record once) -> Clip -> Caption -> Upload -> back to Start`

The core architectural shift is:
- TikTok source configuration moves fully into the `Start` node
- `Start` owns polling and live detection for one flow only
- `Record` only consumes a resolved stream and records it once
- a `flow_run` is created only when `Start` detects a live session worth recording
- runtime orchestration for this slice moves from Python sidecar to `Rust/Tauri`

This keeps the flow mental model intact: each flow has its own source, its own polling behavior, and its own execution loop.

---

## 2. Goals And Non-Goals

### Goals

- Port the TikTok `check live` logic from Python into Rust
- Port the ffmpeg recording runtime from Python into Rust
- Make polling, cookies, proxy, and related source settings configurable per flow via `Start`
- Keep `Record` focused on recording only
- Create a new `flow_run` only when a real live session is detected
- Preserve ownership of outputs via `account_id + flow_run_id`, with runtime auto-upsert of `accounts` rows when needed
- Prevent two flows from monitoring the same `username` concurrently
- Preserve the current fixed pipeline shape `Start -> Record -> Clip -> Caption -> Upload`

### Non-Goals

- Supporting multiple enabled flows for the same `username`
- Persisting the full long-running polling session as a new DB table in phase 1
- Refactoring the full workflow engine into a generic event/DAG engine
- Migrating clip, caption, product, or upload runtime ownership out of their current layers in this phase
- Preserving Python's `max_duration` segment chaining behavior

---

## 3. Chosen Direction

Three directions were considered:

1. Put both polling and recording into an upgraded `Record` node
2. Run a global Rust watcher service and let flow nodes subscribe to it
3. Put polling in `Start`, keep `Record` as a pure recorder, and let the flow loop back to `Start` after one run

### Recommendation

Direction 3 is the chosen design.

It matches the product semantics more closely:
- `Start` behaves like a source node with flow-owned TikTok configuration
- `Record` behaves like an action node that records one resolved stream input
- the flow engine still advances through the same fixed node order
- each completed run represents one detected live session, not hours of idle waiting

This also avoids turning the new Rust implementation into another global watcher shared across flows.

---

## 4. Product Semantics

### 4.1 Start Node Responsibility

`Start` becomes the source node for TikTok live detection.

It owns:
- `username`
- `cookies_json`
- `proxy_url`
- polling interval
- retry and backoff settings
- any future TikTok-specific source toggles for this flow

`Start` does not immediately complete when the flow is enabled. Instead, the flow's automation session stays in a polling loop until live is detected.

`Start` also owns account binding for the run:
- the runtime resolves `account_id` from `accounts.username`
- if no matching account exists, the runtime auto-upserts a `monitored` account row for that username before a run starts
- the resolved `account_id` is then used for recordings, clips, dashboard queries, and downstream pipeline ownership

### 4.2 Record Node Responsibility

`Record` no longer owns source discovery or polling.

It only:
- accepts the resolved stream input from `Start`
- spawns and monitors `ffmpeg`
- writes the recording row and emits runtime progress
- completes when the single recording attempt ends

`Record` does not re-check TikTok after completion.

### 4.3 Flow Loop

The runtime loop for one enabled flow is:

1. `Start` polls TikTok using that flow's published config
2. When the source is live and a valid `stream_url` is resolved, the engine creates a new `flow_run`
3. The run advances to `Record`
4. The run continues through `Clip`, `Caption`, and `Upload`
5. When the run ends, the automation session returns to `Start` polling for the next live session

This means the long-lived polling loop exists outside an active `flow_run`, while each actual live session produces its own run history.

---

## 5. Execution Semantics

### 5.1 Automation Session Versus Flow Run

Phase 1 introduces an in-memory Rust automation session per enabled flow.

The automation session:
- loads the published `Start` and `Record` config for the flow
- resolves or creates the bound `account_id` for the published `username`
- acquires a `username` lease
- polls until live is detected
- creates and advances a `flow_run` when a real session starts
- returns to polling after the run finishes

The automation session is not a new durable DB entity in phase 1.

The `flow_run` remains the durable record of one concrete execution instance.

### 5.2 When A Flow Run Starts

A `flow_run` starts only when all of the following are true:
- `Start` successfully checks live status
- the target is currently live
- a valid `room_id` is known or can be derived
- a valid `stream_url` is resolved for recording
- the current live room is not the same room that this session has already completed for the current on-air cycle

If TikTok reports live but no reliable `stream_url` can be resolved, the session stays in `Start` polling and does not create a `flow_run` yet.

To avoid duplicate runs for the same ongoing live session:
- the automation session stores the last completed live identity, keyed by `room_id`
- after a run finishes, the session returns to polling but does not create another run while the same `room_id` remains live
- a new run is allowed only after the source goes offline once, or a different `room_id` appears

### 5.3 What Counts As Record Completion

`Record` is a one-shot recorder in this design.

The run proceeds to the next node when any of these happen:
- the livestream stops and `ffmpeg` exits cleanly enough to keep the output
- the recording reaches the runtime max duration derived from `max_duration_minutes`

The key intentional behavior change from Python is:
- when the runtime max duration is reached, `Record` is considered complete
- the flow does not re-check live and does not chain another segment automatically

This is an explicit product decision for the new flow model.

---

## 6. Ported Python Logic

Phase 1 should carry over the full Python TikTok and recording slice except where this spec explicitly changes behavior.

### 6.1 TikTok Live Detection Logic To Port

Port from `sidecar/src/tiktok/*` and `sidecar/src/core/watcher.py`:
- cookie normalization and summary helpers
- proxy-aware HTTP transport
- live page fetch and room id extraction patterns
- webcast `room/info` fetch with `aid=1988`
- fallback `check_alive` request when `room/info` fails
- stream URL selection from webcast payloads
- viewer count extraction
- per-request error handling and logging decisions
- invalid `cookies_json` handling

### 6.2 Recording Runtime Logic To Port

Port from `sidecar/src/core/recorder.py` and `sidecar/src/core/worker.py`:
- ffmpeg command construction
- output path generation
- process monitoring
- periodic progress updates
- stop/terminate/kill behavior
- status mapping for `pending`, `recording`, `completed`, `stopped`, `error`
- final file size and duration refresh

### 6.3 Intentional Behavior Difference

One Python behavior is not preserved:

- Python chains another recording segment when `max_duration_seconds` ends but the host is still live
- the new flow design does not do segment chaining
- one `Record` node execution produces one recording attempt only

This difference is intentional and must be documented in both implementation and rollout notes.

---

## 7. Architecture In Rust

### 7.1 Module Boundaries

The Rust implementation should be split into focused units:

- `src-tauri/src/tiktok/*`
  - TikTok cookies, HTTP transport, live status resolution, room id extraction, and stream selection
- `src-tauri/src/workflow/start_node.rs`
  - validation and runtime entry for `Start`
- `src-tauri/src/workflow/record_node.rs`
  - validation and runtime entry for `Record`
- `src-tauri/src/live_runtime/*`
  - flow automation sessions, polling loop, username lease registry, and runtime status
- `src-tauri/src/recording_runtime/*`
  - ffmpeg worker lifecycle and progress tracking
- `src-tauri/src/workflow/runtime_store.rs`
  - DB helpers for `flow_runs`, `flow_node_runs`, and runtime transitions

The implementation should prefer small Rust structs with clear ownership boundaries over one large shared manager.

### 7.2 Tauri Integration

The runtime is owned by `Rust/Tauri` and managed in `lib.rs`.

It should:
- register any new commands in `tauri::generate_handler!`
- store shared runtime managers via `.manage(...)`
- emit frontend runtime events via `AppHandle` / Tauri event emission
- avoid blocking the main thread with polling or process I/O

The sidecar remains for the other domains in phase 1, but not for this TikTok `Start/Record` slice.

### 7.3 Session Lifecycle And Command Boundaries

Phase 1 adds a `LiveRuntimeManager` managed by Tauri alongside `AppState`.

Lifecycle rules:
- app startup: after SQLite is initialized, the manager scans enabled flows and starts one automation session per eligible flow
- app shutdown: the manager stops all sessions, terminates active recording workers, and releases all username leases
- `set_flow_enabled(flow_id, true)`: after the DB update commits, the manager starts or reconciles that flow session immediately from the latest published config
- `set_flow_enabled(flow_id, false)`: after the DB update commits, the manager stops the session immediately, cancels any active run, and releases the username lease
- `publish_flow_definition(flow_id)`: after the publish transaction commits, if the flow is enabled the manager stops the current session, releases the lease, and starts a fresh session from the new published config immediately

Atomicity rule:
- after a publish or enable change, there must never be two active sessions for the same flow
- if session restart fails, the old session stays stopped and the new failure is surfaced through runtime error state rather than silently continuing on stale config

### 7.4 Rust Design Constraints

The implementation should follow the existing repo style and Rust best practices:
- prefer owned command inputs at the Tauri boundary
- avoid `unwrap()` and `expect()` outside tests
- return structured `Result` values and flatten to stable UI-facing errors
- borrow instead of clone where possible
- keep runtime state mutation explicit and localized

---

## 8. Node Configuration Shape

### 8.1 Start Node Published Config

`Start` published config should become the single source of flow-owned TikTok runtime configuration.

Persisted config should keep the repo's current snake_case convention so DB JSON, Rust, and frontend forms stay aligned.

Example shape:

```json
{
  "username": "shop_abc",
  "cookies_json": "{\"sessionid\":\"...\"}",
  "proxy_url": "http://127.0.0.1:9000",
  "poll_interval_seconds": 20,
  "watcher_mode": "live_polling",
  "retry_limit": 3,
  "last_live_at": null,
  "last_run_at": null,
  "last_error": null
}
```

Required fields should be minimal:
- `username`

Optional fields:
- `cookies_json`
- `proxy_url`
- polling and retry values

### 8.2 Record Node Published Config

`Record` published config should remain small and focused.

Persisted config should also stay snake_case in phase 1.

Example shape:

```json
{
  "max_duration_minutes": 120
}
```

`Record` config must not contain TikTok polling concerns such as cookies or proxy.

---

## 9. Runtime State Model

### 9.1 Flow Automation Session State

Each enabled flow gets one in-memory automation session.

The session keeps at least:
- `flow_id`
- `account_id`
- `username_normalized`
- `start_config`
- `record_config`
- `status`
- `last_check_at`
- `last_live_at`
- `last_error`
- `active_flow_run_id`
- `last_completed_room_id`

Suggested session statuses:
- `idle`
- `polling`
- `waiting_live`
- `starting_run`
- `run_active`
- `backoff`
- `error`
- `conflict`

### 9.2 Username Lease Registry

The runtime owns a global username lease registry keyed by normalized lowercase username.

Rules:
- one enabled flow may hold one username lease at a time
- another flow with the same username cannot start polling concurrently
- lease is acquired before polling begins
- lease is released when the flow is disabled, restarted, unpublished in a conflicting way, or the app shuts down

Conflicts should be explicit and user-visible.

---

## 10. Data Flow

### 10.1 Start Polling Path

1. Automation session loads published `Start` config
2. It validates config and acquires the username lease
3. It polls TikTok using that flow's own cookies and proxy
4. If the source is offline, the session waits until the next poll interval
5. If the source is live and a `stream_url` is available, the engine creates a `flow_run`

### 10.2 Start Output Payload

When live is detected, `Start` completes inside the newly created run and writes `output_json` that includes at least:
- `account_id`
- `username`
- `room_id`
- `stream_url`
- `viewer_count`
- `detected_at`

This payload becomes the input to `Record`.

### 10.3 Record Path

1. `Record` validates its config and input payload
2. It creates a recording worker with the resolved `stream_url`
3. It creates a Rust-owned external recording key using the existing `recordings.sidecar_recording_id` column
4. It inserts or updates a `recordings` row tied to `account_id`, `flow_id`, `flow_run_id`, and that external recording key
5. It emits runtime progress until the worker exits
6. On successful completion, it calls the existing sidecar clip-processing HTTP path using that same external recording key so downstream sidecar work can map back into SQLite unchanged
7. It finalizes its node run and lets the engine continue to `Clip`

### 10.4 Loop Back To Start

When the run finishes, the automation session releases run-local state but keeps the username lease.

The session then returns to `Start` polling for the next live session.

---

## 11. Database Model

### 11.1 Existing Tables Reused

Phase 1 should reuse the current durable workflow tables:
- `flows`
- `flow_nodes`
- `flow_runs`
- `flow_node_runs`
- `recordings`

This design does not require a new durable `automation_sessions` table in phase 1.

It also continues to use `recordings.sidecar_recording_id` as the durable external-processing key for sidecar clip and caption work, even though recording ownership has moved to Rust.

### 11.2 Flow Runs

`flow_runs` continue to represent one real workflow execution.

In the new model:
- no `flow_run` exists while the flow is only waiting for live
- a `flow_run` starts when `Start` detects recordable live state
- a `flow_run` ends after `Record -> Clip -> Caption -> Upload` completes or fails

### 11.3 Flow Node Runs

`flow_node_runs` should reflect only the concrete run, not the long idle wait.

For a successful live detection and recording path:
- `Start` gets a `completed` node run with the resolved payload
- `Record` gets a `running` then `completed` node run
- downstream nodes behave as they do today

### 11.4 Recordings Ownership

`recordings` should keep ownership tied to:
- `account_id`
- `flow_id`
- `flow_run_id`

Account binding rule:
- `Start.username` is the canonical source for resolving runtime ownership
- on session start or restart, the runtime looks up `accounts.username` case-insensitively
- if no account exists, the runtime auto-creates a `monitored` account row with that username before any `flow_run` or `recordings` row is created
- if the flow later publishes a different username, the next session restart resolves or creates a different account row for future runs only

`account_id` remains important for dashboarding and existing lookup patterns even though source runtime config now lives in `Start`.

Historical recordings keep their original `account_id` even if the flow later changes usernames.

---

## 12. Failure Handling

### 12.1 Start-Side Errors

Transient TikTok errors should not permanently fail the flow automation session.

Examples:
- temporary HTTP failure
- temporary webcast endpoint failure
- temporary room id extraction failure
- temporary missing `stream_url`

Behavior:
- record the error in runtime state
- emit runtime status for UI visibility
- retry on the next poll or after backoff

### 12.2 Configuration Errors

Stable configuration problems should put the automation session into a visible error or conflict state.

Examples:
- invalid `cookies_json`
- missing or invalid `username`
- username lease conflict

These should not silently retry forever.

### 12.3 Record-Side Errors

Recording failures should be mapped explicitly.

Examples:
- ffmpeg process launch failure
- immediate stream open failure
- write path failure

These failures should mark the `Record` node run and the `flow_run` failed, then return the automation session to `Start` after the failure path is finalized.

If sidecar clip processing could not be scheduled after a completed recording, the `Record` step is still considered complete, but the downstream handoff failure must be surfaced explicitly so the run can fail in the `Clip` stage rather than disappearing silently.

### 12.4 Live Ending Mid-Recording

If the livestream ends while recording is in progress and the produced file is still usable, `Record` should be treated as completed and the flow should continue.

This is not considered a flow failure.

### 12.5 User Stop Or Disable During Recording

If the user disables the flow or explicitly stops the runtime while `Record` is active:
- the runtime terminates the ffmpeg worker gracefully, then force-kills if needed using the same worker shutdown policy
- the current `recordings` row is updated with the final partial file information when a file exists
- the `Record` node run is marked `cancelled`
- the `flow_run` is marked `cancelled`
- downstream nodes do not run for that cancelled execution

After cancellation is finalized, the automation session either stops completely or restarts from `Start` depending on the user action that triggered the stop.

---

## 13. Runtime Events And UI

### 13.1 Runtime Visibility

The frontend should be able to inspect the long-running status of each enabled flow without relying on a Python WebSocket for this slice.

Useful runtime fields for UI:
- `status`
- `last_checked_at`
- `last_live_at`
- `last_error`
- `username_conflict`
- `active_flow_run_id`

### 13.2 Event Shape

Rust/Tauri should emit runtime events for:
- session status changes for `Start`
- run creation when live is detected
- recording start
- recording progress
- recording finish

Event names do not need to match the old Python WebSocket names exactly, but the semantic coverage should be preserved.

For downstream processing, the Rust runtime should continue passing the external recording key through the same sidecar-oriented contract currently keyed by `sidecar_recording_id`, so `insert_clip_from_sidecar` and `insert_speech_segment` can keep resolving `recordings.id`, `flow_id`, and `flow_run_id` from SQLite.

### 13.3 Editor Configuration

The flow editor should show:
- TikTok source configuration on `Start`
- recording-specific options on `Record`

The editor should not imply that cookies, proxy, or polling are shared across flows.

---

## 14. Migration And Rollout

### 14.1 Existing Flow Definitions

Existing flows should migrate toward:
- `Start` holding TikTok source and polling config
- `Record` holding only recording config

Any old assumptions that runtime source config lives on `accounts` should be treated as transitional compatibility only.

Config compatibility rule:
- phase 1 keeps snake_case persisted JSON for `draft_config_json` and `published_config_json`
- existing keys such as `cookies_json`, `proxy_url`, `poll_interval_seconds`, `retry_limit`, and `max_duration_minutes` remain canonical
- new fields added in this phase must also use snake_case
- frontend parsers and serializers should migrate additively, not by changing the canonical stored key names to camelCase

### 14.2 Existing Accounts

`accounts` remain in phase 1 for:
- output ownership
- dashboard views
- existing queries and joins
- auto-upsert target rows when a flow introduces a username that does not yet exist

They are no longer the runtime source of truth for TikTok polling behavior in this slice.

### 14.3 Sidecar Boundary After Phase 1

The Python sidecar remains responsible for the other domains such as clips, captions, products, and related processing.

For this phase, TikTok `Start` polling and `Record` runtime move to Rust.

---

## 15. Testing Strategy

### 15.1 Start Config Tests

Add Rust tests for:
- valid and invalid `Start` config parsing
- invalid `cookies_json`
- invalid poll interval values

### 15.2 TikTok Resolution Tests

Add Rust tests for:
- room id extraction patterns
- webcast `room/info` success path
- fallback `check_alive` path
- stream URL selection priority
- viewer count extraction

### 15.3 Username Lease Tests

Add Rust tests for:
- acquire success
- conflict on duplicate username
- release and reacquire

### 15.4 Recording Runtime Tests

Add Rust tests for:
- output path generation
- ffmpeg command generation
- worker status transitions
- final recording row ownership with `account_id + flow_run_id`

### 15.5 Workflow Integration Tests

Add workflow-level tests for:
- `Start` detects live and creates a `flow_run`
- `Start` writes the expected output payload
- `Record` consumes the payload and marks node run state correctly
- completed run returns the automation session to polling
- failed recording returns the automation session to polling after failure finalization

---

## 16. Explicit Behavior Change Summary

One behavior change must remain explicit across implementation and release notes:

- Python currently chains a new recording segment after `max_duration_seconds` if the host is still live
- the new Rust flow design does not chain segments
- one detected live session can therefore produce only one recording attempt per flow run

This change is intentional because the chosen flow model treats `Record` as a one-shot node and returns to `Start` only after the current run completes.
