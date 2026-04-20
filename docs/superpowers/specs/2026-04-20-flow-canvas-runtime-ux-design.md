# Flow Canvas Runtime UX Design

**Date:** 2026-04-20
**Status:** Draft for review

---

## 1. Overview

The current Flow detail screen makes runtime monitoring harder than it should be.

Today the user has to look at three separate areas to understand what is happening:

- the canvas for the workflow structure
- the runtime cards below the canvas
- the logs panel at the bottom

This splits attention vertically, reduces the canvas footprint, and makes the most important runtime answer hard to see: **which node is running right now**.

This change redesigns the Flow detail screen around a canvas-first monitoring experience:

- the canvas becomes the primary runtime surface
- runtime state is shown directly on nodes
- the currently running node gets a red pulsing border
- the heavy bottom runtime panels are removed from the default view
- diagnostics move behind an explicit on-demand entry point

The goal is to make the screen feel like a live operations surface rather than an editor with debug panels attached underneath.

---

## 2. Problems In The Current Screen

The current UI has four concrete UX problems:

### 2.1 Runtime State Is Not Immediate

The canvas nodes do not communicate active execution strongly enough. The user cannot glance at the canvas and instantly identify the active node.

### 2.2 The Bottom Panels Compete With The Canvas

`FlowRuntimeTimeline`, `FlowRuntimeLanes`, and `RuntimeLogsPanel` take too much vertical space. This compresses the canvas and makes the primary screen area feel secondary.

### 2.3 Logs Are Always Present But Rarely Actionable

The logs panel is large and persistent, but most of the time the user does not need raw diagnostics. It becomes layout weight instead of helping the primary workflow.

### 2.4 Editing And Monitoring Are Not Balanced Well

The screen currently behaves like an editor first and a runtime monitor second. For this product, the runtime state needs to be visible at the same level as editing actions.

---

## 3. Goals And Non-Goals

### Goals

- Make the active node visible directly on the canvas
- Use a strong visual treatment for the running node, including a red pulsing border
- Reduce or eliminate large bottom runtime panels from the default layout
- Preserve access to diagnostics without keeping them permanently open
- Give more screen area to the canvas so it can surface more useful node information
- Keep the implementation inside the existing Flow detail feature surface without changing runtime backend semantics

### Non-Goals

- Redesigning the whole Flow editor architecture
- Changing Rust runtime execution semantics or log generation formats
- Adding new workflow nodes or changing flow behavior
- Building a new page or separate monitor mode in this pass
- Reworking publish/edit dialogs beyond what the new layout requires

---

## 4. Approaches Considered

Three directions were considered:

1. Keep the current layout but improve styling on the existing bottom panels
2. Move runtime information to a right sidebar while keeping the canvas central
3. Make the canvas the runtime surface and move diagnostics behind an explicit modal dialog

### Recommendation

Approach 3 is the chosen direction.

Why:

- it answers the main user complaint directly: the canvas should show what is running
- it removes the need to scan separate sections vertically
- it gives back the largest amount of visual space to the workflow itself
- it keeps diagnostics available without letting them dominate the layout

Approach 1 is too incremental and would preserve the current structural problem. Approach 2 is viable, but it still keeps runtime state outside the canvas instead of embedding it where the user is already looking.

---

## 5. Chosen UX Direction

The Flow detail screen becomes a **canvas-first runtime monitor**.

The user should be able to open a flow and answer these questions immediately without scrolling:

- Is the flow idle, watching, recording, processing, disabled, or in error?
- Which node is active right now?
- Which nodes already completed in the current run?
- Is there an error I should care about right now?

The new visual hierarchy becomes:

1. Flow header and controls
2. Large canvas with runtime-aware nodes
3. Slim runtime status strip under or inside the canvas edge
4. Optional diagnostics dialog when the user explicitly asks for details

---

## 6. Layout Design

### 6.1 Header

The existing header remains, but should stay visually compact.

It keeps:

- flow name
- published and draft version badges
- unpublished draft badge when needed
- `Back`
- `Publish`

The header should not grow vertically. It is not the focus of this redesign.

### 6.2 Canvas As The Main Surface

`FlowCanvas` becomes taller and visually more central than it is now.

The canvas should:

- occupy more vertical space than the current implementation
- feel like the primary panel on the page
- avoid being visually crowded by permanent runtime cards below it

The canvas is no longer only a structural map. It becomes the main monitoring interface for the flow.

### 6.3 Runtime Strip

The bulky runtime cards below the canvas are replaced with a compact runtime strip.

This strip should show only high-value summary data:

- current flow status
- current node
- username
- active run id
- last error only when present
- a `Diagnostics` action

The strip must remain slim and secondary to the canvas. It should support quick confirmation, not become a second dashboard.

