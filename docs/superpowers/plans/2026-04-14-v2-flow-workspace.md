# V2 Flow Workspace Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current `Recordings` + `Clips`-first workflow with a `Flows` workspace that manages one fixed pipeline per account, while keeping `Settings` focused on app-level configuration.

**Architecture:** Add a thin `flows` persistence layer in Rust/SQLite, keep the sidecar as the runtime engine, and reorganize the frontend around a new `FlowsPage` with `Flow List` and `Flow Detail`. To keep V2 proportional, enforce one flow per account, keep node order fixed, and store generated caption output directly on `clips` instead of introducing a separate caption entity.

**Tech Stack:** Tauri v2 + React 19 + TypeScript + Zustand (frontend), Rust + rusqlite (desktop backend), Python + FastAPI + WebSocket sidecar (runtime), SQLite migrations (desktop persistence)

**Spec:** `docs/superpowers/specs/2026-04-14-v2-flow-workspace-design.md`

---

## File Structure

### New Files


| Path                                             | Purpose                                                                              |
| ------------------------------------------------ | ------------------------------------------------------------------------------------ |
| `src-tauri/src/db/migrations/007_flows.sql`      | Add `flows`, `flow_node_configs`, and flow/caption columns on `recordings` + `clips` |
| `src-tauri/src/commands/flows.rs`                | Flow CRUD, list/detail queries, node config persistence                              |
| `src/pages/flows.tsx`                            | Main `Flows` page with internal `list/detail` state                                  |
| `src/stores/flow-store.ts`                       | Zustand state for flow list, active flow, selected node, filters                     |
| `src/components/flows/flow-list.tsx`             | Flow dashboard list                                                                  |
| `src/components/flows/flow-card.tsx`             | Flow summary card with mini-pipeline                                                 |
| `src/components/flows/flow-detail.tsx`           | Main flow workspace shell                                                            |
| `src/components/flows/flow-pipeline.tsx`         | Connected fixed-node pipeline header                                                 |
| `src/components/flows/flow-node-inspector.tsx`   | Quick-edit inspector for selected node                                               |
| `src/components/flows/flow-recordings-panel.tsx` | Flow-scoped recording workspace                                                      |
| `src/components/flows/flow-clips-panel.tsx`      | Flow-scoped clip workspace                                                           |
| `src/components/flows/flow-captions-panel.tsx`   | Flow-scoped caption workspace                                                        |
| `sidecar/src/core/captioner.py`                  | Minimal caption generation from clip/transcript context                              |


### Modified Files


| Path                                           | Changes                                                                |
| ---------------------------------------------- | ---------------------------------------------------------------------- |
| `src-tauri/src/db/init.rs`                     | Register migration 007                                                 |
| `src-tauri/src/db/models.rs`                   | Add `Flow`, `FlowNodeConfig`, `FlowDetail`, and clip caption fields    |
| `src-tauri/src/commands/mod.rs`                | Register `flows` module                                                |
| `src-tauri/src/lib.rs`                         | Register flow commands in `invoke_handler`                             |
| `src-tauri/src/commands/clips.rs`              | Add flow-scoped clip queries and caption update helpers                |
| `src-tauri/src/commands/recordings.rs`         | Persist and query `flow_id` on synced recordings                       |
| `src/components/layout/sidebar.tsx`            | Replace `Recordings` and `Clips` with `Flows`                          |
| `src/components/layout/app-shell.tsx`          | Mount `FlowsPage` and update WS handling to refresh flow runtime state |
| `src/stores/app-store.ts`                      | Extend navigation target to support `flows` and `flowId`               |
| `src/types/index.ts`                           | Add flow types and clip caption fields                                 |
| `src/lib/api.ts`                               | Add flow commands, flow detail queries, caption generation wrappers    |
| `src/pages/settings.tsx`                       | Remove workflow-first sections from the primary settings surface       |
| `src/components/recordings/recording-list.tsx` | Extract reusable list logic or wrap for flow context                   |
| `src/components/clips/clip-toolbar.tsx`        | Allow hidden account filter or flow-scoped mode                        |
| `src/components/clips/clip-grid.tsx`           | Allow embedding inside `Flow Detail`                                   |
| `src/components/clips/clip-detail.tsx`         | Allow returning to flow workspace without page-level coupling          |
| `sidecar/src/models/schemas.py`                | Add caption request/response schemas                                   |
| `sidecar/src/routes/clips.py`                  | Trigger caption generation after clip output or via explicit route     |


### Explicit V2 Constraints

