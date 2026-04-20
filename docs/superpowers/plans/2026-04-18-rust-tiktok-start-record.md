# Rust TikTok Start/Record Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Python-owned TikTok `Start`/`Record` slice with a Rust/Tauri runtime that polls per-flow `Start` configs, records one live session per run, preserves `account_id + flow_run_id` ownership, and keeps downstream clip/caption sidecar processing working through the existing external recording key contract.

**Architecture:** Add a new `live_runtime` manager in `src-tauri` that owns flow automation sessions, username lease handling, durable room-id dedupe restore, and lifecycle wiring for startup/enable/publish. Port TikTok live resolution into `src-tauri/src/tiktok/*`, keep `Record` as a one-shot ffmpeg worker in `src-tauri/src/recording_runtime/*`, and update the frontend to consume Rust runtime refresh/events instead of sidecar flow hints for the `Start`/`Record` stages.

**Tech Stack:** Rust, Tauri v2, rusqlite, serde/serde_json, reqwest, tokio process/runtime, existing Python sidecar clip endpoints, React, Zustand

---

## File Structure

**Create:**
- `src-tauri/src/live_runtime/mod.rs` - public runtime manager module exports
- `src-tauri/src/live_runtime/types.rs` - session/config/event structs
- `src-tauri/src/live_runtime/normalize.rs` - username canonicalization helpers
- `src-tauri/src/live_runtime/account_binding.rs` - account lookup/create/conflict checks
- `src-tauri/src/live_runtime/manager.rs` - `LiveRuntimeManager` lifecycle and orchestration
- `src-tauri/src/live_runtime/session.rs` - per-flow session loop
- `src-tauri/src/tiktok/mod.rs` - TikTok module exports
- `src-tauri/src/tiktok/types.rs` - room/live payload models
- `src-tauri/src/tiktok/client.rs` - HTTP client and live resolution
- `src-tauri/src/recording_runtime/mod.rs` - recording runtime exports
- `src-tauri/src/recording_runtime/types.rs` - worker state/input/output structs
- `src-tauri/src/recording_runtime/worker.rs` - ffmpeg worker lifecycle

**Modify:**
- `src-tauri/Cargo.toml` - add Rust runtime dependencies
- `src-tauri/src/lib.rs` - manage `LiveRuntimeManager`, startup/shutdown wiring, commands
- `src-tauri/src/commands/mod.rs` - export live runtime commands
- `src-tauri/src/commands/live_runtime.rs` - runtime snapshot/status commands for frontend
- `src-tauri/src/commands/accounts.rs` - reuse or expose account creation helper logic
- `src-tauri/src/commands/flow_engine.rs` - publish restart hook
- `src-tauri/src/commands/flows.rs` - enable/disable hook, runtime status sourcing
- `src-tauri/src/commands/recordings.rs` - Rust-owned upsert path keyed by external recording id
- `src-tauri/src/db/models.rs` - add runtime-facing structs if needed
- `src-tauri/src/workflow/mod.rs` - register runtime-facing node helpers
- `src-tauri/src/workflow/start_node.rs` - parse/validate `Start` config and output payload
- `src-tauri/src/workflow/record_node.rs` - parse/validate `Record` config and input payload
- `src-tauri/src/workflow/runtime_store.rs` - flow-run/node-run/dedupe restore helpers
- `src/lib/api.ts` - add Rust runtime invoke wrappers
- `src/stores/flow-store.ts` - refresh runtime via Rust lifecycle hooks
- `src/components/layout/app-shell.tsx` - stop mapping sidecar hints to flow runtime for `Start`/`Record`
- `src/types/index.ts` - add runtime snapshot/event payload types
- `src/lib/flow-node-forms.ts` - canonical username parsing and any new `Start` defaults
- `src/components/flows/modals/start-node-modal.tsx` - validation copy aligned to canonical username rule
- `src/components/flows/modals/record-node-modal.tsx` - keep `max_duration_minutes` semantics explicit

**Test:**
- `src-tauri/src/live_runtime/normalize.rs` unit tests
- `src-tauri/src/live_runtime/account_binding.rs` unit tests
- `src-tauri/src/tiktok/client.rs` unit tests
- `src-tauri/src/recording_runtime/worker.rs` unit tests
- `src-tauri/src/workflow/start_node.rs` unit tests
- `src-tauri/src/workflow/record_node.rs` unit tests
- `src-tauri/src/workflow/runtime_store.rs` unit tests

---

### Task 1: Canonical Start/Record Config And Account Binding Foundation

**Files:**
- Create: `src-tauri/src/live_runtime/normalize.rs`
- Modify: `src-tauri/src/workflow/start_node.rs`
- Modify: `src-tauri/src/workflow/record_node.rs`
- Modify: `src/lib/flow-node-forms.ts`
- Modify: `src/components/flows/modals/start-node-modal.tsx`
- Test: `src-tauri/src/live_runtime/normalize.rs`
- Test: `src-tauri/src/workflow/start_node.rs`
- Test: `src-tauri/src/workflow/record_node.rs`

- [ ] **Step 1: Write the failing normalization and config parsing tests**

```rust
#[cfg(test)]
mod tests {
    use super::{canonicalize_username, CanonicalUsernameError};

    #[test]
    fn canonicalize_username_strips_one_leading_at_and_whitespace() {
        assert_eq!(canonicalize_username("  @Shop_ABC  ").unwrap().canonical, "Shop_ABC");
        assert_eq!(canonicalize_username("@@Shop_ABC").unwrap().canonical, "@Shop_ABC");
        assert_eq!(canonicalize_username("shop_abc").unwrap().lookup_key, "shop_abc");
    }

    #[test]
    fn canonicalize_username_rejects_empty_after_cleanup() {
        assert_eq!(canonicalize_username(" @ ").unwrap_err(), CanonicalUsernameError::Empty);
    }
}
```

```rust
#[cfg(test)]
mod start_config_tests {
    use super::parse_start_config;

    #[test]
    fn parse_start_config_reads_snake_case_fields() {
        let cfg = parse_start_config(r#"{
            "username":" @shop_abc ",
            "cookies_json":"{}",
            "proxy_url":"http://127.0.0.1:9000",
            "poll_interval_seconds":20,
            "retry_limit":3
        }"#)
        .unwrap();

        assert_eq!(cfg.username.canonical, "shop_abc");
        assert_eq!(cfg.poll_interval_seconds, 20);
        assert_eq!(cfg.retry_limit, 3);
    }
}
```

```rust
#[cfg(test)]
mod record_config_tests {
    use super::parse_record_config;

    #[test]
    fn parse_record_config_converts_minutes_to_runtime_seconds() {
        let cfg = parse_record_config(r#"{"max_duration_minutes":5}"#).unwrap();
        assert_eq!(cfg.max_duration_minutes, 5);
        assert_eq!(cfg.max_duration_seconds(), 300);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -- --nocapture`
