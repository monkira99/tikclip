# Thuật ngữ

Status: Canonical  
Owner: Product  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/types/index.ts`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/db/migrations`

## Terms

- Account: TikTok account được app quản lý hoặc theo dõi. Account có username, cookies, proxy, live status và metadata.
- Flow: cấu hình workflow gắn với một username bắt đầu. Flow có draft version và published version.
- Flow node: một bước cố định trong pipeline. Node key hợp lệ là `start`, `record`, `clip`, `caption`, `upload`.
- Flow run: một lần runtime chạy pipeline cho một flow.
- Node run: trạng thái thực thi của một node trong một flow run.
- Recording: file ghi live stream hoặc row đại diện cho quá trình ghi.
- External recording id: khóa ngoài do recording runtime tạo để map worker/process với row SQLite.
- Clip: đoạn video cắt ra từ recording, có transcript/caption/product tags.
- Product: sản phẩm trong catalog cục bộ, có metadata và media để suggestion.
- Storage root: thư mục gốc chứa `data/app.db` và media files.
- Runtime snapshot: trạng thái hiện tại của flow live runtime để UI render.
- Runtime log: log có cấu trúc của runtime, có `flow_id`, stage, event, message và context.

## Naming rules

- Serialized boundary ưu tiên snake_case, trừ khi struct Rust đã khai báo camelCase cho debug/session snapshot.
- Flow node/status là closed set. Không thêm giá trị mới nếu chưa update contract, frontend types, Rust constants, DB check constraints và tests liên quan.
- Username được normalize gần boundary tiêu thụ: trim, bỏ prefix `@` khi cần, và tránh duplicate theo canonical username.

