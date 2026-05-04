# Tauri commands contract

Status: Canonical  
Owner: Engineering  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/lib/api`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/lib.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands`

## Rules

- Mọi command trong file này phải được đăng ký trong `tauri::generate_handler!`.
- Frontend gọi command qua wrapper trong `src/lib/api/*` nếu command phục vụ feature UI.
- Command trả `Result<T, String>`.
- Đổi input/output shape phải update wrapper, TypeScript types và docs này.
- Command mới phải được phân loại vào đúng section bên dưới; nếu payload/result có nhiều hơn primitive đơn giản, ghi rõ fields hoặc link tới canonical type/contract.
- Debug/control commands không được trở thành user-facing workflow chính nếu chưa có product/feature doc tương ứng.

## Accounts

- `list_accounts`
- `create_account`
- `delete_account`
- `sync_accounts_live_status`

## Clips

- `update_clip_caption`
- `generate_clip_caption`
- `suggest_product_for_clip`

## Products

- `list_products`
- `create_product`
- `update_product`
- `delete_product`
- `tag_clip_product`
- `fetch_product_from_url`
- `index_product_embeddings`
- `delete_product_embeddings`

## Dashboard

- `get_dashboard_stats`

## Flows

- `list_flows`
- `create_flow`
- `apply_flow_runtime_hint`
- `set_flow_enabled`
- `delete_flow`

## Flow engine

- `get_flow_definition`
- `save_flow_node_draft`
- `publish_flow_definition`
- `restart_flow_run`

## Live runtime debug/control

- `list_live_runtime_sessions`
- `list_live_runtime_logs`
- `trigger_start_live_detected`
- `mark_start_run_completed`
- `mark_source_offline`

## Recordings

- `list_active_rust_recordings`

## Notifications

- `insert_notification`
- `list_notifications`
- `mark_notification_read`
- `mark_all_notifications_read`

## Settings

- `get_setting`
- `set_setting`

## Storage

- `get_storage_stats`
- `list_activity_feed`
- `run_storage_cleanup_now`

## Paths

- `get_app_data_paths`
- `open_path`
- `storage_root_is_custom`
- `pick_storage_root_folder`
- `apply_storage_root`
- `reset_storage_root_default`

## Change checklist

Khi thêm/sửa command:

1. Cập nhật Rust command và `generate_handler`.
2. Cập nhật hoặc thêm wrapper trong `src/lib/api/*`.
3. Cập nhật TypeScript request/result type nếu có.
4. Cập nhật docs này.
5. Chạy verification hẹp cho layer bị chạm.
