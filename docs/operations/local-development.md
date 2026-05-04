# Local development

Status: Canonical  
Owner: Engineering  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/package.json`
- `/Users/monkira/Tiktok_App_reup/src-tauri`

## Chạy local

Đọc `README.md` tại repo root trước nếu cần setup môi trường. Khi làm việc trong repo này, ưu tiên scripts có sẵn trong `package.json`.

## Nguyên tắc

- Frontend code nằm trong `src`.
- Rust/Tauri code nằm trong `src-tauri`.
- DB cục bộ nằm trong storage root của app, không nằm cố định trong repo.
- Không thêm dependency mới nếu task không thật sự cần.

## Khi debug

- UI state: kiểm tra stores và API wrapper.
- IPC: kiểm tra command name trong `src/lib/api/*` và `src-tauri/src/lib.rs`.
- Runtime: kiểm tra `flow-runtime-log` và Rust logs.
- DB/schema: kiểm tra migrations và `schema_version`.
- Media/storage: kiểm tra storage root, resolved path và cleanup settings.

