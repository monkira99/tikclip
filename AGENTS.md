# AGENTS.md

## 1. Overview

TikClip is a desktop workflow for monitoring TikTok Live accounts, running fixed recording flows, and managing recordings, clips, captions, products, storage, and product-suggestion metadata. The app is a React/Tauri desktop surface backed by a Rust runtime and local SQLite persistence.

## 2. Folder Structure

- `src`: React desktop UI, feature components, frontend state, and Tauri command wrappers.
  - `components`: UI by feature area plus shared primitives.
    - `layout`: `AppShell`, top bar, sidebar, notification menu, and runtime/event bootstrap hooks.
    - `flows`: flow list/detail screens, canvas rendering, node modals, runtime strip, publish dialog, and flow-specific tests.
    - `dashboard`, `products`, `recordings`, `accounts`: page-level feature widgets and forms.
    - `ui`: shared shadcn-style primitives such as buttons, cards, dialogs, inputs, tabs, badges, and switches.
  - `features/settings`: settings page config and path-row controls for storage/runtime options.
  - `pages`: top-level page components selected by the app shell.
  - `stores`: Zustand stores for accounts, app shell state, clips, flows, notifications, products, and recordings.
  - `lib`: frontend boundary helpers, including `@/lib/api/*` Tauri `invoke` wrappers, flow-node form parsing, formatting, notification sync, product-image helpers, settings value parsing, and runtime notification mapping.
  - `types`: shared TypeScript domain types, request/result shapes, closed status unions, runtime snapshots, and runtime logs.
- `src-tauri`: Rust desktop shell, SQLite persistence layer, live runtime, and recording runtime.
  - `src/commands`: thin Tauri command modules for accounts, clips, dashboard, flows, flow engine, live runtime, notifications, paths, products, recordings, settings, and storage.
  - `src/db`: SQLite initialization, ordered schema migrations, migration tests, and DB model structs.
  - `src/live_runtime`: flow-owned polling sessions, account binding, generation-based cancellation, runtime log buffering, recording coordination, snapshots, and Tauri event emission.
  - `src/recording_runtime`: ffmpeg input/output construction, recording process spawn/cancel handling, output paths, and finish payloads.
  - `src/workflow`: fixed-node pipeline (`start -> record -> clip -> caption -> upload`), node config parsing/canonicalization, node runners, runtime persistence helpers, clip extraction, caption generation, product suggestions, and product embedding vectors.
  - `src/tiktok`: TikTok live-status and product fetching, HTTP transport, cookie/proxy normalization, stream URL extraction, and payload parsing.
  - `src/lib.rs`, `src/main.rs`, `src/tray.rs`, `src/app_paths.rs`, `src/time_hcm.rs`: app bootstrap, shared state registration, tray setup, storage root resolution, shutdown wiring, and GMT+7 timestamp helpers.
  - `tauri.conf.json`, `capabilities`, `icons`, `gen`: desktop runtime capabilities, generated schemas, and packaging assets.
- `docs`: canonical Vietnamese project documentation for product intent, architecture, contracts, feature handbooks, operations, and ADRs. Start every non-trivial task at `docs/README.md`, then read the relevant docs before planning.
  - `product`: product overview, terminology, and user workflows.
  - `architecture`: layer ownership, data flow, Rust runtime, storage/time, frontend structure, and Tauri boundary.
  - `contracts`: stable cross-layer contracts for Tauri commands, runtime events, flow pipeline, DB schema, settings, and media paths.
  - `features`: feature handbooks for accounts, flows, recordings, clips, captions, products, notifications, and storage cleanup.
  - `operations`: local development, verification, troubleshooting, and build/release guidance.
  - `decisions`: ADRs for technical decisions such as Rust-owned runtime and fixed flow pipeline.
- `public`: static assets used by the Vite frontend.
- `DESIGN.md`: canonical design-system reference for frontend styling and interaction work.

## 3. Core Behaviors & Patterns

