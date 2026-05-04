# Tài liệu dự án

Status: Canonical  
Owner: Engineering  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src`
- `/Users/monkira/Tiktok_App_reup/src-tauri`

## Mục đích

Thư mục `docs/` là source of truth cho intent, architecture, contract và workflow của TikClip. Code và test vẫn là source of truth cho implementation chi tiết; docs giải thích hệ thống phải vận hành như thế nào, boundary nào không được đổi tùy tiện, và khi sửa một feature thì cần đọc gì trước.

## Đọc gì trước

- Bảo trì/rà soát docs: `operations/docs-maintenance.md`
- Tổng quan sản phẩm: `product/overview.md`
- Luồng người dùng: `product/user-workflows.md`
- Tổng quan hệ thống: `architecture/overview.md`
- Boundary Frontend/Tauri/Rust: `architecture/tauri-boundary.md`
- Runtime live/recording: `architecture/rust-runtime.md`
- Flow pipeline: `contracts/flow-pipeline.md`
- Runtime events: `contracts/runtime-events.md`
- Tauri commands: `contracts/tauri-commands.md`
- Database contract: `contracts/database.md`
- Cách verify: `operations/verification.md`

## Quy tắc source of truth

1. Mỗi chủ đề chỉ có một file canonical. File khác chỉ tóm tắt và link tới file canonical.
2. Đổi Tauri command, event, payload, flow node, status, DB schema, settings key hoặc media path rule thì phải update `docs/contracts/*`.
3. Đổi behavior người dùng thấy được thì phải update `docs/product/*` hoặc `docs/features/*`.
4. Đổi architecture hoặc ownership giữa frontend/Rust/runtime/DB thì phải update `docs/architecture/*` và tạo ADR nếu quyết định khó đảo ngược.
5. Không tạo docs theo ngày cho feature mới. Chỉ ADR có số thứ tự.
6. README và AGENTS chỉ được tóm tắt/link, không duplicate contract chi tiết.

## Ownership map

- `product/*`: sản phẩm làm gì, user workflow nào được hỗ trợ, ngoài scope là gì.
- `architecture/*`: layer nào sở hữu state/side effect, data đi qua boundary nào.
- `contracts/*`: giá trị đóng, command/event/payload/settings/schema/media-path rules ổn định.
- `features/*`: handbook để sửa một feature cụ thể, chỉ tóm tắt và link contract canonical.
- `operations/*`: cách chạy, verify, debug, release và giữ docs không drift.
- `decisions/*`: quyết định khó đảo ngược, context và tradeoff.

## Cấu trúc

- `product/`: truth về sản phẩm, thuật ngữ và user workflows.
- `architecture/`: truth về ownership, data flow, runtime và storage.
- `contracts/`: truth về boundary ổn định giữa các layer.
- `features/`: handbook theo feature để sửa code nhanh và đúng.
- `operations/`: chạy local, verify, debug, build/release.
- `decisions/`: ADR ngắn cho quyết định kỹ thuật quan trọng.

## Template doc canonical

```md
# Title

Status: Canonical
Owner: Frontend | Rust Runtime | Product | Storage | Engineering
Last reviewed: YYYY-MM-DD
Code refs:
- path/to/code

## Purpose
## Current Behavior
## Invariants
## Contracts
## Failure Modes
## Verification
## Open Questions
```

## Khi docs và code lệch nhau

Docs là source of truth cho expected behavior và stable contracts; code/test là source of truth cho implementation hiện tại. Khi phát hiện lệch:

1. Nếu docs sai với intent hiện tại, sửa docs và ghi rõ contract đúng.
2. Nếu code sai với contract, giữ docs làm expected behavior và tạo/sửa test trước khi sửa code.
3. Nếu intent chưa rõ, thêm `Open Questions` trong file canonical liên quan thay vì đoán.
