# Feature: products

Status: Canonical  
Owner: Product Suggestion  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/components/products`
- `/Users/monkira/Tiktok_App_reup/src/lib/api/products.ts`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/products.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/workflow/clip_node/product_suggest.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/workflow/clip_node/product_vectors.rs`

## Purpose

Products là catalog sản phẩm cục bộ dùng để gắn clip và suggestion dựa trên frame/image/text embeddings.

## Invariants

- Product media path phải tuân thủ `../contracts/media-paths.md`.
- Embedding settings phải tuân thủ `../contracts/settings.md`.
- Secret/API key không đưa vào logs.
- Auto-tag phải không làm hỏng clip processing nếu suggestion fail; lỗi nên được log/context hóa.

## Verification

- Frontend: `npm run lint:js`
- Rust/product suggestion: `npm run lint:rust`

