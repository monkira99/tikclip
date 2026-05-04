# Feature: recordings

Status: Canonical  
Owner: Rust Runtime  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/components/recordings`
- `/Users/monkira/Tiktok_App_reup/src/lib/api/recordings.ts`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/recordings.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/recording_runtime`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/live_runtime/manager/recordings.rs`

## Purpose

Recordings theo dõi recording workers, DB rows và output files từ live streams.

## Invariants

- `external_recording_id` map runtime outcome với row SQLite.
- Late progress update không được regress row terminal `done`, `error`, `cancelled`.
- Recording row phải giữ flow/account/run ownership khi sync/finalize.
- Cancel/disable phải cập nhật runtime và DB coherent.
- DB recording statuses phải mirror sang frontend types nếu UI đọc durable recording rows; xem `../contracts/database.md`.

## Verification

- Rust recording changes: `npm run lint:rust`
- Ưu tiên tests trong `src-tauri/src/live_runtime/manager/tests/recordings` và `src-tauri/src/commands/recordings.rs`.