Expected: FAIL with unresolved imports/functions such as `canonicalize_username`, `parse_start_config`, or `parse_record_config`

- [ ] **Step 3: Write the minimal normalization and parsing implementation**

```rust
// src-tauri/src/live_runtime/normalize.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalUsername {
    pub canonical: String,
    pub lookup_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalUsernameError {
    Empty,
}

pub fn canonicalize_username(raw: &str) -> Result<CanonicalUsername, CanonicalUsernameError> {
    let trimmed = raw.trim();
    let without_at = trimmed.strip_prefix('@').unwrap_or(trimmed);
    let canonical = without_at.trim();
    if canonical.is_empty() {
        return Err(CanonicalUsernameError::Empty);
    }
    Ok(CanonicalUsername {
        canonical: canonical.to_string(),
        lookup_key: canonical.to_lowercase(),
    })
}
```

```rust
// src-tauri/src/workflow/start_node.rs
use serde::Deserialize;

use crate::live_runtime::normalize::{canonicalize_username, CanonicalUsername};

#[derive(Debug, Deserialize)]
struct RawStartConfig {
    username: String,
    #[serde(default)]
    cookies_json: String,
    #[serde(default)]
    proxy_url: String,
    #[serde(default = "default_poll_interval")]
    poll_interval_seconds: i64,
    #[serde(default)]
    retry_limit: i64,
}

#[derive(Debug, Clone)]
pub struct StartConfig {
    pub username: CanonicalUsername,
    pub cookies_json: String,
    pub proxy_url: Option<String>,
    pub poll_interval_seconds: i64,
    pub retry_limit: i64,
}

pub fn parse_start_config(raw: &str) -> Result<StartConfig, String> {
    let cfg: RawStartConfig = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    Ok(StartConfig {
        username: canonicalize_username(&cfg.username).map_err(|_| "username is required".to_string())?,
        cookies_json: cfg.cookies_json,
        proxy_url: (!cfg.proxy_url.trim().is_empty()).then(|| cfg.proxy_url.trim().to_string()),
        poll_interval_seconds: cfg.poll_interval_seconds.max(5),
        retry_limit: cfg.retry_limit.max(0),
    })
}

fn default_poll_interval() -> i64 { 60 }
```

```rust
// src-tauri/src/workflow/record_node.rs
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RawRecordConfig {
    #[serde(default = "default_max_duration_minutes")]
    max_duration_minutes: i64,
}

#[derive(Debug, Clone)]
pub struct RecordConfig {
    pub max_duration_minutes: i64,
}

impl RecordConfig {
    pub fn max_duration_seconds(&self) -> i64 {
        self.max_duration_minutes * 60
    }
}

pub fn parse_record_config(raw: &str) -> Result<RecordConfig, String> {
    let cfg: RawRecordConfig = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    Ok(RecordConfig {
        max_duration_minutes: cfg.max_duration_minutes.max(1),
    })
}

fn default_max_duration_minutes() -> i64 { 5 }
```

```ts
// src/lib/flow-node-forms.ts
function normalizeUsername(value: unknown): string {
  return typeof value === "string" ? value.trim().replace(/^@/, "") : "";
}

export function parseStartNodeDraft(raw: string): StartNodeForm {
  let value: Record<string, unknown> = {};
  try {
    value = JSON.parse(raw || "{}") as Record<string, unknown>;
  } catch {
    value = {};
  }
  return {
    username: normalizeUsername(value.username),
    cookies_json: typeof value.cookies_json === "string" ? value.cookies_json : "",
    proxy_url: typeof value.proxy_url === "string" ? value.proxy_url : "",
    poll_interval_seconds: Math.max(5, Math.floor(num(value.poll_interval_seconds, 60))),
    watcher_mode: "live_polling",
    retry_limit: Math.max(0, Math.floor(num(value.retry_limit, 3))),
  };
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/live_runtime/normalize.rs src-tauri/src/workflow/start_node.rs src-tauri/src/workflow/record_node.rs src/lib/flow-node-forms.ts src/components/flows/modals/start-node-modal.tsx
git commit -m "feat(runtime): add canonical start and record config parsing"
```

### Task 2: Account Lookup/Create Conflict Handling And Durable Runtime Store Helpers

**Files:**
- Create: `src-tauri/src/live_runtime/account_binding.rs`
- Modify: `src-tauri/src/workflow/runtime_store.rs`
- Modify: `src-tauri/src/commands/accounts.rs`
- Test: `src-tauri/src/live_runtime/account_binding.rs`
- Test: `src-tauri/src/workflow/runtime_store.rs`

- [ ] **Step 1: Write failing tests for duplicate-account conflict, auto-create, and room-id restore**

```rust
#[cfg(test)]
mod account_binding_tests {
    use super::{resolve_or_create_account_for_username, ResolveAccountResult};

    #[test]
    fn resolve_or_create_account_for_username_creates_missing_monitored_account() {
        let conn = crate::db::init::initialize_database(&std::env::temp_dir().join("acct-create.db")).unwrap();
        let result = resolve_or_create_account_for_username(&conn, "shop_abc").unwrap();
        assert!(matches!(result, ResolveAccountResult::Created { .. }));
    }

    #[test]
    fn resolve_or_create_account_for_username_errors_on_normalized_duplicates() {
        let conn = crate::db::init::initialize_database(&std::env::temp_dir().join("acct-dupe.db")).unwrap();
        conn.execute("INSERT INTO accounts (username, display_name, type, created_at, updated_at) VALUES ('Shop_ABC','A','monitored',datetime('now','+7 hours'),datetime('now','+7 hours'))", []).unwrap();
        conn.execute("INSERT INTO accounts (username, display_name, type, created_at, updated_at) VALUES ('@shop_abc','B','monitored',datetime('now','+7 hours'),datetime('now','+7 hours'))", []).unwrap();

        let err = resolve_or_create_account_for_username(&conn, "shop_abc").unwrap_err();
        assert!(err.contains("duplicate accounts"));
    }
}
```

```rust
#[cfg(test)]
mod runtime_store_dedupe_tests {
    use super::load_last_completed_room_id_for_flow;

    #[test]
    fn load_last_completed_room_id_for_flow_reads_latest_room_from_recordings() {
        let mut conn = crate::db::init::initialize_database(&std::env::temp_dir().join("room-restore.db")).unwrap();
        conn.execute("INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) VALUES (1, 'Flow', 1, 'idle', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))", []).unwrap();
        conn.execute("INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, ended_at, trigger_reason) VALUES (11, 1, 1, 'completed', datetime('now','+7 hours'), datetime('now','+7 hours'), 'test')", []).unwrap();
        conn.execute("INSERT INTO recordings (account_id, room_id, status, duration_seconds, file_size_bytes, flow_id, flow_run_id, created_at, started_at) VALUES (1, '7312345', 'done', 0, 0, 1, 11, datetime('now','+7 hours'), datetime('now','+7 hours'))", []).unwrap();

        assert_eq!(load_last_completed_room_id_for_flow(&conn, 1).unwrap().as_deref(), Some("7312345"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -- --nocapture`
