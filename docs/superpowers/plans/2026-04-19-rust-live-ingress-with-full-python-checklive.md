# Rust Live Ingress With Full Python Check-Live Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove sidecar from the `Start/Record` live ingress path by making each enabled Rust runtime session poll TikTok directly with the full Python check-live logic, then transition into the existing Rust record path without frontend orchestration.

**Architecture:** Extend `src-tauri/src/tiktok/*` from parser-only helpers into a full live-resolution client that ports the Python watcher/API behavior, then add per-flow polling tasks inside `LiveRuntimeManager` that call `handle_live_detected(...)` and `mark_source_offline(...)` directly. Remove the sidecar-driven live/account ingress glue from `AppShell`, while keeping downstream sidecar clip/caption/product integration intact.

**Tech Stack:** Rust, Tauri v2, reqwest, rusqlite, serde/serde_json, std::thread + cancellation flags, React, Zustand, existing Python sidecar kept only for downstream domains

---

## File Structure

**Create:**
- `src-tauri/src/tiktok/http_transport.rs` - proxy-aware TikTok HTTP transport, shared headers, typed HTTP errors
- `src-tauri/src/tiktok/check_live.rs` - full Rust live-resolution pipeline ported from Python (`/@user/live` -> room id -> room/info -> check_alive fallback -> `LiveStatus`)
- `src-tauri/src/tiktok/check_live.test.rs` is **not** created; tests stay colocated in the Rust modules they validate

**Modify:**
- `src-tauri/src/tiktok/mod.rs` - export new TikTok transport/check-live modules
- `src-tauri/src/tiktok/types.rs` - extend typed live/check result structs if needed for richer runtime diagnostics
- `src-tauri/src/tiktok/client.rs` - shrink to shared parsing helpers or move logic into the new module while preserving tested public surface as needed
- `src-tauri/src/live_runtime/manager.rs` - per-flow polling task lifecycle, direct Rust-only live ingress, offline transitions, diagnostics
- `src-tauri/src/live_runtime/session.rs` - session/task state helpers if polling-task state needs explicit modeling
- `src-tauri/src/lib.rs` - no semantic change expected beyond using the updated manager, but adjust imports if module split requires it
- `src/components/layout/app-shell.tsx` - remove sidecar-driven `triggerStartLiveDetected` / `markSourceOffline` ingress for the `Start/Record` slice
- `src/lib/api.ts` - remove or clearly demote sidecar live-ingress helpers that are no longer part of the production path for this slice
- `src/stores/flow-store.ts` - only if needed to keep runtime snapshots/logs coherent after the frontend ingress removal
- `docs/superpowers/specs/2026-04-19-rust-live-ingress-with-full-python-checklive-design.md` - update status/wording only if implementation reveals a spec contradiction

**Test:**
- `src-tauri/src/tiktok/http_transport.rs`
- `src-tauri/src/tiktok/check_live.rs`
- `src-tauri/src/live_runtime/manager.rs`
- `src/components/layout/app-shell.tsx` via a small colocated frontend test only if removing ingress glue needs regression protection

---

### Task 1: Build Rust TikTok HTTP Transport And Cookie Normalization Parity

**Files:**
- Create: `src-tauri/src/tiktok/http_transport.rs`
- Modify: `src-tauri/src/tiktok/mod.rs`
- Modify: `src-tauri/src/tiktok/client.rs`
- Test: `src-tauri/src/tiktok/http_transport.rs`

- [ ] **Step 1: Write the failing Rust tests for cookie alias normalization, cookie summaries, proxy validation, and typed HTTP error shape**

