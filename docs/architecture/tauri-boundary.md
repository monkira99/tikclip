# Tauri boundary

Status: Canonical  
Owner: Engineering  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/lib/api`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/lib.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands`

## Purpose

Tauri boundary là hợp đồng giữa React và Rust. Frontend gọi named commands bằng `invoke`; Rust trả `Result<T, String>` và flatten lỗi thành string tại boundary.

## Rules

- Command name trong frontend phải khớp chính xác tên Rust `#[tauri::command]`.
- Thêm command mới phải đăng ký trong `tauri::generate_handler!` tại `src-tauri/src/lib.rs`.
- Command nên mỏng: validate input, lock DB ngắn gọn, gọi helper/runtime/workflow module.
- Long-running/stateful work thuộc `live_runtime`, `recording_runtime`, `workflow`, `tiktok` hoặc storage helpers.
- Payload shape thay đổi phải update `../contracts/tauri-commands.md` và TypeScript types/API wrapper.

## Error handling

- Rust command trả `Result<_, String>`.
- Internal errors nên thêm context tại module xử lý, nhưng boundary không expose nested error type.
- Frontend hiển thị message thân thiện nếu lỗi là user-facing; debug detail nên đi vào runtime logs khi có runtime context.