- **Rust-Owned Runtime Boundary**: Durable data and processing work cross the frontend/backend boundary through Tauri `invoke(...)` wrappers in `src/lib/api/*`. Rust owns SQLite writes, TikTok requests, ffmpeg recording, clip extraction, caption generation, product fetching, product vectors, storage cleanup, and runtime event emission.
- **Shell Event Orchestration**: `src/components/layout/app-shell.tsx` stays thin and delegates bootstrap to `app-shell-effects.ts`. Effects hydrate notifications, sync active recording counts, listen for `flow-runtime-updated`, `flow-runtime-log`, `rust-clip-ready`, `cleanup_completed`, and `storage_warning`, then bump the relevant Zustand revisions or runtime buckets.
- **Flow Lifecycle**: Flows follow the closed pipeline `start -> record -> clip -> caption -> upload`. Draft node JSON is edited separately from published JSON; publishing validates and canonicalizes start/record/clip configs, bumps flow versions, reconciles enabled runtime sessions, and rolls back published state if runtime reconciliation fails.
- **Runtime Ownership**: `LiveRuntimeManager` holds in-memory sessions, leases by normalized username, active poll tasks, active recordings, failed snapshots, and capped runtime logs. It persists flow/run/node-run transitions through workflow/runtime helpers, emits Tauri snapshots/logs, and coordinates recording completion into downstream workflow nodes.
- **Concurrency Control**: Frontend stores use request tokens, per-flow log tokens, optimistic snapshots, revision bumps, and capped runtime log buckets to keep stale async responses from resurrecting deleted or outdated state. Rust polling uses generation tokens and cancellation flags to suppress superseded poll iterations and recording work.
- **Boundary Normalization**: TypeScript parses and serializes flow-node forms in `src/lib/flow-node-forms.ts`; Rust revalidates and canonicalizes node config at publish/runtime boundaries. Usernames, IDs, optional strings, settings values, storage paths, cookies, proxy values, timestamps, and product media paths are normalized near the boundary that consumes them.
- **Error Handling**: Tauri commands validate inputs, lock shared SQLite state, delegate longer work to focused runtime/workflow modules, and flatten errors to `Result<_, String>`. Rust workflow code logs operational context, records flow/node failures in SQLite, and emits runtime logs so the UI can show failures without duplicating backend state machines.
- **Storage And Time Semantics**: App data is rooted through `app_paths`, persisted in `storage_root/data/app.db`, and timestamped with GMT+7 helpers from `time_hcm.rs`. Storage cleanup runs both as a command and a background worker, computes quota usage from SQLite plus file paths, emits desktop events, and stores notifications.
- **Shared Resource Management**: SQLite is shared through `AppState.db: Mutex<Connection>`, live state through `LiveRuntimeManager`, cleanup scheduling through `StorageCleanupWorker`, media paths through storage-root helpers, product-vector settings through `app_settings`, and frontend durable mutations through the API wrappers rather than direct component calls.

## 4. Conventions

- Use `@/` imports for frontend internals; keep cross-process transport in `src/lib/api/*` and shared parsing/formatting helpers in `src/lib` instead of embedding `invoke` calls or coercion logic deep in components.
- `docs/README.md` is the entrypoint for canonical project docs. Do not duplicate detailed contracts in README or AGENTS; link or summarize and update the owning doc instead.
- `DESIGN.md` is the source of truth for frontend styling, layout, and interaction changes unless the user explicitly asks to deviate.
- Naming stays explicit by layer: React components/types use PascalCase, hooks use `useX`, component files are usually kebab-case, page/store/type modules stay lowercase, Rust modules/functions/command ids use snake_case, and serialized structs use `serde(rename_all = "snake_case")` or `camelCase` only where the existing boundary expects it.
- Boundary names must line up across layers: Rust `#[tauri::command]` names match frontend `invoke("...")` strings exactly, event names stay stable (`flow-runtime-updated`, `flow-runtime-log`, `rust-clip-ready`, `cleanup_completed`, `storage_warning`), and TypeScript result types mirror Rust response structs.
- Flow node keys and statuses are closed sets. Update frontend `FlowNodeKey` / `FlowStatus`, Rust workflow constants and node runners, flow canvas/runtime UI, command validation, DB migrations, and tests together when changing them.
- Rust commands stay thin: validate input, lock `AppState.db` only for DB work, use direct SQL or focused runtime/workflow helpers, and return `map_err(|e| e.to_string())` at the desktop boundary. Long-running or stateful logic belongs in `live_runtime`, `recording_runtime`, `workflow`, `tiktok`, or storage helpers.
- SQLite migrations are append-only and ordered in `src-tauri/src/db/init.rs`; schema changes should preserve older data, update models/types, and include migration coverage when behavior is non-trivial.
- Settings and config use explicit string boundaries: frontend settings convert form state to string values, Rust reads `app_settings` through typed helpers, empty strings become `None` where appropriate, and defaults stay close to the Rust consumer.
- Storage/media code must resolve relative paths against the configured storage root and reject media paths outside that root before reading, embedding, extracting frames, or deleting files.
- Comments are short and rare; add them only for desktop/runtime coordination, race prevention, migration intent, fallback behavior, storage/vector safety, or TikTok-specific quirks that are not obvious from code.
- Prefer explicit `null`/`Option`/error handling around cookies JSON, proxy URLs, storage paths, external recording ids, flow run ids, runtime snapshots, caption/vector availability, and live-status synchronization paths.

