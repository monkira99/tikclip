# Feature: notifications

Status: Canonical  
Owner: Frontend + Rust Runtime  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/stores/notification-store.ts`
- `/Users/monkira/Tiktok_App_reup/src/lib/notifications-sync.ts`
- `/Users/monkira/Tiktok_App_reup/src/lib/runtime-notifications.ts`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/notifications.rs`

## Purpose

Notifications hiển thị activity user-facing từ runtime/storage/clip events và persisted rows.

## Invariants

- Runtime/storage events là signal để sync notification state.
- Persisted notification rows nằm trong SQLite.
- UI mapping từ runtime event sang notification phải tolerant với payload thiếu field optional.

## Verification

- Frontend: `npm run lint:js`
- Rust notification command/storage emit: `npm run lint:rust`

