# ADR-0002: Fixed flow pipeline

Status: Accepted  
Owner: Product + Engineering  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/types/index.ts`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/workflow/constants.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/db/migrations/008_flow_engine_rebuild.sql`

## Context

TikClip cần workflow để monitor live, record, clip, caption và upload. Hiện nhu cầu sản phẩm là pipeline cố định, không phải visual programming engine tổng quát.

## Decision

Flow pipeline là closed sequence:

```txt
start -> record -> clip -> caption -> upload
```

## Consequences

- UI canvas có thể tập trung vào cấu hình node và runtime visibility.
- Rust workflow có thể giữ node runners rõ ràng, ít abstraction.
- DB có check constraints cho node keys.
- Thêm node mới là schema/contract change, không phải chỉ thêm UI item.

## Alternatives rejected

- Arbitrary graph/DAG: quá nặng so với scope hiện tại, tăng complexity validation/runtime.
- Plugin node system: chưa có nhu cầu production rõ ràng.

