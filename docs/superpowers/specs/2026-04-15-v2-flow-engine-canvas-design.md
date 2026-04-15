# TikClip V2 - Flow Engine And Canvas Workflow Design

**Date:** 2026-04-15
**Status:** Draft for review
**Supersedes:** `2026-04-14-v2-flow-workspace-design.md`

---

## 1. Overview

V2 is no longer only a page-level redesign. It is a shift in architecture and product model.

TikClip becomes a workflow-driven application where each `Flow` is an independent runtime unit with its own node logic, node configuration, execution history, and outputs.

The fixed V2 workflow is:

`Start -> Record -> Clip -> Caption -> Upload`

The key architectural changes are:
- `Flows` becomes the primary and only workflow entry point
- `Accounts` is removed from the product information architecture
- account/source configuration moves into the `Start` node
- each flow owns its own workflow logic and configuration
- orchestration moves into `Rust/Tauri`
- the Python sidecar becomes a third-party executor/integration service rather than the workflow orchestrator

V2 should feel closer to a fixed-node workflow tool than a collection of utility pages.

---

## 2. Goals And Non-Goals

### Goals

- Make each flow an independent workflow runtime
- Remove `Accounts` as a first-class product surface
- Move all account/watch/source configuration into `Start`
- Keep node types fixed while allowing per-flow logic and config on each node
- Run orchestration in `Rust/Tauri` with durable SQLite-backed state
- Present the workflow visually as a canvas scene with node-focused editing
- Support draft editing with explicit publish semantics

### Non-goals

- Freeform graph editing or drag-and-drop node placement
- Reordering nodes in V2
- Custom user-defined node types in V2
- Global workflow defaults that remain active at runtime
- Letting multiple flows share one account/source definition
- Letting the sidecar decide workflow progression

---

## 3. Product Direction

Three architectural directions were considered:

1. Keep the current runtime split and only redesign UI around flows
2. Make `Rust/Tauri` the workflow engine, with the sidecar acting as an executor
3. Put workflow orchestration in frontend state and use backend only for persistence

### Recommendation

Direction 2 is the chosen design.

It matches the desired mental model:
- `Flow` is the unit of execution
- node logic belongs to the app's workflow engine
- the sidecar performs external tasks but does not own orchestration

This also gives the product durable runtime state, resumable execution, and cleaner separation between editor, engine, and executor.

---

## 4. Information Architecture

### 4.1 Sidebar

Sidebar should become:
- `Dashboard`
- `Flows`
- `Products`
- `Statistics`
- `Settings`

`Accounts` is removed from the sidebar.

### 4.2 Flows As The Only Workflow Surface

`Flows` is now the only place where users:
- create a new source/account workflow
- configure source/account details
- edit node behavior
- publish workflow changes
- monitor runtime progress
- inspect outputs

### 4.3 Settings Boundary

`Settings` keeps only app-level or machine-level concerns such as:
- storage paths
- cleanup policy
- sidecar connectivity and infrastructure
- API keys and system integrations

`Settings` no longer provides workflow defaults for active runtime behavior.

---

## 5. Workflow Model

### 5.1 Flow As Runtime Unit

Each flow is an independent workflow instance.

Each flow has:
- one fixed ordered set of nodes
- one source/account definition inside `Start`
- one published runtime definition used by the engine
- one draft definition being edited in the UI
- its own execution history and node runtime state

There are no shared workflow defaults at runtime.

### 5.2 Fixed Node Types

V2 keeps the node set fixed:
- `Start`
- `Record`
- `Clip`
- `Caption`
- `Upload`

The node order is fixed.

The important flexibility is not graph editing. It is that each node in each flow has independent logic and configuration.

### 5.3 Start Owns Source Configuration

`Start` contains the full source/account configuration that was previously modeled as `Accounts`.

`Start` config includes:
- username or source identity
- cookies/session values
- proxy settings
- watcher/live detection configuration
- polling behavior
- source-specific retry and reconnect behavior

This means:
- product-level `Accounts` disappears
- `1 flow = 1 source/account definition`
- multiple flows cannot share one account definition

---

## 6. Data Model

The data model must separate workflow definition from workflow execution.

### 6.1 Flow Definition

`flows`
- `id`
- `name`
- `enabled`
- `status`
- `current_node`
- `published_version`
- `draft_version`
- `created_at`
- `updated_at`

This table is the identity of the workflow.

It does not contain `account_id`.

### 6.2 Flow Nodes