- Enforce **one flow per account** with a unique constraint on `flows.account_id`
- Keep node order fixed: `start`, `record`, `clip`, `caption`, `upload`
- Store generated caption output on `clips` (`caption_text`, `caption_status`, `caption_error`, `caption_generated_at`) instead of creating a separate captions table
- Keep `Upload` visible in UI but non-executable in V2

---

### Task 1: Database Migration and Rust Models

**Files:**

- Create: `src-tauri/src/db/migrations/007_flows.sql`
- Modify: `src-tauri/src/db/init.rs`
- Modify: `src-tauri/src/db/models.rs`
- **Step 1: Create migration 007 for flows and caption columns**

Create `src-tauri/src/db/migrations/007_flows.sql`:

```sql
CREATE TABLE IF NOT EXISTS flows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL UNIQUE REFERENCES accounts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'idle',
    current_node TEXT NOT NULL DEFAULT 'start' CHECK (current_node IN ('start', 'record', 'clip', 'caption', 'upload')),
    last_live_at TEXT,
    last_run_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'))
);

CREATE TABLE IF NOT EXISTS flow_node_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
    node_key TEXT NOT NULL CHECK (node_key IN ('start', 'record', 'clip', 'caption', 'upload')),
    config_json TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
    UNIQUE(flow_id, node_key)
);

ALTER TABLE recordings ADD COLUMN flow_id INTEGER REFERENCES flows(id) ON DELETE SET NULL;

ALTER TABLE clips ADD COLUMN flow_id INTEGER REFERENCES flows(id) ON DELETE SET NULL;
ALTER TABLE clips ADD COLUMN caption_text TEXT;
ALTER TABLE clips ADD COLUMN caption_status TEXT NOT NULL DEFAULT 'pending' CHECK (caption_status IN ('pending', 'generating', 'completed', 'failed'));
ALTER TABLE clips ADD COLUMN caption_error TEXT;
ALTER TABLE clips ADD COLUMN caption_generated_at TEXT;

CREATE INDEX IF NOT EXISTS idx_flows_status ON flows(status);
CREATE INDEX IF NOT EXISTS idx_recordings_flow ON recordings(flow_id);
CREATE INDEX IF NOT EXISTS idx_clips_flow ON clips(flow_id);
CREATE INDEX IF NOT EXISTS idx_clips_caption_status ON clips(caption_status);
```

- **Step 2: Register migration 007 in the database initializer**

In `src-tauri/src/db/init.rs`, extend the migration list:

```rust
let migrations: Vec<(i64, &str)> = vec![
    (1, include_str!("migrations/001_initial.sql")),
    (2, include_str!("migrations/002_sidecar_recording_id.sql")),
    (3, include_str!("migrations/003_timestamps_gmt_plus_7.sql")),
    (4, include_str!("migrations/004_product_enhancements.sql")),
    (5, include_str!("migrations/005_product_media_files.sql")),
    (6, include_str!("migrations/006_speech_segments.sql")),
    (7, include_str!("migrations/007_flows.sql")),
];
```

- **Step 3: Add new Rust DTOs for flow data**

In `src-tauri/src/db/models.rs`, add:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Flow {
    pub id: i64,
    pub account_id: i64,
    pub name: String,
    pub enabled: bool,
    pub status: String,
    pub current_node: String,
    pub last_live_at: Option<String>,
    pub last_run_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FlowNodeConfig {
    pub id: i64,
    pub flow_id: i64,
    pub node_key: String,
    pub config_json: String,
    pub updated_at: String,
}
```

Also extend `Clip`:

```rust
pub flow_id: Option<i64>,
pub caption_text: Option<String>,
pub caption_status: String,
pub caption_error: Option<String>,
pub caption_generated_at: Option<String>,
```

- **Step 4: Verify Rust builds after schema/model changes**

Run from `src-tauri/`:

```bash
cargo check
```

Expected: the new migration is discovered and the model changes compile cleanly.

- **Step 5: Commit**

```bash
git add src-tauri/src/db/migrations/007_flows.sql src-tauri/src/db/init.rs src-tauri/src/db/models.rs
git commit -m "feat(db): add flow workspace schema"
```

---

### Task 2: Rust Flow Commands and Flow-Scoped Queries

**Files:**

- Create: `src-tauri/src/commands/flows.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/recordings.rs`
- Modify: `src-tauri/src/commands/clips.rs`
- **Step 1: Create the flow commands module**

Create `src-tauri/src/commands/flows.rs` with a minimal command surface:

```rust
#[tauri::command]
pub fn list_flows(state: State<'_, AppState>) -> Result<Vec<FlowSummaryRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT f.id, f.account_id, f.name, f.enabled, f.status, f.current_node, \
         f.last_live_at, f.last_run_at, f.last_error, a.username, \
         COUNT(DISTINCT r.id) AS recent_recordings, \
         COUNT(DISTINCT c.id) AS recent_clips, \
         COUNT(DISTINCT CASE WHEN c.caption_text IS NOT NULL AND trim(c.caption_text) <> '' THEN c.id END) AS recent_captions \
         FROM flows f \
         INNER JOIN accounts a ON a.id = f.account_id \
         LEFT JOIN recordings r ON r.flow_id = f.id \
         LEFT JOIN clips c ON c.flow_id = f.id \
         GROUP BY f.id, a.username \
         ORDER BY f.updated_at DESC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], map_flow_summary_row).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub fn get_flow_detail(state: State<'_, AppState>, flow_id: i64) -> Result<FlowDetail, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let flow = load_flow_row(&conn, flow_id)?;
    let node_configs = load_flow_node_configs(&conn, flow_id)?;
    let recordings = list_recordings_by_flow_conn(&conn, flow_id)?;
    let clips = list_clips_by_flow_conn(&conn, flow_id)?;
    Ok(FlowDetail { flow, node_configs, recordings, clips })
}

