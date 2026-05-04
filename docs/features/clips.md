# Feature: clips

Status: Canonical  
Owner: Rust Workflow + Frontend  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/components/recordings`
- `/Users/monkira/Tiktok_App_reup/src/lib/api/clips.ts`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/clips.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/workflow/clip_node`

## Purpose

Clips là output video từ recording, có metadata, transcript, caption và product tags.

## Invariants

- Clip files phải resolve trong storage root khi Rust đọc/xử lý.
- `rust-clip-ready` chỉ là signal; UI phải refetch DB-backed state.
- Clip insert nên idempotent theo recording/file khi cùng file được report lại.
- Caption fields nằm trên clip row và có status riêng.

## Verification

- Frontend: `npm run lint:js`
- Rust workflow: `npm run lint:rust`

