# Storage và time

Status: Canonical  
Owner: Storage  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/app_paths.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/time_hcm.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/storage.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/db/migrations`

## Storage root

Storage root là thư mục gốc của app data và media. SQLite nằm tại:

```txt
storage_root/data/app.db
```

Người dùng có thể apply custom storage root từ Settings. Sau khi đổi root, app cần restart để đọc DB/media từ root mới.

## Media ownership

- Recording files và clips được tính vào quota cleanup.
- Product media được theo dõi trong storage stats nhưng không tính vào quota recordings/clips.
- Media path dùng cho embedding/extract/delete phải resolve trong storage root và reject path outside root.

## Time semantics

- SQLite migrations và Rust helpers đang dùng GMT+7/HCM semantics cho timestamp và cleanup age.
- Cleanup age tính theo calendar day HCM, không chỉ theo duration milliseconds.

## Cleanup

- Background worker chạy theo chu kỳ sau app setup.
- Manual cleanup đi qua `run_storage_cleanup_now`.
- Cleanup có thể emit `storage_warning` hoặc `cleanup_completed`.