#[tauri::command]
pub fn create_flow(state: State<'_, AppState>, input: CreateFlowInput) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        &format!(
            "INSERT INTO flows (account_id, name, enabled, status, current_node, created_at, updated_at) \
             VALUES (?1, ?2, 1, 'idle', 'start', {}, {})",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![input.account_id, input.name.trim()],
    ).map_err(|e| e.to_string())?;
    let flow_id = conn.last_insert_rowid();
    for node_key in ["start", "record", "clip", "caption", "upload"] {
        conn.execute(
            &format!(
                "INSERT INTO flow_node_configs (flow_id, node_key, config_json, updated_at) VALUES (?1, ?2, ?3, {})",
                SQL_NOW_HCM
            ),
            params![flow_id, node_key, "{}"],
        ).map_err(|e| e.to_string())?;
    }
    Ok(flow_id)
}

#[tauri::command]
pub fn update_flow(state: State<'_, AppState>, flow_id: i64, input: UpdateFlowInput) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        &format!("UPDATE flows SET name = ?1, updated_at = {} WHERE id = ?2", SQL_NOW_HCM),
        params![input.name.trim(), flow_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_flow_enabled(state: State<'_, AppState>, flow_id: i64, enabled: bool) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let status = if enabled { "idle" } else { "disabled" };
    conn.execute(
        &format!(
            "UPDATE flows SET enabled = ?1, status = ?2, updated_at = {} WHERE id = ?3",
            SQL_NOW_HCM
        ),
        params![enabled, status, flow_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn save_flow_node_config(
    state: State<'_, AppState>,
    flow_id: i64,
    node_key: String,
    config_json: String,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        &format!(
            "UPDATE flow_node_configs SET config_json = ?1, updated_at = {} WHERE flow_id = ?2 AND node_key = ?3",
            SQL_NOW_HCM
        ),
        params![config_json, flow_id, node_key],
    ).map_err(|e| e.to_string())?;
    Ok(())
}
```

Use one query per responsibility and keep SQL explicit, following the current command style.

- **Step 2: Register the new module and Tauri handlers**

In `src-tauri/src/commands/mod.rs`:

```rust
pub mod flows;
```

In `src-tauri/src/lib.rs`, add the flow commands to `invoke_handler`:

```rust
commands::flows::list_flows,
commands::flows::get_flow_detail,
commands::flows::create_flow,
commands::flows::update_flow,
commands::flows::set_flow_enabled,
commands::flows::save_flow_node_config,
commands::flows::list_recordings_by_flow,
commands::flows::list_clips_by_flow,
```

- **Step 3: Make recording sync flow-aware**

In `src-tauri/src/commands/recordings.rs`, resolve `flow_id` by `account_id` when syncing sidecar events:

```rust
let flow_id: Option<i64> = conn
    .query_row(
        "SELECT id FROM flows WHERE account_id = ?1 LIMIT 1",
        [input.account_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())?;
```

Use that `flow_id` in both insert and update statements so new and existing recording rows keep their flow linkage.

- **Step 4: Extend clip queries for flow detail and caption fields**

In `src-tauri/src/commands/clips.rs`, update `map_clip_row` and all `SELECT` statements so they include:

```rust
c.flow_id,
c.caption_text,
c.caption_status,
c.caption_error,
c.caption_generated_at,
```

Then add one helper command for caption updates:

```rust
#[tauri::command]
pub fn update_clip_caption(
    state: State<'_, AppState>,
    clip_id: i64,
    caption_text: Option<String>,
    caption_status: String,
    caption_error: Option<String>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        &format!(
            "UPDATE clips SET caption_text = ?1, caption_status = ?2, caption_error = ?3, \
             caption_generated_at = CASE WHEN ?2 = 'completed' THEN {} ELSE caption_generated_at END, \
             updated_at = {} WHERE id = ?4",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![caption_text, caption_status, caption_error, clip_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}
```

- **Step 5: Verify the new commands compile and lint**

Run from `src-tauri/`:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Expected: formatting and clippy both pass with the new flow commands and query changes.

- **Step 6: Commit**

```bash
git add src-tauri/src/commands/flows.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/src/commands/recordings.rs src-tauri/src/commands/clips.rs
git commit -m "feat(tauri): add flow workspace commands"
```

---

### Task 3: Frontend Types, API Layer, and Store Foundation

**Files:**

- Modify: `src/types/index.ts`
- Modify: `src/lib/api.ts`
- Create: `src/stores/flow-store.ts`
- Modify: `src/stores/app-store.ts`
- **Step 1: Add flow and caption types**

In `src/types/index.ts`, add:

```ts
export type FlowNodeKey = "start" | "record" | "clip" | "caption" | "upload";
export type FlowStatus = "idle" | "watching" | "recording" | "processing" | "error" | "disabled";

export interface FlowNodeConfig {
  id: number;
  flow_id: number;
  node_key: FlowNodeKey;
  config_json: string;
  updated_at: string;
}

export interface FlowSummary {
  id: number;
  account_id: number;
  name: string;
  enabled: boolean;
  status: FlowStatus;
  current_node: FlowNodeKey;
  last_live_at: string | null;
  last_run_at: string | null;
  last_error: string | null;
  recent_recordings: number;
  recent_clips: number;
  recent_captions: number;
  account_username?: string;
}

export interface FlowDetail {
  flow: FlowSummary;
  node_configs: FlowNodeConfig[];
  recordings: Recording[];
  clips: Clip[];
}

export interface CreateFlowInput {
  account_id: number;
  name: string;
}
```

Extend `Clip` with:

```ts
flow_id?: number | null;
caption_text?: string | null;
caption_status?: "pending" | "generating" | "completed" | "failed";
caption_error?: string | null;
caption_generated_at?: string | null;
```

- **Step 2: Add flow API wrappers**

In `src/lib/api.ts`, add wrappers that match the new Tauri commands:

```ts
export async function listFlows(): Promise<FlowSummary[]> {
  return invoke<FlowSummary[]>("list_flows");
}

export async function getFlowDetail(flowId: number): Promise<FlowDetail> {
  return invoke<FlowDetail>("get_flow_detail", { flowId });
}

export async function createFlow(input: CreateFlowInput): Promise<number> {
  return invoke<number>("create_flow", { input });
}
```

Add wrappers for `set_flow_enabled`, `save_flow_node_config`, `list_recordings_by_flow`, `list_clips_by_flow`, and `update_clip_caption` in the same file.

- **Step 3: Create the flow Zustand store**

Create `src/stores/flow-store.ts`:

```ts
import { create } from "zustand";
import * as api from "@/lib/api";
import type { FlowDetail, FlowNodeKey, FlowSummary } from "@/types";

export const useFlowStore = create<FlowStore>((set, get) => ({
  flows: [],
  activeFlowId: null,
  activeFlow: null,
  view: "list",
  selectedNode: "start",
  loading: false,
  filters: { search: "", status: "all" },
  fetchFlows: async () => {
    set({ loading: true });
    try {
      const flows = await api.listFlows();
      set({ flows, loading: false });
    } catch {
      set({ flows: [], loading: false });
    }
  },
  fetchFlowDetail: async (flowId) => {
    set({ loading: true, activeFlowId: flowId, view: "detail" });
    const detail = await api.getFlowDetail(flowId);
    set({
      activeFlow: detail,
      selectedNode: detail.flow.current_node ?? "start",
      loading: false,
    });
  },
  saveFlowConfig: async (flowId, node, configJson) => {
    await api.saveFlowNodeConfig(flowId, node, configJson);
    await get().fetchFlowDetail(flowId);
  },
  setSelectedNode: (node) => set({ selectedNode: node }),
}));
```

Keep the store explicit and close to the current `clip-store.ts` pattern.

- **Step 4: Extend app navigation state for flows**

In `src/stores/app-store.ts`, change the navigation target to support flow detail deep links:

```ts
export type NavigationTarget = {
  page: string;
  clipId?: number;
  flowId?: number;
};
```

This lets the app route activity or future notifications into `Flows`.

- **Step 5: Verify the frontend foundation compiles**

Run from the repo root:

```bash
npm run lint:js
```

Expected: the new types, API wrappers, and store compile without TS or ESLint errors.

- **Step 6: Commit**

```bash
git add src/types/index.ts src/lib/api.ts src/stores/flow-store.ts src/stores/app-store.ts
git commit -m "feat(frontend): add flow workspace state foundation"
```

---

### Task 4: Navigation and Flow List

**Files:**

- Create: `src/pages/flows.tsx`
- Create: `src/components/flows/flow-list.tsx`
- Create: `src/components/flows/flow-card.tsx`
- Modify: `src/components/layout/sidebar.tsx`
- Modify: `src/components/layout/app-shell.tsx`
- **Step 1: Replace sidebar entries with a new `Flows` destination**

In `src/components/layout/sidebar.tsx`, change the navigation items from:

```ts
{ id: "recordings", label: "Recordings", icon: "🔴" },
{ id: "clips", label: "Clips", icon: "✂️" },
```

to:

```ts
{ id: "flows", label: "Flows", icon: "🧭" },
```

Keep the existing compact operational styling and status footer unchanged.

- **Step 2: Mount `FlowsPage` in the app shell**

In `src/components/layout/app-shell.tsx`, update the page union and component map:

```ts
type PageId =
  | "dashboard"
  | "accounts"
  | "flows"
  | "products"
  | "statistics"
  | "settings";

const pageMeta = {
  flows: { title: "Flows", subtitle: "Manage shop pipelines" },
};
```

Register `FlowsPage` and remove `RecordingsPage` / `ClipsPage` from the main page map.

- **Step 3: Create the `FlowsPage` list/detail state shell**

Create `src/pages/flows.tsx`:

```tsx
import { FlowDetail } from "@/components/flows/flow-detail";
import { FlowList } from "@/components/flows/flow-list";
import { useFlowStore } from "@/stores/flow-store";

export function FlowsPage() {
  const view = useFlowStore((s) => s.view);
  const activeFlowId = useFlowStore((s) => s.activeFlowId);
  return view === "detail" && activeFlowId != null ? <FlowDetail flowId={activeFlowId} /> : <FlowList />;
}
```

- **Step 4: Build the `Flow List` operations dashboard**

Create `src/components/flows/flow-list.tsx` and `flow-card.tsx` so each flow shows:

```tsx
<FlowCard
  flow={flow}
  onOpen={() => openFlow(flow.id)}
  onToggleEnabled={(enabled) => void toggleFlowEnabled(flow.id, enabled)}
/>
```

The card body should include:

- name and account
- overall status
- current node
- last run / last live / last error
- recent counts for recordings, clips, captions
- a mini fixed pipeline row
- **Step 5: Verify `Flows` replaces the old sidebar workflow cleanly**

Run from the repo root:

```bash
npm run lint:js
```

Expected: the app shell, sidebar, and new flows page render without TS or ESLint regressions.

- **Step 6: Commit**

```bash
git add src/pages/flows.tsx src/components/flows/flow-list.tsx src/components/flows/flow-card.tsx src/components/layout/sidebar.tsx src/components/layout/app-shell.tsx
git commit -m "feat(ui): add flows navigation and list"
```

---

### Task 5: Flow Detail Shell, Pipeline, and Inspector

**Files:**

- Create: `src/components/flows/flow-detail.tsx`
- Create: `src/components/flows/flow-pipeline.tsx`
- Create: `src/components/flows/flow-node-inspector.tsx`
- **Step 1: Create the connected pipeline header**

Create `src/components/flows/flow-pipeline.tsx`:

```tsx
const FLOW_NODES: FlowNodeKey[] = ["start", "record", "clip", "caption", "upload"];

export function FlowPipeline({ selectedNode, onSelect, flow }: Props) {
  return (
    <div className="flex flex-wrap items-center gap-3 rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
      {FLOW_NODES.map((node, index) => (
        <Fragment key={node}>
          <button type="button" onClick={() => onSelect(node)}>{node}</button>
          {index < FLOW_NODES.length - 1 ? <span className="text-[var(--color-text-muted)]">-></span> : null}
        </Fragment>
      ))}
    </div>
  );
}
```

Each node button should surface status color, current summary, and selected state.

- **Step 2: Create the quick-edit inspector**

Create `src/components/flows/flow-node-inspector.tsx`:

```tsx
export function FlowNodeInspector({ node, config, onSave }: Props) {
  return (
    <aside className="app-panel-subtle rounded-2xl p-4">
      <p className="text-xs uppercase tracking-[0.16em] text-[var(--color-text-muted)]">Node Inspector</p>
      <h3 className="mt-2 text-base font-semibold text-[var(--color-text)]">{node}</h3>
      <Label className="mt-4 block text-xs text-[var(--color-text-muted)]">Raw config JSON</Label>
      <Textarea defaultValue={config?.config_json ?? "{}"} className="mt-2 min-h-32" />
      <Button className="mt-4" onClick={() => void onSave()}>
        Save node config
      </Button>
    </aside>
  );
}
```

Do not put the full workspace into the inspector; keep it focused on quick edits.

- **Step 3: Build the `FlowDetail` layout shell**

Create `src/components/flows/flow-detail.tsx`:

```tsx
export function FlowDetail({ flowId }: { flowId: number }) {
  const selectedNode = useFlowStore((s) => s.selectedNode);
  const activeFlow = useFlowStore((s) => s.activeFlow);
  const setSelectedNode = useFlowStore((s) => s.setSelectedNode);
  return (
    <div className="space-y-6">
      <Button type="button" variant="outline">Back to Flows</Button>
      <FlowPipeline
        flow={activeFlow?.flow ?? null}
        selectedNode={selectedNode}
        onSelect={setSelectedNode}
      />
      <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_320px]">
        <div className="space-y-6">
          <FlowRecordingsPanel recordings={activeFlow?.recordings ?? []} />
          <FlowClipsPanel clips={activeFlow?.clips ?? []} />
          <FlowCaptionsPanel clips={activeFlow?.clips ?? []} />
        </div>
        <FlowNodeInspector
          node={selectedNode}
          config={activeFlow?.node_configs.find((row) => row.node_key === selectedNode) ?? null}
          onSave={async () => {
            if (!activeFlow) return;
            await useFlowStore.getState().saveFlowConfig(activeFlow.flow.id, selectedNode, "{}");
          }}
        />
      </div>
    </div>
  );
}
```

Use the hybrid layout from the approved spec: pipeline on top, workspace center, inspector right, sections below.

- **Step 4: Verify node selection updates both pipeline and inspector**

Run from the repo root:

```bash
npm run lint:js
```

Expected: `selectedNode` is fully typed and the shell compiles without prop mismatches.

- **Step 5: Commit**

```bash
git add src/components/flows/flow-detail.tsx src/components/flows/flow-pipeline.tsx src/components/flows/flow-node-inspector.tsx
git commit -m "feat(ui): add flow detail shell"
```

---

### Task 6: Reuse Recordings and Clips Inside Flow Detail

**Files:**

- Create: `src/components/flows/flow-recordings-panel.tsx`
- Create: `src/components/flows/flow-clips-panel.tsx`
- Create: `src/components/flows/flow-captions-panel.tsx`
- Modify: `src/components/recordings/recording-list.tsx`
- Modify: `src/components/clips/clip-toolbar.tsx`
- Modify: `src/components/clips/clip-grid.tsx`
- Modify: `src/components/clips/clip-detail.tsx`
- **Step 1: Extract flow-scoped recording rendering**

Wrap the existing recording UI so `Flow Detail` can pass a filtered list instead of relying only on global active sidecar state:

```tsx
export function FlowRecordingsPanel({ recordings }: { recordings: Recording[] }) {
  if (recordings.length === 0) {
    return <p className="text-sm text-[var(--color-text-muted)]">No recordings for this flow yet.</p>;
  }
  return <RecordingList recordings={recordings} compact />;
}
```

If `recording-list.tsx` is too page-specific, split the rendering portion into a prop-driven component and keep the old wrapper thin.

- **Step 2: Make clip components embeddable in flow context**

Update `clip-toolbar.tsx` to support a flow-scoped mode:

```tsx
export function ClipToolbar({ hideAccountFilter = false }: { hideAccountFilter?: boolean }) {
  const filters = useClipStore((s) => s.filters);
  const setFilter = useClipStore((s) => s.setFilter);
  return !hideAccountFilter ? (
    <select
      value={filters.accountId ?? ""}
      onChange={(e) => setFilter({ accountId: e.target.value === "" ? null : Number(e.target.value) })}
    />
  ) : null;
}
```

Use the same pattern for `ClipGrid` / `ClipDetail` so they can be shown inside `Flow Detail` without assuming they own the whole page.

- **Step 3: Create the flow caption panel from clip caption fields**

Create `src/components/flows/flow-captions-panel.tsx`:

```tsx
export function FlowCaptionsPanel({ clips }: { clips: Clip[] }) {
  const captioned = clips.filter((clip) => clip.caption_text?.trim());
  return (
    <div className="space-y-3">
      {captioned.map((clip) => (
        <article key={clip.id} className="app-panel-subtle rounded-xl p-4">
          <p className="text-sm font-medium text-[var(--color-text)]">{clip.title ?? `Clip #${clip.id}`}</p>
          <p className="mt-2 text-sm text-[var(--color-text-soft)]">{clip.caption_text}</p>
        </article>
      ))}
    </div>
  );
}
```

This keeps V2 caption output lightweight and avoids inventing a new caption entity too early.

- **Step 4: Mount the three workspace panels in `FlowDetail`**

In `src/components/flows/flow-detail.tsx`, add:

```tsx
<FlowRecordingsPanel recordings={detail.recordings} />
<FlowClipsPanel clips={detail.clips} />
<FlowCaptionsPanel clips={detail.clips} />
```

The selected node can control which panel is visually emphasized, but all three should remain available in the detail workspace.

- **Step 5: Verify flow detail now covers the old practical workflow**

Run from the repo root:

```bash
npm run lint:js
```

Expected: `Flow Detail` compiles with reusable recordings/clips panels and no duplicate-page assumptions.

- **Step 6: Commit**

```bash
git add src/components/flows/flow-recordings-panel.tsx src/components/flows/flow-clips-panel.tsx src/components/flows/flow-captions-panel.tsx src/components/recordings/recording-list.tsx src/components/clips/clip-toolbar.tsx src/components/clips/clip-grid.tsx src/components/clips/clip-detail.tsx src/components/flows/flow-detail.tsx
git commit -m "feat(ui): embed recordings clips and captions in flow detail"
```

---

### Task 7: Sidecar Caption Generation and Runtime Mapping

**Files:**

- Create: `sidecar/src/core/captioner.py`
- Modify: `sidecar/src/models/schemas.py`
- Modify: `sidecar/src/routes/clips.py`
- Modify: `src/components/layout/app-shell.tsx`
- Modify: `src/lib/sidecar-db-sync.ts`
- Modify: `src/lib/api.ts`
- **Step 1: Create a minimal caption generator in the sidecar**

Create `sidecar/src/core/captioner.py`:

```python
from __future__ import annotations

def generate_caption(*, username: str, transcript_text: str | None, clip_title: str | None) -> str:
    source = (transcript_text or "").strip()
    if source:
        line = source.split(".")[0].strip()
        return f"@{username} {line}"[:2200].strip()
    title = (clip_title or "Clip hay tu livestream").strip()
    return f"@{username} {title}"[:2200].strip()
```

This is intentionally simple for V2: deterministic, no new API dependency, and good enough to power the caption node.

- **Step 2: Add caption schemas and an explicit sidecar route helper**

In `sidecar/src/models/schemas.py`, add:

```python
class GenerateCaptionRequest(BaseModel):
    clip_id: int
    username: str
    clip_title: str | None = None
    transcript_text: str | None = None


class GenerateCaptionResponse(BaseModel):
    clip_id: int
    caption_text: str
    status: str = "completed"
```

```

Then in `sidecar/src/routes/clips.py`, add:

```python
@router.post("/api/captions/generate", response_model=GenerateCaptionResponse)
async def generate_caption_route(body: GenerateCaptionRequest):
    text = generate_caption(
        username=body.username,
        transcript_text=body.transcript_text,
        clip_title=body.clip_title,
    )
    await ws_manager.broadcast("caption_ready", {"clip_id": body.clip_id, "caption_text": text})
    return GenerateCaptionResponse(clip_id=body.clip_id, caption_text=text)
```

- **Step 3: Trigger caption generation after clip insertion**

In `src/components/layout/app-shell.tsx`, extend the existing `clip_ready` handler so that after a clip is inserted into SQLite, the app requests caption generation:

```ts
const clipId = await insertClipFromSidecarWsPayload(data);
if (clipId != null) {
  await api.generateCaptionForClip({
    clip_id: clipId,
    username: String(data.username ?? ""),
    clip_title: null,
    transcript_text: typeof data.transcript_text === "string" ? data.transcript_text : null,
  });
}
```

This keeps the runtime automatic through `Clip -> Caption` without inventing a new queue system yet.

- **Step 4: Persist `caption_ready` events into SQLite-backed clip rows**

Add a new helper to `src/lib/sidecar-db-sync.ts`:

```ts
export async function syncClipCaptionFromWsPayload(data: Record<string, unknown>): Promise<void> {
  const clipId = typeof data.clip_id === "number" ? data.clip_id : Number(data.clip_id);
  const captionText = typeof data.caption_text === "string" ? data.caption_text : null;
  if (!Number.isFinite(clipId) || !captionText) return;
  await invoke("update_clip_caption", {
    clipId,
    captionText,
    captionStatus: "completed",
    captionError: null,
  });
}
```

Wire that helper into `app-shell.tsx` with a new WebSocket subscription for `caption_ready`.

- **Step 5: Verify sidecar and frontend integration**

Run from `sidecar/`:

```bash
uv run ruff check src tests
uv run ruff format --check src tests
uv run ty check .
uv run pytest tests/ -q
```

Then run from the repo root:

```bash
npm run lint:js
```

Expected: caption generation compiles cleanly on both sides and the new event handling does not break existing websocket logic.

- **Step 6: Commit**

```bash
git add sidecar/src/core/captioner.py sidecar/src/models/schemas.py sidecar/src/routes/clips.py src/components/layout/app-shell.tsx src/lib/sidecar-db-sync.ts src/lib/api.ts
git commit -m "feat(runtime): auto-generate captions in flow pipeline"
```

---

### Task 8: Settings Boundary, Flow Defaults, and Final Verification

**Files:**

- Modify: `src/pages/settings.tsx`
- Modify: `src/components/flows/flow-node-inspector.tsx`
- Modify: `src/components/flows/flow-detail.tsx`
- Modify: `src/lib/api.ts`
- **Step 1: Reduce `Settings` to app-level concerns**

In `src/pages/settings.tsx`, remove the workflow-first sections from the main page flow and keep:

```tsx
<CardTitle>Storage</CardTitle>
<CardTitle>Paths</CardTitle>
<CardTitle>API Keys</CardTitle>
<CardTitle>Cleanup</CardTitle>
```

Move copy such as recording/clip defaults out of the main narrative so the page reads as system configuration rather than the daily workflow surface.

- **Step 2: Hydrate new flows from existing settings defaults**

In the flow creation path, read the current workflow settings and write them into `flow_node_configs` as the initial payload:

```ts
const recordDefaults = {
  maxDurationMinutes: await getSetting("TIKCLIP_MAX_DURATION_MINUTES"),
};
const clipDefaults = {
  clipMin: await getSetting("clip_min_duration"),
  clipMax: await getSetting("clip_max_duration"),
  autoProcessAfterRecord: await getSetting("auto_process_after_record"),
};
```

Use these only to seed a new flow; after creation, `Flow Detail` becomes the editing surface.

- **Step 3: Add one manual verification checklist to the plan implementation notes**

Before calling the feature complete, manually verify:

```text
1. Create a flow for an existing account
2. See it appear in Flow List with idle/watching status
3. Open Flow Detail and save one Record node override
4. Trigger or simulate a live account and confirm Record -> Clip -> Caption updates
5. Confirm the same outputs are visible without using the old Recordings/Clips pages
6. Confirm Settings still handles storage/path/API key concerns only
```

- **Step 4: Run full repo verification for touched layers**

Run from the repo root:

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

Expected: all touched layers pass their repo-standard verification commands.

- **Step 5: Commit**

```bash
git add src/pages/settings.tsx src/components/flows/flow-node-inspector.tsx src/components/flows/flow-detail.tsx src/lib/api.ts
git commit -m "feat(ui): finish flow workspace rollout"
```

---

## Self-Review Checklist

### Spec Coverage

- `Flows` replaces `Recordings` and `Clips` in navigation: covered by Tasks 3-4
- `Flow List` with operational summary and counts: covered by Task 4
- `Flow Detail` with connected nodes and inspector: covered by Task 5
- Flow-scoped recordings, clips, captions: covered by Task 6
- Fixed node order and per-node config: covered by Tasks 1-2 and Task 5
- Automatic `Start -> Record -> Clip -> Caption`: covered by Tasks 2 and 7
- `Upload` placeholder only: covered by Task 5 UI shell
- `Settings` reduced to app-level concerns: covered by Task 8

### Plan Notes

- This plan intentionally avoids a dynamic graph editor
- This plan intentionally keeps one flow per account in V2 to remove ambiguous runtime attribution
- This plan intentionally stores caption output on `clips` to keep the caption node practical without growing a second content model too early

