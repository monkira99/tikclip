# Rust Live Ingress With Full Python Check-Live Port Design

**Date:** 2026-04-19
**Status:** Draft for review

---

## 1. Overview

This design closes the remaining gap in the Rust `Start/Record` migration by moving live detection ingress fully into `Rust/Tauri`.

Today, Rust owns the runtime session state machine, run creation, recording execution, and runtime diagnostics, but it still relies on Python sidecar `account_live` / `account_status` events to enter the `watching -> recording` transition.

That split leaves a real failure mode:
- the Rust runtime session can be healthy and sitting in `watching`
- the target account can actually be live
- but if sidecar live ingress does not broadcast or does not map back to the flow correctly, Rust never receives `live_detected`
- the flow never starts recording

This design removes that dependency.

The new ownership model is:
- Rust `Start` sessions poll TikTok directly per enabled flow
- Rust resolves `room_id`, live status, and `stream_url` directly
- Rust calls `handle_live_detected(...)` directly when a flow-owned source is live
- sidecar is no longer part of the live/account ingress path for the `Start/Record` slice

This is not a partial bugfix. It is the intended completion of the earlier `Start(check live) -> Record(record once)` migration.

---

## 2. Goals And Non-Goals

### Goals

- Remove Python sidecar from the live/account ingress path for `Start/Record`
- Make each enabled Rust runtime session poll its own TikTok source directly
- Port the full Python `check live` logic into Rust instead of a simplified approximation
- Preserve the existing Rust-owned run creation and recording execution path
- Preserve the current downstream handoff to clip/caption/product layers
- Make runtime diagnostics explain exactly why a session is still `watching`

### Non-Goals

- Migrating clip, caption, product suggestion, storage utilities, or downstream media processing out of sidecar in this phase
- Reintroducing a global watcher shared across all flows
- Changing the one-record-per-run semantics already chosen for the Rust `Record` node
- Removing manual/debug commands such as `trigger_start_live_detected` if they remain useful for tests or support

---

## 3. Chosen Direction

Three directions were considered:

1. Keep sidecar live ingress and only patch the current flow/account mapping bug
2. Make Rust the primary poller but keep sidecar live ingress running in parallel as a fallback
3. Move live ingress fully into Rust and remove sidecar from the `Start/Record` live/account domain

### Recommendation

Direction 3 is the chosen design.

Why:
- it matches the original `Start owns polling` spec instead of keeping a split brain
- it removes the exact architectural gap causing sessions to stay in `watching`
- it keeps boundaries clearer: Rust owns `Start/Record`, sidecar owns downstream domains only
- it avoids carrying two separate sources of truth for live status

---

## 4. Product Semantics

### 4.1 Start Node Owns Polling End-To-End

For enabled flows, `Start` now owns the full live-detection lifecycle:
- poll cadence
- TikTok cookies and proxy usage
- room resolution
- live confirmation
- stream URL resolution
- transition into `flow_run` creation

No sidecar event is required to start a run.

### 4.2 Record Node Semantics Stay The Same

This design does not change the already-shipped Rust `Record` semantics:
- one resolved live session creates one `flow_run`
- `Record` records one stream input once
- no max-duration segment chaining

### 4.3 Sidecar No Longer Owns Live/Account Ingress For This Slice

For the `Start/Record` slice, sidecar no longer:
- polls watched accounts for flow automation
- broadcasts `account_live` / `account_status` to drive `watching -> recording`
- acts as a required source of truth for Rust runtime live transitions

Sidecar may still exist in the application for other domains, but not as part of this slice's ingress.

---

## 5. Full Python Check-Live Logic To Port

This phase must carry over the full Python live-resolution behavior from:
- `sidecar/src/core/watcher.py`
- `sidecar/src/tiktok/api.py`
- `sidecar/src/tiktok/stream.py`
- `sidecar/src/tiktok/cookies.py`
- `sidecar/src/tiktok/http_transport.py`

The Rust implementation must not replace this with a reduced or “good enough” version.

### 5.1 Cookie Handling

