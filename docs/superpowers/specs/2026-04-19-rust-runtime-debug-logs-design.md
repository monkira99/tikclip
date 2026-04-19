# Rust Runtime Debug Logs Design

**Date:** 2026-04-19
**Status:** Draft for review

---

## 1. Overview

This design adds a dedicated debug log surface for the new Rust-owned `Start/Record` runtime so that runtime failures can be diagnosed from either:

- terminal output during development and desktop runs
- an in-app debug log panel inside the flow UI

The primary product requirement is diagnostic portability:

- a user can copy the recent runtime logs from the app
- paste them into chat
- the assistant can determine where the failure happened and what the next debugging step should be

The design therefore prioritizes structured, copy-friendly diagnostic logs over generic unstructured logging.

---

## 2. Goals And Non-Goals

### Goals

- Show Rust runtime debug logs in both terminal output and the desktop UI
- Make copied logs self-describing enough to diagnose the failing stage without needing the user to summarize manually
- Preserve one canonical event shape across Rust terminal logs, Tauri events, and UI rendering
- Avoid logging secrets such as raw cookies or proxy credentials
- Keep the first version lightweight enough for local development and internal debug builds

### Non-Goals

- Building a long-term audit log or compliance log store in phase 1
- Persisting all debug logs in SQLite as first step
- Replacing existing flow runtime summary fields such as `status`, `current_node`, or `last_error`
- Building a full observability system with retention, search, or remote shipping in this phase

---

## 3. Chosen Direction

Three directions were considered:

1. Terminal-only logs through `env_logger`
2. Persist all logs into SQLite or a file and have the UI read from persistence
3. A structured runtime log bus with terminal output, in-memory ring buffer, Tauri event emission, and UI viewer

### Recommendation

Direction 3 is the chosen design.

It gives the best balance for the current app state:

- terminal visibility for developers
- real-time in-app visibility for desktop debugging
- copyable structured diagnostics for assistant support
- minimal extra storage complexity compared to file or SQLite persistence

The first version should therefore use an in-memory ring buffer as the UI source of truth and keep terminal logging in parallel from the same event source.

---

## 4. Product Requirement

The most important requirement is not merely “show logs”, but “make them diagnosable after copy/paste”.

The copied log bundle must let a reader infer:

- which flow failed
- which run failed
- which stage was active
- what the last successful step was
- which transition failed next
- whether the problem belongs to TikTok fetch, Rust runtime state, recording execution, or sidecar handoff

This means every log entry must carry both:

1. a compact human-readable summary line
2. a structured context payload for machine-like inspection by humans or assistants

---

## 5. Log Entry Model

### 5.1 Canonical Runtime Log Entry

Rust should define a single canonical type such as `FlowRuntimeLogEntry` with at least these fields:

- `id`
- `timestamp`
- `level`
- `flow_id`
- `flow_run_id`
- `external_recording_id`
- `stage`
- `event`
- `code`
- `message`
- `context`

Suggested shape:

```json
{
  "id": "log-018f...",
  "timestamp": "2026-04-19T09:41:12.381+07:00",
  "level": "info",
  "flow_id": 7,
  "flow_run_id": 42,
  "external_recording_id": "rec-42-a1b2",
  "stage": "record",
  "event": "record_spawned",
  "code": null,
  "message": "Spawned Rust-owned recording worker",
  "context": {
    "room_id": "7312345",
    "output_path": ".../42-rec-42-a1b2.mp4"
  }
}
```

### 5.2 Required Top-Level Fields

Every entry must include:

- `timestamp`
- `level`
- `flow_id`
- `stage`
- `event`
- `message`

These fields are required because they are the minimum needed for meaningful copy/paste diagnostics.

### 5.3 Optional Correlation Fields

These should be present whenever available:

- `flow_run_id`
- `external_recording_id`
- `account_id`
- `room_id`
- `current_node`

These are what allow fast correlation across Start, Record, Clip, and Caption transitions.

---

## 6. Display Format

### 6.1 Terminal Format

Terminal output should use a stable, compact summary line:

```text
[09:41:12.381] INFO flow=7 run=42 stage=record event=record_spawned code=-
Spawned Rust-owned recording worker
context: {"room_id":"7312345","output_path":".../42-rec-42-a1b2.mp4"}
```

This format is optimized for human scanning while still preserving structure.

### 6.2 UI Format

The UI should render the same entry using:

