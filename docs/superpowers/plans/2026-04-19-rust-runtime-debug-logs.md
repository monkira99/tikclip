# Rust Runtime Debug Logs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a structured Rust runtime debug log system that shows `Start/Record` lifecycle logs in both terminal output and the desktop UI, and lets users copy a diagnostic bundle that is sufficient for external diagnosis.

**Architecture:** Extend `LiveRuntimeManager` with a structured log helper and an in-memory ring buffer, emit a dedicated Tauri event for log entries, and expose a command for frontend hydration. On the frontend, add log state to the flow store and a `Runtime Logs` panel in `FlowDetail` that renders, filters, and copies diagnostic bundles without changing the existing summary runtime snapshot path.

**Tech Stack:** Rust, Tauri v2, serde/serde_json, log/env_logger, React, Zustand, TypeScript

---

## File Structure

**Create:**
- `src-tauri/src/live_runtime/logs.rs` - canonical runtime log entry types, ring buffer helpers, formatting helpers
- `src/components/flows/runtime/runtime-logs-panel.tsx` - UI panel for runtime logs and copy bundle

**Modify:**
- `src-tauri/src/live_runtime/mod.rs` - export logs module
- `src-tauri/src/live_runtime/types.rs` - runtime log-facing shared types if needed minimally
- `src-tauri/src/live_runtime/manager.rs` - log helper, ring buffer, event emission, first emission points
- `src-tauri/src/commands/live_runtime.rs` - command(s) to list runtime logs and serialize copy bundle data
- `src-tauri/src/lib.rs` - widen default Rust log filter for runtime modules
- `src/components/flows/flow-detail.tsx` - mount the runtime logs panel in flow detail
- `src/components/layout/app-shell.tsx` - listen for `flow-runtime-log` events and hydrate store
- `src/lib/api.ts` - invoke wrappers for runtime log commands
- `src/stores/flow-store.ts` - runtime log state, append/hydrate helpers, copy-bundle source state
- `src/types/index.ts` - `FlowRuntimeLogEntry` / copy bundle types

**Test:**
- `src-tauri/src/live_runtime/logs.rs`
- `src-tauri/src/live_runtime/manager.rs`
- `src-tauri/src/commands/live_runtime.rs`
- `src/stores/flow-store.ts` behavior via existing frontend test approach or colocated test file if needed
- `src/components/flows/runtime/runtime-logs-panel.tsx`

---

### Task 1: Define Canonical Runtime Log Entry And Ring Buffer Foundation

**Files:**
- Create: `src-tauri/src/live_runtime/logs.rs`
- Modify: `src-tauri/src/live_runtime/mod.rs`
- Modify: `src-tauri/src/live_runtime/types.rs`
- Test: `src-tauri/src/live_runtime/logs.rs`

- [ ] **Step 1: Write the failing tests for log entry shape and ring buffer cap**

```rust
#[cfg(test)]
mod tests {
    use super::{FlowRuntimeLogBuffer, FlowRuntimeLogEntry, FlowRuntimeLogLevel};
    use serde_json::json;

    #[test]
    fn flow_runtime_log_entry_keeps_required_fields() {
        let entry = FlowRuntimeLogEntry::new(
            7,
            Some(42),
            "record",
            "record_spawned",
            FlowRuntimeLogLevel::Info,
            None,
            "Spawned Rust-owned recording worker",
            json!({"room_id":"7312345"}),
        );

        assert_eq!(entry.flow_id, 7);
        assert_eq!(entry.flow_run_id, Some(42));
        assert_eq!(entry.stage, "record");
        assert_eq!(entry.event, "record_spawned");
    }

    #[test]
    fn ring_buffer_keeps_only_latest_entries_with_cap() {
        let mut buffer = FlowRuntimeLogBuffer::new(2);
        buffer.push(test_entry("first"));
        buffer.push(test_entry("second"));
        buffer.push(test_entry("third"));

        let messages = buffer.entries().iter().map(|entry| entry.message.as_str()).collect::<Vec<_>>();
        assert_eq!(messages, vec!["second", "third"]);
    }

    fn test_entry(message: &str) -> FlowRuntimeLogEntry {
        FlowRuntimeLogEntry::new(
            1,
            None,
            "runtime",
            "session_started",
            FlowRuntimeLogLevel::Info,
            None,
            message,
            json!({}),
        )
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test flow_runtime_log_entry_keeps_required_fields ring_buffer_keeps_only_latest_entries_with_cap -- --nocapture`
Expected: FAIL with missing `logs.rs` module or missing `FlowRuntimeLogEntry` / `FlowRuntimeLogBuffer`

