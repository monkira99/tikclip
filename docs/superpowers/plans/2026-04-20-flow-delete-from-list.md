# Flow Delete From List Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a confirmed `Delete` action to each flow card in the list view and remove the deleted flow cleanly from both frontend state and Tauri runtime/SQLite state.

**Architecture:** Extend the existing flow stack with one new API/store action and one new Tauri command. Keep the list-level busy handling in `FlowList`, the confirm dialog state in `FlowCard`, and the actual deletion logic in `flow-store` plus a transactional Rust command that also stops any active runtime session for the deleted flow.

**Tech Stack:** React 19, TypeScript, Zustand, Tauri v2, Rust, rusqlite, node:test

---

## File Structure

**Create:**
- None

**Modify:**
- `src/lib/api.ts` - add `deleteFlow(flowId)` invoke wrapper for the new `delete_flow` command
- `src/stores/flow-store.ts` - add delete action and local state cleanup for deleted flows
- `src/stores/flow-store.test.ts` - cover store cleanup and rollback/error behavior
- `src/components/flows/flow-list.tsx` - wire list-level delete handler into the existing `busyFlowIds` pattern
- `src/components/flows/flow-card.tsx` - add delete button, confirmation dialog, and pending-state disabling
- `src/types/index.ts` - no new type shape required unless a local prop helper becomes necessary; avoid changes unless implementation proves it needs them
- `src-tauri/src/commands/flows.rs` - add `delete_flow` command and tests around transactional/runtime cleanup
- `src-tauri/src/lib.rs` - register `delete_flow` in the Tauri invoke handler list

**Reference / inspect while implementing:**
- `src/components/products/product-card.tsx` - existing dialog-based delete confirmation pattern
- `src/components/ui/dialog.tsx` - dialog primitives already used in the app
- `src-tauri/src/live_runtime/manager.rs` - `stop_flow_session(flow_id)` runtime cleanup path
- `src-tauri/src/db/migrations/008_flow_engine_rebuild.sql` - `flows`, `flow_nodes`, `flow_runs`, `flow_node_runs` cascade relationships
- `src-tauri/src/db/migrations/009_repair_flow_foreign_keys.sql` - `recordings.flow_id` and `clips.flow_id` set-null behavior during flow deletion

**Test:**
- `src/stores/flow-store.test.ts`
- `src-tauri/src/commands/flows.rs`

---

### Task 1: Add The Frontend Delete API And Store Cleanup

**Files:**
- Modify: `src/lib/api.ts`
- Modify: `src/stores/flow-store.ts`
- Modify: `src/stores/flow-store.test.ts`

- [ ] **Step 1: Write the failing store tests for successful deletion and failed rollback**

Add these tests near the existing `flow-store` tests in `src/stores/flow-store.test.ts`:

