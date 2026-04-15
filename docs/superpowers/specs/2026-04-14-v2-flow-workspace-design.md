# TikClip V2 - Flow Workspace Redesign

**Date:** 2026-04-14
**Status:** Draft for review
**Builds on:** Phase 2 Smart Clips (`2026-04-12-tikclip-phase2-smart-clips-design.md`)

---

## 1. Overview

V2 changes TikClip's primary operating model from separate pages (`Recordings`, `Clips`, and workflow-related `Settings`) into a flow-centered workspace. The new main entry point is `Flows`, where each flow represents a fixed pipeline bound to one TikTok shop/account.

The fixed pipeline for V2 is:

`Start -> Record -> Clip -> Caption -> Upload`

`Upload` remains visible in the pipeline for continuity, but is a placeholder node in V2 and does not execute real posting yet.

The redesign has two goals:
- Make flow execution and monitoring the main user experience instead of scattered pages
- Bring flow-specific configuration into the same workspace as runtime status and outputs

### Goals

- Replace sidebar-first recording/clip workflow with a flow-first workspace
- Support multiple flows, each bound to exactly one account/shop
- Keep node order fixed to avoid overbuilding a graph editor in V2
- Let each flow override node config while still falling back to global defaults
- Make `Flow Detail` the main workspace for recordings, clips, and generated captions

### Non-goals

- Dynamic graph editing, node reordering, or custom node types
- Real TikTok upload integration in this phase
- Rewriting the sidecar recording/clip engine from scratch
- Removing existing recordings/clips data structures; they should be reused and re-framed

---

## 2. Product Direction

Three approaches were considered:

1. Use `Flows` only as a high-level coordinator while keeping `Recordings` and `Clips` as primary workspaces
2. Use `Flows` with two sub-screens (`Flow List` and `Flow Detail`), with `Flow Detail` replacing the practical role of `Recordings` and `Clips`
3. Build a full canvas-style graph editor with direct node editing

### Recommendation

Approach 2 is the chosen direction.

It delivers a real redesign instead of another navigation layer, fits the current app structure, and stays proportional to the fixed-node scope. It also avoids the complexity of a freeform graph editor while still making the app feel flow-native.

---

## 3. Information Architecture

### 3.1 Sidebar

Sidebar changes:
- Remove `Recordings`
- Remove `Clips`
- Add `Flows`
- Keep `Dashboard`, `Accounts`, `Products`, `Statistics`, and `Settings`

`Settings` becomes system-level configuration only.

### 3.2 Flows Area

`Flows` contains two sub-screens:

1. `Flow List`
- Management view for all flows
- Operational summary per flow
- Create, enable/disable, duplicate, delete

2. `Flow Detail`
- Main workspace for one flow
- Pipeline visualization with connected nodes
- Node-level configuration and runtime state
- Embedded recordings/clips/captions views for that flow

### 3.3 Settings Boundary

`Settings` keeps only app-level/system-level concerns such as:
- storage paths and cleanup policy
- sidecar and runtime infrastructure settings
- API keys
- other machine-wide defaults

Workflow-specific settings currently living in `Settings` become flow defaults and flow overrides rather than the primary editing surface.

---

## 4. Flow Model

### 4.1 Flow Scope

Each flow:
- is bound to exactly one account/shop
- uses the fixed node order `Start -> Record -> Clip -> Caption -> Upload`
- has flow-level enabled/disabled state
- has node-specific configuration

There may be multiple flows in the system, but all share the same fixed node set and order in V2.

### 4.2 Config Inheritance

Each node reads config in this order:

1. flow-specific override
2. global default from `Settings`
3. hardcoded fallback already used by the app

This preserves current behavior while moving day-to-day editing into `Flows`.

### 4.3 Runtime Behavior

`Start` watches a shop/account and triggers `Record` when livestream activity is detected. If the shop remains live or goes live again, the flow continues the cycle for that account. V2 is designed for automatic progression through `Start`, `Record`, `Clip`, and `Caption` without requiring manual approval gates.