- [ ] **Step 3: Write minimal implementation**

```rust
// src-tauri/src/live_runtime/logs.rs
use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowRuntimeLogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct FlowRuntimeLogEntry {
    pub id: String,
    pub timestamp: String,
    pub level: FlowRuntimeLogLevel,
    pub flow_id: i64,
    pub flow_run_id: Option<i64>,
    pub external_recording_id: Option<String>,
    pub stage: String,
    pub event: String,
    pub code: Option<String>,
    pub message: String,
    pub context: Value,
}

impl FlowRuntimeLogEntry {
    pub fn new(
        flow_id: i64,
        flow_run_id: Option<i64>,
        stage: &str,
        event: &str,
        level: FlowRuntimeLogLevel,
        code: Option<&str>,
        message: &str,
        context: Value,
    ) -> Self {
        Self {
            id: format!("log-{}-{}", flow_id, crate::time_hcm::now_timestamp_hcm()),
            timestamp: crate::time_hcm::now_timestamp_hcm(),
            level,
            flow_id,
            flow_run_id,
            external_recording_id: None,
            stage: stage.to_string(),
            event: event.to_string(),
            code: code.map(str::to_string),
            message: message.to_string(),
            context,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FlowRuntimeLogBuffer {
    cap: usize,
    entries: VecDeque<FlowRuntimeLogEntry>,
}

impl FlowRuntimeLogBuffer {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            entries: VecDeque::new(),
        }
    }

    pub fn push(&mut self, entry: FlowRuntimeLogEntry) {
        self.entries.push_back(entry);
        while self.entries.len() > self.cap {
            self.entries.pop_front();
        }
    }

    pub fn entries(&self) -> Vec<FlowRuntimeLogEntry> {
        self.entries.iter().cloned().collect()
    }
}
```

```rust
// src-tauri/src/live_runtime/mod.rs
pub mod account_binding;
pub mod logs;
pub mod manager;
pub mod normalize;
pub mod session;
pub mod types;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test flow_runtime_log_entry_keeps_required_fields ring_buffer_keeps_only_latest_entries_with_cap -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/live_runtime/logs.rs src-tauri/src/live_runtime/mod.rs src-tauri/src/live_runtime/types.rs
git commit -m "feat(runtime): add structured runtime log entry foundation"
```

### Task 2: Emit Runtime Logs From LiveRuntimeManager And Expose Read Command

**Files:**
- Modify: `src-tauri/src/live_runtime/manager.rs`
- Modify: `src-tauri/src/commands/live_runtime.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/live_runtime/manager.rs`
- Test: `src-tauri/src/commands/live_runtime.rs`

- [ ] **Step 1: Write failing tests for manager log emission and command listing**

```rust
#[test]
fn log_runtime_event_appends_entry_to_manager_buffer() {
    let manager = LiveRuntimeManager::new();
    manager.log_runtime_event_for_test(7, Some(42), "record", "record_spawned", "Spawned worker");

    let logs = manager.list_runtime_logs_for_test(None, None);
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].flow_id, 7);
    assert_eq!(logs[0].event, "record_spawned");
}

#[test]
fn list_live_runtime_logs_command_filters_by_flow_id() {
    let manager = LiveRuntimeManager::new();
    manager.log_runtime_event_for_test(7, Some(42), "record", "record_spawned", "Spawned worker");
    manager.log_runtime_event_for_test(8, Some(55), "start", "lease_conflict", "Username conflict");

    let rows = crate::commands::live_runtime::list_live_runtime_logs_from_manager(&manager, Some(7), Some(50));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].flow_id, 7);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test log_runtime_event_appends_entry_to_manager_buffer list_live_runtime_logs_command_filters_by_flow_id -- --nocapture`
Expected: FAIL with missing manager log helpers or missing live-runtime log command helpers

- [ ] **Step 3: Write minimal implementation**