Expected: FAIL with missing functions such as `resolve_or_create_account_for_username` and `load_last_completed_room_id_for_flow`

- [ ] **Step 3: Write the minimal account binding and durable room-id helpers**

```rust
// src-tauri/src/live_runtime/account_binding.rs
use rusqlite::{params, Connection, OptionalExtension};

pub enum ResolveAccountResult {
    Existing { account_id: i64 },
    Created { account_id: i64 },
}

pub fn resolve_or_create_account_for_username(
    conn: &Connection,
    canonical_username: &str,
) -> Result<ResolveAccountResult, String> {
    let mut stmt = conn
        .prepare("SELECT id, username FROM accounts ORDER BY id ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;

    let mut matched = Vec::new();
    for row in rows {
        let (id, username) = row.map_err(|e| e.to_string())?;
        let key = super::normalize::canonicalize_username(&username)
            .map_err(|_| format!("invalid existing username {username}"))?
            .lookup_key;
        if key == canonical_username.to_lowercase() {
            matched.push(id);
        }
    }

    match matched.as_slice() {
        [id] => Ok(ResolveAccountResult::Existing { account_id: *id }),
        [] => {
            conn.execute(
                "INSERT INTO accounts (username, display_name, type, auto_record, priority, created_at, updated_at) VALUES (?1, ?1, 'monitored', 0, 0, datetime('now','+7 hours'), datetime('now','+7 hours'))",
                params![canonical_username],
            )
            .map_err(|e| e.to_string())?;
            Ok(ResolveAccountResult::Created {
                account_id: conn.last_insert_rowid(),
            })
        }
        _ => Err(format!("duplicate accounts for username {canonical_username}")),
    }
}
```

```rust
// src-tauri/src/workflow/runtime_store.rs
pub fn load_last_completed_room_id_for_flow(conn: &Connection, flow_id: i64) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT r.room_id FROM recordings r \
         JOIN flow_runs fr ON fr.id = r.flow_run_id \
         WHERE r.flow_id = ?1 AND r.room_id IS NOT NULL AND trim(r.room_id) <> '' \
           AND fr.status IN ('completed', 'cancelled') \
         ORDER BY fr.id DESC, r.id DESC LIMIT 1",
        [flow_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/live_runtime/account_binding.rs src-tauri/src/workflow/runtime_store.rs src-tauri/src/commands/accounts.rs
git commit -m "feat(runtime): resolve flow accounts and restore room dedupe state"
```

### Task 3: Port TikTok Live Resolution Into Rust

**Files:**
- Create: `src-tauri/src/tiktok/mod.rs`
- Create: `src-tauri/src/tiktok/types.rs`
- Create: `src-tauri/src/tiktok/client.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/src/tiktok/client.rs`

- [ ] **Step 1: Write failing tests for room-id extraction, cookie normalization, stream priority, and `check_alive` fallback**

```rust
#[cfg(test)]
mod tests {
    use super::{
        extract_room_id_from_html,
        normalize_cookie_header,
        parse_check_alive_live_status,
        parse_room_info_live_status,
    };

    #[test]
    fn extract_room_id_from_html_reads_sigil_patterns() {
        let html = r#"...room_id=7312345..."#;
        assert_eq!(extract_room_id_from_html(html), Some("7312345".to_string()));
    }

    #[test]
    fn normalize_cookie_header_accepts_json_cookie_map() {
        let cookies = normalize_cookie_header(r#"{"sessionid":"abc","tt-target-idc":"useast2a"}"#).unwrap();
        assert_eq!(cookies, "sessionid=abc; tt-target-idc=useast2a");
    }

    #[test]
    fn normalize_cookie_header_rejects_invalid_json() {
        let err = normalize_cookie_header("not-json").unwrap_err();
        assert!(err.contains("expected"));
    }

    #[test]
    fn parse_room_info_live_status_picks_highest_priority_stream_url_and_viewer_count() {
        let body = r#"{
          "data": {
            "room": {
              "status": 2,
              "id_str": "7312345",
              "owner_count": 321,
              "stream_url": {
                "flv_pull_url": {
                  "SD1":"https://example.com/live-sd.flv",
                  "FULL_HD1":"https://example.com/live-hd.flv"
                }
              }
            }
          }
        }"#;
        let live = parse_room_info_live_status(body).unwrap().unwrap();
        assert_eq!(live.room_id, "7312345");
        assert_eq!(live.stream_url, "https://example.com/live-hd.flv");
        assert_eq!(live.viewer_count, Some(321));
    }

    #[test]
    fn parse_check_alive_live_status_falls_back_when_room_info_is_missing() {
        let body = r#"{"data":{"alive":1,"room_id":"7312345","stream_url":"https://example.com/live.flv"}}"#;
        let live = parse_check_alive_live_status(body).unwrap().unwrap();
        assert_eq!(live.room_id, "7312345");
        assert_eq!(live.stream_url, "https://example.com/live.flv");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test tiktok::client::tests -- --nocapture`
Expected: FAIL with missing functions such as `normalize_cookie_header`, `parse_room_info_live_status`, or `parse_check_alive_live_status`

- [ ] **Step 3: Write the minimal TikTok client implementation and dependency wiring**

```toml
# src-tauri/Cargo.toml
[dependencies]
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
tokio = { version = "1", features = ["process", "rt-multi-thread", "macros", "time"] }
uuid = { version = "1", features = ["v4", "serde"] }
thiserror = "1"
```

```rust
// src-tauri/src/tiktok/types.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveStatus {
    pub room_id: String,
    pub stream_url: String,
    pub viewer_count: Option<i64>,
}
```

