# Build và release

Status: Draft  
Owner: Engineering  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/package.json`
- `/Users/monkira/Tiktok_App_reup/src-tauri/tauri.conf.json`

## Purpose

File này giữ checklist build/release. Hiện chưa canonical hóa đầy đủ release process, nên status là Draft.

## Checklist tối thiểu

- Chạy verification theo `verification.md`.
- Đảm bảo migrations append-only và đã đăng ký.
- Đảm bảo command/event/settings contracts đã update nếu có thay đổi.
- Đảm bảo `tauri.conf.json` và capabilities phù hợp với feature mới.
- Smoke test app desktop với storage root thật.

## Open questions

- Release target platforms chính thức.
- Signing/notarization requirements.
- Versioning và changelog policy.