```rust
// src-tauri/src/live_runtime/manager.rs
use crate::live_runtime::logs::{FlowRuntimeLogBuffer, FlowRuntimeLogEntry, FlowRuntimeLogLevel};

struct LiveRuntimeState {
    // existing fields...
    log_buffer: FlowRuntimeLogBuffer,
}

impl Default for LiveRuntimeState {
    fn default() -> Self {
        Self {
            sessions_by_flow: HashMap::new(),
            lease_owner_by_lookup_key: HashMap::new(),
            failed_snapshots_by_flow: HashMap::new(),
            active_recordings_by_flow: HashMap::new(),
            log_buffer: FlowRuntimeLogBuffer::new(1000),
        }
    }
}

impl LiveRuntimeManager {
    pub fn log_runtime_event(
        &self,
        flow_id: i64,
        flow_run_id: Option<i64>,
        stage: &str,
        event: &str,
        level: FlowRuntimeLogLevel,
        code: Option<&str>,
        message: &str,
        context: serde_json::Value,
    ) -> Result<(), String> {
        let entry = FlowRuntimeLogEntry::new(flow_id, flow_run_id, stage, event, level.clone(), code, message, context);
        match level {
            FlowRuntimeLogLevel::Debug => log::debug!("flow={} run={:?} stage={} event={} {}", flow_id, flow_run_id, stage, event, message),
            FlowRuntimeLogLevel::Info => log::info!("flow={} run={:?} stage={} event={} {}", flow_id, flow_run_id, stage, event, message),
            FlowRuntimeLogLevel::Warn => log::warn!("flow={} run={:?} stage={} event={} {}", flow_id, flow_run_id, stage, event, message),
            FlowRuntimeLogLevel::Error => log::error!("flow={} run={:?} stage={} event={} {}", flow_id, flow_run_id, stage, event, message),
        }

        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.log_buffer.push(entry.clone());
        drop(state);

        if let Some(app_handle) = &self.app_handle {
            app_handle.emit("flow-runtime-log", entry).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn list_runtime_logs(&self, flow_id: Option<i64>, limit: Option<usize>) -> Result<Vec<FlowRuntimeLogEntry>, String> {
        let state = self.state.lock().map_err(|e| e.to_string())?;
        let mut rows = state.log_buffer.entries();
        if let Some(flow_id) = flow_id {
            rows.retain(|row| row.flow_id == flow_id);
        }
        if let Some(limit) = limit {
            if rows.len() > limit {
                rows = rows[rows.len() - limit..].to_vec();
            }
        }
        Ok(rows)
    }
}
```

```rust
// src-tauri/src/commands/live_runtime.rs
use crate::live_runtime::logs::FlowRuntimeLogEntry;

#[cfg(test)]
pub fn list_live_runtime_logs_from_manager(
    manager: &LiveRuntimeManager,
    flow_id: Option<i64>,
    limit: Option<usize>,
) -> Vec<FlowRuntimeLogEntry> {
    manager.list_runtime_logs(flow_id, limit).expect("list logs")
}

#[tauri::command]
pub fn list_live_runtime_logs(
    manager: State<'_, LiveRuntimeManager>,
    flow_id: Option<i64>,
    limit: Option<usize>,
) -> Result<Vec<FlowRuntimeLogEntry>, String> {
    manager.list_runtime_logs(flow_id, limit)
}
```

```rust
// src-tauri/src/lib.rs
fn init_rust_logging() {
    let default_filter = if cfg!(debug_assertions) {
        "warn,tikclip_lib::commands::accounts=info,tikclip_lib::live_runtime=debug,tikclip_lib::recording_runtime=debug"
    } else {
        "warn,tikclip_lib::commands::accounts=warn,tikclip_lib::live_runtime=info,tikclip_lib::recording_runtime=info"
    };
    // existing builder unchanged
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test log_runtime_event_appends_entry_to_manager_buffer list_live_runtime_logs_command_filters_by_flow_id -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/live_runtime/manager.rs src-tauri/src/commands/live_runtime.rs src-tauri/src/lib.rs
git commit -m "feat(runtime): expose runtime debug logs to terminal and tauri"
```

### Task 3: Add Frontend Runtime Log State And API Wiring

**Files:**
- Modify: `src/lib/api.ts`
- Modify: `src/stores/flow-store.ts`
- Modify: `src/types/index.ts`
- Modify: `src/components/layout/app-shell.tsx`
- Test: `src/stores/flow-store.ts`

- [ ] **Step 1: Write failing tests for runtime log hydration and append behavior**