```rust
#[cfg(test)]
mod tests {
    use super::{
        cookie_key_summary, normalize_tiktok_cookies, proxy_url_for_reqwest,
        TikTokHttpStatusError,
    };
    use serde_json::json;

    #[test]
    fn normalize_tiktok_cookies_copies_sessionid_from_sessionid_ss() {
        let cookies = normalize_tiktok_cookies(Some(&json!({
            "sessionid_ss": "abc",
            "tt-target-idc": "useast2a",
        })))
        .expect("normalize cookies");

        assert_eq!(cookies.get("sessionid").map(String::as_str), Some("abc"));
        assert_eq!(cookies.get("sessionid_ss").map(String::as_str), Some("abc"));
        assert_eq!(cookies.get("tt-target-idc").map(String::as_str), Some("useast2a"));
    }

    #[test]
    fn cookie_key_summary_lists_sorted_keys_without_values() {
        let summary = cookie_key_summary(Some(&json!({
            "sessionid": "secret",
            "tt-target-idc": "useast2a",
        })))
        .expect("cookie summary");

        assert_eq!(summary, "sessionid,tt-target-idc");
        assert!(!summary.contains("secret"));
    }

    #[test]
    fn proxy_url_for_reqwest_rejects_non_http_proxy_urls() {
        let err = proxy_url_for_reqwest(Some("socks5://127.0.0.1:9000")).unwrap_err();
        assert!(err.contains("proxy_url must start with http:// or https://"));
    }

    #[test]
    fn tik_tok_http_status_error_keeps_status_url_and_body_preview() {
        let err = TikTokHttpStatusError::new(
            403,
            "https://www.tiktok.com/@shop/live".to_string(),
            "access denied body".to_string(),
        );

        assert_eq!(err.status_code, 403);
        assert!(err.url.contains("tiktok.com"));
        assert!(err.text.contains("access denied"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test tiktok::http_transport -- --nocapture`
Expected: FAIL with unresolved module/functions such as `normalize_tiktok_cookies`, `cookie_key_summary`, or `TikTokHttpStatusError`

- [ ] **Step 3: Write the minimal transport module and exports**

```rust
// src-tauri/src/tiktok/http_transport.rs
use reqwest::{header, Client, Proxy};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct TikTokHttpStatusError {
    pub status_code: u16,
    pub url: String,
    pub text: String,
}

impl TikTokHttpStatusError {
    pub fn new(status_code: u16, url: String, text: String) -> Self {
        Self { status_code, url, text }
    }
}

pub fn normalize_tiktok_cookies(value: Option<&Value>) -> Result<BTreeMap<String, String>, String> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| "cookies_json must be a JSON object".to_string())?;

    let mut cookies = BTreeMap::new();
    for (key, value) in object {
        if value.is_null() {
            continue;
        }
        cookies.insert(key.clone(), value.as_str().unwrap_or_default().to_string());
    }
    if !cookies.contains_key("sessionid") {
        if let Some(sessionid_ss) = cookies.get("sessionid_ss").cloned() {
            cookies.insert("sessionid".to_string(), sessionid_ss);
        }
    }
    Ok(cookies)
}

pub fn cookie_key_summary(value: Option<&Value>) -> Result<String, String> {
    let cookies = normalize_tiktok_cookies(value)?;
    if cookies.is_empty() {
        return Ok("no cookies".to_string());
    }
    Ok(cookies.keys().cloned().collect::<Vec<_>>().join(","))
}

pub fn proxy_url_for_reqwest(raw: Option<&str>) -> Result<Option<String>, String> {
    let trimmed = raw.unwrap_or_default().trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err("proxy_url must start with http:// or https://".to_string());
    }
    Proxy::all(trimmed).map_err(|e| e.to_string())?;
    Ok(Some(trimmed.to_string()))
}

pub fn build_tiktok_http_client(cookies: Option<&Value>, proxy_url: Option<&str>) -> Result<Client, String> {
    let normalized = normalize_tiktok_cookies(cookies)?;
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        ),
    );
    headers.insert(
        header::REFERER,
        header::HeaderValue::from_static("https://www.tiktok.com/"),
    );
    if !normalized.is_empty() {
        let cookie_header = normalized
            .into_iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("; ");
        headers.insert(
            header::COOKIE,
            header::HeaderValue::from_str(&cookie_header).map_err(|e| e.to_string())?,
        );
    }

    let mut builder = Client::builder().default_headers(headers).timeout(std::time::Duration::from_secs(15));
    if let Some(proxy) = proxy_url_for_reqwest(proxy_url)? {
        builder = builder.proxy(Proxy::all(proxy).map_err(|e| e.to_string())?);
    }
    builder.build().map_err(|e| e.to_string())
}
```

