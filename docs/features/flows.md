# Feature: flows

Status: Canonical  
Owner: Frontend + Rust Runtime  
Last reviewed: 2026-05-03  
Code refs:
- `/Users/monkira/Tiktok_App_reup/src/components/flows`
- `/Users/monkira/Tiktok_App_reup/src/lib/api/flows.ts`
- `/Users/monkira/Tiktok_App_reup/src/lib/flow-node-forms.ts`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/flows.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/commands/flow_engine.rs`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/live_runtime`
- `/Users/monkira/Tiktok_App_reup/src-tauri/src/workflow`

## Purpose

Flows là feature điều phối live monitoring và pipeline `start -> record -> clip -> caption -> upload`.

## Current behavior

- Flow có draft config và published config.
- UI sửa draft node configs.
- Publish validate/canonicalize và reconcile runtime nếu flow enabled.
- Enabled flow được Rust runtime poll.
- Runtime snapshots/logs cập nhật UI strip/detail.

## Invariants

- Flow node set là closed set.
- UI không tự tạo runtime transition; Rust runtime owns lifecycle.
- Draft changes không ảnh hưởng runtime cho đến khi publish.
- Runtime reconcile fail không được để DB và in-memory runtime bị split-brain.

## Contracts

- Pipeline: `../contracts/flow-pipeline.md`
- Runtime events: `../contracts/runtime-events.md`
- Tauri commands: `../contracts/tauri-commands.md`

## Verification

- Frontend-only: `npm run lint:js`
- Rust/runtime: `npm run lint:rust`
- Nếu đổi publish/reconcile/runtime, chạy hoặc thêm tests quanh `src-tauri/src/commands/flow_engine/tests` và `src-tauri/src/live_runtime/manager/tests`.

