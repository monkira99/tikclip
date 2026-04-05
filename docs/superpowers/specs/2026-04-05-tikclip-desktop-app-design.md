# TikClip — TikTok Live Recorder & Clip Manager

**Date:** 2026-04-05
**Status:** Approved
**Author:** AI-assisted design session

---

## 1. Overview

TikClip is a cross-platform desktop application (macOS + Windows) for TikTok content creators who live stream. It automates the workflow of recording TikTok live streams, splitting recordings into short clips, and preparing them for posting with product tagging.

### Problem

Content creators who live stream from multiple locations cannot use the platform's built-in recording. They currently rely on manual screen recording, manual video editing to cut clips, and manual posting — a time-consuming, error-prone process.

### Solution

A desktop tool that:
- Automatically detects when monitored TikTok accounts go live
- Records live streams by downloading the stream directly (no screen capture)
- Splits recordings into short clips using scene detection (v1) and AI analysis (future)
- Provides a management dashboard for accounts, recordings, clips, and products
- Prepares clips with product tags for efficient manual posting (v1), with auto-posting planned for future phases

### Target User

Content creators and e-commerce sellers on TikTok who manage multiple accounts, live stream frequently, and want to repurpose live content into short-form clips for product promotion.

---

## 2. Architecture

### Approach: Tauri + Python Sidecar Service

The system consists of two processes running on the user's machine:

1. **Tauri Desktop App** — UI shell + local data management (Rust backend + React frontend)
2. **Python Sidecar Service** — recording engine + video processing (FastAPI server)

```
┌──────────────────────────┐  HTTP REST    ┌──────────────────────┐
│   Tauri Desktop App      │  + WebSocket  │  Python Sidecar      │
│                          │◄─────────────►│                      │
│  ┌────────────────────┐  │               │  - FastAPI Server    │
│  │ React + TypeScript │  │               │  - Recording Engine  │
│  │ (UI Layer)         │  │               │    (tiktok-recorder) │
│  └────────────────────┘  │               │  - Video Processor   │
│  ┌────────────────────┐  │               │    (FFmpeg + OpenCV) │
│  │ Rust Backend       │  │               │  - AI Pipeline       │
│  │ (Data + Lifecycle) │──── start/stop ─►│    (future module)   │
│  └────────────────────┘  │               └──────────────────────┘
└──────────────────────────┘
                │
                ▼
        📁 ~/TikTokApp/
        ├── data/app.db (SQLite)
        ├── recordings/{account}/{date}/
        ├── clips/{account}/{date}/
        ├── products/images/
        ├── exports/reports/
        ├── logs/
        └── config/
```

### Why This Approach

- **Python ecosystem** is essential for video processing (FFmpeg, OpenCV, PySceneDetect) and future AI integration (OpenAI, local models)
- **Separation of concerns**: UI and processing are independent, can be developed/tested separately
- **Scalability**: sidecar can later run on a separate machine for distributed recording
- **Tauri** provides lightweight, cross-platform desktop shell with native performance

### Communication Protocol

**HTTP REST API** (commands from UI to sidecar):

| Method | Endpoint | Purpose |
|--------|----------|---------|
| POST | /api/recording/start | Start recording a stream |
| POST | /api/recording/stop | Stop recording |
| GET | /api/recording/status | Status of all recordings |
| GET | /api/accounts | List monitored accounts |
| POST | /api/accounts | Add new account |
| POST | /api/video/process | Request video processing |
| GET | /api/clips | List generated clips |
| GET | /api/stats | Statistics data |

**WebSocket** (realtime events from sidecar to UI):

| Event | Trigger |
|-------|---------|
| account_live | Monitored account starts live streaming |
| recording_started | Recording successfully initiated |
| recording_progress | Periodic update (every 5s): duration, file size |
| recording_finished | Recording completed |
| processing_progress | Video processing progress |
| clip_ready | New clip generated and ready |
| error | Error occurred in any subsystem |

---

## 3. Tech Stack

### Tauri App (Frontend + Shell)

| Component | Technology |
|-----------|-----------|
| Framework | Tauri v2 |
| Frontend | React 19 + TypeScript |
| UI Library | shadcn/ui + Tailwind CSS |
| State Management | Zustand (global) + TanStack Query (server state) |
| Video Player | video.js (supports local MP4/FLV playback) |
| Charts | Recharts |
| Router | TanStack Router |
| Backend | Rust (Tauri commands) |
| Database | SQLite via rusqlite |
| Notifications | Tauri notification plugin + System Tray |

### Python Sidecar Service

