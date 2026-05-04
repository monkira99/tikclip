# Settings contract

Status: Canonical  
Owner: Frontend + Storage + Product Suggestion  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/features/settings/settings-config.ts`
- `/Users/monkira/Tiktok_App_reup/src/features/settings/settings-page.tsx`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/settings.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/flows.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/storage.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/workflow/record_node.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/workflow/clip_node/product_suggest.rs`

## Storage format

Settings được lưu dưới dạng key/value string. Consumer chịu trách nhiệm parse thành bool/int/float/string và gán default gần nơi tiêu thụ.

Empty string là unset cho optional secret/string settings nếu consumer định nghĩa như vậy. Numeric settings phải clamp/validate gần consumer để tránh làm hỏng runtime.

## Flow/record defaults

Các key này được dùng khi tạo flow/default record node và khi record node xử lý audio:

- `speech_merge_gap_sec`
- `stt_num_threads`
- `stt_quantize`

Defaults hiện tại:

- `speech_merge_gap_sec`: `0.5`
- `stt_num_threads`: `4`
- `stt_quantize`: `auto`

`stt_quantize` hợp lệ: `auto`, `fp32`, `int8`.

## Product/vector keys

- `product_vector_enabled`
- `gemini_api_key`
- `gemini_embedding_model`
- `gemini_embedding_dimensions`
- `auto_tag_clip_product_enabled`
- `auto_tag_clip_frame_count`
- `auto_tag_clip_max_score`
- `suggest_weight_image`
- `suggest_weight_text`
- `suggest_min_fused_score`
- `debug_keep_suggest_clip_frames`
- `suggest_image_embed_focus_prompt`

Frontend defaults:

- `gemini_embedding_model`: `gemini-embedding-2-preview`
- `gemini_embedding_dimensions`: `1536`
- `auto_tag_clip_frame_count`: `4`
- `auto_tag_clip_max_score`: `0.35`
- `suggest_weight_image`: `0.6`
- `suggest_weight_text`: `0.4`
- `suggest_min_fused_score`: `0.25`

## Storage cleanup keys

- `max_storage_gb`
- `TIKCLIP_RAW_RETENTION_DAYS`
- `TIKCLIP_ARCHIVE_RETENTION_DAYS`
- `TIKCLIP_STORAGE_WARN_PERCENT`
- `TIKCLIP_STORAGE_CLEANUP_PERCENT`

## Rules

- Empty string nên được consumer xử lý như unset nếu setting optional.
- Secret values như `gemini_api_key` không được đưa vào logs/docs examples.
- Thêm key mới phải update settings UI, Rust consumer và docs này.
- Không đổi meaning của key cũ nếu có thể thêm key mới rõ hơn.