```ts
test("deleteFlow removes the flow and clears active runtime/editor state on success", async (t) => {
  resetFlowStore();

  useFlowStore.setState({
    flows: [
      {
        id: 7,
        account_id: 44,
        account_username: "shop_abc",
        name: "Night Shift Recorder",
        enabled: true,
        status: "watching",
        current_node: "start",
        last_live_at: null,
        last_run_at: null,
        last_error: null,
        published_version: 1,
        draft_version: 1,
        recordings_count: 0,
        clips_count: 0,
        captions_count: 0,
        created_at: "2026-04-20T10:00:00.000+07:00",
        updated_at: "2026-04-20T10:00:00.000+07:00",
      },
      {
        id: 8,
        account_id: 45,
        account_username: "shop_xyz",
        name: "Fallback Flow",
        enabled: true,
        status: "idle",
        current_node: null,
        last_live_at: null,
        last_run_at: null,
        last_error: null,
        published_version: 1,
        draft_version: 1,
        recordings_count: 0,
        clips_count: 0,
        captions_count: 0,
        created_at: "2026-04-20T10:00:00.000+07:00",
        updated_at: "2026-04-20T10:00:00.000+07:00",
      },
    ],
    runtimeSnapshots: {
      7: createRuntimeSnapshot({ flow_id: 7 }),
      8: createRuntimeSnapshot({ flow_id: 8, username: "shop_xyz" }),
    },
    runtimeLogs: {
      7: [runtimeLogEntry({ id: "log-7", flow_id: 7 })],
      8: [runtimeLogEntry({ id: "log-8", flow_id: 8 })],
    },
    activeFlowId: 7,
    activeFlow: createActiveFlowPayload(7, "Night Shift Recorder"),
    view: "detail",
    selectedNode: "record",
    error: "stale error",
  });

  const deleteFlow = t.mock.method(flowStoreApi, "deleteFlow", async (flowId: number) => {
    assert.equal(flowId, 7);
  });

  await useFlowStore.getState().deleteFlow(7);

  assert.equal(deleteFlow.mock.callCount(), 1);
  assert.deepEqual(useFlowStore.getState().flows.map((flow) => flow.id), [8]);
  assert.equal(useFlowStore.getState().runtimeSnapshots[7], undefined);
  assert.equal(useFlowStore.getState().runtimeLogs[7], undefined);
  assert.equal(useFlowStore.getState().activeFlowId, null);
  assert.equal(useFlowStore.getState().activeFlow, null);
  assert.equal(useFlowStore.getState().selectedNode, null);
  assert.equal(useFlowStore.getState().view, "list");
  assert.equal(useFlowStore.getState().error, null);
});

test("deleteFlow keeps local state and surfaces an error when backend deletion fails", async (t) => {
  resetFlowStore();

  useFlowStore.setState({
    flows: [
      {
        id: 7,
        account_id: 44,
        account_username: "shop_abc",
        name: "Night Shift Recorder",
        enabled: true,
        status: "watching",
        current_node: "start",
        last_live_at: null,
        last_run_at: null,
        last_error: null,
        published_version: 1,
        draft_version: 1,
        recordings_count: 0,
        clips_count: 0,
        captions_count: 0,
        created_at: "2026-04-20T10:00:00.000+07:00",
        updated_at: "2026-04-20T10:00:00.000+07:00",
      },
    ],
    runtimeSnapshots: {
      7: createRuntimeSnapshot({ flow_id: 7 }),
    },
    runtimeLogs: {
      7: [runtimeLogEntry({ id: "log-7", flow_id: 7 })],
    },
    activeFlowId: 7,
    activeFlow: createActiveFlowPayload(7, "Night Shift Recorder"),
    view: "detail",
    selectedNode: "record",
    error: null,
  });

  t.mock.method(flowStoreApi, "deleteFlow", async () => {
    throw new Error("delete failed in backend");
  });

  await assert.rejects(() => useFlowStore.getState().deleteFlow(7), /delete failed in backend/);

  assert.deepEqual(useFlowStore.getState().flows.map((flow) => flow.id), [7]);
  assert.notEqual(useFlowStore.getState().runtimeSnapshots[7], undefined);
  assert.notEqual(useFlowStore.getState().runtimeLogs[7], undefined);
  assert.equal(useFlowStore.getState().activeFlowId, 7);
  assert.equal(useFlowStore.getState().view, "detail");
  assert.equal(useFlowStore.getState().selectedNode, "record");
  assert.equal(useFlowStore.getState().error, "delete failed in backend");
});
```

Also add these helpers near the existing test helpers in the same file so the tests compile:

```ts
import type { FlowEditorPayload, FlowRuntimeLogEntry, FlowRuntimeSnapshot } from "@/types";

function createRuntimeSnapshot(overrides: Partial<FlowRuntimeSnapshot> = {}): FlowRuntimeSnapshot {
  return {
    flow_id: overrides.flow_id ?? 7,
    status: overrides.status ?? "watching",
    current_node: overrides.current_node ?? "start",
    account_id: overrides.account_id ?? 44,
    username: overrides.username ?? "shop_abc",
    last_live_at: overrides.last_live_at ?? null,
    last_error: overrides.last_error ?? null,
    active_flow_run_id: overrides.active_flow_run_id ?? 42,
  };
}

function createActiveFlowPayload(flowId: number, name: string): FlowEditorPayload {
  return {
    flow: {
      id: flowId,
      account_id: 44,
      name,
      enabled: true,
      status: "watching",
      current_node: "start",
      last_live_at: null,
      last_run_at: null,
      last_error: null,
      published_version: 1,
      draft_version: 1,
      created_at: "2026-04-20T10:00:00.000+07:00",
      updated_at: "2026-04-20T10:00:00.000+07:00",
    },
    nodes: [],
    runs: [],
    nodeRuns: [],
    recordings_count: 0,
    clips_count: 0,
  };
}
```

- [ ] **Step 2: Run the store test file to verify the new tests fail**

Run: `node --test src/stores/flow-store.test.ts`
Expected: FAIL because `flowStoreApi.deleteFlow` and `useFlowStore().deleteFlow` do not exist yet.

- [ ] **Step 3: Add the API wrapper and store action with rollback-safe cleanup**

In `src/lib/api.ts`, add a wrapper immediately after `setFlowEnabled`:

```ts
export async function deleteFlow(flowId: number): Promise<void> {
  await invoke("delete_flow", { flowId });
}
```

In `src/stores/flow-store.ts`, update the imports and store contract:

```ts
import {
  createFlow,
  deleteFlow,
  getFlowDefinition,
  listLiveRuntimeLogs,
  listLiveRuntimeSessions,
  listFlows,
  publishFlowDefinition,
  restartFlowRun,
  saveFlowNodeDraft,
  setFlowEnabled,
} from "@/lib/api";
```

```ts
  deleteFlow: (flowId: number) => Promise<void>;
```

```ts
  deleteFlow,
```

Then add the store implementation just before `createFlow`:

```ts
  deleteFlow: async (flowId) => {
    const previousState = get();

    set({ error: null });

    try {
      await flowStoreApi.deleteFlow(flowId);
      set((state) => {
        const nextRuntimeSnapshots = { ...state.runtimeSnapshots };
        delete nextRuntimeSnapshots[flowId];

        const nextRuntimeLogs = { ...state.runtimeLogs };
        delete nextRuntimeLogs[flowId];

        const deletingActiveFlow = state.activeFlowId === flowId || state.activeFlow?.flow.id === flowId;

        return {
          flows: state.flows.filter((flow) => flow.id !== flowId),
          runtimeSnapshots: nextRuntimeSnapshots,
          runtimeLogs: nextRuntimeLogs,
          activeFlowId: deletingActiveFlow ? null : state.activeFlowId,
          activeFlow: deletingActiveFlow ? null : state.activeFlow,
          selectedNode: deletingActiveFlow ? null : state.selectedNode,
          view: deletingActiveFlow ? "list" : state.view,
          error: null,
        };
      });
    } catch (error) {
      set({
        flows: previousState.flows,
        runtimeSnapshots: previousState.runtimeSnapshots,
        runtimeLogs: previousState.runtimeLogs,
        activeFlowId: previousState.activeFlowId,
        activeFlow: previousState.activeFlow,
        selectedNode: previousState.selectedNode,
        view: previousState.view,
        error: getErrorMessage(error, "Failed to delete flow"),
      });
      throw error;
    }
  },
```

- [ ] **Step 4: Run the store tests to verify they pass**

Run: `node --test src/stores/flow-store.test.ts`
Expected: PASS, including the two new deletion tests.

- [ ] **Step 5: Commit the API/store change**

```bash
git add src/lib/api.ts src/stores/flow-store.ts src/stores/flow-store.test.ts
git commit -m "feat(flows): add store-backed flow deletion"
```

### Task 2: Add The List-View Delete UI And Confirmation Dialog