`flow_nodes`
- `id`
- `flow_id`
- `node_key` (`start`, `record`, `clip`, `caption`, `upload`)
- `position`
- `draft_config_json`
- `published_config_json`
- `draft_updated_at`
- `published_at`

Even though node order is fixed, a dedicated node table keeps the model explicit and makes node-specific versioning straightforward.

### 6.3 Flow Runs

`flow_runs`
- `id`
- `flow_id`
- `definition_version`
- `status`
- `started_at`
- `ended_at`
- `trigger_reason`
- `error`

`definition_version` is critical because active runs should stay tied to the published version they started with.

### 6.4 Node Runs

`flow_node_runs`
- `id`
- `flow_run_id`
- `flow_id`
- `node_key`
- `status`
- `started_at`
- `ended_at`
- `input_json`
- `output_json`
- `error`

This allows runtime inspection at the node level and supports timeline + lane views in the UI.

### 6.5 Outputs

Existing outputs such as recordings and clips remain domain entities, but they must be traceable to workflow execution.

`recordings`
- keep domain-specific recording fields
- add `flow_id`
- add `flow_run_id`

`clips`
- keep domain-specific clip fields
- add `flow_id`
- add `flow_run_id`
- keep caption output fields if V2 still stores caption results on clips

If a dedicated caption entity is added later, it must follow the same traceability pattern.

### 6.6 Transitional Compatibility Note

If the current implementation temporarily still stores or reads `accounts` during migration, that should be treated as transitional storage or migration input only.

The product and architecture model for V2 no longer treats `accounts` as a first-class editable entity.

---

## 7. Execution Model

### 7.1 Engine Location

The workflow engine runs in `Rust/Tauri`.

Responsibilities of the engine:
- load published node definitions
- create and advance `flow_runs`
- create and update `flow_node_runs`
- decide which node runs next
- apply node-specific logic and retry rules
- persist runtime state and outputs

### 7.2 Sidecar Role

The sidecar becomes an executor/integration service.

Responsibilities of the sidecar:
- watch or query external TikTok/live state when asked
- record streams when asked
- process recordings into clips when asked
- generate captions when asked
- emit execution results back to the app

The sidecar does not decide workflow progression.

### 7.3 Node Contract

Each node should be treated through a common contract:
- `config_json`
- `input_json`
- `status`
- `output_json`
- `error`

This allows the engine to treat nodes consistently while still letting each node type have specialized internals.

### 7.4 Node Progression

V2 uses a fixed progression model:

1. `Start`
- evaluate source config
- detect or poll livestream/source readiness
- open a new `flow_run` when trigger conditions are met

2. `Record`
- dispatch recording work to the sidecar
- persist node result
- move to `Clip` on success or configured retry/failure behavior

3. `Clip`
- dispatch clip generation work
- persist outputs
- move to `Caption`

4. `Caption`
- dispatch or run caption generation
- persist outputs
- complete the active V2 workflow path

5. `Upload`
- remains present as a future placeholder
- does not perform real execution in V2

### 7.5 Runtime Semantics

Each flow is independent.

This means:
- one flow's `Start` logic is not shared with another flow's `Start`
- one flow's `Record` rules do not leak into another flow's `Record`
- retries, timing, and node behavior are per-flow concerns

---

## 8. Draft And Publish Model

### 8.1 Two Config Layers

Each node definition stores:
- `draft_config_json`
- `published_config_json`

Editing behavior:
- node config changes auto-save into `draft`
- the workflow engine only reads `published`
- runtime behavior never reads `draft`

### 8.2 Publish Action

`Flow Detail` has a top-level `Publish` action.

When the user publishes:
- the current draft definition becomes the new published definition
- the flow version increments
- future runs use the new published definition

### 8.3 Active Run Behavior

If the flow is not currently running:
- the new published config applies to the next run immediately

If the flow is already running:
- the current run continues with the old published definition
- the new published definition applies only to the next run

### 8.4 Restart Prompt

If a user publishes while a flow is running, the UI should show a dialog with two actions:

1. `Keep current run`
- let the active run finish with the old published definition
- apply new config on the next run

2. `Stop and restart`
- stop the active run
- start again from `Start` using the new published definition

This preserves runtime consistency while still giving the user a fast recovery path.

---

## 9. UI Design

### 9.1 Flow List

`Flow List` remains the management screen for all flows.

Each card or row shows:
- flow name
- source/account identity from `Start`
- overall runtime status
- current node
- last run time
- last error
- output counts
- enabled state

Creating a flow should happen from `Flow List`, but the important next step is entering `Flow Detail` to configure `Start`.