Layout rules for the strip:

- the default visual priority is `status`, then `current node`, then `last error`, then secondary metadata
- `last error` replaces lower-priority metadata on narrow widths instead of forcing the strip to grow indefinitely
- `username` and `active run id` must use truncation and `min-w-0` behavior inside flexible containers
- `last error` must be line-clamped to one line in the default strip
- the strip may wrap to two rows on narrower widths, but must not expand into a multi-card dashboard
- the `Diagnostics` trigger stays visible in all supported widths

### 6.4 Diagnostics Dialog

Detailed runtime logs move out of the default layout and into an explicit diagnostics surface.

Recommended behavior:

- closed by default
- opened from the runtime strip via `Diagnostics`
- rendered with the existing Radix-based dialog primitive already used in the repo
- contains the existing detailed runtime log and correlation information

This preserves debugging power without forcing every user to pay the layout cost all the time.

Accessibility and interaction requirements for diagnostics:

- the trigger must be a real button
- focus is trapped while diagnostics is open
- `Escape` closes the diagnostics surface
- closing returns focus to the trigger button
- the surface must expose an accessible title relationship via the dialog title
- internal log content must scroll inside the diagnostics surface instead of growing the full page height

---

## 7. Node Visual States

The canvas node becomes the primary runtime status indicator.

Each node must support both **selection state** and **runtime state** at the same time. Selection should not hide or override runtime meaning.

### 7.1 Running Node

The currently active node gets the strongest visual treatment:

- red pulsing border
- pulsing outer ring rather than a static danger border
- a small `Running` badge or equivalent status chip
- a dedicated non-error icon or marker for active execution
- runtime copy that describes what is happening in plain language, such as `Recording live` or `Creating clips`

This is the most important new signal in the redesign.

To keep `Running` distinct from `Error`, the running treatment must be differentiated by more than motion alone:

- `Running` uses the pulse animation and explicit `Running` label
- `Error` uses a static danger state and explicit `Error` label
- when reduced motion is enabled, `Running` still remains visually distinct through the `Running` badge and active marker, even without pulsing

### 7.2 Completed Node

Nodes already completed in the current run should display a subtle success state:

- soft green or success-tinted border/accent
- a `Done` or equivalent status chip
- no pulsing animation

This should read as completed progress, not as the primary active focus.

### 7.3 Error Node

If the current runtime state points to a failed node or flow-level failure associated with a node, the node should show:

- static red danger border
- `Error` indicator
- short inline error summary when available

The error treatment should be clearly different from the running pulse state.

If a flow-level error cannot be mapped to a specific node, the runtime strip carries the primary error message and the canvas should avoid inventing a false node-level error assignment.

### 7.4 Waiting / Upcoming Node

Nodes not yet reached in the active run should remain visually quieter than active or completed nodes.

They should still show configuration summaries, but with reduced emphasis.

If a flow has never run, or the runtime snapshot has not loaded yet, nodes should stay in this neutral non-runtime state rather than claiming `Done` or `Error`.

### 7.5 Selected Node

Node selection remains important for editing, but it becomes a secondary layer over runtime state.

Design rule:

- selected + running should still look running first
- selected + error should still look error first
- selected-only should keep the existing accent-oriented highlight

---

## 8. Information Inside Each Node

The current node card shows label, draft/live tag, runtime label, and config summary. The redesign keeps that pattern but makes the hierarchy more useful.

Each node should prioritize:

1. node label
2. runtime state line
3. concise config or output summary

Guidelines:

- runtime line should be short and legible at a glance
- config summary should stay compact and avoid log-like detail
- avoid dense debug text inside nodes
- do not expand nodes into mini dashboards

Content guardrails:

- the runtime line is limited to one line
- the config or output summary is limited to two lines
- inline error summary, when shown, is limited to one line in the node surface
- node height should remain fixed across runtime states; overflow must truncate rather than resize the canvas layout

The node remains readable and compact, but substantially more informative than it is now.

---

## 9. Runtime Data Mapping

The runtime-aware canvas should not rely solely on `activeFlow.flow.current_node` from the editor payload.

Instead, the UI should use the same runtime information already flowing through the store:

- `activeFlow` for editor/config structure
- `runtimeSnapshots[flowId]` for current flow-level runtime state
- `activeFlow.nodeRuns` and the currently active run inside `activeFlow.runs` for node-level completion state within the current run
- `runtimeLogs[flowId]` only when diagnostics are opened

### Key Decision

The canvas should render node runtime state by combining:

- `runtimeSnapshots[flowId]` for the live current node and flow-level status
- `activeFlow.nodeRuns` scoped to the current active or most recent relevant run for node-level state already persisted by the backend
- `activeFlow.flow` only as the structural/editor baseline