---

## 5. Flow List

`Flow List` is an operations dashboard, not a thin CRUD list.

### 5.1 Row/Card Content

Each flow item shows:
- flow name
- bound shop/account
- overall status (`idle`, `watching`, `recording`, `processing`, `error`, `disabled`)
- current pipeline step
- last live detected time
- last run time
- last error summary
- recent output counts: recordings, clips, captions
- enable/disable toggle

### 5.2 Actions

Each flow item supports:
- open flow detail
- enable/disable
- duplicate
- delete

Top-bar actions:
- create flow
- search
- filter by status

### 5.3 Visual Treatment

Each flow item should include a compact mini-pipeline indicator showing where the flow is currently active or blocked. The page should follow the existing dark Raycast-inspired visual system from `DESIGN.md`: deep near-black background, restrained card surfaces, subtle borders, blue for interactive emphasis, and red reserved for error or destructive states.

---

## 6. Flow Detail Workspace

`Flow Detail` is the main workspace for operating a single flow.

### 6.1 Layout

The layout uses a hybrid model:
- top: fixed horizontal pipeline with connected nodes
- right side: `Node Inspector` for quick edits and summary state
- main content: operational workspace for runtime data and outputs
- lower sections: full detail sections for each node

This hybrid layout was chosen because it keeps the mental model of a flow while still giving enough space for recording/clip/caption work.

### 6.2 Pipeline Header

The top pipeline shows:
- `Start`
- `Record`
- `Clip`
- `Caption`
- `Upload`

Each node shows:
- node label
- status color/state
- short summary text
- warning/error marker if needed

Nodes are connected visually with lines so the sequence reads as one system. Clicking a node:
- updates the inspector to that node
- focuses the corresponding detail section below
- updates the main workspace context when appropriate

### 6.3 Inspector

The right-hand inspector is optimized for quick edits:
- node status
- key config fields
- compact summary of input/output state
- save action for node-level changes

The inspector is not the only editing surface. It is intentionally shallow enough for quick adjustments and status checks.

### 6.4 Detail Sections

Below the workspace, each node has a dedicated section:
- `Start`
- `Record`
- `Clip`
- `Caption`
- `Upload`

These sections handle deeper editing and richer data views than the inspector can comfortably support.

### 6.5 Workspace Role

`Flow Detail` replaces the practical role of `Recordings` and `Clips`.

It should include:
- current and recent recordings for the active flow
- generated clips for the active flow
- generated captions for the active flow
- quick operational actions that were previously split across pages

This is a primary workspace, not only a config screen.

---

## 7. Node Definitions

Each node has three layers of information:
- `Config`
- `Runtime status`
- `Outputs`

### 7.1 Start

Purpose:
- bind the flow to one shop/account
- watch for livestream activity
- trigger the recording cycle
- continue the cycle if the shop remains live or becomes live again

Config:
- account/shop binding
- polling interval override if needed
- retry/reconnect rules
- restart eligibility after a previous session ends

Runtime status:
- `idle`
- `watching`
- `live_detected`
- `waiting_reconnect`
- `error`

Outputs:
- live detection events
- session start metadata
- account/session context used by downstream nodes

### 7.2 Record

Purpose:
- run livestream recording when `Start` detects a live session
- segment or end recordings according to current record behavior
- continue if the live session still requires more recording work

Config:
- max duration per recording
- output path override if needed
- auto-restart after segment end
- record strategy settings already supported by the sidecar/runtime

Runtime status:
- `queued`
- `recording`
- `stopping`
- `completed`
- `failed`

Outputs:
- recording files
- duration and timestamps
- record errors and retry state

### 7.3 Clip

Purpose:
- automatically generate clips from recordings using configured rules
- pass outputs directly to `Caption`

Config:
- min clip duration
- max clip duration
- auto-process-after-record behavior
- clip generation thresholds and heuristics
- optional product/tagging-related settings if they stay coupled to clip generation

