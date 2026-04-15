# V2 Flow Engine And Canvas Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current flow-workspace implementation with a real workflow engine where `Flow` is the runtime unit, `Start` owns source/account config, edits are draft-first with explicit publish, and `Flow Detail` becomes a fixed-node canvas with modal configuration and runtime monitoring.

**Architecture:** Split the work into two coordinated layers. First, reshape SQLite, Rust commands, and runtime orchestration so `Rust/Tauri` becomes the workflow engine and sidecar acts as an executor. Second, replace the current inspector-based flow detail UI with a fixed-node canvas, modal form editors, draft/publish controls, and runtime views built on top of the new engine state.

**Tech Stack:** React 19 + TypeScript + Zustand + shadcn-style UI + Vite (frontend), Rust + Tauri + rusqlite (workflow engine and persistence), Python FastAPI sidecar (executor/integration), SQLite migrations

**Spec:** `docs/superpowers/specs/2026-04-15-v2-flow-engine-canvas-design.md`

---

## Scope Check

This spec covers two large but tightly related subsystems:

1. Workflow engine and persistence redesign
2. Canvas editor and runtime monitor UI redesign

They should still live in one implementation plan because the UI depends directly on the new draft/publish and runtime engine model. The tasks below sequence them so each stage leaves the branch in a working state.

---

## File Structure

### New Files

| Path | Responsibility |
|---|---|
| `src-tauri/src/db/migrations/008_flow_engine_rebuild.sql` | Replace `account_id`-based flow schema with workflow-engine tables (`flows`, `flow_nodes`, `flow_runs`, `flow_node_runs`) and migrate legacy flow/account data into `Start` nodes |
| `src-tauri/src/commands/flow_engine.rs` | Workflow-engine commands: draft save, publish, runtime run creation, node-run history, modal form data helpers |
| `src-tauri/src/workflow/mod.rs` | Rust workflow engine entrypoint and shared types |
| `src-tauri/src/workflow/node_runner.rs` | Engine logic for fixed-node progression and sidecar dispatch contracts |
| `src-tauri/src/workflow/runtime_store.rs` | Runtime state persistence helpers for `flow_runs` and `flow_node_runs` |
| `src-tauri/src/workflow/start_node.rs` | `Start` node orchestration and source polling/watcher contract |
| `src-tauri/src/workflow/record_node.rs` | `Record` node orchestration contract |
| `src-tauri/src/workflow/clip_node.rs` | `Clip` node orchestration contract |
| `src-tauri/src/workflow/caption_node.rs` | `Caption` node orchestration contract |
| `src/components/flows/canvas/flow-canvas.tsx` | Main fixed-node workflow scene |
| `src/components/flows/canvas/flow-canvas-node.tsx` | Visual node card inside the canvas |
| `src/components/flows/canvas/flow-canvas-edge.tsx` | Fixed visual connectors between nodes |
| `src/components/flows/modals/start-node-modal.tsx` | Form-based modal for `Start` node config |
| `src/components/flows/modals/record-node-modal.tsx` | Form-based modal for `Record` node config |
| `src/components/flows/modals/clip-node-modal.tsx` | Form-based modal for `Clip` node config |
| `src/components/flows/modals/caption-node-modal.tsx` | Form-based modal for `Caption` node config |
| `src/components/flows/modals/upload-node-modal.tsx` | Placeholder modal for `Upload` node |
| `src/components/flows/runtime/flow-runtime-timeline.tsx` | Chronological flow-run event timeline |
| `src/components/flows/runtime/flow-runtime-lanes.tsx` | Per-node runtime lane monitor |
| `src/components/flows/runtime/flow-runtime-lane.tsx` | One node lane row |
| `src/components/flows/runtime/publish-flow-dialog.tsx` | Publish/restart confirmation dialog |
| `src/lib/flow-node-forms.ts` | Shared form schemas, labels, defaults, and parsing helpers for node modals |

### Modified Files

