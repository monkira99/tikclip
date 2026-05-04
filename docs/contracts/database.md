# Database contract

Status: Canonical  
Owner: Rust Runtime + Storage  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/db/init.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/db/migrations`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/db/models.rs`

## Database location

SQLite database nằm tại:

```txt
storage_root/data/app.db
```

Database được mở với:

- `PRAGMA journal_mode=WAL`
- `PRAGMA foreign_keys=ON`

## Migration rules

- Migrations append-only và ordered.
- Không sửa migration đã release trừ khi project chưa có user data phụ thuộc và đã được đồng ý rõ.
- Schema change phải preserve old data nếu có thể.
- Thêm migration phải đăng ký trong `src-tauri/src/db/init.rs`.
- Thay đổi schema phải update models/types/commands và docs này.

## Core tables

- `accounts`: TikTok account, cookies/proxy, live status và metadata.
- `recordings`: recording lifecycle, file path, flow ownership, external recording id.
- `clips`: generated/manual clips, caption fields, transcript, flow ownership.
- `clip_products`: many-to-many clip/product tags.
- `products`: product catalog.
- `notifications`: persisted app notifications/activity references.
- `app_settings`: key/value string settings consumed by frontend and Rust modules.
- `speech_segments`: STT segments per recording.
- `flows`: flow summary, enabled/status/current node/version fields.
- `flow_nodes`: draft/published node configs.
- `flow_runs`: pipeline run instances.
- `flow_node_runs`: node execution rows.
- `product_embedding_vectors`: product vector search support.
- `schema_version`: applied migration versions.

## Closed constraints

- Flow node keys: `start`, `record`, `clip`, `caption`, `upload`.
- Flow statuses: `idle`, `watching`, `recording`, `processing`, `error`, `disabled`.
- Flow run statuses: `pending`, `running`, `completed`, `failed`, `cancelled`.
- Flow node run statuses: `pending`, `running`, `completed`, `failed`, `skipped`, `cancelled`.
- Recording statuses include `recording`, `done`, `error`, `processing`, `cancelled`.
- Clip caption statuses: `pending`, `generating`, `completed`, `failed`.
- Product embedding modalities: `image`, `video`, `text`.

## External recording id

`recordings.external_recording_id` là unique nullable key dùng để map Rust recording worker/process outcome với DB row. Tên cũ `sidecar_recording_id` đã được migration 011 rename.

## Ownership rules

- Rust owns all SQLite writes.
- Frontend không đọc/ghi SQLite trực tiếp.
- TypeScript mirror types phải khớp DB/Rust response shapes hoặc ghi rõ lý do nếu type chỉ đại diện subset runtime-only.
- Foreign keys phải giữ behavior flow-owned rows coherent khi delete flow/account/recording.
- Media cleanup phải null/delete DB refs nhất quán với file deletion.