Rust must port the same cookie behavior as Python:
- parse `cookies_json` as a JSON object
- reject invalid shapes where appropriate
- normalize common aliases such as `sessionid_ss -> sessionid`
- keep the safe cookie-key summary pattern for diagnostics instead of logging raw values

### 5.2 Proxy-Aware Transport

Rust must keep proxy-aware HTTP transport semantics:
- validate proxy URL shape
- apply proxy to page fetches and webcast requests
- preserve request headers needed for TikTok web behavior
- keep timeout and follow-redirect behavior aligned with the current Python logic

### 5.3 Room Resolution

Rust must port the room-id resolution chain in full:
- fetch `https://www.tiktok.com/@{username}/live`
- extract `room_id` from HTML using the same known patterns
- preserve the optional signed room-id path if that feature is enabled in settings
- preserve debug/save-body behavior for blocked or WAF-style pages where useful

### 5.4 Webcast Room Info And Fallback

Rust must port the two-step confirmation path:
- primary: `webcast/room/info` with `aid=1988`
- fallback: `webcast/room/check_alive` when `room/info` fails
- preserve the region hint logic derived from cookies such as `tt-target-idc`
- preserve the behavior where `check_alive=false` may still be treated as live if the merged payload status is authoritative enough

### 5.5 Stream URL And Viewer Count Extraction

Rust must port the full stream selection semantics:
- prefer FLV/HLS by quality order `FULL_HD1 -> HD1 -> SD1 -> SD2`
- accept fallback map iteration when preferred keys are missing
- accept raw string fields when map-shaped payloads are missing
- extract viewer count and title from the merged room payload where available

### 5.6 Error Classification

Rust must preserve Python's practical classification of results:
- hard network / HTTP / malformed JSON paths should not incorrectly create a run
- many failures should degrade to `watching` with diagnostics, not crash the runtime
- debug logs should distinguish:
  - page fetch failure
  - room-id missing
  - webcast info failure
  - fallback check_alive path used
  - live but missing stream URL

---

## 6. Runtime Flow After This Change

### 6.1 Poll Loop Ownership

Each enabled flow session in `LiveRuntimeManager` owns its own background polling loop.

Each loop:
- loads the published `Start` config for that flow
- sleeps according to `poll_interval_seconds`
- polls TikTok directly using that flow's cookies/proxy settings
- evaluates the `LiveStatus`
- calls `handle_live_detected(...)` directly when a recordable live session is found

This preserves the per-flow source model and avoids a shared global watcher.

### 6.2 Watching State

`watching` means:
- the runtime session exists
- the username lease is held
- the poll loop is healthy enough to continue
- no active `flow_run` is currently recording for that flow

Temporary poll failures do not automatically force `error`.

### 6.3 Offline And Dedupe Reset

When polling observes that the source is no longer live:
- the session remains or returns to `watching`
- the completed-room dedupe memory may be reset for the next on-air cycle
- runtime diagnostics emit an offline/reset event only when the state meaningfully changes

### 6.4 Transition To Recording

When polling returns a live status with a valid `room_id` and `stream_url`:
- Rust logs `live_detected`
- Rust applies the existing dedupe checks
- Rust creates a `flow_run`
- Rust starts the recording worker
- Rust updates runtime state to `recording`

No sidecar websocket event participates in this transition.

---

## 7. Failure Semantics

### 7.1 Poll Errors

Most polling failures are recoverable.

Examples:
- transient network failure
- TikTok page fetch timeout
- webcast endpoint returning a temporary HTTP error
- fallback `check_alive` returning unusable data

For these paths:
- keep the session in `watching`
- emit runtime diagnostics with enough context to debug the failure
- retry on the next poll interval

### 7.2 Runtime Errors

The flow should move into a runtime `error` state only for non-recoverable conditions such as:
- invalid or missing normalized username
- invalid `cookies_json` configuration when that makes polling impossible
- invalid proxy configuration
- duplicate username lease conflict
- session bootstrap failure that prevents the loop from starting

### 7.3 Live Without Stream URL

