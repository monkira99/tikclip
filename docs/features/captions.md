# Feature: captions

Status: Canonical  
Owner: Rust Workflow + Frontend  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/clips.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/workflow/caption_node.rs`
- `/Users/monkira/Tiktok_App_reup/src/types/index.ts`

## Purpose

Captions tạo text cho clip dựa trên transcript/runtime fallback và lưu vào clip row.

## Statuses

Caption status hợp lệ:

```txt
pending
generating
completed
failed
```

## Invariants

- Caption generation phải cập nhật `caption_text`, `caption_status`, `caption_error`, `caption_generated_at` nhất quán.
- Manual update caption phải đi qua command, không sửa DB từ frontend.

## Verification

- `npm run lint:js` nếu đổi UI.
- `npm run lint:rust` nếu đổi command/workflow.