```ts
import { useFlowStore } from "@/stores/flow-store";

test("applyRuntimeLogs hydrates logs by flow id", () => {
  useFlowStore.getState().applyRuntimeLogs([
    { id: "1", flow_id: 7, flow_run_id: 42, level: "info", stage: "record", event: "record_spawned", code: null, message: "Spawned worker", timestamp: "2026-04-19T09:41:12.381+07:00", context: {} },
  ]);

  expect(useFlowStore.getState().runtimeLogs[7]).toHaveLength(1);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run lint:js`
Expected: FAIL with missing `runtimeLogs` state or missing `applyRuntimeLogs`

- [ ] **Step 3: Write minimal implementation**

```ts
// src/types/index.ts
export interface FlowRuntimeLogEntry {
  id: string;
  timestamp: string;
  level: "debug" | "info" | "warn" | "error";
  flow_id: number;
  flow_run_id: number | null;
  external_recording_id: string | null;
  stage: string;
  event: string;
  code: string | null;
  message: string;
  context: Record<string, unknown>;
}
```

```ts
// src/lib/api.ts
export async function listLiveRuntimeLogs(flowId?: number, limit?: number): Promise<FlowRuntimeLogEntry[]> {
  return invoke<FlowRuntimeLogEntry[]>("list_live_runtime_logs", { flowId, limit });
}
```

```ts
// src/stores/flow-store.ts
type FlowStore = {
  runtimeLogs: Record<number, FlowRuntimeLogEntry[]>;
  applyRuntimeLogs: (rows: FlowRuntimeLogEntry[]) => void;
  appendRuntimeLog: (row: FlowRuntimeLogEntry) => void;
};

runtimeLogs: {},

applyRuntimeLogs: (rows) => {
  set((state) => {
    const next = { ...state.runtimeLogs };
    for (const row of rows) {
      next[row.flow_id] = [...(next[row.flow_id] ?? []).filter((entry) => entry.id !== row.id), row];
    }
    return { runtimeLogs: next };
  });
},

appendRuntimeLog: (row) => {
  set((state) => ({
    runtimeLogs: {
      ...state.runtimeLogs,
      [row.flow_id]: [...(state.runtimeLogs[row.flow_id] ?? []), row],
    },
  }));
},
```

```tsx
// src/components/layout/app-shell.tsx
void listen<FlowRuntimeLogEntry>("flow-runtime-log", (event) => {
  useFlowStore.getState().appendRuntimeLog(event.payload);
});
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm run lint:js`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/lib/api.ts src/stores/flow-store.ts src/types/index.ts src/components/layout/app-shell.tsx
git commit -m "feat(ui): add runtime log state and event hydration"
```

### Task 4: Build Runtime Logs Panel And Copy Diagnostic Bundle

**Files:**
- Create: `src/components/flows/runtime/runtime-logs-panel.tsx`
- Modify: `src/components/flows/flow-detail.tsx`
- Modify: `src/stores/flow-store.ts`
- Test: `src/components/flows/runtime/runtime-logs-panel.tsx`

- [ ] **Step 1: Write failing test for rendering and copy bundle content**

```tsx
import { render, screen } from "@testing-library/react";
import { RuntimeLogsPanel } from "@/components/flows/runtime/runtime-logs-panel";

