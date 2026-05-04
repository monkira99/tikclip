# Rust runtime

Status: Canonical  
Owner: Rust Runtime  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/live_runtime`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/recording_runtime`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/workflow`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/tiktok`

## Purpose

Rust runtime sở hữu live polling, recording lifecycle, workflow transitions, runtime logs và Tauri event emission. Frontend không điều khiển state machine trực tiếp.

## LiveRuntimeManager

Manager giữ:

- sessions theo flow.
- lease theo normalized username.
- poll generation và cancellation.
- active recordings theo flow/external recording id.
- failed snapshots.
- capped runtime logs.
- app handle để emit events.

## Runtime lifecycle

1. App boot khởi tạo SQLite và bootstrap enabled flows.
2. Enable flow tạo session và poll task.
3. Poll TikTok live status bằng start node config.
4. Khi live và stream URL hợp lệ, manager tạo flow run, completed start node, upsert running record node và spawn recording worker.
5. Recording finalize sẽ update row, node run và tiếp tục downstream clip/caption/product suggestion.
6. Disable/delete/shutdown dừng poll task và cancel active work khi cần.

## Concurrency invariants

- Generation token ngăn poll iteration cũ ghi đè state mới.
- Cancellation flag ngăn recording/downstream work cũ tiếp tục sau disable/restart.
- Runtime events là notification/snapshot channel, không thay thế SQLite persistence.
- Active run restart khi publish phải giữ DB và runtime coherent.

## Verification

- Rust/runtime changes: chạy `npm run lint:rust`.
- Nếu đổi flow runtime, ưu tiên thêm/chạy tests trong `src-tauri/src/live_runtime/manager/tests` và `src-tauri/src/commands/flow_engine/tests` nếu cần.