If the source is live but no reliable `stream_url` is resolved:
- keep the session in `watching`
- emit `stream_url_missing`
- do not create a `flow_run`

This matches the earlier Rust design and prevents empty or broken recordings.

### 7.4 Record Failures Stay In Record Domain

If a run was created and the recording worker fails afterward:
- that remains a `record`-stage runtime failure
- it does not mean the polling loop itself is broken

The poll loop should still be able to return to `watching` after the run is finalized, subject to current lifecycle rules.

---

## 8. Architecture And File Boundaries

### 8.1 `src-tauri/src/tiktok/*`

This module becomes the Rust home of the full Python check-live stack.

It should own:
- cookie normalization helpers
- proxy-aware HTTP client setup
- live page fetch and HTML room-id extraction
- optional signed room-id path support
- webcast `room/info` fetch
- `check_alive` fallback
- stream URL selection
- viewer count and title extraction
- typed live-resolution result structs

### 8.2 `src-tauri/src/live_runtime/manager.rs`

`LiveRuntimeManager` should own:
- per-flow poll loop start/stop
- session cancellation handles
- poll scheduling and retry cadence
- direct calls into the Rust TikTok client
- direct transition into `handle_live_detected(...)`
- offline reset handling
- runtime diagnostics for polling behavior

### 8.3 `src/components/layout/app-shell.tsx`

`AppShell` must stop acting as a live-ingress orchestrator for this slice.

It should no longer map sidecar live/account events into:
- `triggerStartLiveDetected(...)`
- `markSourceOffline(...)`

It remains responsible for:
- consuming runtime snapshots and logs
- sidecar-backed downstream domains unrelated to `Start/Record`

### 8.4 Sidecar Boundary After This Change

Sidecar remains in the app for other domains, but for this slice it is not on the critical path for:
- detecting live status
- confirming on-air state
- choosing the stream URL for record start
- starting a `flow_run`

---

## 9. Rollout Constraints

- Do not change the already-shipped Rust `Record` one-shot semantics
- Do not migrate clip/caption/product ownership in this phase
- Keep existing downstream handoff behavior working
- Avoid long-lived dual-ingress behavior where both sidecar and Rust try to drive `watching -> recording`
- Keep manual/debug commands if useful, but do not use them as the production ingress path
- Ensure startup, enable/disable, publish, and shutdown all cleanly start/stop the per-flow poll loops

---

## 10. Verification Requirements

Implementation must prove all of the following:

- an enabled flow can bootstrap and autonomously poll without sidecar live/account ingress
- a live source with `room_id + stream_url` creates a run and enters `recording`
- a live source without `stream_url` stays in `watching` and logs `stream_url_missing`
- an offline transition resets dedupe state correctly
- disabling or republishing a flow stops the old poll loop and starts the new one without duplicate sessions
- no secrets appear in runtime logs, terminal output, UI log buffers, or copied diagnostic bundles
- frontend runtime state stays correct after removing sidecar live/account ingress wiring for this slice

---

## 11. Risks And Mitigations

### Risk: Poll loop races or duplicate sessions

Mitigation:
- keep lifecycle ownership centralized in `LiveRuntimeManager`
- reuse the existing lease/session discipline already established for Rust runtime sessions

### Risk: Python and Rust live-resolution logic drift

Mitigation:
- port from Python source modules directly, not from memory
- preserve the fallback chain explicitly in tests and docs

### Risk: Overly noisy diagnostics

Mitigation:
- keep milestone events at `info`
- use `debug` for lower-level fetch/fallback details where needed
- preserve secret redaction at the canonical log entry boundary

### Risk: Removing sidecar ingress breaks downstream assumptions

Mitigation:
- keep downstream sidecar paths untouched in this phase
- only remove the `Start/Record` live/account ingress coupling from `AppShell`

---

## 12. Decision Summary

This change completes the original Rust migration intent:
- `Start` owns live detection fully
- `Record` owns one recording execution fully
- sidecar no longer decides when a Rust flow may start recording

The key implementation rule is explicit:

**When moving live ingress into Rust, port the full Python check-live logic. Do not ship a reduced or approximate version.**
