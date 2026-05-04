# Media paths contract

Status: Canonical  
Owner: Storage + Rust Runtime  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/lib/product-image.ts`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/storage.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/workflow/clip_node/product_suggest.rs`

## Rules

- Local media được resolve từ storage root.
- Rust code đọc/extract/embed/delete local media phải reject path outside storage root.
- Relative paths nên được hiểu là relative to storage root.
- Remote `http://` và `https://` URLs chỉ nên được dùng cho UI display hoặc fetch có chủ đích, không coi là local file.
- Cleanup chỉ xóa media trong phạm vi được quản lý.

## Quota

- Recordings và clips tính vào quota cleanup.
- Products/media catalog có stats riêng và không tính vào quota cleanup recordings/clips.

## Change checklist

Khi đổi path layout:

1. Cập nhật storage/path helpers.
2. Cập nhật commands/workflow đọc/extract/delete media.
3. Cập nhật UI path display/thumbnail helpers nếu cần.
4. Cập nhật docs này và `architecture/storage-and-time.md`.