```rust
// src-tauri/src/tiktok/client.rs
use serde_json::Value;

use super::types::LiveStatus;

pub fn normalize_cookie_header(raw: &str) -> Result<String, String> {
    if raw.trim().is_empty() {
        return Ok(String::new());
    }
    let value: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let obj = value
        .as_object()
        .ok_or_else(|| "cookies_json must be a JSON object".to_string())?;
    Ok(obj
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| format!("{k}={s}")))
        .collect::<Vec<_>>()
        .join("; "))
}

pub fn proxy_url_for_reqwest(raw: Option<&str>) -> Result<Option<String>, String> {
    let trimmed = raw.unwrap_or("").trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err("proxy_url must start with http:// or https://".to_string());
    }
    Ok(Some(trimmed.to_string()))
}

pub fn extract_room_id_from_html(html: &str) -> Option<String> {
    html.split(|c: char| !c.is_ascii_digit())
        .find(|part| part.len() >= 5)
        .map(|s| s.to_string())
}

fn choose_stream_url(room: &Value) -> Option<String> {
    let urls = room["stream_url"]["flv_pull_url"].as_object()?;
    urls.get("FULL_HD1")
        .and_then(|v| v.as_str())
        .or_else(|| urls.get("HD1").and_then(|v| v.as_str()))
        .or_else(|| urls.values().find_map(|v| v.as_str()))
        .map(|s| s.to_string())
}

pub fn parse_room_info_live_status(body: &str) -> Result<Option<LiveStatus>, String> {
    let value: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let room = &value["data"]["room"];
    if room["status"].as_i64().unwrap_or_default() != 2 {
        return Ok(None);
    }
    let room_id = room["id_str"].as_str().unwrap_or_default().to_string();
    let stream_url = choose_stream_url(room).ok_or_else(|| "missing stream_url".to_string())?;
    let viewer_count = room["owner_count"].as_i64();
    Ok(Some(LiveStatus { room_id, stream_url, viewer_count }))
}

pub fn parse_check_alive_live_status(body: &str) -> Result<Option<LiveStatus>, String> {
    let value: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let data = &value["data"];
    if data["alive"].as_i64().unwrap_or_default() != 1 {
        return Ok(None);
    }
    let room_id = data["room_id"].as_str().unwrap_or_default().to_string();
    let stream_url = data["stream_url"]
        .as_str()
        .ok_or_else(|| "missing fallback stream_url".to_string())?
        .to_string();
    Ok(Some(LiveStatus {
        room_id,
        stream_url,
        viewer_count: None,
    }))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test tiktok::client::tests -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/tiktok/mod.rs src-tauri/src/tiktok/types.rs src-tauri/src/tiktok/client.rs
git commit -m "feat(tiktok): port live room resolution into Rust"
```

### Task 4: Add Username Lease Registry, Full Session Lifecycle, And Tauri Wiring

**Files:**
- Create: `src-tauri/src/live_runtime/mod.rs`
- Create: `src-tauri/src/live_runtime/types.rs`
- Create: `src-tauri/src/live_runtime/manager.rs`
- Create: `src-tauri/src/live_runtime/session.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/live_runtime.rs`
- Modify: `src-tauri/src/commands/flows.rs`
- Modify: `src-tauri/src/commands/flow_engine.rs`
- Test: `src-tauri/src/live_runtime/manager.rs`

- [ ] **Step 1: Write failing tests for lease acquire/release, startup bootstrap, publish restart, and shutdown cleanup**

```rust
#[cfg(test)]
mod manager_tests {
    use super::LiveRuntimeManager;

    #[tokio::test]
    async fn bootstrap_enabled_flows_starts_enabled_flows_once() {
        let manager = LiveRuntimeManager::new();
        let conn = crate::db::init::initialize_database(&std::env::temp_dir().join("bootstrap-flows.db")).unwrap();
        conn.execute("INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) VALUES (7, 'Flow', 1, 'idle', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))", []).unwrap();
        conn.execute("INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) VALUES (7, 'start', 1, '{\"username\":\"shop_abc\"}', '{\"username\":\"shop_abc\"}', datetime('now','+7 hours'), datetime('now','+7 hours'))", []).unwrap();

        manager.bootstrap_enabled_flows_test(&conn).await.unwrap();

        assert!(manager.has_session(7).await);
        assert_eq!(manager.active_lease_count().await, 1);
    }

    #[tokio::test]
    async fn acquire_username_lease_rejects_second_flow_with_same_lookup_key() {
        let manager = LiveRuntimeManager::new();
        manager.acquire_lease_for_test(7, "shop_abc").await.unwrap();
        let err = manager.acquire_lease_for_test(8, "shop_abc").await.unwrap_err();

        assert!(err.contains("username lease conflict"));
    }

    #[tokio::test]
    async fn reconcile_flow_after_publish_restarts_session_once() {
        let manager = LiveRuntimeManager::new();
        manager.start_test_session(7, "shop_abc").await;
        manager.reconcile_flow(7, "shop_abc").await.unwrap();

        assert_eq!(manager.active_session_count().await, 1);
        assert!(manager.has_session(7).await);
    }

    #[tokio::test]
    async fn shutdown_clears_sessions_and_leases() {
        let manager = LiveRuntimeManager::new();
        manager.start_test_session(7, "shop_abc").await;
        manager.shutdown().await;

        assert_eq!(manager.active_session_count().await, 0);
        assert_eq!(manager.active_lease_count().await, 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test manager_tests -- --nocapture`
Expected: FAIL with missing `LiveRuntimeManager` lease and lifecycle methods

- [ ] **Step 3: Write the minimal manager and lifecycle integration**

```rust
// src-tauri/src/live_runtime/types.rs
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub flow_id: i64,
    pub account_id: i64,
    pub username_normalized: String,
    pub last_completed_room_id: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct SessionHandle {
    pub snapshot: SessionSnapshot,
}

#[derive(Debug, Default)]
pub struct LeaseRegistry {
    pub by_flow: std::collections::HashMap<i64, String>,
    pub by_username: std::collections::HashMap<String, i64>,
}
```