**Files:**
- Modify: `src/components/flows/flow-list.tsx`
- Modify: `src/components/flows/flow-card.tsx`
- Reference: `src/components/products/product-card.tsx`
- Reference: `src/components/ui/dialog.tsx`

- [ ] **Step 1: Add a failing card-level unit test or, if no React component test harness exists, document the deliberate no-new-test choice in the diff and proceed with manual verification only**

Because this repo currently has `node:test` coverage for pure helpers/store logic but no established `@testing-library/react` setup for `FlowCard`, do not introduce a new frontend test harness just for this button. Instead, keep the code change small and rely on existing store tests plus manual verification for the card interaction.

Add this inline comment to your task notes or commit message rationale, not to the source code:

```text
No new component test added because the repo does not currently have a React DOM test harness for flow cards, and introducing one would exceed the scope of this small list-action change.
```

- [ ] **Step 2: Wire the delete handler into `FlowList` using the existing busy map**

In `src/components/flows/flow-list.tsx`, read the store action and add a delete handler:

```tsx
  const deleteFlow = useFlowStore((s) => s.deleteFlow);
```

```tsx
  const handleDelete = (flowId: number) => {
    setBusyFlowIds((prev) => ({ ...prev, [flowId]: true }));
    void deleteFlow(flowId)
      .catch(() => {
        /* store already keeps user-facing error state */
      })
      .finally(() => {
        setBusyFlowIds((prev) => {
          const next = { ...prev };
          delete next[flowId];
          return next;
        });
      });
  };
```

Then pass the callback into each card:

```tsx
            <FlowCard
              key={flow.id}
              flow={flow}
              busy={busyFlowIds[flow.id] === true}
              onOpen={onOpenFlow}
              onToggleEnabled={handleToggle}
              onDelete={handleDelete}
            />
```

- [ ] **Step 3: Add the delete button and dialog to `FlowCard`**

In `src/components/flows/flow-card.tsx`, add the imports:

```tsx
import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
```

Extend the props and component state:

```tsx
type FlowCardProps = {
  flow: FlowSummary;
  busy?: boolean;
  onOpen: (flowId: number) => void;
  onToggleEnabled: (flowId: number, enabled: boolean) => void;
  onDelete: (flowId: number) => void;
};

export function FlowCard({ flow, busy = false, onOpen, onToggleEnabled, onDelete }: FlowCardProps) {
  const [confirmOpen, setConfirmOpen] = useState(false);
  const currentNodeIndex = flow.current_node ? FLOW_STEPS.indexOf(flow.current_node) : -1;
```

Update the footer action cluster:

```tsx
          <Button variant="outline" size="sm" onClick={() => onOpen(flow.id)} disabled={busy}>
            Open
          </Button>
          <Button variant="destructive" size="sm" onClick={() => setConfirmOpen(true)} disabled={busy}>
            Delete
          </Button>
```

Add the dialog below the card content:

```tsx
      <Dialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <DialogContent showCloseButton={!busy} className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Delete flow?</DialogTitle>
            <DialogDescription>
              “{flow.name}” will be removed permanently. This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter className="border-0 bg-transparent p-0 sm:justify-end">
            <Button type="button" variant="outline" disabled={busy} onClick={() => setConfirmOpen(false)}>
              Cancel
            </Button>
            <Button
              type="button"
              variant="destructive"
              disabled={busy}
              onClick={() => {
                onDelete(flow.id);
                setConfirmOpen(false);
              }}
            >
              {busy ? "Deleting..." : "Delete"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
```

Keep the existing `Switch` disabled through the same `busy` prop.

- [ ] **Step 4: Run frontend lint/build verification for the UI change**

Run: `npm run lint:js`
Expected: PASS, including the build step included inside `lint:js`.

- [ ] **Step 5: Commit the list/card UI change**

```bash
git add src/components/flows/flow-list.tsx src/components/flows/flow-card.tsx
git commit -m "feat(flows): add delete action to flow cards"
```

### Task 3: Add The Tauri `delete_flow` Command And Runtime Cleanup

