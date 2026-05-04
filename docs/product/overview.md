# Tổng quan sản phẩm

Status: Canonical  
Owner: Product  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/pages`
- `/Users/monkira/Tiktok_App_reup/src/components`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands`

## Purpose

TikClip là desktop app để theo dõi TikTok Live account, ghi live stream theo flow cố định, cắt clip, tạo caption, gắn sản phẩm và quản lý storage cục bộ.

## Giá trị chính

- Giảm thao tác thủ công khi theo dõi nhiều account live.
- Ghi lại live stream bằng Rust-owned recording runtime.
- Tạo clip và metadata từ recording.
- Hỗ trợ caption, product suggestion và tag product vào clip.
- Quản lý file cục bộ và cảnh báo/dọn dẹp storage.

## Scope hiện tại

- App desktop React/Tauri.
- Persistence cục bộ bằng SQLite trong storage root.
- Pipeline flow đóng: `start -> record -> clip -> caption -> upload`.
- Runtime live polling và recording do Rust sở hữu.
- UI dùng để cấu hình, quan sát trạng thái, review outputs và kích hoạt thao tác thủ công.

## Ngoài scope nếu chưa có yêu cầu riêng

- Cloud sync.
- Multi-user collaboration.
- Pipeline builder tùy ý node/edge.
- Remote worker cluster.
- Full upload automation production-grade.

## Source of truth liên quan

- Thuật ngữ: `terminology.md`
- User workflows: `user-workflows.md`
- Flow pipeline contract: `../contracts/flow-pipeline.md`
- Runtime architecture: `../architecture/rust-runtime.md`