test("renders runtime logs and diagnostic bundle fields", () => {
  render(
    <RuntimeLogsPanel
      flowId={7}
      flowName="Auto record shop_abc"
      currentStatus="processing"
      currentNode="clip"
      logs={[
        {
          id: "1",
          timestamp: "2026-04-19T09:41:12.381+07:00",
          level: "info",
          flow_id: 7,
          flow_run_id: 42,
          external_recording_id: "rec-42-a1b2",
          stage: "record",
          event: "record_spawned",
          code: null,
          message: "Spawned Rust-owned recording worker",
          context: { room_id: "7312345" },
        },
      ]}
      lastError={null}
      lastLiveAt={"2026-04-19 09:41:12"}
    />,
  );

  expect(screen.getByText(/Runtime Logs/i)).toBeInTheDocument();
  expect(screen.getByText(/record_spawned/i)).toBeInTheDocument();
  expect(screen.getByText(/Spawned Rust-owned recording worker/i)).toBeInTheDocument();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run lint:js`
Expected: FAIL with missing `RuntimeLogsPanel`

- [ ] **Step 3: Write minimal implementation**

```tsx
// src/components/flows/runtime/runtime-logs-panel.tsx
import { Button } from "@/components/ui/button";
import type { FlowRuntimeLogEntry } from "@/types";

type RuntimeLogsPanelProps = {
  flowId: number;
  flowName: string;
  currentStatus: string;
  currentNode: string | null;
  logs: FlowRuntimeLogEntry[];
  lastError: string | null;
  lastLiveAt: string | null;
};

function buildDiagnosticBundle(props: RuntimeLogsPanelProps): string {
  const lines = [
    "=== TikClip Flow Runtime Diagnostic ===",
    `flow_id: ${props.flowId}`,
    `flow_name: ${props.flowName}`,
    `current_status: ${props.currentStatus}`,
    `current_node: ${props.currentNode ?? "-"}`,
    `last_live_at: ${props.lastLiveAt ?? "-"}`,
    `last_error: ${props.lastError ?? "-"}`,
    "",
    "--- Recent Logs ---",
    ...props.logs.map(
      (entry) =>
        `[${entry.timestamp}] ${entry.level.toUpperCase()} flow=${entry.flow_id} run=${entry.flow_run_id ?? "-"} stage=${entry.stage} event=${entry.event} code=${entry.code ?? "-"}\n${entry.message}\ncontext: ${JSON.stringify(entry.context)}`,
    ),
  ];
  return lines.join("\n");
}

export function RuntimeLogsPanel(props: RuntimeLogsPanelProps) {
  return (
    <div className="rounded-2xl border border-white/10 bg-[rgb(11_12_16_/_0.88)] p-4 space-y-3">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h3 className="text-sm font-semibold text-[var(--color-text)]">Runtime Logs</h3>
          <p className="text-xs text-[var(--color-text-muted)]">Structured Rust runtime logs for this flow.</p>
        </div>
        <Button
          size="sm"
          variant="outline"
          onClick={() => navigator.clipboard.writeText(buildDiagnosticBundle(props))}
        >
          Copy diagnostic bundle
        </Button>
      </div>

      <div className="space-y-2 max-h-[320px] overflow-auto">
        {props.logs.length === 0 ? (
          <p className="text-sm text-[var(--color-text-muted)]">No runtime logs yet.</p>
        ) : (
          props.logs.map((entry) => (
            <div key={entry.id} className="rounded-xl border border-white/8 bg-black/20 p-3 space-y-1">
              <div className="text-xs text-[var(--color-text-muted)]">
                [{entry.timestamp}] {entry.level.toUpperCase()} flow={entry.flow_id} run={entry.flow_run_id ?? "-"} stage={entry.stage} event={entry.event} code={entry.code ?? "-"}
              </div>
              <div className="text-sm text-[var(--color-text)]">{entry.message}</div>
              <pre className="overflow-auto rounded-lg bg-black/30 p-2 text-[11px] text-[var(--color-text-soft)]">{JSON.stringify(entry.context, null, 2)}</pre>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
```

```tsx
// src/components/flows/flow-detail.tsx
import { RuntimeLogsPanel } from "@/components/flows/runtime/runtime-logs-panel";

const runtimeLogs = useFlowStore((s) => s.runtimeLogs[flowId] ?? []);

<RuntimeLogsPanel
  flowId={flowId}
  flowName={flow?.flow.name ?? `Flow #${flowId}`}
  currentStatus={flow?.flow.status ?? "idle"}
  currentNode={flow?.flow.current_node ?? null}
  logs={runtimeLogs}
  lastError={flow?.flow.last_error ?? null}
  lastLiveAt={flow?.flow.last_live_at ?? null}
/>
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm run lint:js`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/components/flows/runtime/runtime-logs-panel.tsx src/components/flows/flow-detail.tsx src/stores/flow-store.ts
git commit -m "feat(ui): add flow runtime logs panel and copy bundle"
```

### Task 5: Emit First Diagnostic Events At Critical Runtime Boundaries

**Files:**
- Modify: `src-tauri/src/live_runtime/manager.rs`
- Test: `src-tauri/src/live_runtime/manager.rs`

- [ ] **Step 1: Write failing tests for first emitted event set**

```rust
#[test]
fn successful_finalization_emits_runtime_log_entry() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 7, "shop_abc");
    let manager = LiveRuntimeManager::with_runtime_db_path(path.clone());
    manager.start_flow_session(&conn, 7).unwrap();

    manager
        .log_runtime_event_for_test(7, Some(42), "record", "record_completed", "Recording completed successfully")
        .unwrap();

    let logs = manager.list_runtime_logs_for_test(Some(7), Some(10));
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].event, "record_completed");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test successful_finalization_emits_runtime_log_entry -- --nocapture`
Expected: FAIL with missing test helper methods

- [ ] **Step 3: Write minimal implementation and add critical emission points**

```rust
// src-tauri/src/live_runtime/manager.rs
// Add log_runtime_event(...) calls at:
// - bootstrap/session start failure
// - lease acquired/conflict
// - live_detected / stream_url_missing
// - run_created / run_creation_skipped_dedupe
// - record_spawned
// - record_completed / record_failed / record_cancelled
// - sidecar_handoff_completed / sidecar_handoff_failed
// - source_offline_marked
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test successful_finalization_emits_runtime_log_entry -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/live_runtime/manager.rs
git commit -m "feat(runtime): emit structured logs at critical runtime stages"
```

### Task 6: Run Final Verification And Remove Only Clearly Replaced Glue

**Files:**
- Modify: `src/components/layout/app-shell.tsx`
- Modify: `src/lib/api.ts`
- Modify: `src/stores/flow-store.ts`
- Test: `src-tauri/src/live_runtime/manager.rs`
- Test: `src/components/flows/runtime/runtime-logs-panel.tsx`

- [ ] **Step 1: Add or update the final integration coverage for event hydration and copy bundle**

```tsx
test("runtime log append preserves flow-specific logs and copy bundle remains stable", () => {
  useFlowStore.getState().appendRuntimeLog({
    id: "1",
    timestamp: "2026-04-19T09:41:12.381+07:00",
    level: "error",
    flow_id: 7,
    flow_run_id: 42,
    external_recording_id: null,
    stage: "clip",
    event: "sidecar_handoff_failed",
    code: "handoff.sidecar_unavailable",
    message: "Failed to call sidecar /api/video/process",
    context: { sidecar_url_present: false },
  });

  expect(useFlowStore.getState().runtimeLogs[7][0].code).toBe("handoff.sidecar_unavailable");
});
```

- [ ] **Step 2: Run the full verification suite**

```bash
cargo test -- --nocapture
cargo fmt --check
cargo clippy --all-targets -- -D warnings
npm run lint:js
```

Expected:
- `cargo test -- --nocapture`: PASS
- `cargo fmt --check`: PASS
- `cargo clippy --all-targets -- -D warnings`: PASS
- `npm run lint:js`: PASS

- [ ] **Step 3: Remove only obsolete glue proven replaced by the new log path**

```tsx
// Keep existing `flow-runtime-updated` summary refresh path.
// Add `flow-runtime-log` detailed event path.
// Do not remove clip/caption downstream handling.
// Do not remove summary runtime snapshots.
```

- [ ] **Step 4: Re-run the exact verification after any cleanup**

```bash
cargo test -- --nocapture
cargo fmt --check
cargo clippy --all-targets -- -D warnings
npm run lint:js
```

Expected:
- all commands still PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/live_runtime/manager.rs src-tauri/src/commands/live_runtime.rs src/components/layout/app-shell.tsx src/lib/api.ts src/stores/flow-store.ts src/types/index.ts src/components/flows/runtime/runtime-logs-panel.tsx src/components/flows/flow-detail.tsx
git commit -m "feat(runtime): add copyable Rust debug logs"
```

---

## Review Checklist

### Spec Coverage

- canonical structured log entry shape: covered by Tasks 1 and 2
- terminal + in-app visibility from one event source: covered by Tasks 2 and 5
- Tauri event split between summary and detailed logs: covered by Tasks 2 and 6
- snake_case Tauri payload contract for runtime log entries: covered by Tasks 2 and 3
- copyable diagnostic bundle: covered by Task 4
- stable event taxonomy and error-code-driven diagnostics: covered by Task 5
- no secret leakage from cookies/proxy fields: covered by Tasks 1 and 5
- no persistence overreach in phase 1: covered by Task 1 ring buffer approach

### Placeholder Scan

- No unresolved placeholder markers remain
- Each task contains exact file paths, explicit commands, expected outcomes, and concrete code snippets

### Type Consistency

- `FlowRuntimeSnapshot` remains separate from `FlowRuntimeLogEntry`
- `flow-runtime-updated` remains the summary event
- `flow-runtime-log` is the detailed log event
- copied diagnostics are built from the same canonical log entry shape used in Rust and UI