```rust
// src-tauri/src/tiktok/mod.rs
pub mod client;
pub mod http_transport;
pub mod types;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test tiktok::http_transport -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/tiktok/http_transport.rs src-tauri/src/tiktok/mod.rs src-tauri/src/tiktok/client.rs
git commit -m "feat(runtime): add TikTok HTTP transport parity"
```

### Task 2: Port Full Python Check-Live Resolution Into Rust TikTok Module

**Files:**
- Create: `src-tauri/src/tiktok/check_live.rs`
- Modify: `src-tauri/src/tiktok/mod.rs`
- Modify: `src-tauri/src/tiktok/types.rs`
- Modify: `src-tauri/src/tiktok/client.rs`
- Test: `src-tauri/src/tiktok/check_live.rs`

- [ ] **Step 1: Write the failing tests for HTML room-id extraction, merged webcast fallback semantics, stream selection, and region hint logic**

```rust
#[cfg(test)]
mod tests {
    use super::{
        extract_room_id_from_live_html, merge_live_status_from_room_payload,
        pick_stream_url_from_room_payload, webcast_region_hint,
    };
    use serde_json::json;

    #[test]
    fn extract_room_id_from_live_html_matches_python_patterns() {
        let html = r#"...\"roomId\":\"7312345\"..."#;
        assert_eq!(extract_room_id_from_live_html(html), Some("7312345".to_string()));
    }

    #[test]
    fn webcast_region_hint_prefers_target_idc_rules() {
        let cookies = json!({"tt-target-idc": "useast2a"});
        assert_eq!(webcast_region_hint(Some(&cookies)), "US");
    }

    #[test]
    fn pick_stream_url_from_room_payload_prefers_full_hd_then_hls_then_raw() {
        let payload = json!({
            "stream_url": {
                "flv_pull_url": {
                    "SD1": "https://example.com/sd.flv",
                    "FULL_HD1": "https://example.com/fullhd.flv"
                },
                "hls_pull_url_map": {
                    "HD1": "https://example.com/hd.m3u8"
                }
            }
        });
        assert_eq!(
            pick_stream_url_from_room_payload(&payload).as_deref(),
            Some("https://example.com/fullhd.flv")
        );
    }

    #[test]
    fn merge_live_status_from_room_payload_treats_check_alive_false_as_live_when_status_hint_is_two() {
        let payload = json!({
            "room_id": "7312345",
            "status": 2,
            "stream_url": {
                "flv_pull_url": {
                    "HD1": "https://example.com/live.flv"
                }
            }
        });

        let live = merge_live_status_from_room_payload(&payload, false).expect("merge payload");
        assert!(live.is_live);
        assert_eq!(live.room_id.as_deref(), Some("7312345"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test tiktok::check_live -- --nocapture`
Expected: FAIL with unresolved imports/functions for the new check-live helpers

- [ ] **Step 3: Implement the minimal full-check-live helper surface**