This avoids relying on snapshot data for execution history it does not currently contain, while still preventing the editor payload from acting as the only runtime source of truth.

Runtime mapping rules:

- if `runtimeSnapshot.current_node` is present, that node is the active node
- if `runtimeSnapshot.status` indicates an active runtime state but `current_node` is null, the runtime strip shows the flow-level status and the canvas keeps nodes in their last known non-active state rather than guessing an active node
- node-level derived state uses only rows from the selected run context for that flow
- within a single run and node key, the latest `flow_node_runs` row wins
- `Done` is shown only when the latest row for that node in that run has status `completed`
- `Error` is shown only when the latest row for that node in that run has status `failed`
- if no reliable run-scoped node completion data is available, the UI must omit `Done` state rather than fabricating one
- flow-level orchestration errors without a reliable node association stay at flow-strip level and diagnostics level, not on an arbitrary node

This `latest row wins` rule is the canonical aggregation rule for repeated `clip` or `caption` node rows within the same run.

---

## 10. What Changes In `FlowDetail`

The existing layout in `FlowDetail` is:

- header
- canvas
- timeline + lanes grid
- logs panel

The new layout becomes:

- header
- enlarged runtime-aware canvas
- compact runtime strip
- diagnostics dialog entry point

Default removals from the always-visible layout:

- `FlowRuntimeTimeline`
- `FlowRuntimeLanes`
- large always-open `RuntimeLogsPanel`

These components can only appear inside the on-demand diagnostics surface. They should not remain as fixed-height blocks under the canvas in the default screen.

---

## 11. Diagnostics Scope

Diagnostics should become intentionally secondary.

The user should open diagnostics when they need to answer deep questions like:

- why a handoff failed
- which runtime event happened most recently
- what error code was emitted

The user should **not** need diagnostics to answer:

- which node is active
- whether the flow is running
- whether record already completed

That information must be visible on the canvas itself.

---

## 12. Accessibility And Motion

The red pulse on the running node should be noticeable but controlled.

Guidelines:

- animation should be soft and rhythmic, not aggressive
- border/glow pulse should not create layout shift
- the running state must still be understandable if animation is reduced or disabled
- color must not be the only signal; a text badge such as `Running` and an active marker must remain present

Implementation requirement:

- `prefers-reduced-motion` disables the pulse animation
- disabling the animation must not collapse the distinction between `Running` and `Error`

This keeps the state legible without relying entirely on motion.

---

## 13. Responsive Behavior

The redesign should preserve the same core hierarchy on smaller widths.

### Desktop

- canvas remains dominant
- runtime strip stays compact
- diagnostics opens as a modal dialog overlay

### Narrower Layouts

- canvas can scroll horizontally as it already does
- runtime strip can wrap into two rows on narrower widths
- diagnostics should still stay hidden by default
- the canvas must keep a usable minimum vertical footprint instead of collapsing behind runtime chrome
- diagnostics must keep its own internal scroll region when viewport height is limited
- when vertical space is tight, canvas visibility is prioritized over showing more diagnostics content at once

The responsive strategy should not reintroduce the old permanent bottom stack.

---

## 14. Implementation Boundaries

This redesign should remain focused on the Flow detail presentation layer.

Expected files and surfaces likely involved:

- `src/components/flows/flow-detail.tsx`
- `src/components/flows/canvas/flow-canvas.tsx`
- `src/components/flows/canvas/flow-canvas-node.tsx`
- runtime panel components that are currently rendered by default

Out of scope unless required by a concrete rendering gap:

- Rust runtime behavior changes
- new runtime event schemas
- major store refactors unrelated to node-state overlay

If the implementation discovers a small missing mapping helper is needed, that is acceptable. Broad store or backend refactors are not part of this design.

---

## 15. Verification

Manual verification should confirm:

- opening a flow shows the canvas as the dominant surface
- the active node is immediately identifiable without reading bottom panels
- the running node has a red pulsing border and a persistent non-motion state label
- completed and error nodes are distinguishable from running nodes
- the default view no longer includes a large diagnostics/log area below the canvas
- diagnostics remain accessible on demand
- selecting a node for editing still works and does not erase runtime state meaning

Code verification for the touched layer should include:

- `npm run lint:js`

---

## 16. Success Criteria

The redesign is successful when a user can open the Flow screen and answer, within one glance:

- what the flow is doing now
- which node is active now
- whether the flow is still recording or already in a later processing stage, and which node is currently active when the runtime data provides that mapping
- whether there is a visible issue worth opening diagnostics for

The canvas should feel like the operational center of the feature, and the diagnostics should feel optional rather than structurally mandatory.