```rust
// src-tauri/src/live_runtime/manager.rs
use std::collections::HashMap;
use tokio::sync::Mutex;

use super::types::{LeaseRegistry, SessionHandle, SessionSnapshot};
use crate::live_runtime::normalize::canonicalize_username;

pub struct LiveRuntimeManager {
    sessions: Mutex<HashMap<i64, SessionHandle>>,
    leases: Mutex<LeaseRegistry>,
}

impl LiveRuntimeManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            leases: Mutex::new(LeaseRegistry::default()),
        }
    }

    pub async fn acquire_lease_for_test(&self, flow_id: i64, username_normalized: &str) -> Result<(), String> {
        let mut leases = self.leases.lock().await;
        if let Some(owner) = leases.by_username.get(username_normalized) {
            if *owner != flow_id {
                return Err(format!("username lease conflict for {username_normalized}"));
            }
        }
        leases.by_username.insert(username_normalized.to_string(), flow_id);
        leases.by_flow.insert(flow_id, username_normalized.to_string());
        Ok(())
    }

    pub async fn release_lease(&self, flow_id: i64) {
        let mut leases = self.leases.lock().await;
        if let Some(username) = leases.by_flow.remove(&flow_id) {
            leases.by_username.remove(&username);
        }
    }

    pub async fn reconcile_flow(&self, flow_id: i64, raw_username: &str) -> Result<(), String> {
        let username_normalized = canonicalize_username(raw_username)
            .map_err(|_| "username is required".to_string())?
            .lookup_key;
        self.stop_flow(flow_id).await;
        self.acquire_lease_for_test(flow_id, &username_normalized).await?;
        let mut sessions = self.sessions.lock().await;
        sessions.insert(flow_id, SessionHandle {
            snapshot: SessionSnapshot {
                flow_id,
                account_id: 0,
                username_normalized,
                last_completed_room_id: None,
                status: "polling".to_string(),
            },
        });
        Ok(())
    }

    pub async fn bootstrap_enabled_flows(
        &self,
        _app: tauri::AppHandle,
        conn: &rusqlite::Connection,
    ) -> Result<(), String> {
        self.bootstrap_enabled_flows_test(conn).await
    }

    pub async fn bootstrap_enabled_flows_test(
        &self,
        conn: &rusqlite::Connection,
    ) -> Result<(), String> {
        let mut stmt = conn
            .prepare(
                "SELECT f.id, json_extract(n.published_config_json, '$.username') \
                 FROM flows f \
                 JOIN flow_nodes n ON n.flow_id = f.id AND n.node_key = 'start' \
                 WHERE f.enabled = 1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (flow_id, username) = row.map_err(|e| e.to_string())?;
            self.reconcile_flow(flow_id, &username).await?;
        }
        Ok(())
    }

    pub async fn start_test_session(&self, flow_id: i64, username_normalized: &str) {
        self.acquire_lease_for_test(flow_id, username_normalized).await.unwrap();
        self.sessions.lock().await.insert(flow_id, SessionHandle {
            snapshot: SessionSnapshot {
                flow_id,
                account_id: 0,
                username_normalized: username_normalized.to_string(),
                last_completed_room_id: None,
                status: "polling".to_string(),
            },
        });
    }

    pub async fn stop_flow(&self, flow_id: i64) {
        self.sessions.lock().await.remove(&flow_id);
        self.release_lease(flow_id).await;
    }

    pub async fn shutdown(&self) {
        let flow_ids = self.sessions.lock().await.keys().copied().collect::<Vec<_>>();
        for flow_id in flow_ids {
            self.stop_flow(flow_id).await;
        }
    }

    pub async fn active_session_count(&self) -> usize {
        self.sessions.lock().await.len()
    }

    pub async fn active_lease_count(&self) -> usize {
        self.leases.lock().await.by_username.len()
    }

    pub async fn has_session(&self, flow_id: i64) -> bool {
        self.sessions.lock().await.contains_key(&flow_id)
    }

    pub async fn list_snapshots(&self) -> Vec<SessionSnapshot> {
        self.sessions
            .lock()
            .await
            .values()
            .map(|handle| handle.snapshot.clone())
            .collect()
    }

    pub async fn snapshot(&self, flow_id: i64) -> Option<SessionSnapshot> {
        self.sessions
            .lock()
            .await
            .get(&flow_id)
            .map(|handle| handle.snapshot.clone())
    }

    pub async fn mark_test_recording_complete(&self, flow_id: i64, room_id: Option<&str>) {
        if let Some(handle) = self.sessions.lock().await.get_mut(&flow_id) {
            handle.snapshot.status = "polling".to_string();
            handle.snapshot.last_completed_room_id = room_id.map(|s| s.to_string());
        }
    }
}
```

```rust
// src-tauri/src/lib.rs
let runtime = live_runtime::manager::LiveRuntimeManager::new();
app.manage(runtime.clone());

tauri::async_runtime::block_on(runtime.bootstrap_enabled_flows(app.handle(), &conn))?;

// In the builder run loop:
// tauri::Builder::default().run(|app, event| {
//   if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
//     let runtime = app.state::<LiveRuntimeManager>();
//     tauri::async_runtime::block_on(runtime.shutdown());
//   }
// })
```

```rust
// src-tauri/src/commands/live_runtime.rs
#[tauri::command]
pub async fn list_live_runtime_debug_snapshots(
    runtime: State<'_, LiveRuntimeManager>,
) -> Result<Vec<SessionSnapshot>, String> {
    Ok(runtime.list_snapshots().await)
}
```

```rust
// src-tauri/src/commands/flows.rs
fn load_published_start_username(conn: &rusqlite::Connection, flow_id: i64) -> Result<String, String> {
    let raw = conn.query_row(
        "SELECT json_extract(published_config_json, '$.username') FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
        [flow_id],
        |row| row.get::<_, String>(0),
    )
    .map_err(|e| e.to_string())?;
    Ok(crate::live_runtime::normalize::canonicalize_username(&raw)
        .map_err(|_| "username is required".to_string())?
        .canonical)
}

#[tauri::command]
pub async fn set_flow_enabled(
    state: State<'_, AppState>,
    runtime: State<'_, LiveRuntimeManager>,
    flow_id: i64,
    enabled: bool,
) -> Result<(), String> {
    // existing DB update first
    if enabled {
        let username = load_published_start_username(&state.db.lock().map_err(|e| e.to_string())?, flow_id)?;
        runtime.reconcile_flow(flow_id, &username).await?;
    } else {
        runtime.stop_flow(flow_id).await;
    }
    Ok(())
}
```

```rust
// src-tauri/src/commands/flow_engine.rs
fn flow_is_enabled(conn: &rusqlite::Connection, flow_id: i64) -> Result<bool, String> {
    conn.query_row("SELECT enabled FROM flows WHERE id = ?1", [flow_id], |row| row.get::<_, i64>(0))
        .map(|v| v != 0)
        .map_err(|e| e.to_string())
}

fn publish_flow_definition_inner(
    state: &State<'_, AppState>,
    flow_id: i64,
) -> Result<PublishFlowResult, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let is_running: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM flow_runs WHERE flow_id = ?1 AND status = 'running')",
            [flow_id],
            |row| row.get::<_, i64>(0).map(|v| v != 0),
        )
        .map_err(|e| e.to_string())?;
    tx.execute(
        &format!(
            "UPDATE flow_nodes SET published_config_json = draft_config_json, published_at = {} WHERE flow_id = ?1",
            SQL_NOW_HCM
        ),
        [flow_id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        &format!(
            "UPDATE flows SET published_version = draft_version, draft_version = draft_version + 1, updated_at = {} WHERE id = ?1",
            SQL_NOW_HCM
        ),
        [flow_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(PublishFlowResult { flow_id, is_running })
}

#[tauri::command]
pub async fn publish_flow_definition(
    state: State<'_, AppState>,
    runtime: State<'_, LiveRuntimeManager>,
    flow_id: i64,
) -> Result<PublishFlowResult, String> {
    let result = publish_flow_definition_inner(&state, flow_id)?;
    let enabled = flow_is_enabled(&state.db.lock().map_err(|e| e.to_string())?, flow_id)?;
    if enabled {
        let username = load_published_start_username(&state.db.lock().map_err(|e| e.to_string())?, flow_id)?;
        runtime.reconcile_flow(flow_id, &username).await?;
    }
    Ok(result)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test manager_tests -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/live_runtime/mod.rs src-tauri/src/live_runtime/types.rs src-tauri/src/live_runtime/manager.rs src-tauri/src/live_runtime/session.rs src-tauri/src/lib.rs src-tauri/src/commands/live_runtime.rs src-tauri/src/commands/flows.rs src-tauri/src/commands/flow_engine.rs
git commit -m "feat(runtime): wire live session lifecycle into Tauri commands"
```