```rust
// src-tauri/src/tiktok/check_live.rs
use crate::tiktok::types::LiveStatus;
use serde_json::Value;

const STREAM_QUALITY_ORDER: &[&str] = &["FULL_HD1", "HD1", "SD1", "SD2"];

pub fn extract_room_id_from_live_html(html: &str) -> Option<String> {
    for prefix in [
        "\"roomId\":\"",
        "\"room_id\":\"",
        "\"room_id\":",
        "room_id=",
        "roomId=",
        "\"id_str\":\"",
        "room/",
        "\"web_rid\":\"",
    ] {
        if let Some(index) = html.find(prefix) {
            let suffix = &html[index + prefix.len()..];
            let digits: String = suffix.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.len() >= 5 {
                return Some(digits);
            }
        }
    }
    None
}

pub fn webcast_region_hint(cookies: Option<&Value>) -> &'static str {
    let idc = cookies
        .and_then(Value::as_object)
        .and_then(|object| object.get("tt-target-idc"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if idc.contains("alisg") {
        "SG"
    } else if idc.contains("useast") {
        "US"
    } else if idc.contains("eu") || idc.contains("gcp") {
        "EU"
    } else {
        "CH"
    }
}

pub fn pick_stream_url_from_room_payload(room: &Value) -> Option<String> {
    let stream_url = room.get("stream_url")?;
    for field in ["flv_pull_url", "hls_pull_url_map", "hls_pull_url"] {
        if let Some(map) = stream_url.get(field).and_then(Value::as_object) {
            for quality in STREAM_QUALITY_ORDER {
                if let Some(url) = map.get(*quality).and_then(Value::as_str) {
                    if !url.is_empty() {
                        return Some(url.to_string());
                    }
                }
            }
            if let Some(url) = map.values().find_map(Value::as_str) {
                if !url.is_empty() {
                    return Some(url.to_string());
                }
            }
        }
    }
    for field in ["flv_pull_url", "hls_pull_url"] {
        if let Some(url) = stream_url.get(field).and_then(Value::as_str) {
            if !url.is_empty() {
                return Some(url.to_string());
            }
        }
    }
    None
}

pub fn merge_live_status_from_room_payload(room: &Value, check_alive_live: bool) -> Result<LiveStatus, String> {
    let status_hint = room
        .get("LiveRoomInfo")
        .and_then(|value| value.get("status"))
        .and_then(Value::as_i64)
        .or_else(|| room.get("status").and_then(Value::as_i64));
    let room_id = room
        .get("room_id")
        .and_then(Value::as_str)
        .or_else(|| room.get("id_str").and_then(Value::as_str))
        .or_else(|| room.get("web_rid").and_then(Value::as_str))
        .map(str::to_string);
    let is_live = check_alive_live || status_hint == Some(2);
    let viewer_count = room
        .get("LiveRoomInfo")
        .and_then(|value| value.get("liveRoomStats"))
        .and_then(|value| value.get("userCount"))
        .and_then(Value::as_i64)
        .or_else(|| room.get("owner_count").and_then(Value::as_i64))
        .or_else(|| room.get("user_count").and_then(Value::as_i64))
        .or_else(|| room.get("viewer_count").and_then(Value::as_i64));
    let title = room
        .get("LiveRoomInfo")
        .and_then(|value| value.get("title"))
        .and_then(Value::as_str)
        .or_else(|| room.get("title").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    Ok(LiveStatus {
        room_id: room_id.unwrap_or_default(),
        stream_url: if is_live {
            pick_stream_url_from_room_payload(room)
        } else {
            None
        },
        viewer_count,
        is_live,
        title,
    })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test tiktok::check_live -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/tiktok/check_live.rs src-tauri/src/tiktok/mod.rs src-tauri/src/tiktok/types.rs src-tauri/src/tiktok/client.rs
git commit -m "feat(runtime): port TikTok check-live resolution"
```

### Task 3: Add Per-Flow Polling Tasks To LiveRuntimeManager

**Files:**
- Modify: `src-tauri/src/live_runtime/manager.rs`
- Modify: `src-tauri/src/live_runtime/session.rs`
- Test: `src-tauri/src/live_runtime/manager.rs`
- Test: `src-tauri/src/live_runtime/session.rs`

- [ ] **Step 1: Write the failing tests for session-owned polling lifecycle and stale-task cancellation**

```rust
#[test]
fn start_flow_session_spawns_one_polling_task_for_enabled_flow() {
    let (conn, _path) = open_runtime_db("poll-start");
    seed_enabled_start_record_flow(&conn, 3, "myckuyaa");
    let manager = LiveRuntimeManager::with_runtime_db_path_for_test(temp_db_path("poll-start"));

    manager.start_flow_session(&conn, 3).expect("start session");

    assert!(manager.session_has_poll_task_for_test(3));
}

#[test]
fn stop_flow_session_cancels_existing_polling_task() {
    let (conn, _path) = open_runtime_db("poll-stop");
    seed_enabled_start_record_flow(&conn, 3, "myckuyaa");
    let manager = LiveRuntimeManager::with_runtime_db_path_for_test(temp_db_path("poll-stop"));

    manager.start_flow_session(&conn, 3).expect("start session");
    manager.stop_flow_session(3).expect("stop session");

    assert!(!manager.session_has_poll_task_for_test(3));
}

#[test]
fn reconcile_flow_replaces_old_polling_generation_without_duplicates() {
    let (conn, _path) = open_runtime_db("poll-reconcile");
    seed_enabled_start_record_flow(&conn, 3, "myckuyaa");
    let manager = LiveRuntimeManager::with_runtime_db_path_for_test(temp_db_path("poll-reconcile"));

    manager.start_flow_session(&conn, 3).expect("start session");
    let first_generation = manager.session_generation_for_test(3).expect("generation");
    manager.reconcile_flow(&conn, 3).expect("reconcile flow");

    assert_eq!(manager.active_poll_task_count_for_test(3), 1);
    assert!(manager.session_generation_for_test(3).expect("generation") > first_generation);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test live_runtime::manager -- --nocapture`
