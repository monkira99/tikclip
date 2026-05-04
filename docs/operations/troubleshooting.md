# Troubleshooting

Status: Canonical  
Owner: Engineering  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/components/layout/app-shell-effects.ts`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/live_runtime`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/storage.rs`

## UI không cập nhật runtime

- Kiểm tra event `flow-runtime-updated` có emit không.
- Kiểm tra `app-shell-effects.ts` có listener active không.
- Kiểm tra store runtime bucket/revision có bị stale token overwrite không.

## Flow enabled nhưng không poll

- Kiểm tra published start config có username hợp lệ không.
- Kiểm tra runtime session listing qua `list_live_runtime_sessions`.
- Kiểm tra poll interval/retry config.
- Kiểm tra username collision/duplicate account binding.

## Recording không finalize

- Kiểm tra `external_recording_id`.
- Kiểm tra active recording handle trong runtime.
- Kiểm tra worker outcome và DB row terminal status.
- Kiểm tra cancellation khi disable/restart/delete flow.

## Clip/product suggestion fail

- Kiểm tra media path có nằm trong storage root không.
- Kiểm tra ffmpeg/ffprobe availability.
- Kiểm tra product vector settings và API key nếu feature embedding bật.
- Kiểm tra runtime logs stage `clip`.

## Storage warning/cleanup bất thường

- Kiểm tra `max_storage_gb` và retention settings.
- Kiểm tra product media có bị tính nhầm vào quota không.
- Kiểm tra path có resolve đúng storage root không.
- Kiểm tra event `storage_warning`/`cleanup_completed`.