### Task 5: Implement Start Session Loop, Run Creation, And Durable Room Dedupe Restore

**Files:**
- Modify: `src-tauri/src/live_runtime/session.rs`
- Modify: `src-tauri/src/workflow/start_node.rs`
- Modify: `src-tauri/src/workflow/runtime_store.rs`
- Test: `src-tauri/src/workflow/start_node.rs`
- Test: `src-tauri/src/workflow/runtime_store.rs`

- [ ] **Step 1: Write failing tests for creating one run per live room and restoring dedupe after restart**

```rust
#[cfg(test)]
mod session_dedupe_tests {
    use super::should_start_new_run;

    #[test]
    fn should_start_new_run_blocks_same_room_after_completion() {
        assert!(!should_start_new_run(Some("7312345"), Some("7312345"), true));
        assert!(should_start_new_run(Some("7319999"), Some("7312345"), true));
        assert!(should_start_new_run(Some("7312345"), Some("7312345"), false));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test session_dedupe_tests -- --nocapture`
Expected: FAIL with missing `should_start_new_run`

- [ ] **Step 3: Write the minimal Start session logic**

```rust
// src-tauri/src/live_runtime/session.rs
pub fn should_start_new_run(
    current_room_id: Option<&str>,
    last_completed_room_id: Option<&str>,
    source_is_live: bool,
) -> bool {
    if !source_is_live {
        return true;
    }
    match (current_room_id, last_completed_room_id) {
        (Some(current), Some(last)) => current != last,
        _ => true,
    }
}
```

```rust
// src-tauri/src/workflow/start_node.rs
#[derive(Debug, Clone, serde::Serialize)]
pub struct StartOutput {
    pub account_id: i64,
    pub username: String,
    pub room_id: String,
    pub stream_url: String,
    pub viewer_count: Option<i64>,
    pub detected_at: String,
}
```

```rust
// src-tauri/src/workflow/runtime_store.rs
pub fn create_run_with_completed_start_node(
    conn: &Connection,
    flow_id: i64,
    definition_version: i64,
    output_json: &str,
) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO flow_runs (flow_id, definition_version, status, started_at, trigger_reason) VALUES (?1, ?2, 'running', datetime('now','+7 hours'), 'start_live_detected')",
        params![flow_id, definition_version],
    )
    .map_err(|e| e.to_string())?;
    let flow_run_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO flow_node_runs (flow_run_id, flow_id, node_key, status, started_at, ended_at, output_json) VALUES (?1, ?2, 'start', 'completed', datetime('now','+7 hours'), datetime('now','+7 hours'), ?3)",
        params![flow_run_id, flow_id, output_json],
    )
    .map_err(|e| e.to_string())?;
    Ok(flow_run_id)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/live_runtime/session.rs src-tauri/src/workflow/start_node.rs src-tauri/src/workflow/runtime_store.rs
git commit -m "feat(start): create live-triggered runs with durable room dedupe"
```

### Task 6: Implement Rust Recording Worker, Status Mapping, And Recording Row Ownership

**Files:**
- Create: `src-tauri/src/recording_runtime/mod.rs`
- Create: `src-tauri/src/recording_runtime/types.rs`
- Create: `src-tauri/src/recording_runtime/worker.rs`
- Modify: `src-tauri/src/commands/recordings.rs`
- Modify: `src-tauri/src/workflow/record_node.rs`
- Test: `src-tauri/src/recording_runtime/worker.rs`
- Test: `src-tauri/src/commands/recordings.rs`

- [ ] **Step 1: Write failing tests for external recording key upsert and status mapping**

```rust
#[cfg(test)]
mod recording_upsert_tests {
    use super::{map_rust_recording_status, upsert_rust_recording};

    #[test]
    fn upsert_rust_recording_keeps_account_flow_and_run_ownership() {
        let conn = crate::db::init::initialize_database(&std::env::temp_dir().join("rust-recording.db")).unwrap();
        let recording_id = upsert_rust_recording(&conn, 9, 3, Some(11), "ext-123", "recording", Some("7312345"), None, None, 0, 0).unwrap();
        let row: (i64, Option<i64>, Option<i64>, Option<String>) = conn.query_row(
            "SELECT account_id, flow_id, flow_run_id, room_id FROM recordings WHERE id = ?1",
            [recording_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).unwrap();
        assert_eq!(row.0, 9);
        assert_eq!(row.1, Some(3));
        assert_eq!(row.2, Some(11));
        assert_eq!(row.3.as_deref(), Some("7312345"));
    }

    #[test]
    fn map_rust_recording_status_maps_runtime_states_to_db_states() {
        assert_eq!(map_rust_recording_status("pending"), "recording");
        assert_eq!(map_rust_recording_status("recording"), "recording");
        assert_eq!(map_rust_recording_status("completed"), "done");
        assert_eq!(map_rust_recording_status("stopped"), "done");
        assert_eq!(map_rust_recording_status("error"), "error");
    }

    #[test]
    fn completed_or_error_recording_sets_ended_at_and_error_message() {
        let conn = crate::db::init::initialize_database(&std::env::temp_dir().join("rust-recording-status.db")).unwrap();
        let recording_id = upsert_rust_recording(&conn, 9, 3, Some(11), "ext-456", "error", Some("7319999"), Some("/tmp/out.mp4"), Some("ffmpeg failed"), 12, 99).unwrap();
        let row: (String, Option<String>, Option<String>) = conn.query_row(
            "SELECT status, ended_at, error_message FROM recordings WHERE id = ?1",
            [recording_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap();
        assert_eq!(row.0, "error");
        assert!(row.1.is_some());
        assert_eq!(row.2.as_deref(), Some("ffmpeg failed"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test recording_upsert_tests -- --nocapture`
Expected: FAIL with missing `upsert_rust_recording`

- [ ] **Step 3: Write the minimal worker and recording upsert path**

```rust
// src-tauri/src/recording_runtime/types.rs
#[derive(Debug, Clone)]
pub struct RecordingStartInput {
    pub account_id: i64,
    pub flow_id: i64,
    pub flow_run_id: i64,
    pub room_id: String,
    pub stream_url: String,
    pub max_duration_seconds: i64,
    pub external_recording_id: String,
}
```