Runtime status:
- `waiting_for_recordings`
- `processing`
- `completed`
- `partial`
- `failed`

Outputs:
- generated clips
- clip creation counts
- evidence or score metadata where available
- failure summaries for skipped or failed clip jobs

### 7.4 Caption

Purpose:
- automatically generate captions from clips
- store caption output for future upload integration

Config:
- caption generation mode
- prompt/template
- hashtag rules
- style preset if needed later

Runtime status:
- `waiting_for_clips`
- `generating`
- `completed`
- `failed`

Outputs:
- caption text
- supporting metadata
- generation time and error state

### 7.5 Upload

Purpose in V2:
- placeholder only
- visible in the flow for future continuity

Config:
- no deep editable config in V2

Runtime status:
- `inactive`

Outputs:
- none in V2

---

## 8. Data Model

The redesign should preserve existing data structures where possible and add a flow layer above them.

### 8.1 New Core Tables

#### `flows`

- `id`
- `name`
- `account_id`
- `enabled`
- `status`
- `current_node`
- `last_live_at`
- `last_run_at`
- `last_error`
- `created_at`
- `updated_at`

#### `flow_node_configs`

- `id`
- `flow_id`
- `node_key` (`start`, `record`, `clip`, `caption`, `upload`)
- `config_json`
- `updated_at`

#### `flow_runs` (or equivalent session table)

- `id`
- `flow_id`
- `started_at`
- `ended_at`
- `status`
- `trigger_source`
- `error`
- `live_session_key` or equivalent session identifier

### 8.2 Existing Tables Extended

`accounts`
- unchanged as the source entity a flow binds to

`recordings`
- add `flow_id`
- add `flow_run_id` if needed for per-run traceability

`clips`
- add `flow_id`
- add `flow_run_id` if needed

`captions`
- if a dedicated caption table exists later, it should also carry `flow_id` and optionally `flow_run_id`
- if caption output is initially stored elsewhere, the same flow linkage rule still applies

### 8.3 Why This Model

This model avoids introducing a dynamic graph schema while still giving the UI enough structure for:
- flow list summaries
- flow detail runtime views
- node-specific config
- run history
- filtering recordings/clips/captions by flow

---

## 9. Frontend State Shape

Introduce a dedicated `flow-store.ts` to manage the new workspace.

Suggested state:

```ts
type FlowView = "list" | "detail";
type FlowNodeKey = "start" | "record" | "clip" | "caption" | "upload";

type FlowStore = {
  flows: FlowSummary[];
  activeFlowId: number | null;
  view: FlowView;
  selectedNode: FlowNodeKey;
  loading: boolean;
  filters: {
    search: string;
    status: "all" | "idle" | "watching" | "recording" | "processing" | "error" | "disabled";
  };

  fetchFlows: () => Promise<void>;
  fetchFlowDetail: (flowId: number) => Promise<void>;
  setActiveFlowId: (flowId: number | null) => void;
  setView: (view: FlowView) => void;
  setSelectedNode: (node: FlowNodeKey) => void;
  saveFlowConfig: (flowId: number, node: FlowNodeKey, patch: unknown) => Promise<void>;
  toggleFlowEnabled: (flowId: number, enabled: boolean) => Promise<void>;
  createFlow: (input: CreateFlowInput) => Promise<void>;
  deleteFlow: (flowId: number) => Promise<void>;
};
```

`selectedNode` should default to either:
- `start`, or
- the most active/recent runtime node for that flow

The page-level navigation can follow the existing app pattern by keeping `Flows` as one main page and using internal view state for `list/detail`, similar to the current `ClipsPage` list/detail approach.

---

## 10. Backend and Runtime Integration

### 10.1 Reuse Existing Engines

The redesign should not rewrite the sidecar recording, clip generation, or websocket machinery first. Instead:
- continue using existing sidecar events and HTTP data sources
- continue syncing runtime state into SQLite/frontend stores
- layer `flow_id` and flow-aware summary logic on top

