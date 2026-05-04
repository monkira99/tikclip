# Verification

Status: Canonical  
Owner: Engineering  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/package.json`

## Commands chính

Frontend changes:

```bash
npm run lint:js
```

Rust/Tauri changes:

```bash
npm run lint:rust
```

## Chọn verification

- Chỉ sửa docs: không cần lint code, nhưng nên kiểm tra links/path bằng `rg --files docs`.
- Rà soát docs source-of-truth: đọc `docs/operations/docs-maintenance.md`, chạy docs inventory và các drift checks phù hợp với chủ đề đã sửa.
- Sửa React component/store/API wrapper: `npm run lint:js`.
- Sửa Rust command/runtime/workflow/storage/db: `npm run lint:rust`.
- Sửa cả frontend và Rust boundary: chạy cả hai.
- Sửa UI/UX: đọc `DESIGN.md`, chạy frontend lint và verify màn hình liên quan.
- Sửa DB migration: chạy Rust lint/tests liên quan nếu có migration tests.

## Docs-only checks

Inventory:

```bash
rg --files docs | sort
```

Tìm marker chưa chốt:

```bash
rg "TODO|TBD|Open Questions|Status: Draft" docs
```

Khi sửa contract commands/events/settings/status/schema, chạy drift check tương ứng trong `docs/operations/docs-maintenance.md`.

## Checklist trước khi kết thúc task

- Contract docs đã update nếu đổi boundary.
- Types frontend/Rust/DB khớp nhau.
- Runtime events nếu đổi đã update listener/mapping.
- Settings keys, DB statuses và event payload fields đã được đối chiếu với source nếu docs contract bị chạm.
- Không có unrelated formatting/refactor churn.
- Verification đã chạy hoặc lý do không chạy được được ghi rõ.