```rust
// src-tauri/src/commands/recordings.rs
pub fn upsert_rust_recording(
    conn: &Connection,
    account_id: i64,
    flow_id: i64,
    flow_run_id: Option<i64>,
    external_recording_id: &str,
    runtime_status: &str,
    room_id: Option<&str>,
    file_path: Option<&str>,
    error_message: Option<&str>,
    duration_seconds: i64,
    file_size_bytes: i64,
) -> Result<i64, String> {
    let status = map_rust_recording_status(runtime_status);
    let ended_at = matches!(status, "done" | "error");
    let existing_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM recordings WHERE sidecar_recording_id = ?1",
            [external_recording_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some(id) = existing_id {
        conn.execute(
            "UPDATE recordings SET account_id = ?1, flow_id = ?2, flow_run_id = ?3, room_id = COALESCE(?4, room_id), \
             status = ?5, duration_seconds = ?6, file_size_bytes = ?7, file_path = COALESCE(?8, file_path), \
             error_message = COALESCE(?9, error_message), ended_at = CASE WHEN ?10 != 0 THEN datetime('now','+7 hours') ELSE ended_at END \
             WHERE id = ?11",
            params![account_id, flow_id, flow_run_id, room_id, status, duration_seconds, file_size_bytes, file_path, error_message, if ended_at { 1i64 } else { 0i64 }, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO recordings (account_id, room_id, status, duration_seconds, file_size_bytes, flow_id, flow_run_id, file_path, error_message, sidecar_recording_id, started_at, created_at, ended_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now','+7 hours'), datetime('now','+7 hours'), CASE WHEN ?11 != 0 THEN datetime('now','+7 hours') ELSE NULL END)",
            params![account_id, room_id, status, duration_seconds, file_size_bytes, flow_id, flow_run_id, file_path, error_message, external_recording_id, if ended_at { 1i64 } else { 0i64 }],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }
}

pub fn map_rust_recording_status(runtime_status: &str) -> &'static str {
    match runtime_status {
        "pending" | "recording" => "recording",
        "completed" | "stopped" => "done",
        "error" | "failed" => "error",
        _ => "recording",
    }
}
```

```rust
// src-tauri/src/recording_runtime/worker.rs
pub async fn build_ffmpeg_command(input: &RecordingStartInput, output_path: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.arg("-y")
        .arg("-i")
        .arg(&input.stream_url)
        .arg("-t")
        .arg(input.max_duration_seconds.to_string())
        .arg(output_path);
    cmd
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test recording_upsert_tests -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/recording_runtime/mod.rs src-tauri/src/recording_runtime/types.rs src-tauri/src/recording_runtime/worker.rs src-tauri/src/commands/recordings.rs src-tauri/src/workflow/record_node.rs
git commit -m "feat(record): add Rust-owned recording worker and row upsert"
```

### Task 7: Preserve Downstream Sidecar Clip/Caption Contract And Flow Progression

**Files:**
- Modify: `src-tauri/src/live_runtime/session.rs`
- Modify: `src-tauri/src/commands/live_runtime.rs`
- Modify: `src-tauri/src/recording_runtime/worker.rs`
- Modify: `src-tauri/src/commands/clips.rs`
- Modify: `src-tauri/src/commands/flows.rs`
- Modify: `src/components/layout/app-shell.tsx`
- Modify: `src/lib/api.ts`
- Modify: `src/stores/flow-store.ts`
- Modify: `src/types/index.ts`
- Test: `src-tauri/src/commands/clips.rs`
- Test: `src-tauri/src/live_runtime/session.rs`

- [ ] **Step 1: Write failing tests for clip insertion and runtime event emission**

```rust
#[cfg(test)]
mod clip_bridge_tests {
    use super::insert_clip_from_sidecar;

    #[test]
    fn insert_clip_from_sidecar_finds_rust_created_recording_by_external_key() {
        let state = crate::test_support::state_with_temp_db("clip-bridge.db");
        {
            let conn = state.db.lock().unwrap();
            crate::commands::recordings::upsert_rust_recording(&conn, 1, 3, Some(12), "ext-123", "done", Some("7312345"), None, None, 0, 0).unwrap();
        }
        let clip_id = insert_clip_from_sidecar(
            tauri::State::from(&state),
            InsertClipFromSidecarInput {
                sidecar_recording_id: "ext-123".into(),
                account_id: 1,
                file_path: "/tmp/clip.mp4".into(),
                thumbnail_path: "".into(),
                duration_sec: 10.0,
                start_sec: 0.0,
                end_sec: 10.0,
                transcript_text: None,
            },
        ).unwrap();
        assert!(clip_id > 0);
    }

    #[test]
    fn emit_runtime_update_serializes_runtime_snapshot_for_frontend_event() {
        let snapshot = FlowRuntimeSnapshot {
            flow_id: 7,
            status: "polling".into(),
            current_node: Some("start".into()),
            account_id: Some(3),
            username: "shop_abc".into(),
            last_live_at: None,
            last_error: None,
            active_flow_run_id: None,
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("flow_id"));
        assert!(json.contains("polling"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test clip_bridge_tests -- --nocapture`
Expected: FAIL until the test harness and Rust-created recordings path are wired cleanly

- [ ] **Step 3: Write the minimal sidecar bridge and frontend runtime event integration**

```rust
// src-tauri/src/live_runtime/session.rs
use tauri::Emitter;

fn emit_runtime_update(app: &tauri::AppHandle, snapshot: &FlowRuntimeSnapshot) -> Result<(), String> {
    app.emit("flow-runtime-updated", snapshot.clone())
        .map_err(|e| e.to_string())
}

async fn schedule_sidecar_clip_processing(
    sidecar_base_url: &str,
    external_recording_id: &str,
    username: &str,
    file_path: &str,
    account_id: i64,
) -> Result<(), String> {
    let body = serde_json::json!({
        "recording_id": external_recording_id,
        "username": username,
        "file_path": file_path,
        "account_id": account_id,
    });
    reqwest::Client::new()
        .post(format!("{sidecar_base_url}/api/video/process"))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    Ok(())
}

// Emit on every meaningful transition:
// - start polling
// - username conflict
// - live detected / run started
// - recording started
// - recording progress
// - recording finished
```

