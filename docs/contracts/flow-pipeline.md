# Flow pipeline contract

Status: Canonical  
Owner: Rust Runtime + Frontend  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/types/index.ts`
- `/Users/monkira/Tiktok_App_reup/src/lib/flow-node-forms.ts`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/workflow/constants.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/db/migrations/008_flow_engine_rebuild.sql`

## Closed node set

Flow node keys hợp lệ:

```txt
start -> record -> clip -> caption -> upload
```

Không thêm/xóa/rename node nếu chưa update:

- TypeScript `FlowNodeKey`.
- Rust `FLOW_NODE_KEYS`.
- SQLite check constraints trong migration mới.
- Flow canvas/UI node rendering.
- Draft parse/serialize trong `src/lib/flow-node-forms.ts`.
- Runtime/workflow node runners.
- Tests liên quan.

## Flow statuses

Flow status hợp lệ:

```txt
idle
watching
recording
processing
error
disabled
```

`disabled` là trạng thái khi flow không được runtime poll. `watching` là đã enabled và đang poll live status. `recording` là đang có active recording. `processing` là đang xử lý downstream sau record. `error` là runtime/publish/session lỗi cần hiển thị.

## Run statuses

Flow run status hợp lệ ở TypeScript:

```txt
pending
running
completed
failed
cancelled
```

DB node run statuses hợp lệ:

```txt
pending
running
completed
failed
skipped
cancelled
```

## Node configs

### start

Canonical draft/published keys:

- `username`
- `cookies_json`
- `proxy_url`
- `waf_bypass_enabled`
- `poll_interval_seconds`
- `retry_limit`

Legacy camelCase có thể được frontend/Rust parse khi migration/backward compatibility cần.

### record

Canonical keys:

- `max_duration_minutes`
- `speech_merge_gap_sec`
- `stt_num_threads`
- `stt_quantize`

Legacy seconds-based configs có thể được chuyển sang minutes bằng round-up.

Record audio defaults có thể lấy từ `app_settings`; xem `settings.md`.

### clip

Canonical keys:

- `clip_min_duration`
- `clip_max_duration`
- `speech_cut_tolerance_sec`

### caption

Canonical keys:

- `inherit_global_defaults`
- `model`

### upload

Upload hiện là node trong pipeline contract, nhưng automation upload production-grade chưa được mở rộng thành hệ thống riêng.

## Publish behavior

- Draft và published config là hai boundary riêng.
- Publish validate/canonicalize start và record configs.
- Publish tăng version và reconcile runtime nếu flow enabled.
- Nếu runtime reconcile fail, published DB state phải rollback hoặc để lại error state coherent theo tests hiện có.