### 10.2 Sidecar and Tauri Responsibilities

Rust/Tauri should own:
- flow CRUD
- flow node config persistence
- list/detail queries for flow summaries and linked recordings/clips

The sidecar should continue to own:
- live detection
- recording execution
- clip generation
- caption generation when introduced in execution scope
- runtime event broadcasting

### 10.3 Runtime Mapping

At runtime, events such as recording started/finished and clip created should be attributable to a flow so the `Flows` UI can summarize status without forcing a separate workflow page.

---

## 11. Error Handling

### 11.1 Flow List

`Flow List` should surface:
- last error summary per flow
- disabled state distinctly from error state
- stale/no recent activity as a neutral state, not an error

### 11.2 Flow Detail

Each node should expose:
- current node status
- last failure reason
- last successful output time where useful

If downstream nodes fail, upstream data should remain visible so the user can understand what already succeeded.

### 11.3 Config Validation

Node config editors should validate locally before save, especially for numeric values currently controlled by `Settings`, such as:
- polling interval
- recording max duration
- clip min/max duration
- threshold values

Invalid node config should fail fast in the editor rather than leaving broken runtime state for the sidecar to discover later.

---

## 12. UI and Interaction Principles

The redesign should preserve the established dark desktop utility feel from `DESIGN.md`.

### 12.1 Visual Direction

- Use the existing near-black background and restrained surface hierarchy
- Keep cards and panels compact, operational, and information-dense
- Use blue for selected/active interactive state
- Use green for successful active processing states
- Use yellow for warnings or partial completion
- Use red only for failures, destructive actions, and blocking errors

### 12.2 Flow Detail Behavior

- Clicking a node should feel immediate and lightweight
- Inspector edits should be quick and not require navigating away
- Rich operational views belong in the main content area and lower sections
- Mobile and narrow widths should stack inspector below the main workspace while keeping pipeline readability intact

### 12.3 Flow List Behavior

- Optimize for at-a-glance scanning across many shops
- Prefer cards or dense rows with strong status hierarchy over decorative layouts
- Do not bury recent output counts or errors behind detail navigation

---

## 13. Rollout Plan

The rollout should be incremental to reduce migration risk.

1. Add core flow schema and persistence
2. Add `Flows` page shell and sidebar entry
3. Build `Flow List`
4. Build `Flow Detail` header pipeline and inspector
5. Reuse/adapt current recordings and clips UI inside `Flow Detail`
6. Move workflow-related editing from `Settings` into flow node config
7. Remove `Recordings` and `Clips` from sidebar after `Flows` reaches functional parity

This sequencing allows runtime logic to stay stable while the UI and data ownership are reorganized.

---

## 14. Verification Strategy

Implementation should verify three levels:

### 14.1 Data and Persistence

- flow CRUD works correctly
- per-node config saves and reloads correctly
- fallback from flow override -> global default -> hardcoded fallback behaves as expected

### 14.2 UI Behavior

- `Flows` navigation works from list to detail and back
- node click updates inspector and section focus consistently
- recordings/clips/caption views are correctly scoped to the active flow
- `Recordings` and `Clips` can be removed from sidebar without losing practical workflow coverage

### 14.3 Runtime Behavior

- live detection updates the correct flow
- recording events update flow status accurately
- clip generation and caption generation appear in the correct flow workspace
- flow-level error summaries stay understandable when downstream nodes fail

---

## 15. Final Scope Summary

V2 turns TikClip into a flow-centered desktop workspace:
- `Flows` becomes the main operational entry point
- each flow is bound to one shop/account
- node order is fixed
- node config and runtime data live in one workspace
- `Flow Detail` becomes the practical home for recordings, clips, and captions
- `Settings` remains system-focused
- `Upload` stays visible as a placeholder for the next phase

This keeps the redesign substantial for users while staying compatible with the current desktop, SQLite, and sidecar architecture.
