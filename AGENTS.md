# AGENTS.md

## 1. Overview

TikClip is a desktop workflow for watching TikTok Live accounts through flows, recording sessions, and managing clips, products, captions, and storage metadata. The repo combines a React/Tauri UI, a Rust desktop runtime with SQLite, and a Python sidecar for post-recording processing, captions, product scraping, embeddings, and cleanup.

## 2. Folder Structure

- `src`: React desktop UI, app state, and frontend-side transport glue.
  - `components`: feature UI plus shared primitives; `layout` owns the shell/sidebar/topbar, `flows` owns canvas, node modals, runtime strip, diagnostics, and flow-linked panels, and `ui` wraps base controls.
  - `pages`: top-level screens selected by `AppShell` for dashboard, flows, products, settings, and placeholders.
  - `stores`: Zustand mutation surfaces for accounts, clips, flows, products, notifications, recordings, and app shell state.
  - `lib`: Tauri `invoke` wrappers, sidecar REST/WebSocket helpers, DB sync adapters, runtime-to-account live derivation, notification sync, product media helpers, and error formatting.
  - `hooks`: shell bootstrap hooks, especially sidecar status polling and WebSocket connection helpers.
  - `types`: shared frontend domain types, request/result shapes, runtime snapshots, and runtime logs.
- `src-tauri`: Rust desktop shell, SQLite persistence layer, live runtime, and recording runtime.
  - `src/commands`: thin Tauri command modules for accounts, clips, flows, flow engine, live runtime, notifications, paths, products, recordings, settings, storage, and dashboard queries.
  - `src/db`: SQLite initialization, timestamp-aware schema migrations, and DB model structs.
  - `src/live_runtime`: flow-owned live polling sessions, account binding, username normalization, runtime log buffering, recording handoff, and Tauri event emission.
  - `src/recording_runtime`: ffmpeg command construction, recording process spawning/cancelation, output paths, and finish payloads.
  - `src/workflow`: fixed flow-node pipeline (`start -> record -> clip -> caption -> upload`), node config canonicalization, node runners, and `flow_runs` / `flow_node_runs` persistence helpers.
  - `src/tiktok`: TikTok live-status resolution, HTTP transport, cookie/proxy normalization, stream URL extraction, and payload parsing.
  - `src/sidecar`: Python sidecar lifecycle, `SIDECAR_PORT=...` discovery, restart/status commands, and process shutdown.
  - `src/lib.rs`, `src/main.rs`, `src/tray.rs`, `src/sidecar_env.rs`, `src/app_paths.rs`, `src/time_hcm.rs`: app bootstrap, shared state registration, tray setup, sidecar env construction, storage root resolution, and GMT+7 helpers.
  - `tauri.conf.json`, `capabilities`, `icons`, `gen`: desktop runtime capabilities, generated schemas, and packaging assets.
- `sidecar`: Python FastAPI service for processing completed recordings and product/caption assistance.
  - `src/routes`: thin endpoints for video processing/status, speech segments, captions, product scraping/embeddings/search, trim, storage cleanup, and health.
  - `src/core`: post-recording video/audio processing, caption generation, model preload/status, storage cleanup worker, and HCM date helpers.
  - `src/models`: Pydantic request/response schemas shared by routes.
  - `src/tiktok`: product scraping and TikTok HTTP helpers used by product import workflows.
  - `src/embeddings`: Gemini/zvec product media and text indexing, search, clip-to-product suggestion, frame extraction, and vector runtime setup.
  - `src/ws`: process-local WebSocket manager that broadcasts sidecar events to the desktop shell.
  - `src/config.py`, `src/logging_config.py`, `src/app.py`, `src/main.py`, `src/onnx_runtime_preload.py`: settings, logging, FastAPI assembly, port binding/startup, and ONNX runtime preload.
  - `tests`: pytest coverage for clip scheduling, processing, and health behavior.
- `docs/superpowers`: product specs and phased implementation plans; align behavior changes with these docs when relevant.
- `public`: static assets used by the Vite frontend.
- `DESIGN.md`: canonical design-system reference for frontend styling and interaction work.

## 3. Core Behaviors & Patterns

- **Durability Boundary**: SQLite-backed state goes through Tauri `invoke(...)`; processing/caption/product work goes through sidecar HTTP and `/ws`. Durable sidecar events are normalized in `src/lib/sidecar-db-sync.ts` before SQLite writes, then stores refresh by revision or runtime sync.
- **Runtime Ownership**: Rust owns live polling, flow execution state, account binding, and ffmpeg recording. `LiveRuntimeManager` keeps in-memory sessions/log buffers, persists flow/run transitions through `workflow::runtime_store`, emits `flow-runtime-updated` / `flow-runtime-log`, and hands completed recordings to sidecar processing.
- **Shell Orchestration**: `src/components/layout/app-shell.tsx` is the frontend hub. It polls sidecar availability through `useSidecar`, registers Tauri runtime-event listeners, wires sidecar WebSocket handlers, hydrates stores from SQLite/runtime snapshots, and derives account live flags from active flow runtime state.
- **Flow Lifecycle**: Flows use a fixed node sequence (`start`, `record`, `clip`, `caption`, `upload`). Draft node JSON is saved separately from published JSON; publishing canonicalizes start/record configs, bumps versions, and can restart active runs so runtime work reads only published definitions.
- **Store Concurrency**: Zustand stores expose imperative async actions and keep stale responses from winning with tokens, generation counters, revision bumps, and capped runtime log buckets. UI components should call store actions rather than fetch or mutate shared state directly.
- **Boundary Normalization**: Frontend and Rust boundary code use guard clauses and local coercion helpers for usernames, ids, optional strings, empty-string-as-null fields, JSON config, sidecar ports, malformed WebSocket messages, and partial sidecar payloads.
- **Error Handling**: Tauri commands flatten boundary errors to `Result<_, String>` after input validation and explicit SQL. Sidecar routes raise `HTTPException` for client errors, return typed Pydantic responses for expected incomplete outcomes, and log unexpected processing failures.
- **Resilience & Recovery**: WebSocket clients auto-reconnect and ignore malformed messages; Rust cancels superseded poll tasks by generation; sidecar startup falls back across ports; vector-store corruption is detected and recreated; DB upserts preserve existing values when incoming payloads are partial.
- **Shared Resource Management**: SQLite is shared through `AppState.db: Mutex<Connection>`, Rust runtime through `LiveRuntimeManager`, sidecar process state through `SidecarManager`, product vectors through cached zvec collections, and sidecar WebSockets through `ws_manager`.

