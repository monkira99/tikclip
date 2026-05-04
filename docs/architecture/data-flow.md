# Data flow

Status: Canonical  
Owner: Engineering  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/components/layout/app-shell-effects.ts`
- `/Users/monkira/Tiktok_App_reup/src/lib/api`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/live_runtime/manager`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/workflow`

## Flow edit to runtime

```mermaid
sequenceDiagram
  participant UI as React Flow UI
  participant API as src/lib/api/flows.ts
  participant CMD as Rust flow commands
  participant DB as SQLite
  participant RT as LiveRuntimeManager

  UI->>API: save draft node
  API->>CMD: save_flow_node_draft
  CMD->>DB: update draft_config_json
  UI->>API: publish
  API->>CMD: publish_flow_definition
  CMD->>DB: validate/canonicalize/persist published config
  CMD->>RT: reconcile enabled runtime
  RT->>UI: flow-runtime-updated
```

## Live to clip

```mermaid
sequenceDiagram
  participant RT as LiveRuntimeManager
  participant TK as TikTok transport
  participant REC as Recording runtime
  participant WF as Workflow
  participant DB as SQLite
  participant UI as React UI

  RT->>TK: poll live status
  TK-->>RT: live + stream_url
  RT->>DB: create flow_run/start node run/record node run
  RT->>REC: spawn recording
  REC-->>RT: outcome
  RT->>DB: finalize recording
  RT->>WF: run clip/caption/product suggestion
  WF->>DB: persist clips/captions/tags
  WF->>UI: rust-clip-ready
  RT->>UI: flow-runtime-updated/log
```

## Shell sync

`app-shell-effects.ts` lắng nghe events và bump/refetch stores liên quan. Event payload không nên được xem là persistent state đầy đủ; DB-backed commands vẫn là nguồn đọc durable state.