| Component | Technology |
|-----------|-----------|
| Framework | FastAPI + Uvicorn |
| Python | 3.11+ |
| Recording | Fork of [tiktok-live-recorder](https://github.com/Michele0303/tiktok-live-recorder) |
| Video Processing | FFmpeg + OpenCV |
| Scene Detection | PySceneDetect |
| WebSocket | FastAPI WebSocket |
| Task Management | asyncio + background tasks |
| AI (future) | OpenAI API / Local models |
| Packaging | PyInstaller (bundle as executable) |

---

## 4. UI Design

### Theme & Style

Dark theme inspired by TikTok's brand colors:
- Primary: #fe2c55 (TikTok red)
- Accent: #25f4ee (TikTok cyan)
- Background: #0a0a0f (near black)
- Surface: #111118
- Border: #222

### App Shell Layout

Sidebar (left, 220px) + Main Content Area (right). Sidebar contains:
- App logo & name ("TikClip")
- Navigation: Dashboard, Accounts, Recordings, Clips, Statistics
- Settings link
- Sidecar connection status + active recordings count

Top bar: Page title + search + notification bell.

### Screens

**Dashboard:**
- 4 stat cards: Active Recordings, Accounts, Clips Today, Storage Used
- Active Recordings list with Stop controls and progress
- Monitored accounts currently live (with Record button)
- Recent Clips with Preview button

**Accounts:**
- List of all accounts with status badges (Live / Offline / Recording)
- Account types: "My accounts" vs "Monitored"
- Per-account config: auto-record toggle, schedule, priority, cookies, proxy
- Live stream history per account

**Recordings:**
- Active recordings with realtime progress (duration, file size, bitrate)
- Start/Stop controls
- Completed recordings list
- Auto-record scheduler
- Batch operations (stop all, delete old)
- Error log per recording

**Clips:**
- Grid and List view toggle
- Thumbnail preview for each clip
- Filter by account, date, status, product
- In-app video player for preview
- Manual trim/adjust clip boundaries
- Product tagging (link clips to products)
- Status workflow: Draft → Ready → Posted → Archived

**Statistics:**
- Recording hours per account/day/week (line chart)
- Clips generated over time (bar chart)
- Clips by status breakdown (pie chart)
- Storage usage trend
- Export CSV/PDF reports

---

## 5. Recording Engine

### 4-Phase Pipeline

#### Phase 1: Monitor

Account Watcher Service polls TikTok API every 30-60 seconds (configurable) for each monitored account:
- Check if account is live
- Retrieve room_id and stream URL when live detected
- Emit `account_live` WebSocket event
- Supports cookies for authentication and proxy per account

Auto-record rules (per account):
- Toggle: auto-record on/off
- Schedule: only record within specific time windows
- Max duration: auto-stop after N hours
- Priority: when max concurrent reached, higher priority accounts take precedence

#### Phase 2: Record

Each recording spawns one FFmpeg subprocess that downloads the FLV/HLS stream directly:
- Save raw video to `recordings/{account}/{date}/{HHMMSS}.flv`
- Auto-split files exceeding 2 hours or 4GB
- Report progress via WebSocket every 5 seconds
- Graceful stop ensures file integrity (no corrupt files)

Worker pool with configurable max concurrent recordings (default: 5). Excess requests queued by priority.

Error handling:
- Stream disconnect → auto-retry 3 times
- Network error → pause + retry with exponential backoff
- User ends live → save file + trigger processing
- Disk full → alert + graceful stop

#### Phase 3: Process

**v1 (MVP) — Scene-based splitting:**
1. Scene Detection (PySceneDetect) — detect major scene changes
2. Segment Grouping — combine nearby scenes into clips of 30-90 seconds
3. Clip Extraction (FFmpeg) — cut video into individual clips
4. Thumbnail Generation — extract representative frame per clip
5. Metadata Save — store clip info in SQLite

**v2 (Future) — AI-powered splitting:**
1. Product Detection (Vision AI) — identify when new products appear
2. Highlight Detection — audio energy, visual engagement, gesture analysis
3. Smart Clip Boundaries — cut at natural points (not mid-sentence)
4. Product Tagging — auto-tag products mentioned/shown
5. Quality Scoring — rate clips by engagement potential

Plugin architecture: AI module plugs into the processing pipeline without changing core logic.

#### Phase 4: Ready

Clips available in the UI library for:
- Preview and review
- Manual trim/adjust
- Product tagging
- Status management (Draft → Ready → Posted)
- Export (future: auto-post to TikTok)

---

## 6. Data Model

### SQLite Schema

**accounts**
- id (PK), username, display_name, avatar_url
- type (own / monitored), tiktok_uid
- cookies_json, proxy_url
- auto_record (bool), auto_record_schedule (JSON: `{"days": [0-6], "start_time": "HH:MM", "end_time": "HH:MM"}`), priority (int, higher = more important)
- is_live, last_live_at, last_checked_at
- notes, created_at, updated_at

**recordings**
- id (PK), account_id (FK → accounts)
- room_id, status (recording / done / error / processing)
- started_at, ended_at, duration_seconds
- file_path, file_size_bytes, stream_url, bitrate
- error_message, auto_process (bool)
- created_at

**clips**
- id (PK), recording_id (FK → recordings), account_id (FK → accounts)
- title, file_path, thumbnail_path
- duration_seconds, file_size_bytes
- start_time (offset in source), end_time
- status (draft / ready / posted / archived)
- quality_score, scene_type (product_intro / highlight / general)
- ai_tags_json, notes
- created_at, updated_at

**products**
- id (PK), name, description, sku
- image_url, tiktok_shop_id, price, category
- created_at

**clip_products** (N:M join table)
- clip_id (FK → clips), product_id (FK → products)

**notifications**
- id (PK), type, title, message
- account_id (FK), recording_id (FK), clip_id (FK)
- is_read, created_at

**app_settings**
- key (PK), value (TEXT), updated_at

### File System Structure

```
~/TikTokApp/
├── data/
│   └── app.db                  # SQLite database
├── recordings/
│   └── {username}/{date}/      # Raw stream files
│       ├── {HHMMSS}.flv
│       └── {HHMMSS}.json       # Recording metadata
├── clips/
│   └── {username}/{date}/      # Generated clips
│       ├── clip_{NNN}.mp4
│       └── clip_{NNN}_thumb.jpg
├── products/images/            # Product images cache
├── exports/reports/            # CSV/PDF exports
├── logs/
│   ├── app.log
│   ├── sidecar.log
│   └── ffmpeg/                 # Per-recording FFmpeg logs
└── config/
    ├── settings.json           # App configuration
    └── cookies/{account}.json  # Per-account cookies
```

### Storage Management

- **Raw recordings**: auto-delete after processing into clips, or retain for N days (configurable, default 7)
- **Archived clips**: clips marked "posted" move to archive, archive auto-deletes after 30 days
- **Storage limit**: configurable max storage (default 100GB), warning at 80%, auto-cleanup oldest at 95%
- **Portable**: entire directory can be copied to another machine

---

## 7. App Settings

```json
{
  "general": {
    "storage_path": "~/TikTokApp",
    "max_storage_gb": 100,
    "language": "vi",
    "start_minimized": false,
    "start_on_boot": false
  },
  "recording": {
    "max_concurrent": 5,
    "max_duration_hours": 4,
    "max_file_size_gb": 4,
    "default_bitrate": "1M",
    "auto_process_after_record": true,
    "poll_interval_seconds": 30,
    "retry_attempts": 3
  },
  "processing": {
    "clip_min_duration_seconds": 15,
    "clip_max_duration_seconds": 90,
    "scene_threshold": 30.0,
    "generate_thumbnails": true,
    "auto_cleanup_raw": true,
    "raw_retention_days": 7
  },
  "notifications": {
    "on_live_detected": true,
    "on_recording_complete": true,
    "on_clips_ready": true,
    "on_error": true,
    "sound_enabled": true
  },
  "sidecar": {
    "port": 18321,
    "port_fallback_range": [18322, 18330],
    "host": "127.0.0.1",
    "log_level": "info"
  }
}
```

---

## 8. Phasing & MVP Strategy

### Phase 1: MVP — Core Recording & Management (3-4 weeks)

**Goal:** Working tool that replaces manual workflow.

Deliverables:
- Tauri v2 + React + TypeScript app shell with sidebar nav, dark theme, system tray
- Auto-start/stop Python sidecar
- SQLite database with migrations
- Account CRUD (own/monitored), cookie import, proxy config, live status polling
- Manual and auto recording with concurrent worker pool
- Realtime progress via WebSocket
- Auto-retry on disconnect, graceful stop
- Scene-based auto-split (PySceneDetect) + thumbnail generation
- Clips list with status
- OS notifications (live detected, recording complete)

### Phase 2: Smart Clips & Polish (2-3 weeks)

**Goal:** Daily driver with smooth workflow.

Deliverables:
- In-app video player with preview and manual trim
- Clip management: grid/list view, filters, batch operations
- Product catalog CRUD + manual product tagging on clips
- Status workflow: Draft → Ready → Posted
- Realtime dashboard updates
- Storage management with auto-cleanup policies

### Phase 3: AI Integration (3-4 weeks)

**Goal:** AI-powered intelligent clip generation.

Deliverables:
- Vision AI product detection + auto-tagging
- Highlight detection (audio energy, visual engagement)
- Quality scoring per clip
- Smart clip boundaries
- Statistics dashboard with charts
- Export CSV/PDF reports

### Phase 4: Auto-Post & Scale (3-4 weeks)

**Goal:** Full automation from recording to posting.

Deliverables:
- Auto-upload video to TikTok
- Auto-attach products from TikTok Shop
- Scheduled posting with caption/hashtag generation
- Remote recording (network sidecar)
- Team collaboration features
- Plugin system

---

## 9. Reference

- Recording engine reference: [tiktok-live-recorder](https://github.com/Michele0303/tiktok-live-recorder) (Python, FFmpeg-based, supports username/URL/room_id, automatic mode with polling)
- Scene detection: [PySceneDetect](https://github.com/Breakthrough/PySceneDetect)
- Desktop framework: [Tauri v2](https://v2.tauri.app/)
- UI components: [shadcn/ui](https://ui.shadcn.com/)