| Path | Responsibility |
|---|---|
| `src-tauri/src/db/init.rs` | Register migration 008 |
| `src-tauri/src/db/models.rs` | Replace current flow DTOs with flow definition / node / run / node-run models |
| `src-tauri/src/commands/mod.rs` | Register `flow_engine` module |
| `src-tauri/src/lib.rs` | Register new engine commands and remove obsolete flow workspace-only handlers where replaced |
| `src-tauri/src/commands/flows.rs` | Slim down or adapt to list/detail surface commands built on new engine schema |
| `src-tauri/src/commands/recordings.rs` | Attach `flow_run_id` / node-run lineage where recordings are created/synced |
| `src-tauri/src/commands/clips.rs` | Attach `flow_run_id` / node-run lineage for clips and caption outputs |
| `src/components/layout/sidebar.tsx` | Remove `Accounts` from sidebar and keep `Flows` as the workflow entry point |
| `src/components/layout/app-shell.tsx` | Reduce frontend orchestration and hand off progression to Rust engine; keep UI refresh and event observation |
| `src/pages/flows.tsx` | Replace list/detail shell assumptions with canvas runtime workflow shell |
| `src/components/flows/flow-list.tsx` | Create-flow flow-first UI (no account page dependency) and runtime-rich summary cards |
| `src/components/flows/flow-card.tsx` | Show source identity from `Start` plus draft/published/runtime indicators |
| `src/components/flows/flow-detail.tsx` | Replace current inspector layout with canvas + publish bar + runtime monitor |
| `src/stores/flow-store.ts` | Redesign state around draft/published definitions, modal editing, publish action, and runtime runs |
| `src/lib/api.ts` | Replace current flow API surface with engine-oriented commands and publish/runtime queries |
| `src/types/index.ts` | Replace account-bound flow types with flow definition, node definition, draft/publish state, run history, and modal form types |
| `src/pages/settings.tsx` | Remove any remaining workflow-default framing; keep only system/app-level settings |
| `src/pages/accounts.tsx` | Remove from navigation surface; keep only as temporary migration shim if needed |
| `src/stores/account-store.ts` | Reduce surface area if still needed only for migration/runtime transitional reads |
| `sidecar/src/routes/accounts.py` | Reduce ownership assumptions; keep executor-facing source checks only if still required by engine |
| `sidecar/src/routes/clips.py` | Align with engine-driven dispatch contracts |
| `sidecar/src/core/watcher.py` | Align with engine as caller instead of primary orchestrator |

### Transitional Decisions

- V2 keeps fixed node types and fixed order.
- `accounts` may remain temporarily in storage for migration only, but the product model no longer exposes it as a first-class editable surface.
- Draft and published configs live on `flow_nodes` in V2 instead of introducing a separate version table.
- `Upload` remains visible and versioned but non-executable in V2.

---

### Task 1: Rebuild Flow Persistence Around Draft/Published Nodes

**Files:**
- Create: `src-tauri/src/db/migrations/008_flow_engine_rebuild.sql`
- Modify: `src-tauri/src/db/init.rs`
- Modify: `src-tauri/src/db/models.rs`

- [ ] **Step 1: Write the migration SQL that replaces the current flow schema**

Create `src-tauri/src/db/migrations/008_flow_engine_rebuild.sql` with these sections:

```sql
ALTER TABLE flows RENAME TO flows_legacy;
ALTER TABLE flow_node_configs RENAME TO flow_node_configs_legacy;

CREATE TABLE flows (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  status TEXT NOT NULL DEFAULT 'idle' CHECK (status IN ('idle', 'watching', 'recording', 'processing', 'error', 'disabled')),
  current_node TEXT CHECK (current_node IN ('start', 'record', 'clip', 'caption', 'upload')),
  published_version INTEGER NOT NULL DEFAULT 1,
  draft_version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'))
);

CREATE TABLE flow_nodes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
  node_key TEXT NOT NULL CHECK (node_key IN ('start', 'record', 'clip', 'caption', 'upload')),
  position INTEGER NOT NULL,
  draft_config_json TEXT NOT NULL DEFAULT '{}',
  published_config_json TEXT NOT NULL DEFAULT '{}',
  draft_updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
  published_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
  UNIQUE(flow_id, node_key)
);

CREATE TABLE flow_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
  definition_version INTEGER NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled')),
  started_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
  ended_at TEXT,
  trigger_reason TEXT,
  error TEXT
);

CREATE TABLE flow_node_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flow_run_id INTEGER NOT NULL REFERENCES flow_runs(id) ON DELETE CASCADE,
  flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
  node_key TEXT NOT NULL CHECK (node_key IN ('start', 'record', 'clip', 'caption', 'upload')),
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'skipped', 'cancelled')),
  started_at TEXT,
  ended_at TEXT,
  input_json TEXT,
  output_json TEXT,
  error TEXT
);

ALTER TABLE recordings ADD COLUMN flow_run_id INTEGER REFERENCES flow_runs(id) ON DELETE SET NULL;
ALTER TABLE clips ADD COLUMN flow_run_id INTEGER REFERENCES flow_runs(id) ON DELETE SET NULL;
```

