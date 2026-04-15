# AGENTS.md

## 1. Overview

TikClip is a desktop workflow for monitoring TikTok Live accounts, recording sessions, and managing clips, products, captions, and storage metadata. The repo combines a React/Tauri UI, a Rust desktop shell with SQLite, and a Python sidecar that owns live polling, recording, and media-processing runtime.

## 2. Folder Structure

- `src`: React desktop UI, app state, and frontend-side transport glue.
  - `components`: feature UI plus shared primitives; `layout` owns the shell/sidebar/topbar, feature folders (`accounts`, `clips`, `dashboard`, `flows`, `products`, `recordings`) keep page-specific widgets together, and `ui` wraps reusable base controls.
  - `pages`: top-level screens rendered by `AppShell`.
  - `stores`: Zustand stores for accounts, clips, flows, products, notifications, recordings, and shell state.
  - `lib`: Tauri `invoke` wrappers, sidecar REST/WebSocket helpers, DB sync adapters, notification helpers, and transport normalization utilities.
  - `hooks`: shell bootstrap hooks such as sidecar status polling.
  - `types`: shared frontend types and request/result shapes.
- `src-tauri`: Rust desktop shell and local persistence layer.
  - `src/commands`: Tauri command modules for accounts, clips, flows, notifications, paths, products, recordings, settings, storage, and dashboard queries.
  - `src/db`: SQLite initialization, schema migrations, and model structs.
  - `src/sidecar`: sidecar lifecycle management and status reporting.
  - `src/lib.rs`, `src/main.rs`, `src/tray.rs`, `src/sidecar_env.rs`, `src/app_paths.rs`: app bootstrap, shared state registration, tray setup, sidecar environment, and storage-path resolution.
  - `tauri.conf.json`, `capabilities`, `icons`: desktop runtime capabilities and packaging assets.
- `sidecar`: Python service for polling, recording, clipping, captioning, and product assistance.
  - `src/routes`: thin FastAPI endpoints for accounts, recordings, clips, products, storage, trim, and health.
  - `src/core`: long-running watcher, recorder, worker, processor, captioner, cleanup, and model-management services.
  - `src/models`: Pydantic schemas shared across routes and services.
  - `src/tiktok`: TikTok HTTP, cookies, and stream-resolution integration.
  - `src/embeddings`: vector-search bootstrap and embedding runtime helpers.
  - `src/ws`: WebSocket connection manager and broadcast helpers.
  - `src/config.py`, `src/logging_config.py`, `src/app.py`, `src/main.py`: settings, logging, app assembly, and process entrypoint.
  - `tests`: pytest coverage for watcher, worker, processor, clips scheduling, live overview, health, and TikTok API behavior.
- `docs/superpowers`: product specs and phased implementation plans; align behavior changes with these docs when relevant.
- `public`: static web assets used by the Vite frontend.
- `DESIGN.md`: canonical design-system reference for frontend styling and interaction work.

## 3. Core Behaviors & Patterns

- Frontend read/write boundaries are split by durability: SQLite-backed data goes through Tauri `invoke(...)`, while live runtime state goes through sidecar HTTP and `/ws`; sidecar events that matter long-term are normalized in `src/lib/sidecar-db-sync.ts` and persisted back into SQLite before stores refresh.
- `src/components/layout/app-shell.tsx` is the orchestration hub. It polls sidecar availability through `useSidecar`, wires WebSocket listeners, hydrates stores from DB and sidecar snapshots, and triggers HTTP `live-overview` or `poll-now` fallback when socket delivery is blocked or delayed.
- Zustand stores are the mutation surface for UI state. They use imperative async actions plus generation/revision counters to stop stale fetches or overlapping WS/HTTP responses from overwriting newer state.
- Frontend boundary code uses guard clauses and local coercion helpers to absorb payload drift: strings and numbers are normalized, missing sidecar ports short-circuit requests, malformed WS messages are ignored, and optional caption/product enrichment fails soft instead of breaking the main recording flow.
- Rust commands stay thin and synchronous: validate inputs, lock the shared `rusqlite::Connection`, run explicit SQL, and flatten failures to `Result<_, String>` so the frontend sees predictable error shapes.
- Stateful sidecar behavior lives in `core/*`, not routes. FastAPI handlers mostly parse and validate input, `core.watcher` owns watched-account lifecycle, `core.recorder` and `core.worker` drive recording progress, and `ws_manager.broadcast(...)` pushes runtime events back to the desktop shell.
- Recovery paths are deliberate: the WebSocket client auto-reconnects, live flags can be resynced by HTTP snapshot, active recordings let the watcher skip repeat TikTok polling, and DB upserts preserve existing values when sidecar payloads arrive partially.

## 4. Conventions

- Use `@/` imports for frontend internals; keep cross-process transport in `src/lib` and store mutations in `src/stores` instead of embedding fetch or `invoke` logic deep in components.
- `DESIGN.md` is the source of truth for frontend styling, layout, and interaction changes unless the user explicitly asks to deviate.
- Naming stays explicit by layer: React component, store, and type symbols use PascalCase; hooks use `useX`; component filenames are usually kebab-case; page/store/type modules stay lowercase; Python modules and Rust command ids stay snake_case.
- Boundary names must line up across layers: Rust `#[tauri::command]` names should match frontend `invoke("...")` strings exactly, and sidecar route paths/types should stay aligned with `src/lib/api.ts` helpers.
- Keep TypeScript normalization explicit with small helpers such as `normalizeUsername`, coercion utilities, and parse functions instead of dense inline casts or clever transformations.
- Python code favors typed request/response models plus small route helpers for validation and parsing before delegating to `core/*`; Rust code favors direct SQL and `map_err(|e| e.to_string())` at the desktop boundary.
- Comments are short and rare; add them only where desktop/sidecar coordination, race prevention, fallback behavior, or TikTok-specific quirks are not obvious from the code.
- Prefer explicit `null`/`Option`/error handling around sidecar payloads, cookies JSON, schedules, storage paths, and live-status synchronization paths.

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