Expected: FAIL because the polling-task helpers/state do not exist yet

- [ ] **Step 3: Implement minimal poll-task ownership in manager/session**

```rust
// src-tauri/src/live_runtime/session.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveRuntimeSession {
    // existing fields...
    poll_generation: u64,
}

impl LiveRuntimeSession {
    pub fn poll_generation(&self) -> u64 {
        self.poll_generation
    }

    pub fn bump_generation(&mut self) -> u64 {
        self.poll_generation += 1;
        self.poll_generation
    }
}
```

```rust
// src-tauri/src/live_runtime/manager.rs
struct PollTaskHandle {
    cancelled: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
    generation: u64,
}

#[derive(Default)]
struct LiveRuntimeState {
    sessions_by_flow: HashMap<i64, LiveRuntimeSession>,
    poll_tasks_by_flow: HashMap<i64, PollTaskHandle>,
    // existing fields...
}

fn restart_poll_task(&self, conn: &Connection, flow_id: i64) -> Result<(), String> {
    self.stop_poll_task(flow_id)?;
    let generation = {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        let session = state
            .sessions_by_flow
            .get_mut(&flow_id)
            .ok_or_else(|| format!("missing live runtime session for flow {flow_id}"))?;
        session.bump_generation()
    };
    let cancelled = Arc::new(AtomicBool::new(false));
    let join = self.spawn_poll_thread(flow_id, generation, Arc::clone(&cancelled))?;
    let mut state = self.state.lock().map_err(|e| e.to_string())?;
    state.poll_tasks_by_flow.insert(
        flow_id,
        PollTaskHandle {
            cancelled,
            join: Some(join),
            generation,
        },
    );
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test live_runtime::manager -- --nocapture`
Expected: PASS for the new poll-task lifecycle assertions

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/live_runtime/manager.rs src-tauri/src/live_runtime/session.rs
git commit -m "feat(runtime): add per-flow live polling tasks"
```

### Task 4: Wire Rust Poll Loop To Full Check-Live Resolution And Runtime Transitions

**Files:**
- Modify: `src-tauri/src/live_runtime/manager.rs`
- Modify: `src-tauri/src/tiktok/check_live.rs`
- Modify: `src-tauri/src/tiktok/types.rs`
- Test: `src-tauri/src/live_runtime/manager.rs`

- [ ] **Step 1: Write the failing manager tests for autonomous live detect, missing stream stay-watching, and offline reset**

```rust
#[test]
fn poll_loop_creates_run_when_live_status_has_room_and_stream() {
    let (conn, _path) = open_runtime_db("autonomous-live");
    seed_enabled_start_record_flow(&conn, 3, "myckuyaa");
    let manager = LiveRuntimeManager::with_stubbed_live_status_for_test(
        temp_db_path("autonomous-live"),
        vec![StubLiveCheckResult::Live {
            room_id: "7312345".to_string(),
            stream_url: "https://example.com/live.flv".to_string(),
            viewer_count: Some(456),
        }],
    );

    manager.start_flow_session(&conn, 3).expect("start session");
    manager.run_one_poll_iteration_for_test(&conn, 3).expect("poll once");

    assert_eq!(manager.latest_runtime_status_for_test(3).as_deref(), Some("recording"));
    assert!(manager.latest_runtime_log_event_for_test(3, "run_created"));
}

