# Feature: storage cleanup

Status: Canonical  
Owner: Storage  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/features/settings/settings-page.tsx`
- `/Users/monkira/Tiktok_App_reup/src/lib/api/storage.ts`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/storage.rs`

## Purpose

Storage cleanup tính usage, cảnh báo quota và xóa raw recordings/archived clips theo retention settings.

## Invariants

- Cleanup chỉ xóa file nằm trong storage root và trong scope quản lý.
- Raw recording cleanup phải null/delete DB refs nhất quán.
- Archived clip cleanup chỉ áp dụng cho clip `archived` đủ điều kiện retention.
- Product media không tính vào quota cleanup recordings/clips.
- Events `storage_warning` và `cleanup_completed` phải giữ tên ổn định.

## Verification

- Settings UI: `npm run lint:js`
- Storage Rust: `npm run lint:rust`

