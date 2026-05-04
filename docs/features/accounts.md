# Feature: accounts

Status: Canonical  
Owner: Frontend + Rust Runtime  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/components/accounts`
- `/Users/monkira/Tiktok_App_reup/src/lib/api/accounts.ts`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/accounts.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/live_runtime/account_binding.rs`

## Purpose

Accounts lưu TikTok usernames, cookies/proxy và live status. Flow runtime có thể resolve account theo start node username.

## Invariants

- Username phải được normalize gần boundary dùng nó.
- Duplicate theo canonical username phải được xử lý cẩn thận vì runtime binding phụ thuộc username.
- Cookies/proxy optional và phải giữ explicit null/empty semantics.

## Verification

- Frontend: `npm run lint:js`
- Rust: `npm run lint:rust`