## 4. Conventions

- Use `@/` imports for frontend internals; keep cross-process transport in `src/lib` and store mutations in `src/stores` instead of embedding fetch or `invoke` logic deep in components.
- `DESIGN.md` is the source of truth for frontend styling, layout, and interaction changes unless the user explicitly asks to deviate.
- Naming stays explicit by layer: React components/types use PascalCase, hooks use `useX`, component files are usually kebab-case, page/store/type modules stay lowercase, Python modules and Rust command ids use snake_case.
- Boundary names must line up across layers: Rust `#[tauri::command]` names match frontend `invoke("...")` strings exactly, sidecar route paths match `src/lib/api.ts`, and WebSocket event names stay stable (`clip_ready`, `caption_ready`, `speech_segment_ready`, `storage_warning`, `cleanup_completed`).
- Keep TypeScript normalization explicit with small helpers such as `normalizeUsername`, `parseClipId`, `deriveAccountLiveFlagsFromRuntime`, coercion utilities, and parse functions instead of dense inline casts.
- Flow node keys and statuses are closed sets. Update frontend `FlowNodeKey` / `FlowStatus`, Rust `FLOW_NODE_KEYS` / status validators, workflow node runners, canvas/runtime UI, and migrations together when changing them.
- Rust commands stay thin: validate inputs, lock the shared SQLite connection, use direct SQL or focused runtime-manager calls, and return `map_err(|e| e.to_string())` at the desktop boundary. Longer state machines belong in `live_runtime`, `recording_runtime`, or `workflow`.
- Python routes stay thin: validate with Pydantic models, trim user input locally, delegate heavy work to `core/*`, `embeddings/*`, or `tiktok/*`, and broadcast runtime-visible side effects through `ws_manager`.
- Config and settings use explicit boundary conversion: Tauri settings become `TIKCLIP_*` env pairs for the sidecar, empty frontend strings become omitted/null values where Rust expects option-like updates, and username/cookie/proxy normalization stays near the consuming boundary.
- Comments are short and rare; add them only for desktop/sidecar coordination, race prevention, fallback behavior, storage/vector recovery, or TikTok-specific quirks that are not obvious from code.
- Prefer explicit `null`/`Option`/error handling around sidecar payloads, cookies JSON, storage paths, recording ids, flow run ids, model/vector availability, and live-status synchronization paths.

## 5. Working Agreements

- Respond in Vietnamese unless the user asks for another language; keep technical terms in English and never translate code blocks.
- Before editing, review related usages and the full frontend/Rust/sidecar flow for the feature you are changing.
- Treat `DESIGN.md` as the canonical design system document for the product and check it before making UI/UX changes.
- Prefer simple, maintainable, production-friendly changes; avoid overengineering, clever abstractions, and extra layers for small features.
- Keep APIs small, behavior explicit, naming clear, and new code colocated with the nearest existing feature/module.
- Preserve current public behavior unless the user asks to change it; call out any unavoidable behavior change.
- Do not add tests, lint tasks, formatting churn, or new dependencies unless the user explicitly asks for them or the change cannot be done safely without them.
- Ask for clarification instead of guessing when a requirement is ambiguous or when a change would affect multiple layers in conflicting ways.
- After code changes, run the verification for the layers you touched: `npm run lint:js` from the repo root for frontend work, `uv run ruff check src tests`, `uv run ruff format --check src tests`, `uv run ty check .`, and `uv run pytest tests/ -q` from `sidecar/` for Python work, and `cargo fmt --check` plus `cargo clippy --all-targets -- -D warnings` from `src-tauri/` for Rust/Tauri work.

## 6. Execution Discipline

- Think before coding: state assumptions explicitly, surface tradeoffs, and do not silently choose between multiple reasonable interpretations.
- Push back when needed: if a requirement is unclear or conflicting, stop and ask instead of implementing a guess.
- Default to the simplest change that solves the request; avoid speculative flexibility, single-use abstractions, or defensive code for scenarios that cannot happen in this app flow.
- Keep changes surgical: touch only files and lines needed for the request, match the surrounding style, and avoid cleanup or refactors outside the task.
- Clean up only what your change makes stale, such as imports, variables, helpers, or branches that become unused because of your own edit.
- If you notice unrelated dead code or design issues, mention them separately instead of deleting or rewriting them without approval.
- For non-trivial work, define a short success path before editing: what will change, what should stay unchanged, and how the result will be verified.
- Each meaningful code change should trace directly back to the user request; if a diff cannot be justified in one sentence, it probably should not be there.