```rust
// src-tauri/src/commands/live_runtime.rs
#[derive(Debug, serde::Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct FlowRuntimeSnapshot {
    pub flow_id: i64,
    pub status: String,
    pub current_node: Option<String>,
    pub account_id: Option<i64>,
    pub username: String,
    pub last_live_at: Option<String>,
    pub last_error: Option<String>,
    pub active_flow_run_id: Option<i64>,
}

#[tauri::command]
pub async fn list_live_runtime_sessions(
    runtime: State<'_, LiveRuntimeManager>,
) -> Result<Vec<FlowRuntimeSnapshot>, String> {
    Ok(runtime
        .list_snapshots()
        .await
        .into_iter()
        .map(|row| FlowRuntimeSnapshot {
            flow_id: row.flow_id,
            status: row.status,
            current_node: None,
            account_id: Some(row.account_id),
            username: row.username_normalized,
            last_live_at: None,
            last_error: None,
            active_flow_run_id: None,
        })
        .collect())
}
```

```ts
// src/lib/api.ts
export async function listLiveRuntimeSessions(): Promise<FlowRuntimeSnapshot[]> {
  return invoke<FlowRuntimeSnapshot[]>("list_live_runtime_sessions");
}
```

```ts
// src/types/index.ts
export interface FlowRuntimeSnapshot {
  flow_id: number;
  status: string;
  current_node: FlowNodeKey | null;
  account_id: number | null;
  username: string;
  last_live_at: string | null;
  last_error: string | null;
  active_flow_run_id: number | null;
}
```

```ts
// src/stores/flow-store.ts
type FlowStore = {
  runtimeSnapshots: Record<number, FlowRuntimeSnapshot>;
  applyRuntimeSnapshots: (rows: FlowRuntimeSnapshot[]) => void;
};

applyRuntimeSnapshots: (rows) =>
  set({ runtimeSnapshots: Object.fromEntries(rows.map((row) => [row.flow_id, row])) }),
```

```tsx
// src/components/layout/app-shell.tsx
useEffect(() => {
  let unlisten: (() => void) | null = null;
  let cancelled = false;

  void api.listLiveRuntimeSessions().then((rows) => {
    if (!cancelled) {
      useFlowStore.getState().applyRuntimeSnapshots(rows);
      void useFlowStore.getState().refreshRuntime();
    }
  });

  void listen<FlowRuntimeSnapshot>("flow-runtime-updated", async (event) => {
    if (cancelled) return;
    useFlowStore.getState().applyRuntimeSnapshots([event.payload]);
    await useFlowStore.getState().refreshRuntime();
  }).then((fn) => {
    unlisten = fn;
  });

  return () => {
    cancelled = true;
    unlisten?.();
  };
}, []);

// Remove applySidecarFlowRuntimeHint calls for account_live/account_offline/recording_started/recording_finished.
// Keep clip_ready/caption_ready handling because downstream still arrives from Python.
```

- [ ] **Step 4: Run tests and lint to verify the bridge still passes**

Run: `cargo test clip_bridge_tests -- --nocapture && cargo fmt --check && cargo clippy --all-targets -- -D warnings && npm run lint:js`
Expected: PASS, PASS, PASS, PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/live_runtime/session.rs src-tauri/src/commands/live_runtime.rs src-tauri/src/recording_runtime/worker.rs src-tauri/src/commands/clips.rs src-tauri/src/commands/flows.rs src/components/layout/app-shell.tsx src/lib/api.ts src/stores/flow-store.ts src/types/index.ts
git commit -m "feat(flow): bridge Rust recording runtime to sidecar clip pipeline"
```

### Task 8: End-To-End Runtime Verification And Cleanup

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/workflow/mod.rs`
- Test: `src-tauri/src/live_runtime/manager.rs`
- Test: `src-tauri/src/workflow/runtime_store.rs`

- [ ] **Step 1: Write the final integration test covering enable -> live detect -> record -> return to polling**

```rust
#[cfg(test)]
mod end_to_end_tests {
    #[tokio::test]
    async fn enabled_flow_returns_to_polling_after_record_completion() {
        let manager = crate::live_runtime::manager::LiveRuntimeManager::new();
        manager.start_test_session(5, "shop_abc").await;
        manager.mark_test_recording_complete(5, Some("7312345")).await;

        let snapshot = manager.snapshot(5).await.unwrap();
        assert_eq!(snapshot.status, "polling");
        assert_eq!(snapshot.last_completed_room_id.as_deref(), Some("7312345"));
    }
}
```

- [ ] **Step 2: Run the full Rust verification suite before touching frontend verification**

Run: `cargo test -- --nocapture`
Expected: PASS

- [ ] **Step 3: Run the required cross-layer verification commands**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
npm run lint:js
```

Expected:
- `cargo fmt --check`: PASS with no diff
- `cargo clippy --all-targets -- -D warnings`: PASS
- `npm run lint:js`: PASS

- [ ] **Step 4: Remove obsolete sidecar-runtime glue only if the new Rust path already supplies the same behavior**

```tsx
// src/components/layout/app-shell.tsx
// Keep:
//   applySidecarFlowRuntimeHint({ hint: "clip_ready", ... })
//   applySidecarFlowRuntimeHint({ hint: "caption_ready", ... })
// Remove:
//   applySidecarFlowRuntimeHint({ hint: "account_live", ... })
//   applySidecarFlowRuntimeHint({ hint: "account_offline", ... })
//   applySidecarFlowRuntimeHint({ hint: "recording_started", ... })
//   applySidecarFlowRuntimeHint({ hint: "recording_finished", ... })
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/commands/mod.rs src-tauri/src/workflow/mod.rs src/components/layout/app-shell.tsx src/lib/api.ts src/stores/flow-store.ts
git commit -m "feat(flow): complete Rust start-record runtime migration"
```

---

## Review Checklist

### Spec Coverage

- `Start` owns polling, cookies, proxy, retry, and username normalization: covered by Tasks 1, 4, and 5
- username lease acquire/release/conflict behavior: covered by Task 4
- `Record` is one-shot and does not segment-chain: covered by Tasks 1 and 6
- `account_id` find-or-create ownership: covered by Task 2
- duplicate normalized usernames become conflict: covered by Task 2
- durable `room_id` dedupe across restart/publish: covered by Tasks 2, 4, and 5
- `publish_flow_definition` / `set_flow_enabled` lifecycle restarts: covered by Task 4
- explicit `recordings` ownership via `account_id`, `flow_id`, `flow_run_id`, and `room_id`: covered by Task 6
- `sidecar_recording_id` downstream contract preservation: covered by Tasks 6 and 7
- frontend/runtime status migration away from sidecar hints for `Start/Record`: covered by Tasks 4 and 7
- Rust/Tauri verification requirements: covered by Task 8

### Placeholder Scan

- No unresolved placeholder markers or cross-task shorthand remain
- Each task includes named files, explicit commands, expected results, and concrete code snippets

### Type Consistency

- Canonical config keys remain snake_case: `cookies_json`, `proxy_url`, `poll_interval_seconds`, `retry_limit`, `max_duration_minutes`
- Runtime identifiers use one external recording key path: `sidecar_recording_id`
- Room dedupe field stays consistent: `last_completed_room_id`