- summary header line
- message body
- expandable JSON context block

This must preserve the same information as the terminal representation so copying from UI or terminal gives equivalent debugging value.

### 6.3 Copy Bundle Format

The in-app debug panel should support `Copy diagnostic bundle` that produces a block like:

```text
=== TikClip Flow Runtime Diagnostic ===
flow_id: 7
flow_name: Auto record shop_abc
current_status: processing
current_node: clip
active_flow_run_id: 42
username: shop_abc
last_live_at: 2026-04-19 09:41:12
last_error: handoff.sidecar_unavailable

--- Recent Logs ---
[09:41:12.381] INFO flow=7 run=42 stage=start event=live_detected code=-
Resolved live room and stream URL
context: {"room_id":"7312345","stream_url_present":true,"viewer_count":77}

[09:41:12.505] INFO flow=7 run=42 stage=record event=record_spawned code=-
Spawned Rust-owned recording worker
context: {"external_recording_id":"rec-42-a1b2","output_path":".../42-rec-42-a1b2.mp4"}

[09:43:33.041] ERROR flow=7 run=42 stage=clip event=sidecar_handoff_failed code=handoff.sidecar_unavailable
Failed to call sidecar /api/video/process
context: {"sidecar_url_present":false}
```

This bundle is the main support/debug artifact for assistant-assisted diagnosis.

---

## 7. Event Taxonomy

Runtime events should use a small, stable taxonomy.

### 7.1 Session Events

- `session_bootstrap_started`
- `session_bootstrap_failed`
- `session_started`
- `session_stopped`
- `session_reconciled`
- `lease_acquired`
- `lease_conflict`
- `source_offline_marked`

### 7.2 Start / Live Detection Events

- `poll_started`
- `poll_skipped_active_run`
- `live_detected`
- `live_not_detected`
- `room_id_missing`
- `stream_url_missing`
- `run_creation_started`
- `run_created`
- `run_creation_skipped_dedupe`
- `run_creation_skipped_missing_stream`

### 7.3 Record Events

- `record_prepare_started`
- `record_row_created`
- `record_spawn_started`
- `record_spawned`
- `record_progress`
- `record_completed`
- `record_failed`
- `record_cancelled`
- `record_finalize_started`
- `record_finalize_completed`

### 7.4 Downstream Events

- `sidecar_handoff_started`
- `sidecar_handoff_completed`
- `sidecar_handoff_failed`
- `clip_ready_received`
- `caption_ready_received`

### 7.5 Integrity Events

- `db_state_restored`
- `active_run_restored`
- `runtime_state_mismatch`
- `rollback_applied`

These event names should be treated as contract-like identifiers for diagnostics.

---

## 8. Error Codes

Warnings and errors should include stable `code` values.

### 8.1 Start Codes

- `start.invalid_config`
- `start.username_conflict`
- `start.account_lookup_failed`
- `start.room_id_missing`
- `start.stream_url_missing`
- `start.tiktok_room_info_failed`
- `start.tiktok_check_alive_failed`

### 8.2 Record Codes

- `record.spawn_failed`
- `record.output_path_failed`
- `record.ffmpeg_exit_error`
- `record.cancel_requested`
- `record.finalize_failed`

### 8.3 Handoff Codes

- `handoff.sidecar_unavailable`
- `handoff.http_failed`
- `handoff.clip_process_rejected`

### 8.4 Runtime Codes

- `runtime.db_locked`
- `runtime.state_mismatch`
- `runtime.session_missing`
- `runtime.rollback_failed`

These codes should be stable enough that a pasted log can be triaged immediately.

---

## 9. Secrets And Safe Logging

The runtime must not log secrets.

Never log:

- raw `cookies_json`
- full proxy credentials
- raw session tokens
- full auth-bearing sidecar headers if they exist later

Instead log boolean or redacted metadata such as:

- `cookies_present: true`
- `proxy_configured: true`
- `proxy_host: "127.0.0.1:9000"` only when safe

This rule applies to terminal logs, UI logs, copied bundles, and test snapshots.

---

## 10. Rust Architecture

### 10.1 Shared Log Helper

`LiveRuntimeManager` should expose a common helper such as `log_runtime_event(...)` that:

1. constructs a `FlowRuntimeLogEntry`
2. writes a terminal log line via `log::info!`, `log::warn!`, or `log::error!`
3. appends the entry to an in-memory ring buffer
4. emits a Tauri event to the frontend

