# Flow Delete From List Design

**Date:** 2026-04-20
**Status:** Draft for review

---

## 1. Overview

This change adds a destructive `Delete` action to each flow card in the list view so a user can remove an obsolete flow without opening the detail screen first.

The design keeps the change intentionally small:

- place the delete entry point in `src/components/flows/flow-card.tsx`
- route the action through `src/stores/flow-store.ts`
- add one new Tauri command to remove the flow and its flow-owned records from SQLite

The resulting behavior should feel consistent with the rest of the app's list-level management actions while avoiding unrelated changes to the flow editor experience.

---

## 2. Goals And Non-Goals

### Goals

- Add a visible delete action directly on each flow card in the list view
- Require explicit confirmation before permanently deleting a flow
- Remove the deleted flow from frontend state immediately after success
- Clean up flow-owned runtime and editor state so the UI does not keep stale references
- Keep the implementation minimal and aligned with the existing React/Zustand/Tauri structure

### Non-Goals

- Adding a delete action to the flow detail page in this change
- Introducing soft delete, archive, undo, or trash behavior
- Changing how flow creation, publish, or enabled toggling currently works
- Deleting unrelated media files from disk as part of this first pass
- Refactoring the broader flow UI or store architecture

---

## 3. Approaches Considered

Three practical approaches were considered for the list-level delete action:

1. Add a `Delete` button to each flow card and use `window.confirm`
2. Add a `Delete` button to each flow card and use the app's dialog component for confirmation
3. Add a compact icon-only delete affordance in the card header

### Recommendation

Approach 2 is the chosen direction.

It gives the clearest destructive-action UX without expanding the scope too much:

- the action remains exactly where the user asked for it, in the list
- the confirmation copy can be explicit and consistent with destructive behavior
- the dialog can hold pending state and prevent accidental repeated clicks more cleanly than `window.confirm`

Approach 1 is viable but rougher. Approach 3 saves space but makes a destructive action easier to miss or misinterpret.

---

## 4. User Experience

### 4.1 Card Placement

Each `FlowCard` footer currently shows:

- an `Enabled` switch
- an `Open` button

The design adds a `Delete` button in the same action cluster, next to `Open`, so the destructive action is visible but not visually dominant.

### 4.2 Confirmation

Selecting `Delete` opens a confirmation dialog that clearly states:

- which flow is about to be deleted
- that the deletion is permanent
- that the action cannot be undone

The confirm action should use destructive visual treatment and stay disabled while the delete request is running.

### 4.3 Busy State

While a flow is being deleted, that card should disable:

- the `Enabled` switch
- the `Open` button
- the `Delete` button

This prevents conflicting actions against a record that is in the middle of removal.

### 4.4 Success Result

After a successful delete:

- the card disappears from the list without requiring a manual refresh
- any card-local confirm state closes automatically
- the user remains on the list page

### 4.5 Failure Result

If deletion fails:

- the card stays visible
- the dialog closes or remains recoverable based on the component flow, but the user-facing error appears through the existing flow store error surface
- no partial local cleanup should make the UI think the delete succeeded when it did not

---

## 5. Frontend Design

### 5.1 API Layer

`src/lib/api.ts` will gain a new function:

- `deleteFlow(flowId: number): Promise<void>`

This should mirror the existing Tauri invoke wrappers and call a new backend command named `delete_flow`.

### 5.2 Store Changes

`src/stores/flow-store.ts` will gain a new action:

- `deleteFlow(flowId: number): Promise<void>`

On success, the store should:

- remove the flow from `flows`
- remove `runtimeSnapshots[flowId]`
- remove `runtimeLogs[flowId]`
- if the deleted flow is the active flow, clear `activeFlowId`, `activeFlow`, and `selectedNode`
- if the deleted flow is active, set `view` back to `list`

The store should also set `error` with a predictable message if the backend delete fails.

### 5.3 List-Level Busy Tracking

`src/components/flows/flow-list.tsx` already keeps `busyFlowIds` for enable/disable operations.

The minimal design is to reuse the same busy-map pattern for deletion instead of adding a second global mechanism. The list component will:

- mark a flow as busy before calling the store delete action
- clear the busy marker in `finally`
- pass the same `busy` flag down to `FlowCard`

This keeps the card API small and matches the current toggle behavior.

### 5.4 Card-Level Confirmation

`src/components/flows/flow-card.tsx` will own the confirm dialog open state because the delete button lives there and the dialog content needs the flow name.

The card will receive:

- `busy`
- `onDelete(flowId: number)`

The card should not directly talk to the API or store. It should remain a presentational component with local dialog state only.

---

## 6. Backend Design

### 6.1 New Tauri Command

`src-tauri/src/commands/flows.rs` will gain:

- `delete_flow(state: State<'_, AppState>, flow_id: i64) -> Result<(), String>`

`src-tauri/src/lib.rs` will register this command in the invoke handler list.

### 6.2 Delete Scope

The backend delete should remove the flow row and flow-owned relational records that should not outlive the flow, including at least:

- `flow_nodes`
- `flow_runs`
- `flow_node_runs`
- runtime-store or runtime-log rows keyed by `flow_id`, if those are persisted in SQLite in the current implementation
- the `flows` row itself

This should happen inside one SQLite transaction so the delete either succeeds completely or fails completely.

### 6.3 Runtime Cleanup

If the runtime manager currently tracks in-memory session, poll-task, or recording bookkeeping by `flow_id`, the command should clear that state as part of deletion so the app does not keep background runtime artifacts for a removed flow.

The design does not require new runtime semantics. It only requires that deleting a flow leaves no active in-memory runtime state for that flow.

---

## 7. Data And Behavior Decisions

### 7.1 Permanent Delete

Deletion is permanent in this phase. There is no recovery path or undo buffer.

### 7.2 Recordings And Clips

This change does not attempt to delete media files from disk.

If recordings or clips reference the deleted flow through nullable foreign-key-like relationships, the implementation should prefer the smallest safe behavior already supported by the schema and existing code paths. The delete should not expand into a media cleanup feature.

### 7.3 Active Detail Safety

Even though the user requested list-only entry, the store must still handle the case where the deleted flow is active in state. This prevents stale editor UI if a delete is triggered against what later becomes the active selection or if future entry points are added.

---

## 8. Error Handling

- Invalid `flow_id` should return a clear backend error
- Deleting a missing flow should return a stable error rather than silently pretending success
- Frontend should surface backend failure through the existing flow error banner
- Local state cleanup should happen only after confirmed backend success

---

## 9. Verification

Because this change touches frontend TypeScript and Rust/Tauri code, verification should cover both layers:

- `npm run lint:js`
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`

Manual verification should confirm:

- a delete button appears on each flow card
- confirmation is required
- deleting a flow removes it from the list immediately
- trying to interact with the card during deletion is blocked
- deleting a flow does not leave the app stuck in detail state if that flow was active

---

## 10. Implementation Boundary

This design is intentionally narrow. A correct implementation should only add what is needed for list-level flow deletion with confirmation and state cleanup. It should not introduce broader workflow-management features or UI restructuring.