- [ ] **Step 2: Add migration data copy for existing flows/accounts into `Start` nodes**

Append migration data-copy statements that preserve existing flow rows and migrate account fields into `Start` draft/published configs:

```sql
INSERT INTO flows (id, name, enabled, status, current_node, published_version, draft_version, created_at, updated_at)
SELECT id, name, enabled, status, current_node, 1, 1, created_at, updated_at
FROM flows_legacy;

INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json)
SELECT
  f.id,
  'start',
  1,
  json_object(
    'username', a.username,
    'cookies_json', a.cookies_json,
    'proxy_url', a.proxy_url,
    'auto_record', a.auto_record,
    'poll_interval_seconds', 60,
    'watcher_mode', 'live_polling'
  ),
  json_object(
    'username', a.username,
    'cookies_json', a.cookies_json,
    'proxy_url', a.proxy_url,
    'auto_record', a.auto_record,
    'poll_interval_seconds', 60,
    'watcher_mode', 'live_polling'
  )
FROM flows_legacy f
LEFT JOIN accounts a ON a.id = f.account_id;
```

Then add the other four fixed nodes with migrated config from `flow_node_configs_legacy` where available.

- [ ] **Step 3: Register migration 008 and replace Rust models**

In `src-tauri/src/db/init.rs`, add:

```rust
(8, include_str!("migrations/008_flow_engine_rebuild.sql")),
```

In `src-tauri/src/db/models.rs`, replace the current flow DTO layer with:

```rust
pub struct FlowDefinition {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub status: String,
    pub current_node: Option<String>,
    pub published_version: i64,
    pub draft_version: i64,
    pub created_at: String,
    pub updated_at: String,
}

pub struct FlowNodeDefinition {
    pub id: i64,
    pub flow_id: i64,
    pub node_key: String,
    pub position: i64,
    pub draft_config_json: String,
    pub published_config_json: String,
    pub draft_updated_at: String,
    pub published_at: String,
}

pub struct FlowRun {
    pub id: i64,
    pub flow_id: i64,
    pub definition_version: i64,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub trigger_reason: Option<String>,
    pub error: Option<String>,
}
```

- [ ] **Step 4: Verify the migration/model layer compiles**

Run from `src-tauri/`:

```bash
cargo check
```

Expected: PASS, with the new schema and Rust DTOs compiling cleanly.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/migrations/008_flow_engine_rebuild.sql src-tauri/src/db/init.rs src-tauri/src/db/models.rs
git commit -m "feat(db): rebuild flow schema for workflow engine"
```

---

### Task 2: Build the Rust Workflow Engine Core

**Files:**
- Create: `src-tauri/src/workflow/mod.rs`
- Create: `src-tauri/src/workflow/runtime_store.rs`
- Create: `src-tauri/src/workflow/node_runner.rs`
- Create: `src-tauri/src/workflow/start_node.rs`
- Create: `src-tauri/src/workflow/record_node.rs`
- Create: `src-tauri/src/workflow/clip_node.rs`
- Create: `src-tauri/src/workflow/caption_node.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add a workflow engine module with fixed-node execution types**

Create `src-tauri/src/workflow/mod.rs`:

```rust
pub mod caption_node;
pub mod clip_node;
pub mod node_runner;
pub mod record_node;
pub mod runtime_store;
pub mod start_node;

pub const FLOW_NODE_ORDER: [&str; 5] = ["start", "record", "clip", "caption", "upload"];

#[derive(Debug, Clone)]
pub struct EngineNodeResult {
    pub status: String,
    pub output_json: Option<String>,
    pub error: Option<String>,
    pub next_node: Option<String>,
}
```

- [ ] **Step 2: Add runtime store helpers for runs and node-runs**