**Files:**
- Modify: `src-tauri/src/commands/flows.rs`
- Modify: `src-tauri/src/lib.rs`
- Reference: `src-tauri/src/live_runtime/manager.rs`
- Reference: `src-tauri/src/db/migrations/008_flow_engine_rebuild.sql`
- Reference: `src-tauri/src/db/migrations/009_repair_flow_foreign_keys.sql`

- [ ] **Step 1: Write failing Rust tests for missing-flow rejection and active-session cleanup**

In `src-tauri/src/commands/flows.rs`, extend the test module imports:

```rust
    use super::{
        apply_sidecar_flow_runtime_hint_with_conn, delete_flow_with_conn, map_flow_node_config_row,
        normalize_flow_node_config_json, set_flow_enabled_with_conn, ApplySidecarFlowRuntimeHintInput,
    };
```

Add these tests in the existing `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn delete_flow_with_conn_rejects_missing_flow() {
        let (mut conn, path) = open_temp_db();
        let runtime_manager = LiveRuntimeManager::new();

        let err = delete_flow_with_conn(&mut conn, &runtime_manager, 999).unwrap_err();

        assert_eq!(err, "flow 999 not found");

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn delete_flow_with_conn_stops_runtime_session_and_removes_flow_owned_rows() {
        let (mut conn, path) = open_temp_db();
        let runtime_manager = LiveRuntimeManager::new();

        conn.execute(
            "INSERT INTO accounts (id, username, display_name, type, created_at, updated_at) \
             VALUES (1, 'shop_abc', 'Shop', 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert account");
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) \
             VALUES (7, 'Night Shift Recorder', 1, 'watching', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert flow");
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
             VALUES (7, 'start', 1, '{\"username\":\"shop_abc\"}', '{\"username\":\"shop_abc\"}', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert start node");
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
             VALUES (7, 'record', 2, '{\"max_duration_minutes\":5}', '{\"max_duration_minutes\":5}', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert record node");
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (70, 7, 1, 'running', datetime('now','+7 hours'), 'test')",
            [],
        )
        .expect("insert flow run");
        conn.execute(
            "INSERT INTO flow_node_runs (flow_run_id, flow_id, node_key, status, started_at) \
             VALUES (70, 7, 'record', 'running', datetime('now','+7 hours'))",
            [],
        )
        .expect("insert flow node run");
        runtime_manager.start_flow_session(&conn, 7).expect("start flow session");

        delete_flow_with_conn(&mut conn, &runtime_manager, 7).expect("delete flow");

        let flow_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM flows WHERE id = 7", [], |row| row.get(0))
            .expect("count flows");
        let node_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM flow_nodes WHERE flow_id = 7", [], |row| row.get(0))
            .expect("count flow nodes");
        let run_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM flow_runs WHERE flow_id = 7", [], |row| row.get(0))
            .expect("count flow runs");
        let node_run_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM flow_node_runs WHERE flow_id = 7", [], |row| row.get(0))
            .expect("count flow node runs");

        assert_eq!(flow_count, 0);
        assert_eq!(node_count, 0);
        assert_eq!(run_count, 0);
        assert_eq!(node_run_count, 0);
        assert!(runtime_manager.list_sessions().is_empty());

        drop(conn);
        let _ = std::fs::remove_file(path);
    }
```

- [ ] **Step 2: Run the targeted Rust tests to verify they fail**

Run: `cargo test delete_flow_with_conn -- --nocapture`
Expected: FAIL because `delete_flow_with_conn` does not exist yet.

- [ ] **Step 3: Add the transactional delete command and register it in Tauri**

In `src-tauri/src/commands/flows.rs`, add a helper near `set_flow_enabled_with_conn`:

