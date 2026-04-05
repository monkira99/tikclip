# AGENTS.md

## 1. Overview

TikClip is a desktop app for monitoring TikTok Live accounts, recording streams, and managing clips and metadata. The codebase is split into a React/Tauri frontend, a Rust desktop shell with SQLite, and a Python sidecar that handles live polling, recording, and clip processing.

## 2. Folder Structure

- `src`: React frontend and desktop UI state.
  - `components`: reusable UI and feature components; `layout` owns app chrome, feature folders hold page-specific widgets, `ui` contains base shadcn-style primitives.
  - `pages`: top-level screen components rendered by `AppShell`.
  - `stores`: Zustand stores for app, account, notification, and recording state.
  - `lib`: Tauri invoke wrappers, sidecar HTTP helpers, WebSocket client, and notification utilities.
  - `hooks`: sidecar/bootstrap hooks used by the shell.
  - `types`: shared frontend type definitions.
- `src-tauri`: Rust desktop layer.
  - `src/commands`: Tauri commands exposed to the frontend for accounts, clips, and settings.
  - `src/db`: SQLite initialization, models, and migrations.
  - `src/sidecar`: sidecar process management and status reporting.
  - `src/tray.rs`, `src/lib.rs`, `src/main.rs`: app bootstrap, plugin wiring, tray setup, and shared state registration.
  - `tauri.conf.json`, `capabilities`, `icons`: desktop app configuration and packaged assets.
- `sidecar`: Python sidecar service.
  - `src/routes`: thin FastAPI route handlers for health, accounts, recordings, and clips.
  - `src/core`: long-running behavior such as watcher polling, recording, workers, and clip processing.
  - `src/tiktok`: TikTok-specific HTTP, cookies, and stream resolution logic.
  - `src/ws`: WebSocket connection manager and broadcast helpers.
  - `src/config.py`, `src/logging_config.py`, `src/app.py`, `src/main.py`: settings, logging, app assembly, and process entrypoint.
  - `tests`: pytest coverage for watcher, processor, worker, TikTok API, and route-level behavior.
- `docs/superpowers`: product specs and phased implementation plans; align behavior changes with these docs when relevant.
- `public`: static web assets used by the Vite frontend.

## 3. Core Behaviors & Patterns

- Frontend data flow is explicit: SQLite-backed operations go through Tauri `invoke`, while live/status operations go through the sidecar REST API and `/ws` socket.
- `src/components/layout/app-shell.tsx` is the main orchestration point. It boots sidecar status polling, connects WebSocket listeners, hydrates stores from sidecar state, and falls back to HTTP polling when socket delivery is unreliable.
- Zustand stores hold durable UI state and mutation helpers; components and pages stay mostly presentational and call store actions or `src/lib/api.ts` wrappers.
- Frontend integration code favors guard clauses, local normalization helpers, and tolerant fallback behavior around external data (`null`, JSON parsing, missing ports, reconnect loops).
- Rust commands keep logic direct: validate inputs, lock the shared `rusqlite::Connection`, run explicit SQL, and return `Result<_, String>` instead of layering extra abstractions.
- Sidecar routes stay thin and delegate to `core/*`. Long-running or stateful behavior belongs in watcher/recorder/worker modules, with WebSocket broadcasts used to push account and recording events back to the UI.
- Logging is scoped and pragmatic: Rust uses `env_logger`, Python uses `tikclip.*` loggers to stderr, and the frontend emits debug/warn logs only in development or failure paths.

## 4. Conventions

- Use `@/` imports in frontend code; keep shared transport/state helpers in `src/lib` and `src/stores` instead of duplicating fetch or socket logic in components.
- React components and store types use PascalCase symbols; hooks use `useX`; component filenames are typically kebab-case, while page/store/type modules stay lowercase and descriptive.
- Keep TypeScript behavior explicit with narrow helper functions such as normalization/parsing utilities instead of clever inline transformations.
- Python code follows snake_case naming, typed request/response models, and small route helpers for validation/parsing before delegating to core services.
- Rust command names are snake_case and should stay aligned with the frontend `invoke(...)` strings to avoid hidden integration drift.
- Comments are short and rare; add them only where desktop/sidecar coordination, fallback behavior, or external service quirks are not obvious from the code.
- Prefer explicit `null`/`Option`/error handling around sidecar payloads, cookies JSON, schedules, and live-status synchronization paths.

## 5. Working Agreements

- Respond in Vietnamese unless the user asks for another language; keep technical terms in English and never translate code blocks.
- Before editing, review related usages and the full frontend/Rust/sidecar flow for the feature you are changing.
- Prefer simple, maintainable, production-friendly changes; avoid overengineering, clever abstractions, and extra layers for small features.
- Keep APIs small, behavior explicit, naming clear, and new code colocated with the nearest existing feature/module.
- Preserve current public behavior unless the user asks to change it; call out any unavoidable behavior change.
- Before closing Python work under `sidecar`, verify `uv run ruff check src tests`, `uv run ruff format --check src tests`, `uv run ty check .`, and `uv run pytest tests/ -q` all pass from `sidecar/`.
- Before closing frontend work under `src`, verify `npm run lint:js` passes from the repo root; in this repo it already runs the production build after ESLint.
- Before closing Rust/Tauri work under `src-tauri`, verify `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass from `src-tauri/`.
- Do not add tests, lint tasks, formatting churn, or new dependencies unless the user explicitly asks for them or the change cannot be done safely without them.
- Ask for clarification instead of guessing when a requirement is ambiguous or when a change would affect multiple layers in conflicting ways.
