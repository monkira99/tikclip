# Runtime events contract

Status: Canonical  
Owner: Rust Runtime + Frontend  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/components/layout/app-shell-effects.ts`
- `/Users/monkira/Tiktok_App_reup/src/types/index.ts`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/live_runtime/manager/events.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/workflow/clip_node/clip_processing.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/storage.rs`

## Events

### `flow-runtime-updated`

Emitted by live runtime khi flow runtime snapshot thay đổi.

Payload align với `FlowRuntimeSnapshot`:

- `flow_id`
- `status`
- `current_node`
- `account_id`
- `username`
- `last_live_at`
- `last_error`
- `active_flow_run_id`
- optional runtime polling fields như `last_checked_at`, `last_check_live`, `next_poll_at`, `poll_interval_seconds`

Frontend dùng event này để update runtime buckets và refetch related state khi cần.

### `flow-runtime-log`

Emitted by live runtime cho structured operational logs.

Payload align với `FlowRuntimeLogEntry`:

- `id`
- `timestamp`
- `level`
- `flow_id`
- `flow_run_id`
- `external_recording_id`
- `stage`
- `event`
- `code`
- `message`
- `context`

Logs nên có context đủ để debug runtime race, TikTok polling, recording spawn/finalize và downstream workflow.

Runtime log context phải redact sensitive keys như cookies/token/authorization/secret/password/stream_url/proxy_url.

### `rust-clip-ready`

Emitted sau khi Rust clip processing tạo/cập nhật một clip. Frontend treats it as signal để bump clip/dashboard revisions và refetch DB-backed lists.

Payload:

- `clip_id`
- `recording_id`
- `account_id`
- `username`
- `clip_index`
- `path`
- `thumbnail_path`
- `start_sec`
- `end_sec`
- `duration_sec`
- `transcript_text`

### `cleanup_completed`

Emitted by storage cleanup sau cleanup cycle xóa eligible media. Payload gồm cleanup counts/bytes khi có. Frontend map event này sang notification/activity refresh.

Payload:

- `deleted_recordings`
- `deleted_clips`
- `freed_bytes`
- `notification_id`

### `storage_warning`

Emitted by storage cleanup/stats khi quota usage vượt warning threshold. Frontend map event này sang notification/activity refresh.

Payload:

- `usage_percent`
- `quota_bytes`
- `total_bytes`
- `critical`
- `notification_id`

## Rules

- Event names là stable public contracts trong app.
- Event payload thay đổi phải update TypeScript types/parsers và docs này.
- Events are signals, not durable truth. UI phải refetch command-backed state nếu cần consistency.
- Runtime logs không thay thế user-facing notifications.