## 5. Working Agreements

- Respond in Vietnamese unless the user asks for another language; keep technical terms in English and never translate code blocks.
- Work in this order for non-trivial tasks: read docs, understand the relevant contracts/flows, combine that context with the user request, state a short plan, write the failing test first, implement the plan, verify code and logic, then update docs if behavior/contracts changed.
- Before editing, review related usages and the full frontend/Rust/runtime flow for the feature you are changing.
- Use test-driven development for features, bug fixes, refactors, and behavior changes: write one focused failing test, run it and confirm the expected failure, write the smallest implementation to pass, then refactor only while tests stay green.
- Treat `docs/README.md` as the entrypoint for canonical project docs. Update `docs/contracts/*` when changing Tauri commands, runtime events, flow nodes/statuses, DB schema, settings keys, or media path rules.
- Update `docs/product/*` or `docs/features/*` when user-visible behavior changes, `docs/architecture/*` plus an ADR when ownership/architecture changes, and `docs/operations/*` when verification, local development, troubleshooting, or release workflow changes.
- Treat `DESIGN.md` as the canonical design system document for the product and check it before making UI/UX changes.
- Prefer simple, maintainable, production-friendly changes; avoid overengineering, clever abstractions, and extra layers for small features.
- Keep APIs small, behavior explicit, naming clear, and new code colocated with the nearest existing feature/module.
- Preserve current public behavior unless the user asks to change it; call out any unavoidable behavior change.
- Do not add broad test suites, unrelated lint tasks, formatting churn, or new dependencies unless the user explicitly asks for them or the change cannot be done safely without them. Focused tests required by TDD are part of code changes, not optional extras.
- Ask for clarification instead of guessing when a requirement is ambiguous or when a change would affect multiple layers in conflicting ways.
- After code changes, run the narrow verification from `docs/operations/verification.md`: `npm run lint:js` for frontend work, `npm run lint:rust` for Rust/Tauri work, and both when a boundary change touches both layers.

## 6. Execution Discipline

- Start by reading `docs/README.md`, then the relevant `docs/product`, `docs/architecture`, `docs/contracts`, `docs/features`, `docs/operations`, or `docs/decisions` files for the request. Summarize the applicable constraints before planning when the task is non-trivial.
- Think before coding: state assumptions explicitly, surface tradeoffs, and do not silently choose between multiple reasonable interpretations.
- Push back when needed: if a requirement is unclear or conflicting, stop and ask instead of implementing a guess.
- For code changes, follow TDD strictly: no production code for a feature, bugfix, refactor, or behavior change before a failing test has been written and observed. If production code was written first, discard it and restart from the test.
- Keep each TDD loop small: RED with one behavior-focused test using real code where practical, verify the expected failure, GREEN with minimal implementation, verify all relevant tests pass, then refactor without changing behavior.
- Plan after reading docs and before editing. The plan should name what changes, what stays unchanged, which tests prove the behavior, which verification commands will run, and which docs must be updated.
- Default to the simplest change that solves the request; avoid speculative flexibility, single-use abstractions, or defensive code for scenarios that cannot happen in this app flow.
- Keep changes surgical: touch only files and lines needed for the request, match the surrounding style, and avoid cleanup or refactors outside the task.
- Clean up only what your change makes stale, such as imports, variables, helpers, or branches that become unused because of your own edit.
- If you notice unrelated dead code or design issues, mention them separately instead of deleting or rewriting them without approval.
- Verify code and logic before finishing: run the focused test that drove the change, then the layer-level command from `docs/operations/verification.md`. For docs-only changes, inspect rendered Markdown structure and check paths/links with repository search.
- Update docs after implementation when contracts, user-visible behavior, architecture, operations, settings, DB schema, media path rules, runtime events, or Tauri commands changed. Do not create date-stamped feature docs; only ADRs are numbered.
- Each meaningful code change should trace directly back to the user request; if a diff cannot be justified in one sentence, it probably should not be there.