Create `src-tauri/src/workflow/runtime_store.rs` with explicit helpers:

```rust
pub fn create_flow_run(conn: &Connection, flow_id: i64, definition_version: i64, trigger_reason: &str) -> Result<i64, String> {
    conn.execute(
        &format!(
            "INSERT INTO flow_runs (flow_id, definition_version, status, started_at, trigger_reason) VALUES (?1, ?2, 'running', {}, ?3)",
            SQL_NOW_HCM
        ),
        params![flow_id, definition_version, trigger_reason],
    ).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}
```

Add sibling helpers for starting/completing/failing `flow_node_runs` and updating `flows.current_node`.

- [ ] **Step 3: Add node-runner logic for the fixed order**

Create `src-tauri/src/workflow/node_runner.rs`:

```rust
pub fn next_node_key(current: &str) -> Option<&'static str> {
    match current {
        "start" => Some("record"),
        "record" => Some("clip"),
        "clip" => Some("caption"),
        "caption" => Some("upload"),
        _ => None,
    }
}
```

Then add one `run_node` dispatcher that calls `start_node::run`, `record_node::run`, `clip_node::run`, or `caption_node::run` based on node key.

- [ ] **Step 4: Add minimal node modules that produce deterministic engine results**

In `start_node.rs`, return source input metadata and `next_node = Some("record".into())` when source conditions are satisfied.

In `record_node.rs`, `clip_node.rs`, and `caption_node.rs`, add function stubs with the real sidecar-dispatch contract shape but keep the first implementation thin:

```rust
pub fn run(_config_json: &str, input_json: Option<&str>) -> Result<EngineNodeResult, String> {
    Ok(EngineNodeResult {
        status: "completed".to_string(),
        output_json: input_json.map(|x| x.to_string()),
        error: None,
        next_node: Some("clip".to_string()),
    })
}
```

These stubs let the engine compile before dispatch is fully integrated.

- [ ] **Step 5: Verify Rust still formats and lints cleanly**

Run from `src-tauri/`:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/workflow src-tauri/src/lib.rs
git commit -m "feat(engine): add workflow runtime core"
```

---

### Task 3: Replace Flow Commands With Draft/Publish And Runtime Queries

**Files:**
- Create: `src-tauri/src/commands/flow_engine.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/flows.rs`

- [ ] **Step 1: Create a new engine-oriented command module**

Create `src-tauri/src/commands/flow_engine.rs` with the following command surface:

```rust
#[tauri::command]
pub fn get_flow_definition(state: State<'_, AppState>, flow_id: i64) -> Result<FlowEditorPayload, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let flow = load_flow_definition(&conn, flow_id)?;
    let nodes = load_flow_nodes(&conn, flow_id)?;
    let runs = load_flow_runs(&conn, flow_id)?;
    let node_runs = load_flow_node_runs(&conn, flow_id)?;
    Ok(FlowEditorPayload { flow, nodes, runs, node_runs })
}

