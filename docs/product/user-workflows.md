# User workflows

Status: Canonical  
Owner: Product  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/pages`
- `/Users/monkira/Tiktok_App_reup/src/components/flows`
- `/Users/monkira/Tiktok_App_reup/src/features/settings`

## Monitor và record live

1. Người dùng tạo account hoặc flow với username TikTok.
2. Người dùng cấu hình start/record/clip/caption nodes.
3. Người dùng publish flow.
4. Khi flow enabled, Rust runtime poll live status theo start config.
5. Khi account live và có stream URL hợp lệ, runtime tạo flow run và bắt đầu record.
6. Khi recording kết thúc, runtime chuyển sang clip/caption/product suggestion theo cấu hình hiện có.

## Quản lý flow

- Draft node config được sửa riêng với published config.
- Publish validate/canonicalize config và tăng version.
- Enable flow khởi động runtime session.
- Disable flow dừng poll task và cancel active run nếu cần.
- Delete flow phải dừng active session và xóa/cập nhật rows flow-owned theo contract hiện tại.

## Quản lý recordings và clips

- Recording row theo dõi trạng thái `recording`, `processing`, `done`, `error`, `cancelled`.
- Clip row theo dõi trạng thái `draft`, `ready`, `posted`, `archived`.
- Caption có status riêng: `pending`, `generating`, `completed`, `failed`.
- Clip có thể được gắn product thủ công hoặc thông qua product suggestion.

## Quản lý storage

- Storage root có thể là default hoặc custom.
- App lưu DB tại `storage_root/data/app.db`.
- Cleanup xóa raw recordings/archived clips theo retention settings.
- Product media không tính vào quota cleanup của recordings/clips.

## Điều cần giữ ổn định

- UI không tự ý tạo durable state ngoài API wrappers.
- Rust là owner của SQLite writes, runtime transitions, recording, clip extraction, caption runtime và storage cleanup.
- Notification/event UI phải xem runtime events là signal để refetch/sync, không thay thế DB source of truth.