### 9.2 Flow Detail As Workflow Canvas

`Flow Detail` becomes a workflow canvas scene inspired by tools like `n8n` or `Dify`, but with fixed node placement.

Canvas rules:
- scene-based layout, not a simple horizontal row
- nodes appear in a workflow space with connective lines
- no drag-and-drop in V2
- no user-edited edges in V2

This preserves the workflow mental model without taking on a full graph editor scope.

### 9.3 Node Interaction

Clicking a node opens a modal.

The modal contains:
- a polished form-based editor
- no raw JSON as the default editing experience
- a top action area with `Save`

Node forms should be domain-specific, for example:
- `Start`: source identity, cookies, proxy, polling, watcher rules
- `Record`: record duration, segmentation, retry, output settings
- `Clip`: clip thresholds and processing rules
- `Caption`: prompt/template/style settings
- `Upload`: placeholder state only in V2

### 9.4 Draft And Publish UI

`Flow Detail` should show:
- a top-level `Publish` button
- visual indication that draft differs from published
- draft/published status badges where useful

Node editing auto-saves draft changes, but users still need to publish to affect runtime behavior.

### 9.5 Runtime Monitor Under The Canvas

Below the canvas, `Flow Detail` should show runtime monitoring in two complementary views:

1. `Timeline`
- a chronological event stream for the active or recent run
- node start, success, failure, retry, output creation

2. `Node lanes`
- one lane per node
- show recent jobs, status, outputs, and errors for that node

This gives both the whole-flow view and the per-node operational view.

---

## 10. Start Node Responsibilities

`Start` now replaces the product role of `Accounts`.

It should contain both:

### 10.1 Source Config

- username/source identity
- cookies/session data
- proxy configuration
- live detection settings
- polling settings
- reconnect and retry behavior

### 10.2 Source Runtime

- current live status
- last checked time
- last live detected time
- watcher or polling health
- current retry/reconnect state

This makes `Start` both the source editor and the runtime monitor for the flow's input side.

---

## 11. Migration Strategy

The current app and current `Flows` implementation already introduced a flow workspace. This new architecture should supersede it with a migration path.

### 11.1 Product Migration

- remove `Accounts` from the sidebar
- move account editing responsibilities into `Start`

### 11.2 Data Migration

For existing account data:
- automatically create one flow per existing account
- migrate account configuration into the `Start` node's draft and published config
- mark those migrated flows clearly if needed during rollout

### 11.3 Runtime Migration

Current event-driven synchronization in frontend/app-shell should be replaced or reduced as the `Rust/Tauri` workflow engine takes over orchestration.

The goal is:
- UI observes workflow state
- engine owns workflow state
- sidecar executes requested tasks

---

## 12. Error Handling

### 12.1 Definition Errors

Invalid node configuration should fail at edit/publish time where possible.

Examples:
- invalid cookie payload shape
- invalid polling interval
- invalid clip min/max relationship

### 12.2 Runtime Errors

Each node run should persist:
- current status
- last error
- retry count or retry attempt if used

The flow should clearly communicate whether failure:
- blocks the workflow
- will retry
- can be resumed or restarted

### 12.3 Publish Safety

Published definitions should remain immutable for the life of an active run.

This is required to keep execution traceable and debuggable.

---

## 13. Verification Strategy

Implementation should verify four layers:

### 13.1 Definition Persistence

- draft edits auto-save correctly
- publish updates published version correctly
- published version remains stable during active runs

### 13.2 Workflow Engine Behavior

- engine creates flow runs correctly
- node transitions follow the fixed order correctly
- per-flow node logic stays isolated

### 13.3 Executor Integration

- sidecar commands are dispatched with correct node inputs
- executor results are mapped back into the correct flow and node run
- executor failure does not corrupt engine state

### 13.4 UI Behavior

- canvas presents the workflow clearly
- node modal editing is form-based and usable
- publish/restart dialog behavior matches runtime rules
- timeline and node lanes reflect actual execution state

---

## 14. Final Scope Summary

This redesign makes TikClip V2 a workflow application rather than a collection of disconnected operational pages.

The final model is:
- `Flow` is the independent unit of execution
- `Start` owns full source/account configuration
- `Accounts` disappears from the product surface
- each node has per-flow logic and configuration
- `Rust/Tauri` is the workflow engine
- the sidecar is a third-party executor
- the UI becomes a fixed-node workflow canvas with modal editing
- edits auto-save as draft and only affect runtime after publish
- active runs keep using the old published version unless explicitly restarted

This is the architecture and UX direction that best matches the requested product change.