#[test]
fn poll_loop_keeps_watching_when_live_stream_url_is_missing() {
    let (conn, _path) = open_runtime_db("missing-stream");
    seed_enabled_start_record_flow(&conn, 3, "myckuyaa");
    let manager = LiveRuntimeManager::with_stubbed_live_status_for_test(
        temp_db_path("missing-stream"),
        vec![StubLiveCheckResult::LiveWithoutStream {
            room_id: "7312345".to_string(),
            viewer_count: Some(456),
        }],
    );

    manager.start_flow_session(&conn, 3).expect("start session");
    manager.run_one_poll_iteration_for_test(&conn, 3).expect("poll once");

    assert_eq!(manager.latest_runtime_status_for_test(3).as_deref(), Some("watching"));
    assert!(manager.latest_runtime_log_event_for_test(3, "stream_url_missing"));
}

#[test]
fn poll_loop_marks_source_offline_after_live_cycle_ends() {
    let (conn, _path) = open_runtime_db("offline-reset");
    seed_enabled_start_record_flow(&conn, 3, "myckuyaa");
    let manager = LiveRuntimeManager::with_stubbed_live_status_for_test(
        temp_db_path("offline-reset"),
        vec![
            StubLiveCheckResult::Live {
                room_id: "7312345".to_string(),
                stream_url: "https://example.com/live.flv".to_string(),
                viewer_count: Some(456),
            },
            StubLiveCheckResult::Offline,
        ],
    );

    manager.start_flow_session(&conn, 3).expect("start session");
    manager.run_one_poll_iteration_for_test(&conn, 3).expect("poll live");
    manager.complete_active_run(&mut conn.clone(), 3, Some("7312345")).expect("complete run");
    manager.run_one_poll_iteration_for_test(&conn, 3).expect("poll offline");

    assert!(manager.latest_runtime_log_event_for_test(3, "source_offline_marked"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test poll_loop_ -- --nocapture`
Expected: FAIL because the poll loop does not yet call a Rust live resolver or expose the test hooks

- [ ] **Step 3: Implement the minimal poll iteration path**

```rust
// src-tauri/src/live_runtime/manager.rs
fn poll_flow_once(&self, conn: &Connection, flow_id: i64) -> Result<(), String> {
    let config = load_flow_runtime_config(conn, flow_id)?;
    let live = crate::tiktok::check_live::check_live_status(
        config.username.as_str(),
        config.cookies_json_value.as_ref(),
        config.proxy_url.as_deref(),
    )?;

    if live.is_live {
        let _ = self.handle_live_detected(conn, flow_id, &crate::tiktok::types::LiveStatus {
            room_id: live.room_id.unwrap_or_default(),
            stream_url: live.stream_url,
            viewer_count: live.viewer_count,
            title: live.title,
            is_live: true,
        })?;
    } else {
        self.mark_source_offline(flow_id, conn)?;
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test poll_loop_ -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/live_runtime/manager.rs src-tauri/src/tiktok/check_live.rs src-tauri/src/tiktok/types.rs
git commit -m "feat(runtime): drive live ingress from Rust polling"
```

### Task 5: Remove Sidecar Live/Account Ingress From AppShell And API Surface

**Files:**
- Modify: `src/components/layout/app-shell.tsx`
- Modify: `src/lib/api.ts`
- Test: `src/components/layout/app-shell.tsx` or a colocated small test file if needed

- [ ] **Step 1: Write the failing frontend regression test for no sidecar-driven live-ingress invoke path**

```ts
import assert from "node:assert/strict";
import test from "node:test";
import { shouldTriggerRustLiveIngressFromSidecar } from "@/components/layout/app-shell";

test("sidecar account status no longer drives Rust Start/Record ingress", () => {
  assert.equal(shouldTriggerRustLiveIngressFromSidecar(), false);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm exec --package tsx -- tsx --test src/components/layout/app-shell.test.ts`
Expected: FAIL because the helper/test seam does not exist and the sidecar ingress path is still wired

- [ ] **Step 3: Remove the live/account ingress glue from AppShell and demote stale sidecar API helpers**

```tsx
// src/components/layout/app-shell.tsx
// Remove:
// - triggerRustRuntimeLiveDetectedForAccount(...)
// - markRustRuntimeOfflineForAccount(...)
// - the account_live/account_status branches that call them

export function shouldTriggerRustLiveIngressFromSidecar(): boolean {
  return false;
}
```

```ts
// src/lib/api.ts
// Keep sidecar helpers only where they still serve dashboard/manual tooling.
// Remove comments that describe them as required for Start/Record flow automation.
```

- [ ] **Step 4: Run the test and frontend verification**

Run: `npm exec --package tsx -- tsx --test src/components/layout/app-shell.test.ts`
Expected: PASS

Run: `npm run lint:js`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/components/layout/app-shell.tsx src/lib/api.ts src/components/layout/app-shell.test.ts
git commit -m "refactor(runtime): remove sidecar live ingress glue"
```

### Task 6: Verify Lifecycle Hooks Still Start And Stop Rust Polling Correctly

**Files:**
- Modify: `src-tauri/src/commands/flows.rs`
- Modify: `src-tauri/src/commands/flow_engine.rs`
- Test: `src-tauri/src/commands/flows.rs`
- Test: `src-tauri/src/commands/flow_engine.rs`

- [ ] **Step 1: Write the failing tests for enable/disable/publish restarting exactly one Rust polling loop**

```rust
#[test]
fn set_flow_enabled_true_starts_polling_loop() {
    let (mut conn, _path) = open_flow_command_db("enable-starts-poll");
    seed_disabled_flow(&conn, 3, "myckuyaa");
    let manager = LiveRuntimeManager::with_runtime_db_path_for_test(temp_db_path("enable-starts-poll"));

    set_flow_enabled_with_conn(&mut conn, &manager, 3, true).expect("enable flow");

    assert!(manager.session_has_poll_task_for_test(3));
}

#[test]
fn set_flow_enabled_false_stops_polling_loop() {
    let (mut conn, _path) = open_flow_command_db("disable-stops-poll");
    seed_enabled_flow(&conn, 3, "myckuyaa");
    let manager = LiveRuntimeManager::with_runtime_db_path_for_test(temp_db_path("disable-stops-poll"));
    manager.start_flow_session(&conn, 3).expect("start session");

    set_flow_enabled_with_conn(&mut conn, &manager, 3, false).expect("disable flow");

    assert!(!manager.session_has_poll_task_for_test(3));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test set_flow_enabled_ -- --nocapture`
Expected: FAIL because the lifecycle assertions for polling tasks are not covered yet

- [ ] **Step 3: Adjust command-level lifecycle hooks minimally**

```rust
// src-tauri/src/commands/flows.rs
// Keep using runtime_manager.start_flow_session(...) and stop_flow_session(...),
// but assert through tests that those manager paths now also own poll-loop start/stop.
```

```rust
// src-tauri/src/commands/flow_engine.rs
// Keep publish -> restart session behavior, but assert the restarted session owns exactly one fresh poll task.
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test set_flow_enabled_ -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/flows.rs src-tauri/src/commands/flow_engine.rs
git commit -m "test(runtime): pin poll lifecycle on flow commands"
```

### Task 7: Final Verification And Minimal Cleanup

**Files:**
- Modify only files made stale by the above tasks

- [ ] **Step 1: Remove stale comments/imports/helpers introduced by sidecar ingress removal**

Examples to clean only if now unused:

```ts
// src/components/layout/app-shell.tsx
// delete unused imports or dead helper functions tied only to sidecar live/account ingress
```

```rust
// src-tauri/src/tiktok/client.rs
// remove helpers moved fully into check_live/http_transport only if no remaining callers exist
```

- [ ] **Step 2: Run full Rust verification**

Run: `cargo test -- --nocapture`
Expected: PASS

Run: `cargo fmt --check`
Expected: PASS

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 3: Run full frontend verification**

Run: `npm run lint:js`
Expected: PASS

- [ ] **Step 4: Re-read the spec and verify coverage manually**

Checklist:
- full Python check-live logic ported into Rust `tiktok/*`
- per-flow poll loop owned by `LiveRuntimeManager`
- no sidecar live/account ingress dependency for `Start/Record`
- offline reset + missing stream semantics preserved
- downstream sidecar domains still intact

- [ ] **Step 5: Commit final cleanup**

```bash
git add src-tauri/src/tiktok src-tauri/src/live_runtime src/components/layout/app-shell.tsx src/lib/api.ts src-tauri/src/commands/flows.rs src-tauri/src/commands/flow_engine.rs
git commit -m "feat(runtime): move live ingress fully to Rust"
```