```rust
fn delete_flow_with_conn(
    conn: &mut Connection,
    runtime_manager: &LiveRuntimeManager,
    flow_id: i64,
) -> Result<(), String> {
    if flow_id <= 0 {
        return Err("flow_id must be positive".to_string());
    }

    let exists: i64 = conn
        .query_row("SELECT COUNT(*) FROM flows WHERE id = ?1", [flow_id], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if exists == 0 {
        return Err(format!("flow {flow_id} not found"));
    }

    runtime_manager.stop_flow_session(flow_id)?;

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM flows WHERE id = ?1", [flow_id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
```

Then add the public command near the other flow commands:

```rust
#[tauri::command]
pub fn delete_flow(
    state: State<'_, AppState>,
    runtime_manager: State<'_, LiveRuntimeManager>,
    flow_id: i64,
) -> Result<(), String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    delete_flow_with_conn(&mut conn, &runtime_manager, flow_id)
}
```

In `src-tauri/src/lib.rs`, register the command in the invoke list next to the other flow commands:

```rust
            commands::flows::delete_flow,
```

Do not add manual deletes for `flow_nodes`, `flow_runs`, or `flow_node_runs`; the schema already uses `ON DELETE CASCADE` for those tables. Let SQLite enforce the relationship. Likewise, let `recordings.flow_id` and `clips.flow_id` fall back to `NULL` through the existing `ON DELETE SET NULL` definitions.

- [ ] **Step 4: Run the targeted Rust tests, then full Rust lint verification**

Run: `cargo test delete_flow_with_conn -- --nocapture`
Expected: PASS.

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit the Rust delete command change**

```bash
git add src-tauri/src/commands/flows.rs src-tauri/src/lib.rs
git commit -m "feat(flows): add tauri flow deletion command"
```

### Task 4: End-To-End Verification In The Existing App Flow

**Files:**
- Modify: none
- Verify: frontend list UI, store state, Tauri deletion behavior

- [ ] **Step 1: Run the repo verification commands for the touched layers**

Run: `npm run lint:js`
Expected: PASS.

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 2: Manually verify the user-visible behavior in the flows page**

Run the app in the usual local workflow and verify this checklist:

```text
1. Open the Flows page.
2. Confirm each flow card now shows a Delete button beside Open.
3. Click Delete and confirm the dialog copy includes the flow name and permanent-delete warning.
4. Cancel once and confirm the flow remains unchanged.
5. Delete the same flow again and confirm the card disappears without a manual refresh.
6. While deletion is in progress, confirm Enabled, Open, and Delete are disabled for that card.
7. If the deleted flow was active in state, confirm the app lands on the list view instead of a stale detail view.
```

Expected: All checklist items pass.

- [ ] **Step 3: Check the final git diff and commit any verification-only fixes if needed**

Run: `git diff -- src/lib/api.ts src/stores/flow-store.ts src/stores/flow-store.test.ts src/components/flows/flow-list.tsx src/components/flows/flow-card.tsx src-tauri/src/commands/flows.rs src-tauri/src/lib.rs`
Expected: Only the planned flow-deletion changes appear.

- [ ] **Step 4: Create the final integration commit if manual verification required a follow-up fix**

```bash
git add src/lib/api.ts src/stores/flow-store.ts src/stores/flow-store.test.ts src/components/flows/flow-list.tsx src/components/flows/flow-card.tsx src-tauri/src/commands/flows.rs src-tauri/src/lib.rs
git commit -m "fix(flows): polish delete flow interactions"
```

Skip this step if no follow-up fix was needed after verification.

---

## Self-Review

- Spec coverage check: the plan covers the list-level delete button, confirmation dialog, store cleanup, active-flow reset, and the new Tauri deletion command. It intentionally does not add detail-page deletion, undo, archive, or media-file cleanup.
- Placeholder scan: no `TODO`, `TBD`, or vague “handle appropriately” steps remain; each implementation step points to concrete files and code.
- Type consistency: the plan uses one consistent API/store name, `deleteFlow` in TypeScript and `delete_flow` in Tauri/Rust, which matches the existing repo naming pattern.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-20-flow-delete-from-list.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
