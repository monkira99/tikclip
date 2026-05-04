# Docs maintenance

Status: Canonical  
Owner: Engineering  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/docs`
- `/Users/monkira/Tiktok_App_reup/src`
- `/Users/monkira/Tiktok_App_reup/src-tauri`

## Purpose

File này là checklist để giữ `docs/` làm source of truth. Dùng khi thêm feature, đổi contract, rà soát docs định kỳ, hoặc thấy docs và code không khớp.

## Review order

1. Đọc `docs/README.md` để xác định file canonical.
2. Đọc file product/architecture/contract/feature liên quan trước khi sửa.
3. Đối chiếu code refs trong file canonical với source hiện tại.
4. Sửa file canonical trước, rồi sửa handbook/README/AGENTS nếu cần tóm tắt/link lại.
5. Verify docs-only bằng các lệnh trong `verification.md`.

## Drift checks

Tauri command drift:

```bash
rg "generate_handler!|invoke(<|\\()" src-tauri/src/lib.rs src/lib src/components src/pages src/stores
```

Runtime event drift:

```bash
rg "emit\\(|listen<|listen\\(" src src-tauri/src
```

Settings drift:

```bash
rg "app_settings|KEY_|TIKCLIP_|max_storage_gb|speech_merge_gap_sec|stt_num_threads|stt_quantize" src src-tauri/src
```

Flow/status/schema drift:

```bash
rg "FlowNodeKey|FlowStatus|FlowRunStatus|caption_status|status IN \\(" src src-tauri/src
```

Docs inventory:

```bash
rg --files docs | sort
```

## Canonical depth

- Product docs phải nêu scope, out-of-scope và workflow user-facing.
- Architecture docs phải nêu ownership, data flow và invariants giữa layer.
- Contract docs phải nêu closed sets, payload fields, storage/settings/schema rules và change checklist.
- Feature docs không duplicate contract chi tiết; chúng phải link tới contract canonical và nêu files/tests cần đọc.
- Operations docs phải có lệnh kiểm tra cụ thể, không chỉ mô tả chung.

## Red flags

- Một enum/status/key xuất hiện trong code nhưng không có trong `contracts/*`.
- Feature handbook mô tả payload/DB schema chi tiết thay vì link contract.
- README hoặc AGENTS chứa contract chi tiết hơn docs canonical.
- File có `Status: Canonical` nhưng thiếu code refs hoặc verification.
- `Status: Draft` nhưng được file khác coi như contract bắt buộc.
