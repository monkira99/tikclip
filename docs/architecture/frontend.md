# Frontend architecture

Status: Canonical  
Owner: Frontend  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/components`
- `/Users/monkira/Tiktok_App_reup/src/lib`
- `/Users/monkira/Tiktok_App_reup/src/stores`
- `/Users/monkira/Tiktok_App_reup/src/types/index.ts`

## Purpose

Frontend là React desktop surface. Nó render state, nhận input người dùng, serialize form configs, gọi Tauri commands qua `src/lib/api/*`, và lắng nghe runtime/storage events để sync stores.

## Ownership

- `src/lib/api/*`: nơi duy nhất nên gọi `invoke(...)` cho feature code.
- `src/types/index.ts`: TypeScript domain contracts mirror Rust/SQLite response shape.
- `src/stores/*`: Zustand stores giữ UI state, request tokens, revisions và optimistic snapshots.
- `src/lib/flow-node-forms.ts`: parse/serialize flow node draft configs.
- `src/components/layout/app-shell-effects.ts`: hydrate notifications, sync recording count, subscribe runtime/storage/clip events.

## Invariants

- Component không nên gọi `invoke` trực tiếp nếu đã có hoặc có thể thêm wrapper trong `src/lib/api/*`.
- Form boundary chuyển về explicit string/number/boolean trước khi gửi Rust.
- Request async phải tránh stale response bằng token/revision pattern nếu có nguy cơ race.
- UI không duplicate Rust state machine; UI chỉ render snapshot/log/result.

## Verification

- Frontend-only changes: chạy `npm run lint:js`.
- UI behavior thay đổi: đọc `DESIGN.md` trước khi sửa và verify màn hình liên quan.