#[tauri::command]
pub fn save_flow_node_draft(state: State<'_, AppState>, flow_id: i64, node_key: String, draft_config_json: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    serde_json::from_str::<serde_json::Value>(&draft_config_json).map_err(|e| e.to_string())?;
    conn.execute(
        &format!(
            "UPDATE flow_nodes SET draft_config_json = ?1, draft_updated_at = {} WHERE flow_id = ?2 AND node_key = ?3",
            SQL_NOW_HCM
        ),
        params![draft_config_json, flow_id, node_key],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn publish_flow_definition(state: State<'_, AppState>, flow_id: i64) -> Result<PublishFlowResult, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let is_running: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM flow_runs WHERE flow_id = ?1 AND status = 'running')",
        [flow_id],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    tx.execute(
        &format!("UPDATE flow_nodes SET published_config_json = draft_config_json, published_at = {} WHERE flow_id = ?1", SQL_NOW_HCM),
        [flow_id],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        &format!("UPDATE flows SET published_version = draft_version, draft_version = draft_version + 1, updated_at = {} WHERE id = ?1", SQL_NOW_HCM),
        [flow_id],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(PublishFlowResult { flow_id, is_running })
}

#[tauri::command]
pub fn list_flow_runs(state: State<'_, AppState>, flow_id: i64) -> Result<Vec<FlowRunRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    load_flow_runs(&conn, flow_id)
}

#[tauri::command]
pub fn list_flow_node_runs(state: State<'_, AppState>, flow_id: i64) -> Result<Vec<FlowNodeRunRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    load_flow_node_runs(&conn, flow_id)
}
```

- [ ] **Step 2: Implement publish semantics in SQL**

The `publish_flow_definition` command must do all of this in one transaction:

```rust
tx.execute(
    &format!("UPDATE flows SET published_version = draft_version + 1, draft_version = draft_version + 1, updated_at = {} WHERE id = ?1", SQL_NOW_HCM),
    [flow_id],
)?;

tx.execute(
    &format!("UPDATE flow_nodes SET published_config_json = draft_config_json, published_at = {} WHERE flow_id = ?1", SQL_NOW_HCM),
    [flow_id],
)?;
```

Return whether the flow is currently running so the UI can show the restart dialog.

- [ ] **Step 3: Adapt list/detail commands to the new flow definition payload**

In `src-tauri/src/commands/flows.rs`, keep only the list-level summary and create-flow shell behavior.

The new `create_flow` should:
- no longer require `account_id`
- create a flow definition row
- create five `flow_nodes`
- seed `Start` with a minimal empty source form config

Use this exact input shape:

```rust
pub struct CreateFlowInput {
    pub name: String,
    pub enabled: Option<bool>,
}
```

- [ ] **Step 4: Register new commands and remove obsolete handler usage**

In `src-tauri/src/commands/mod.rs`:

```rust
pub mod flow_engine;
```

In `src-tauri/src/lib.rs`, register the new engine commands and remove any handler no longer used by the frontend after the next tasks land.

- [ ] **Step 5: Verify Rust compile/lint**

Run from `src-tauri/`:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/flow_engine.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/src/commands/flows.rs
git commit -m "feat(engine): add flow draft publish commands"
```

---

### Task 4: Remove Accounts As A Product Surface And Redefine Types

**Files:**
- Modify: `src/types/index.ts`
- Modify: `src/lib/api.ts`
- Modify: `src/stores/flow-store.ts`
- Modify: `src/components/layout/sidebar.tsx`
- Modify: `src/pages/accounts.tsx`
- Modify: `src/stores/account-store.ts`

- [ ] **Step 1: Replace flow types with draft/publish and run-history shapes**

In `src/types/index.ts`, add and use:

```ts
export type FlowNodeKey = "start" | "record" | "clip" | "caption" | "upload";
export type FlowRunStatus = "pending" | "running" | "completed" | "failed" | "cancelled";

export interface FlowNodeDefinition {
  id: number;
  flow_id: number;
  node_key: FlowNodeKey;
  position: number;
  draft_config_json: string;
  published_config_json: string;
  draft_updated_at: string;
  published_at: string;
}

export interface FlowEditorPayload {
  flow: FlowSummary;
  nodes: FlowNodeDefinition[];
  runs: FlowRunRow[];
  node_runs: FlowNodeRunRow[];
}
```

Remove `account_id` from new `CreateFlowInput`.

- [ ] **Step 2: Replace API wrappers for the new engine commands**

In `src/lib/api.ts`, add:

```ts
export async function getFlowDefinition(flowId: number): Promise<FlowEditorPayload> {
  return invoke<FlowEditorPayload>("get_flow_definition", { flowId });
}

export async function saveFlowNodeDraft(input: { flow_id: number; node_key: FlowNodeKey; draft_config_json: string }): Promise<void> {
  await invoke("save_flow_node_draft", {
    flowId: input.flow_id,
    nodeKey: input.node_key,
    draftConfigJson: input.draft_config_json,
  });
}

export async function publishFlowDefinition(flowId: number): Promise<PublishFlowResult> {
  return invoke<PublishFlowResult>("publish_flow_definition", { flowId });
}
```

- [ ] **Step 3: Redesign flow store around editor payload and modal state**

In `src/stores/flow-store.ts`, replace the current inspector-driven state with:

```ts
type FlowStore = {
  flows: FlowSummary[];
  activeFlowId: number | null;
  activeFlow: FlowEditorPayload | null;
  selectedNode: FlowNodeKey | null;
  editorModalNode: FlowNodeKey | null;
  publishPending: boolean;
  draftDirty: boolean;
  runtimeRefreshTick: number;
  fetchFlows: () => Promise<void>;
  fetchFlowDetail: (flowId: number) => Promise<void>;
  openNodeModal: (node: FlowNodeKey) => void;
  closeNodeModal: () => void;
  saveNodeDraft: (input: { flow_id: number; node_key: FlowNodeKey; draft_config_json: string }) => Promise<void>;
  publishFlow: (flowId: number, options: { restartCurrentRun: boolean }) => Promise<PublishFlowResult>;
  refreshRuntime: () => Promise<void>;
};
```

Add actions `openNodeModal`, `closeNodeModal`, `saveNodeDraft`, `publishFlow`, and `refreshRuntime`.

- [ ] **Step 4: Remove Accounts from primary navigation**

In `src/components/layout/sidebar.tsx`, remove the `Accounts` destination entirely.

In `src/pages/accounts.tsx`, replace page content with a transitional message so the file still compiles if imported accidentally:

```tsx
export function AccountsPage() {
  return <p className="text-sm text-[var(--color-text-muted)]">Accounts moved into the Start node of each flow.</p>;
}
```

- [ ] **Step 5: Verify frontend compile**

Run from repo root:

```bash
npm run lint:js
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/types/index.ts src/lib/api.ts src/stores/flow-store.ts src/components/layout/sidebar.tsx src/pages/accounts.tsx src/stores/account-store.ts
git commit -m "feat(ui): remove accounts from primary workflow surface"
```

---

### Task 5: Replace Inspector UI With Fixed-Node Canvas

**Files:**
- Create: `src/components/flows/canvas/flow-canvas.tsx`
- Create: `src/components/flows/canvas/flow-canvas-node.tsx`
- Create: `src/components/flows/canvas/flow-canvas-edge.tsx`
- Modify: `src/components/flows/flow-detail.tsx`
- Modify: `src/pages/flows.tsx`

- [ ] **Step 1: Create a fixed-node canvas scene**

Create `src/components/flows/canvas/flow-canvas.tsx`:

```tsx
const NODE_SCENE: Array<{ key: FlowNodeKey; x: number; y: number }> = [
  { key: "start", x: 80, y: 160 },
  { key: "record", x: 360, y: 160 },
  { key: "clip", x: 660, y: 120 },
  { key: "caption", x: 940, y: 160 },
  { key: "upload", x: 1240, y: 160 },
];
```

Render each node absolutely in a scene container and render visual edges between them. Do not add drag-and-drop.

- [ ] **Step 2: Add visual node cards that show runtime and draft state**

Create `flow-canvas-node.tsx` so each node shows:
- node label
- current runtime state
- source summary or output summary
- draft-vs-published indicator

Use a prop surface like:

```tsx
type FlowCanvasNodeProps = {
  nodeKey: FlowNodeKey;
  selected: boolean;
  hasDraftChanges: boolean;
  runtimeState: string;
  onClick: () => void;
};
```

- [ ] **Step 3: Replace the current `FlowDetail` layout with canvas + publish bar**

In `src/components/flows/flow-detail.tsx`, remove the right-side JSON inspector and use this top structure:

```tsx
<div className="flex items-center justify-between gap-3">
  <div>{/* flow title + published/draft badges */}</div>
  <div className="flex items-center gap-2">
    <Button variant="outline">Back</Button>
    <Button>Publish</Button>
  </div>
</div>
<FlowCanvas
  flow={activeFlow}
  selectedNode={selectedNode}
  onSelectNode={(node) => openNodeModal(node)}
/>
<FlowRuntimeTimeline runs={activeFlow?.runs ?? []} nodeRuns={activeFlow?.node_runs ?? []} />
<FlowRuntimeLanes nodeRuns={activeFlow?.node_runs ?? []} />
```

- [ ] **Step 4: Keep page routing simple**

In `src/pages/flows.tsx`, keep the existing list/detail split but point detail mode to the new canvas-based `FlowDetail`.

- [ ] **Step 5: Verify frontend compile/build**

Run from repo root:

```bash
npm run lint:js
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/components/flows/canvas src/components/flows/flow-detail.tsx src/pages/flows.tsx
git commit -m "feat(ui): replace flow detail with workflow canvas"
```

---

### Task 6: Add Form-Based Node Modals And Draft Auto-Save

**Files:**
- Create: `src/components/flows/modals/start-node-modal.tsx`
- Create: `src/components/flows/modals/record-node-modal.tsx`
- Create: `src/components/flows/modals/clip-node-modal.tsx`
- Create: `src/components/flows/modals/caption-node-modal.tsx`
- Create: `src/components/flows/modals/upload-node-modal.tsx`
- Create: `src/lib/flow-node-forms.ts`
- Modify: `src/components/flows/flow-detail.tsx`

- [ ] **Step 1: Define typed form schemas for each fixed node**

Create `src/lib/flow-node-forms.ts` with typed helpers:

```ts
export type StartNodeForm = {
  username: string;
  cookies_json: string;
  proxy_url: string;
  poll_interval_seconds: number;
  watcher_mode: "live_polling";
  retry_limit: number;
};

export function parseStartNodeDraft(raw: string): StartNodeForm {
  const value = JSON.parse(raw || "{}");
  return {
    username: typeof value.username === "string" ? value.username : "",
    cookies_json: typeof value.cookies_json === "string" ? value.cookies_json : "",
    proxy_url: typeof value.proxy_url === "string" ? value.proxy_url : "",
    poll_interval_seconds: Number.isFinite(value.poll_interval_seconds) ? value.poll_interval_seconds : 60,
    watcher_mode: "live_polling",
    retry_limit: Number.isFinite(value.retry_limit) ? value.retry_limit : 3,
  };
}
```

Add equivalent parse/serialize helpers for `Record`, `Clip`, and `Caption`.

- [ ] **Step 2: Build a form modal for the `Start` node**

Create `start-node-modal.tsx` using existing dialog primitives. The form must contain visible fields for:
- username
- cookies
- proxy
- poll interval
- retry limit

Auto-save draft changes on field change with a small debounce:

```tsx
useEffect(() => {
  const timer = window.setTimeout(() => {
    void onAutoSave(serializeStartNodeDraft(form));
  }, 300);
  return () => window.clearTimeout(timer);
}, [form, onAutoSave]);
```

- [ ] **Step 3: Add modal variants for the other nodes**

Create form-based modals for `Record`, `Clip`, and `Caption` with domain-specific controls. Keep `Upload` as a lightweight informational modal with disabled fields and a “Coming later” note.

- [ ] **Step 4: Mount node modals from `FlowDetail` and remove JSON editing UI**

In `src/components/flows/flow-detail.tsx`, open the correct modal when a canvas node is clicked. The modal must show a top `Save` button even though edits auto-save draft changes.

The `Save` action should explicitly flush any pending debounce and close the modal; it should not publish the flow.

- [ ] **Step 5: Verify frontend compile/build**

Run from repo root:

```bash
npm run lint:js
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/components/flows/modals src/lib/flow-node-forms.ts src/components/flows/flow-detail.tsx
git commit -m "feat(ui): add form-based node config modals"
```

---

### Task 7: Add Publish Dialog And Runtime Monitor UI

**Files:**
- Create: `src/components/flows/runtime/flow-runtime-timeline.tsx`
- Create: `src/components/flows/runtime/flow-runtime-lanes.tsx`
- Create: `src/components/flows/runtime/flow-runtime-lane.tsx`
- Create: `src/components/flows/runtime/publish-flow-dialog.tsx`
- Modify: `src/components/flows/flow-detail.tsx`
- Modify: `src/stores/flow-store.ts`

- [ ] **Step 1: Add a publish dialog that handles running-flow choices**

Create `publish-flow-dialog.tsx` that shows:

```tsx
<DialogTitle>Publish flow changes</DialogTitle>
<p>If this flow is running, current execution will continue with the previous published config unless you stop and restart.</p>
<Button variant="outline">Keep current run</Button>
<Button>Stop and restart</Button>
```

- [ ] **Step 2: Add runtime timeline view from `flow_runs` and `flow_node_runs`**

Create `flow-runtime-timeline.tsx` to render recent events chronologically using the engine payload.

Use a simple mapper:

```ts
const timelineItems = nodeRuns
  .slice()
  .sort((a, b) => (b.started_at ?? "").localeCompare(a.started_at ?? ""));
```

- [ ] **Step 3: Add per-node lanes below the canvas**

Create `flow-runtime-lanes.tsx` and `flow-runtime-lane.tsx` so each node renders its recent run state and output/error summaries in a dedicated lane.

- [ ] **Step 4: Wire publish action through the flow store**

In `src/stores/flow-store.ts`, add `publishFlow`:

```ts
publishFlow: async (flowId, options) => {
  const result = await api.publishFlowDefinition(flowId);
  if (options.restartCurrentRun) {
    await api.restartFlowRun(flowId);
  }
  await get().fetchFlowDetail(flowId);
  return result;
}
```

- [ ] **Step 5: Verify frontend compile/build**

Run from repo root:

```bash
npm run lint:js
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/components/flows/runtime src/components/flows/flow-detail.tsx src/stores/flow-store.ts
git commit -m "feat(ui): add flow publish dialog and runtime monitor"
```

---

### Task 8: Move Runtime Orchestration Out Of AppShell And Into The Engine

**Files:**
- Modify: `src/components/layout/app-shell.tsx`
- Modify: `src-tauri/src/commands/recordings.rs`
- Modify: `src-tauri/src/commands/clips.rs`
- Modify: `sidecar/src/routes/accounts.py`
- Modify: `sidecar/src/routes/clips.py`
- Modify: `sidecar/src/core/watcher.py`

- [ ] **Step 1: Reduce AppShell to observation and refresh only**

In `src/components/layout/app-shell.tsx`, remove flow-progression responsibilities. Keep only:
- sidecar WS subscriptions
- DB sync for recordings/clips/caption outputs
- store refresh signals

The following shape is the target:

```ts
wsClient.on("recording_started", (data) => {
  applyRecordingWsPayload(data);
  void syncRecordingFromSidecarWsPayload(data);
  useFlowStore.getState().refreshRuntime();
});
```

Do not keep node transition decisions in frontend event handlers.

- [ ] **Step 2: Attach `flow_run_id` lineage when syncing outputs**

In `recordings.rs` and `clips.rs`, add fields and update paths so the workflow engine can attach output rows to the active `flow_run_id` and node-run lineage where available.

- [ ] **Step 3: Align sidecar routes around engine-dispatch contracts**

In `sidecar/src/routes/accounts.py` and `sidecar/src/core/watcher.py`, stop assuming the sidecar is the owner of workflow progression. Keep source polling/executor behavior but expose it as a callable capability used by the engine.

In `sidecar/src/routes/clips.py`, keep clip and caption execution endpoints thin and deterministic.

- [ ] **Step 4: Verify all touched layers**

Run from repo root:

```bash
npm run lint:js
```

Run from `src-tauri/`:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Run from `sidecar/`:

```bash
uv run ruff check src tests
uv run ruff format --check src tests
uv run ty check .
uv run pytest tests/ -q
```

Expected: frontend and Rust pass; if Python tools are unavailable in the environment, capture the exact failure and stop there.

- [ ] **Step 5: Commit**

```bash
git add src/components/layout/app-shell.tsx src-tauri/src/commands/recordings.rs src-tauri/src/commands/clips.rs sidecar/src/routes/accounts.py sidecar/src/routes/clips.py sidecar/src/core/watcher.py
git commit -m "feat(engine): move workflow progression out of app shell"
```

---

## Self-Review Checklist

### Spec Coverage

- `Accounts` removed from product IA: covered by Task 4
- `Start` owns source config: covered by Tasks 1, 4, and 6
- `Rust/Tauri` as workflow engine: covered by Tasks 2, 3, and 8
- draft/publish semantics: covered by Tasks 1, 3, 6, and 7
- fixed-node canvas UI: covered by Tasks 5 and 6
- runtime monitor under canvas: covered by Task 7
- sidecar as executor, not orchestrator: covered by Tasks 2 and 8

### Placeholder Scan

- No `TBD` / `TODO` placeholders remain in tasks.
- Every task names exact files and exact verification commands.
- Code steps include explicit snippets rather than hand-wavy descriptions.

### Type Consistency

- `FlowNodeDefinition`, `FlowEditorPayload`, `FlowRun`, and `FlowNodeRun` are introduced once and then reused consistently.
- `save_flow_node_draft`, `publish_flow_definition`, and `get_flow_definition` are the canonical command names used across Rust, API, and store layers.