This ensures all three sinks share the same event payload and correlation fields.

### 10.2 Ring Buffer

The manager should maintain an in-memory ring buffer, for example `VecDeque<FlowRuntimeLogEntry>`, capped to a small size such as `500` or `1000` entries.

The first version should keep logs only in memory for the current app session.

### 10.3 Tauri Event

Rust should emit an event such as `flow-runtime-log` whenever a new entry is appended.

This event is separate from the existing `flow-runtime-updated` summary event:

- `flow-runtime-updated` = summary/runtime state snapshot
- `flow-runtime-log` = detailed diagnostic entry

### 10.4 Tauri Commands

Add a command such as:

- `list_flow_runtime_logs(flow_id?: i64, limit?: usize)`

Optional later:

- `clear_flow_runtime_logs(flow_id?: i64)`

The first version only needs read access for UI hydration.

---

## 11. Frontend Architecture

### 11.1 Store Model

Frontend should keep runtime logs separately from summary runtime snapshots.

Suggested store shape:

- `logsByFlowId`
- `appendRuntimeLog(entry)`
- `hydrateRuntimeLogs(entries)`

This avoids overloading the existing `runtimeSnapshots` summary model.

### 11.2 UI Placement

The primary surface should be inside `FlowDetail` as a `Runtime Logs` panel or tab.

The panel should support:

- filtering to the current flow
- showing newest-first or grouped by time
- copying visible logs
- copying the full diagnostic bundle

### 11.3 Summary Surface

The flow detail header or runtime card may show a short last-error summary:

- last event
- last error code
- timestamp

This is not a replacement for the log panel; it is only a quick-entry affordance.

---

## 12. Data Flow

The end-to-end data flow should be:

1. Rust runtime reaches a significant state transition
2. `log_runtime_event(...)` builds a structured entry
3. entry is written to terminal log output
4. entry is appended to the in-memory ring buffer
5. Rust emits `flow-runtime-log`
6. Frontend store appends the entry for the relevant flow
7. UI panel updates in real time
8. User can copy the diagnostic bundle at any time

---

## 13. First Emission Points

The first rollout should emit logs at these points only:

1. session/bootstrap start and failure
2. lease acquisition and conflict
3. live detect result
4. run created or skipped
5. record worker spawn
6. record success, error, or cancel
7. sidecar handoff success or failure
8. offline reset / dedupe reset

This covers the main runtime diagnosis path without generating excessive noise.

---

## 14. Dev And Build Scope

This feature is intended for:

- local development
- debug and internal app builds

The first version should therefore:

- always log to terminal in dev
- always expose the in-app panel in the app builds where the new Rust runtime exists
- not require a production-grade persistence solution

If later needed, file-based or SQLite-backed persistence can be added as a phase 2 sink behind the same event model.

---

## 15. Testing Strategy

### 15.1 Rust Tests

Add tests for:

- ring buffer append and cap behavior
- `flow-runtime-log` emission path when manager has an attached `AppHandle`
- safe redaction behavior for cookie/proxy-bearing events
- diagnostic entry formatting and copy-bundle serialization

### 15.2 Frontend Tests

Add tests for:

- log store hydration and append behavior
- `FlowDetail` log panel rendering grouped entries correctly
- copy bundle contains expected summary and recent logs
- current node / stage view does not regress when logs and runtime snapshots update together

### 15.3 Manual Verification

Manual validation should cover:

1. Start a flow in debug mode
2. Observe terminal logs during bootstrap and live detection
3. Observe the same events in the app log panel
4. Trigger one failure path such as missing sidecar handoff
5. Copy the diagnostic bundle
6. Confirm the copied artifact contains enough information to locate the failure stage

---

## 16. Rollout Plan

### Phase 1

- add structured runtime log entry type
- add manager log helper + in-memory ring buffer
- add terminal output from the same helper
- add Tauri event and read command
- add frontend log panel and copy bundle

### Phase 2

Optional later:

- file-backed persistence
- SQLite-backed persistence
- filtering by level and stage
- export as attachment or support bundle

---

## 17. Summary

The chosen design is a structured Rust runtime log bus with:

- terminal output for developers
- in-app log visibility for desktop debugging
- copyable diagnostic bundles for assistant-supported troubleshooting

Its core promise is simple:

- if a user copies the recent runtime logs and pastes them into chat, the failure location and likely root cause should be inferable directly from the log bundle without requiring a separate narrative.
