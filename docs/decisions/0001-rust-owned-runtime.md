# ADR-0001: Rust owns runtime

Status: Accepted  
Owner: Engineering  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/live_runtime`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/recording_runtime`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/workflow`

## Context

TikClip cần live polling, recording process control, workflow transitions, SQLite persistence và event emission. Nếu frontend sở hữu nhiều runtime state, app dễ gặp race, stale UI overwrite và DB/runtime split-brain.

## Decision

Rust owns runtime state machine. Frontend chỉ gọi commands, render snapshots/logs và refetch DB-backed state.

## Consequences

- Rust modules phải giữ concurrency/cancellation discipline.
- Tauri commands cần mỏng và delegate vào runtime/workflow modules.
- UI phải không duplicate state machine.
- Contract events/commands phải được giữ ổn định.

## Alternatives rejected

- Frontend-owned runtime orchestration: dễ race và khó đảm bảo process/DB consistency.
- External sidecar as primary runtime owner: tăng deployment complexity cho desktop app.

