# TikClip Phase 1 (MVP) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a working desktop app that records TikTok live streams, auto-splits into clips, and manages multiple accounts — replacing the manual recording/editing workflow.

**Architecture:** Tauri v2 desktop app (Rust backend + React frontend) communicates with a Python sidecar service (FastAPI) via HTTP REST + WebSocket. The sidecar handles recording (FFmpeg) and video processing (PySceneDetect). SQLite stores all metadata.

**Tech Stack:** Tauri v2, React 19, TypeScript, shadcn/ui, Tailwind CSS, Zustand, TanStack Query, TanStack Router, Rust/rusqlite, Python 3.11+, FastAPI, FFmpeg, PySceneDetect

---

## File Structure

```
tikclip/
├── package.json
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.ts
├── components.json                  # shadcn config
├── index.html
│
├── src/                             # React frontend
│   ├── main.tsx
│   ├── App.tsx
│   ├── index.css
│   ├── types/
│   │   └── index.ts                 # Shared TypeScript types
│   ├── lib/
│   │   ├── api.ts                   # HTTP client for sidecar REST API
│   │   ├── ws.ts                    # WebSocket client for sidecar events
│   │   └── utils.ts                 # Formatters, helpers
│   ├── stores/
│   │   ├── app-store.ts             # Global app state (sidecar status, theme)
│   │   ├── account-store.ts         # Account state
│   │   ├── recording-store.ts       # Recording state
│   │   ├── clip-store.ts            # Clip state
│   │   └── notification-store.ts    # Notification queue
│   ├── hooks/
│   │   ├── use-sidecar.ts           # Sidecar connection hook
│   │   └── use-websocket.ts         # WebSocket subscription hook
│   ├── components/
│   │   ├── ui/                      # shadcn/ui components (auto-generated)
│   │   ├── layout/
│   │   │   ├── app-shell.tsx        # Main layout wrapper
│   │   │   ├── sidebar.tsx          # Left sidebar navigation
│   │   │   └── top-bar.tsx          # Top bar with search + notifications
│   │   ├── dashboard/
│   │   │   ├── stat-cards.tsx       # 4 stat summary cards
│   │   │   ├── active-recordings.tsx# Active recording list widget
│   │   │   ├── live-accounts.tsx    # Monitored accounts currently live
│   │   │   └── recent-clips.tsx     # Recent clips widget
│   │   ├── accounts/
│   │   │   ├── account-list.tsx     # Account table/list
│   │   │   ├── account-form.tsx     # Add/edit account dialog
│   │   │   └── account-badge.tsx    # Live/Offline/Recording badge
│   │   ├── recordings/
│   │   │   ├── recording-list.tsx   # Recording table with controls
│   │   │   ├── recording-progress.tsx # Realtime progress bar
│   │   │   └── recording-controls.tsx # Start/Stop/Retry buttons
│   │   ├── clips/
│   │   │   ├── clip-grid.tsx        # Grid/List view of clips
│   │   │   └── clip-card.tsx        # Individual clip card with thumbnail
│   │   └── notifications/
│   │       └── notification-toast.tsx # Toast notification component
│   └── pages/
│       ├── dashboard.tsx
│       ├── accounts.tsx
│       ├── recordings.tsx
│       ├── clips.tsx
│       └── settings.tsx
│
├── src-tauri/                       # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/
│   │   └── default.json
│   ├── icons/                       # App icons
│   ├── src/
│   │   ├── main.rs                  # Entry point
│   │   ├── lib.rs                   # Tauri app builder + plugin registration
│   │   ├── db/
│   │   │   ├── mod.rs               # Database module
│   │   │   ├── init.rs              # DB initialization + migrations
│   │   │   └── models.rs            # Rust structs for DB rows
│   │   ├── commands/
│   │   │   ├── mod.rs               # Re-exports all commands
│   │   │   ├── accounts.rs          # Account CRUD commands
│   │   │   ├── recordings.rs        # Recording commands
│   │   │   ├── clips.rs             # Clip commands
│   │   │   └── settings.rs          # Settings commands
│   │   ├── sidecar/
│   │   │   └── mod.rs               # Start/stop/health-check Python sidecar
│   │   └── tray.rs                  # System tray setup
│
├── sidecar/                         # Python sidecar service
│   ├── pyproject.toml
│   ├── src/
│   │   ├── __init__.py
│   │   ├── main.py                  # Entry point (uvicorn startup)
│   │   ├── app.py                   # FastAPI app factory
│   │   ├── config.py                # Configuration (port, paths, limits)
│   │   ├── routes/
│   │   │   ├── __init__.py
│   │   │   ├── health.py            # GET /api/health
│   │   │   ├── accounts.py          # Account status routes
│   │   │   ├── recordings.py        # Recording control routes
│   │   │   └── clips.py             # Clip listing routes
│   │   ├── ws/
│   │   │   ├── __init__.py
│   │   │   └── manager.py           # WebSocket connection manager + event broadcast
│   │   ├── core/
│   │   │   ├── __init__.py
│   │   │   ├── watcher.py           # Account live-status poller
│   │   │   ├── recorder.py          # Recording manager (worker pool)
│   │   │   ├── worker.py            # Single recording worker (FFmpeg subprocess)
│   │   │   └── processor.py         # Video processor (scene detect + clip extraction)
│   │   ├── tiktok/
│   │   │   ├── __init__.py
│   │   │   ├── api.py               # TikTok API client (check live, get room_id)
│   │   │   └── stream.py            # Stream URL resolver (FLV/HLS URL extraction)
│   │   └── models/
│   │       ├── __init__.py
│   │       └── schemas.py           # Pydantic request/response models
│   └── tests/
│       ├── __init__.py
│       ├── conftest.py              # Shared fixtures
│       ├── test_health.py
│       ├── test_tiktok_api.py
│       ├── test_watcher.py
│       ├── test_recorder.py
│       ├── test_worker.py
│       └── test_processor.py
│
└── docs/
    └── superpowers/
        ├── specs/
        │   └── 2026-04-05-tikclip-desktop-app-design.md
        └── plans/
            └── 2026-04-05-tikclip-phase1-mvp.md   # This file
```

---

## Task 1: Project Scaffolding — Tauri + React + Tailwind

**Files:**
- Create: `package.json`, `vite.config.ts`, `tsconfig.json`, `tailwind.config.ts`, `index.html`, `components.json`
- Create: `src/main.tsx`, `src/App.tsx`, `src/index.css`
- Create: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`
- Create: `src-tauri/capabilities/default.json`

- [ ] **Step 1: Create Tauri v2 project**

```bash
npm create tauri-app@latest tikclip -- --template react-ts --manager npm
cd tikclip
```

If the CLI doesn't support `--template` directly, use interactive mode and select React + TypeScript.

- [ ] **Step 2: Install frontend dependencies**

```bash
npm install zustand @tanstack/react-query @tanstack/react-router recharts video.js
npm install -D tailwindcss @tailwindcss/vite
```

- [ ] **Step 3: Configure Tailwind CSS**

Replace `src/index.css` with:

```css
@import "tailwindcss";

:root {
  --color-primary: #fe2c55;
  --color-accent: #25f4ee;
  --color-bg: #0a0a0f;
  --color-surface: #111118;
  --color-border: #222222;
  --color-text: #e0e0e0;
  --color-text-muted: #888888;
}

body {
  background-color: var(--color-bg);
  color: var(--color-text);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  margin: 0;
  min-height: 100vh;
  overflow: hidden;
}

* {
  scrollbar-width: thin;
  scrollbar-color: #333 transparent;
}
```

Update `vite.config.ts`:

```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
}));
```

- [ ] **Step 4: Initialize shadcn/ui**

```bash
npx shadcn@latest init
```

Select: New York style, Zinc base color, CSS variables. Then install core components:

```bash
npx shadcn@latest add button card dialog input label table tabs toast badge scroll-area separator dropdown-menu
```

- [ ] **Step 5: Create minimal App.tsx**

Replace `src/App.tsx`:

```tsx
function App() {
  return (
    <div className="flex h-screen bg-[var(--color-bg)] text-[var(--color-text)]">
      <aside className="w-[220px] bg-[var(--color-surface)] border-r border-[var(--color-border)]">
        <div className="p-5 border-b border-[var(--color-border)]">
          <h1 className="text-lg font-bold">TikClip</h1>
          <p className="text-xs text-[var(--color-text-muted)]">Live Recorder</p>
        </div>
      </aside>
      <main className="flex-1 p-6">
        <h2 className="text-xl">Dashboard</h2>
        <p className="text-[var(--color-text-muted)]">App is running.</p>
      </main>
    </div>
  );
}

export default App;
```

- [ ] **Step 6: Configure Tauri capabilities**

Update `src-tauri/capabilities/default.json` to include needed permissions:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capabilities for TikClip",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "shell:allow-open",
    "notification:default",
    "dialog:default"
  ]
}
```

- [ ] **Step 7: Verify build and run**

```bash
npm run tauri dev
```

Expected: Tauri window opens showing dark sidebar with "TikClip" + main area with "Dashboard / App is running."

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: scaffold Tauri v2 + React + TypeScript + Tailwind + shadcn/ui"
```

---

## Task 2: App Shell — Sidebar, Routing, Layout

**Files:**
- Create: `src/components/layout/app-shell.tsx`
- Create: `src/components/layout/sidebar.tsx`
- Create: `src/components/layout/top-bar.tsx`
- Create: `src/pages/dashboard.tsx`, `src/pages/accounts.tsx`, `src/pages/recordings.tsx`, `src/pages/clips.tsx`, `src/pages/settings.tsx`
- Create: `src/types/index.ts`
- Modify: `src/App.tsx`, `src/main.tsx`

- [ ] **Step 1: Create TypeScript types**

Create `src/types/index.ts`:

```typescript
export type AccountType = "own" | "monitored";
export type AccountStatus = "live" | "offline" | "recording";
export type RecordingStatus = "recording" | "done" | "error" | "processing";
export type ClipStatus = "draft" | "ready" | "posted" | "archived";
export type SceneType = "product_intro" | "highlight" | "general";

export interface Account {
  id: number;
  username: string;
  display_name: string;
  avatar_url: string | null;
  type: AccountType;
  tiktok_uid: string | null;
  auto_record: boolean;
  auto_record_schedule: AutoRecordSchedule | null;
  priority: number;
  is_live: boolean;
  last_live_at: string | null;
  last_checked_at: string | null;
  proxy_url: string | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface AutoRecordSchedule {
  days: number[];
  start_time: string;
  end_time: string;
}

export interface Recording {
  id: number;
  account_id: number;
  account_username?: string;
  room_id: string | null;
  status: RecordingStatus;
  started_at: string;
  ended_at: string | null;
  duration_seconds: number;
  file_path: string | null;
  file_size_bytes: number;
  stream_url: string | null;
  bitrate: string | null;
  error_message: string | null;
  auto_process: boolean;
  created_at: string;
}

export interface Clip {
  id: number;
  recording_id: number;
  account_id: number;
  account_username?: string;
  title: string | null;
  file_path: string;
  thumbnail_path: string | null;
  duration_seconds: number;
  file_size_bytes: number;
  start_time: number;
  end_time: number;
  status: ClipStatus;
  quality_score: number | null;
  scene_type: SceneType | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface SidecarStatus {
  connected: boolean;
  port: number | null;
  active_recordings: number;
}

export interface WsEvent {
  type: string;
  data: Record<string, unknown>;
  timestamp: number;
}
```

- [ ] **Step 2: Create Sidebar component**

Create `src/components/layout/sidebar.tsx`:

```tsx
import { useState } from "react";
import { Badge } from "@/components/ui/badge";

const navItems = [
  { id: "dashboard", label: "Dashboard", icon: "📊" },
  { id: "accounts", label: "Accounts", icon: "👤" },
  { id: "recordings", label: "Recordings", icon: "🔴" },
  { id: "clips", label: "Clips", icon: "✂️" },
  { id: "statistics", label: "Statistics", icon: "📈" },
] as const;

type PageId = (typeof navItems)[number]["id"] | "settings";

interface SidebarProps {
  currentPage: PageId;
  onNavigate: (page: PageId) => void;
  sidecarConnected: boolean;
  activeRecordings: number;
}

export function Sidebar({ currentPage, onNavigate, sidecarConnected, activeRecordings }: SidebarProps) {
  return (
    <aside className="w-[220px] bg-[var(--color-surface)] border-r border-[var(--color-border)] flex flex-col">
      <div className="p-5 border-b border-[var(--color-border)] flex items-center gap-3">
        <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-[var(--color-primary)] to-[var(--color-accent)] flex items-center justify-center text-base">
          🎬
        </div>
        <div>
          <div className="font-bold text-white text-sm">TikClip</div>
          <div className="text-[10px] text-[var(--color-text-muted)]">Live Recorder</div>
        </div>
      </div>

      <nav className="flex-1 p-2 space-y-1">
        {navItems.map((item) => (
          <button
            key={item.id}
            onClick={() => onNavigate(item.id)}
            className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-colors ${
              currentPage === item.id
                ? "bg-[var(--color-primary)]/20 text-[var(--color-primary)] font-semibold"
                : "text-[var(--color-text-muted)] hover:bg-white/5"
            }`}
          >
            <span>{item.icon}</span>
            <span>{item.label}</span>
            {item.id === "recordings" && activeRecordings > 0 && (
              <Badge variant="destructive" className="ml-auto text-[10px] px-2 py-0">
                {activeRecordings}
              </Badge>
            )}
          </button>
        ))}

        <div className="pt-4 mt-4 border-t border-[var(--color-border)]">
          <button
            onClick={() => onNavigate("settings")}
            className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-colors ${
              currentPage === "settings"
                ? "bg-[var(--color-primary)]/20 text-[var(--color-primary)] font-semibold"
                : "text-[var(--color-text-muted)] hover:bg-white/5"
            }`}
          >
            <span>⚙️</span>
            <span>Settings</span>
          </button>
        </div>
      </nav>

      <div className="p-4 border-t border-[var(--color-border)] text-xs">
        <div className="flex items-center gap-1.5 mb-1">
          <div className={`w-2 h-2 rounded-full ${sidecarConnected ? "bg-green-500" : "bg-red-500"}`} />
          <span className={sidecarConnected ? "text-green-500" : "text-red-500"}>
            Sidecar: {sidecarConnected ? "Connected" : "Disconnected"}
          </span>
        </div>
        {activeRecordings > 0 && (
          <div className="text-[var(--color-text-muted)]">{activeRecordings} active recordings</div>
        )}
      </div>
    </aside>
  );
}
```

- [ ] **Step 3: Create TopBar component**

Create `src/components/layout/top-bar.tsx`:

```tsx
interface TopBarProps {
  title: string;
  subtitle?: string;
}

export function TopBar({ title, subtitle }: TopBarProps) {
  return (
    <div className="px-6 py-4 border-b border-[var(--color-border)] flex items-center justify-between">
      <div>
        <h2 className="text-lg font-semibold text-white">{title}</h2>
        {subtitle && <p className="text-xs text-[var(--color-text-muted)] mt-0.5">{subtitle}</p>}
      </div>
      <div className="flex gap-2 items-center">
        <div className="px-3 py-1.5 rounded-md border border-[var(--color-border)] text-[var(--color-text-muted)] text-xs">
          🔍 Search...
        </div>
        <button className="w-8 h-8 rounded-md bg-[var(--color-surface)] flex items-center justify-center border border-[var(--color-border)]">
          🔔
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Create page stubs**

Create `src/pages/dashboard.tsx`:

```tsx
export function DashboardPage() {
  return (
    <div>
      <p className="text-[var(--color-text-muted)]">Dashboard content coming in Task 10.</p>
    </div>
  );
}
```

Create `src/pages/accounts.tsx`:

```tsx
export function AccountsPage() {
  return (
    <div>
      <p className="text-[var(--color-text-muted)]">Account management coming in Task 7.</p>
    </div>
  );
}
```

Create `src/pages/recordings.tsx`:

```tsx
export function RecordingsPage() {
  return (
    <div>
      <p className="text-[var(--color-text-muted)]">Recording management coming in Task 8.</p>
    </div>
  );
}
```

Create `src/pages/clips.tsx`:

```tsx
export function ClipsPage() {
  return (
    <div>
      <p className="text-[var(--color-text-muted)]">Clip management coming in Task 9.</p>
    </div>
  );
}
```

Create `src/pages/settings.tsx`:

```tsx
export function SettingsPage() {
  return (
    <div>
      <p className="text-[var(--color-text-muted)]">Settings coming in Task 11.</p>
    </div>
  );
}
```

- [ ] **Step 5: Create AppShell and wire up routing**

Create `src/components/layout/app-shell.tsx`:

```tsx
import { useState } from "react";
import { Sidebar } from "./sidebar";
import { TopBar } from "./top-bar";
import { DashboardPage } from "@/pages/dashboard";
import { AccountsPage } from "@/pages/accounts";
import { RecordingsPage } from "@/pages/recordings";
import { ClipsPage } from "@/pages/clips";
import { SettingsPage } from "@/pages/settings";

type PageId = "dashboard" | "accounts" | "recordings" | "clips" | "statistics" | "settings";

const pageMeta: Record<PageId, { title: string; subtitle: string }> = {
  dashboard: { title: "Dashboard", subtitle: "Overview of all activities" },
  accounts: { title: "Accounts", subtitle: "Manage TikTok accounts" },
  recordings: { title: "Recordings", subtitle: "Active and completed recordings" },
  clips: { title: "Clips", subtitle: "Generated video clips" },
  statistics: { title: "Statistics", subtitle: "Analytics and reports" },
  settings: { title: "Settings", subtitle: "App configuration" },
};

const pageComponents: Record<PageId, React.FC> = {
  dashboard: DashboardPage,
  accounts: AccountsPage,
  recordings: RecordingsPage,
  clips: ClipsPage,
  statistics: () => <p className="text-[var(--color-text-muted)]">Statistics coming in Phase 3.</p>,
  settings: SettingsPage,
};

export function AppShell() {
  const [currentPage, setCurrentPage] = useState<PageId>("dashboard");

  const meta = pageMeta[currentPage];
  const PageComponent = pageComponents[currentPage];

  return (
    <div className="flex h-screen bg-[var(--color-bg)] text-[var(--color-text)]">
      <Sidebar
        currentPage={currentPage}
        onNavigate={setCurrentPage}
        sidecarConnected={false}
        activeRecordings={0}
      />
      <div className="flex-1 flex flex-col overflow-hidden">
        <TopBar title={meta.title} subtitle={meta.subtitle} />
        <main className="flex-1 overflow-y-auto p-6">
          <PageComponent />
        </main>
      </div>
    </div>
  );
}
```

Update `src/App.tsx`:

```tsx
import { AppShell } from "@/components/layout/app-shell";

function App() {
  return <AppShell />;
}

export default App;
```

- [ ] **Step 6: Verify navigation works**

```bash
npm run tauri dev
```

Expected: Sidebar with all nav items visible. Clicking each item changes the page title and content area. Active page highlighted in red.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add app shell with sidebar navigation, routing, and page stubs"
```

---

## Task 3: SQLite Database — Rust Backend

**Files:**
- Create: `src-tauri/src/db/mod.rs`
- Create: `src-tauri/src/db/init.rs`
- Create: `src-tauri/src/db/models.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add Rust dependencies**

Add to `src-tauri/Cargo.toml` under `[dependencies]`:

```toml
rusqlite = { version = "0.31", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
```

- [ ] **Step 2: Create database models**

Create `src-tauri/src/db/models.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Account {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    #[serde(rename = "type")]
    pub account_type: String, // "own" | "monitored"
    pub tiktok_uid: Option<String>,
    pub cookies_json: Option<String>,
    pub proxy_url: Option<String>,
    pub auto_record: bool,
    pub auto_record_schedule: Option<String>, // JSON string
    pub priority: i32,
    pub is_live: bool,
    pub last_live_at: Option<String>,
    pub last_checked_at: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Recording {
    pub id: i64,
    pub account_id: i64,
    pub account_username: Option<String>,
    pub room_id: Option<String>,
    pub status: String, // "recording" | "done" | "error" | "processing"
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: i64,
    pub file_path: Option<String>,
    pub file_size_bytes: i64,
    pub stream_url: Option<String>,
    pub bitrate: Option<String>,
    pub error_message: Option<String>,
    pub auto_process: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Clip {
    pub id: i64,
    pub recording_id: i64,
    pub account_id: i64,
    pub account_username: Option<String>,
    pub title: Option<String>,
    pub file_path: String,
    pub thumbnail_path: Option<String>,
    pub duration_seconds: i64,
    pub file_size_bytes: i64,
    pub start_time: f64,
    pub end_time: f64,
    pub status: String, // "draft" | "ready" | "posted" | "archived"
    pub quality_score: Option<f64>,
    pub scene_type: Option<String>,
    pub ai_tags_json: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Notification {
    pub id: i64,
    pub notification_type: String,
    pub title: String,
    pub message: String,
    pub account_id: Option<i64>,
    pub recording_id: Option<i64>,
    pub clip_id: Option<i64>,
    pub is_read: bool,
    pub created_at: String,
}
```

- [ ] **Step 3: Create database initialization with migrations**

Create `src-tauri/src/db/init.rs`:

```rust
use rusqlite::Connection;
use std::path::Path;

pub fn initialize_database(db_path: &Path) -> Result<Connection, rusqlite::Error> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    run_migrations(&conn)?;
    Ok(conn)
}

fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY
        );",
    )?;

    let current_version: i64 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |row| row.get(0))
        .unwrap_or(0);

    let migrations: Vec<(i64, &str)> = vec![
        (1, include_str!("migrations/001_initial.sql")),
    ];

    for (version, sql) in migrations {
        if version > current_version {
            conn.execute_batch(sql)?;
            conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [version])?;
        }
    }

    Ok(())
}
```

Create `src-tauri/src/db/migrations/001_initial.sql`:

```sql
CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL DEFAULT '',
    avatar_url TEXT,
    type TEXT NOT NULL DEFAULT 'monitored' CHECK (type IN ('own', 'monitored')),
    tiktok_uid TEXT,
    cookies_json TEXT,
    proxy_url TEXT,
    auto_record INTEGER NOT NULL DEFAULT 0,
    auto_record_schedule TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    is_live INTEGER NOT NULL DEFAULT 0,
    last_live_at TEXT,
    last_checked_at TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS recordings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    room_id TEXT,
    status TEXT NOT NULL DEFAULT 'recording' CHECK (status IN ('recording', 'done', 'error', 'processing')),
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    ended_at TEXT,
    duration_seconds INTEGER NOT NULL DEFAULT 0,
    file_path TEXT,
    file_size_bytes INTEGER NOT NULL DEFAULT 0,
    stream_url TEXT,
    bitrate TEXT,
    error_message TEXT,
    auto_process INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS clips (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recording_id INTEGER NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    title TEXT,
    file_path TEXT NOT NULL,
    thumbnail_path TEXT,
    duration_seconds INTEGER NOT NULL DEFAULT 0,
    file_size_bytes INTEGER NOT NULL DEFAULT 0,
    start_time REAL NOT NULL DEFAULT 0,
    end_time REAL NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'ready', 'posted', 'archived')),
    quality_score REAL,
    scene_type TEXT CHECK (scene_type IN ('product_intro', 'highlight', 'general')),
    ai_tags_json TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT,
    sku TEXT,
    image_url TEXT,
    tiktok_shop_id TEXT,
    price REAL,
    category TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS clip_products (
    clip_id INTEGER NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
    product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    PRIMARY KEY (clip_id, product_id)
);

CREATE TABLE IF NOT EXISTS notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    type TEXT NOT NULL,
    title TEXT NOT NULL,
    message TEXT NOT NULL DEFAULT '',
    account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
    recording_id INTEGER REFERENCES recordings(id) ON DELETE SET NULL,
    clip_id INTEGER REFERENCES clips(id) ON DELETE SET NULL,
    is_read INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_recordings_account ON recordings(account_id);
CREATE INDEX IF NOT EXISTS idx_recordings_status ON recordings(status);
CREATE INDEX IF NOT EXISTS idx_clips_recording ON clips(recording_id);
CREATE INDEX IF NOT EXISTS idx_clips_account ON clips(account_id);
CREATE INDEX IF NOT EXISTS idx_clips_status ON clips(status);
CREATE INDEX IF NOT EXISTS idx_notifications_read ON notifications(is_read);
```

- [ ] **Step 4: Create db module**

Create `src-tauri/src/db/mod.rs`:

```rust
pub mod init;
pub mod models;
```

- [ ] **Step 5: Wire database into Tauri app**

Update `src-tauri/src/lib.rs`:

```rust
mod db;

use db::init::initialize_database;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub storage_path: PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir().expect("failed to get app data dir");
            let storage_path = app_data.join("TikTokApp");
            std::fs::create_dir_all(&storage_path).ok();

            let db_path = storage_path.join("data").join("app.db");
            let conn = initialize_database(&db_path).expect("failed to initialize database");

            app.manage(AppState {
                db: Mutex::new(conn),
                storage_path,
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: Verify database creates on startup**

```bash
npm run tauri dev
```

Expected: App starts without errors. SQLite database file created at the app data directory. Check console for no panics/errors.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add SQLite database with schema migrations for accounts, recordings, clips"
```

---

## Task 4: Python Sidecar — FastAPI Server + WebSocket

**Files:**
- Create: `sidecar/pyproject.toml`
- Create: `sidecar/src/__init__.py`, `sidecar/src/main.py`, `sidecar/src/app.py`, `sidecar/src/config.py`
- Create: `sidecar/src/routes/__init__.py`, `sidecar/src/routes/health.py`
- Create: `sidecar/src/ws/__init__.py`, `sidecar/src/ws/manager.py`
- Create: `sidecar/src/models/__init__.py`, `sidecar/src/models/schemas.py`
- Create: `sidecar/tests/__init__.py`, `sidecar/tests/conftest.py`, `sidecar/tests/test_health.py`

- [ ] **Step 1: Create pyproject.toml**

Create `sidecar/pyproject.toml`:

```toml
[project]
name = "tikclip-sidecar"
version = "0.1.0"
description = "TikClip recording and processing sidecar service"
requires-python = ">=3.11"
dependencies = [
    "fastapi>=0.115",
    "uvicorn[standard]>=0.34",
    "websockets>=14.0",
    "httpx>=0.28",
    "pydantic>=2.10",
    "pydantic-settings>=2.7",
]

[project.optional-dependencies]
dev = [
    "pytest>=8.0",
    "pytest-asyncio>=0.25",
    "httpx>=0.28",
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"
```

- [ ] **Step 2: Create config module**

Create `sidecar/src/__init__.py` (empty file).

Create `sidecar/src/config.py`:

```python
from pathlib import Path
from pydantic_settings import BaseSettings


class Settings(BaseSettings):
    host: str = "127.0.0.1"
    port: int = 18321
    port_fallback_range_start: int = 18322
    port_fallback_range_end: int = 18330
    storage_path: Path = Path.home() / "TikTokApp"
    log_level: str = "info"
    poll_interval_seconds: int = 30
    max_concurrent_recordings: int = 5
    max_duration_hours: int = 4
    max_file_size_gb: int = 4
    retry_attempts: int = 3
    clip_min_duration: int = 15
    clip_max_duration: int = 90
    scene_threshold: float = 30.0
    auto_process_after_record: bool = True
    auto_cleanup_raw: bool = True
    raw_retention_days: int = 7

    model_config = {"env_prefix": "TIKCLIP_"}


settings = Settings()
```

- [ ] **Step 3: Create WebSocket manager**

Create `sidecar/src/ws/__init__.py` (empty file).

Create `sidecar/src/ws/manager.py`:

```python
import asyncio
import json
import time
from fastapi import WebSocket


class ConnectionManager:
    def __init__(self):
        self._connections: list[WebSocket] = []
        self._lock = asyncio.Lock()

    async def connect(self, websocket: WebSocket):
        await websocket.accept()
        async with self._lock:
            self._connections.append(websocket)

    async def disconnect(self, websocket: WebSocket):
        async with self._lock:
            self._connections.remove(websocket)

    async def broadcast(self, event_type: str, data: dict):
        message = json.dumps({
            "type": event_type,
            "data": data,
            "timestamp": int(time.time()),
        })
        async with self._lock:
            stale = []
            for ws in self._connections:
                try:
                    await ws.send_text(message)
                except Exception:
                    stale.append(ws)
            for ws in stale:
                self._connections.remove(ws)

    @property
    def active_count(self) -> int:
        return len(self._connections)


ws_manager = ConnectionManager()
```

- [ ] **Step 4: Create Pydantic schemas**

Create `sidecar/src/models/__init__.py` (empty file).

Create `sidecar/src/models/schemas.py`:

```python
from pydantic import BaseModel


class HealthResponse(BaseModel):
    status: str = "ok"
    version: str = "0.1.0"
    active_recordings: int = 0
    ws_connections: int = 0


class AccountStatusRequest(BaseModel):
    username: str
    cookies_json: str | None = None
    proxy_url: str | None = None


class AccountStatusResponse(BaseModel):
    username: str
    is_live: bool
    room_id: str | None = None
    stream_url: str | None = None
    viewer_count: int | None = None


class StartRecordingRequest(BaseModel):
    account_id: int
    username: str
    room_id: str | None = None
    stream_url: str | None = None
    cookies_json: str | None = None
    proxy_url: str | None = None
    max_duration_seconds: int | None = None


class StopRecordingRequest(BaseModel):
    recording_id: str


class RecordingStatusResponse(BaseModel):
    recording_id: str
    account_id: int
    username: str
    status: str
    duration_seconds: int = 0
    file_size_bytes: int = 0
    file_path: str | None = None
    error_message: str | None = None


class ProcessVideoRequest(BaseModel):
    recording_id: int
    file_path: str
    account_id: int
    clip_min_duration: int = 15
    clip_max_duration: int = 90
    scene_threshold: float = 30.0
```

- [ ] **Step 5: Create health route**

Create `sidecar/src/routes/__init__.py` (empty file).

Create `sidecar/src/routes/health.py`:

```python
from fastapi import APIRouter
from ..models.schemas import HealthResponse
from ..ws.manager import ws_manager

router = APIRouter()


@router.get("/api/health", response_model=HealthResponse)
async def health_check():
    return HealthResponse(
        status="ok",
        version="0.1.0",
        active_recordings=0,
        ws_connections=ws_manager.active_count,
    )
```

- [ ] **Step 6: Create FastAPI app factory**

Create `sidecar/src/app.py`:

```python
from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
from .routes import health
from .ws.manager import ws_manager


def create_app() -> FastAPI:
    app = FastAPI(title="TikClip Sidecar", version="0.1.0")

    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],
        allow_methods=["*"],
        allow_headers=["*"],
    )

    app.include_router(health.router)

    @app.websocket("/ws")
    async def websocket_endpoint(websocket: WebSocket):
        await ws_manager.connect(websocket)
        try:
            while True:
                await websocket.receive_text()
        except WebSocketDisconnect:
            await ws_manager.disconnect(websocket)

    return app
```

- [ ] **Step 7: Create entry point**

Create `sidecar/src/main.py`:

```python
import socket
import sys

import uvicorn

from .app import create_app
from .config import settings


def find_available_port() -> int:
    if is_port_available(settings.port):
        return settings.port
    for port in range(settings.port_fallback_range_start, settings.port_fallback_range_end + 1):
        if is_port_available(port):
            return port
    raise RuntimeError("No available port found in configured range")


def is_port_available(port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        try:
            s.bind((settings.host, port))
            return True
        except OSError:
            return False


def main():
    port = find_available_port()
    print(f"SIDECAR_PORT={port}", flush=True)

    app = create_app()
    uvicorn.run(app, host=settings.host, port=port, log_level=settings.log_level)


if __name__ == "__main__":
    main()
```

- [ ] **Step 8: Write health endpoint test**

Create `sidecar/tests/__init__.py` (empty file).

Create `sidecar/tests/conftest.py`:

```python
import pytest
from fastapi.testclient import TestClient
from src.app import create_app


@pytest.fixture
def client():
    app = create_app()
    return TestClient(app)
```

Create `sidecar/tests/test_health.py`:

```python
def test_health_returns_ok(client):
    response = client.get("/api/health")
    assert response.status_code == 200
    data = response.json()
    assert data["status"] == "ok"
    assert data["version"] == "0.1.0"
    assert data["active_recordings"] == 0
```

- [ ] **Step 9: Run test**

```bash
cd sidecar
pip install -e ".[dev]"
pytest tests/test_health.py -v
```

Expected: 1 test passes.

- [ ] **Step 10: Commit**

```bash
cd ..
git add -A
git commit -m "feat: add Python sidecar with FastAPI server, WebSocket manager, and health endpoint"
```

---

## Task 5: Sidecar Manager — Rust Auto-Start/Stop

**Files:**
- Create: `src-tauri/src/sidecar/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src/hooks/use-sidecar.ts`
- Create: `src/stores/app-store.ts`
- Modify: `src/components/layout/app-shell.tsx`

- [ ] **Step 1: Create Rust sidecar manager**

Create `src-tauri/src/sidecar/mod.rs`:

```rust
use std::io::{BufRead, BufReader};
use std::process::{Child, Command};
use std::sync::Mutex;

pub struct SidecarManager {
    process: Mutex<Option<Child>>,
    port: Mutex<Option<u16>>,
}

impl SidecarManager {
    pub fn new() -> Self {
        Self {
            process: Mutex::new(None),
            port: Mutex::new(None),
        }
    }

    pub fn start(&self, sidecar_dir: &std::path::Path) -> Result<u16, String> {
        let mut proc_guard = self.process.lock().map_err(|e| e.to_string())?;

        if proc_guard.is_some() {
            if let Some(port) = *self.port.lock().map_err(|e| e.to_string())? {
                return Ok(port);
            }
        }

        let child = Command::new("python")
            .args(["-m", "src.main"])
            .current_dir(sidecar_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start sidecar: {}", e))?;

        let stdout = child.stdout.as_ref().ok_or("No stdout")?;
        let reader = BufReader::new(stdout);

        let mut port: Option<u16> = None;
        for line in reader.lines().take(10) {
            if let Ok(line) = line {
                if line.starts_with("SIDECAR_PORT=") {
                    port = line.trim_start_matches("SIDECAR_PORT=").parse().ok();
                    break;
                }
            }
        }

        let port = port.ok_or("Failed to read sidecar port from stdout")?;

        *proc_guard = Some(child);
        *self.port.lock().map_err(|e| e.to_string())? = Some(port);

        Ok(port)
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut proc_guard = self.process.lock().map_err(|e| e.to_string())?;
        if let Some(ref mut child) = *proc_guard {
            child.kill().map_err(|e| format!("Failed to kill sidecar: {}", e))?;
            child.wait().ok();
        }
        *proc_guard = None;
        *self.port.lock().map_err(|e| e.to_string())? = None;
        Ok(())
    }

    pub fn port(&self) -> Option<u16> {
        self.port.lock().ok().and_then(|p| *p)
    }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        self.stop().ok();
    }
}
```

Note: This is a simplified version. The stdout reading for port detection is tricky because `child.stdout` ownership moves. A more robust implementation would use a thread to read stdout. For MVP, we will use a health-check polling approach instead:

Replace the `start` method internals — spawn the process, then poll `http://127.0.0.1:{port}/api/health` in a loop until it responds (max 10 seconds). The sidecar prints its port to stdout on the first line.

- [ ] **Step 2: Create Tauri commands for sidecar**

Add to `src-tauri/src/lib.rs`:

```rust
mod sidecar;

use sidecar::SidecarManager;

#[tauri::command]
async fn get_sidecar_status(
    sidecar: tauri::State<'_, SidecarManager>,
) -> Result<serde_json::Value, String> {
    let port = sidecar.port();
    Ok(serde_json::json!({
        "connected": port.is_some(),
        "port": port,
    }))
}
```

Register the command and sidecar in `setup`:

```rust
.setup(|app| {
    // ... database setup ...

    let sidecar = SidecarManager::new();
    // Attempt to start sidecar
    let sidecar_dir = std::env::current_dir()
        .unwrap_or_default()
        .join("sidecar");
    match sidecar.start(&sidecar_dir) {
        Ok(port) => println!("Sidecar started on port {}", port),
        Err(e) => eprintln!("Sidecar start failed: {}", e),
    }
    app.manage(sidecar);

    Ok(())
})
.invoke_handler(tauri::generate_handler![get_sidecar_status])
```

- [ ] **Step 3: Create frontend sidecar hook**

Create `src/stores/app-store.ts`:

```typescript
import { create } from "zustand";
import type { SidecarStatus } from "@/types";

interface AppState {
  sidecar: SidecarStatus;
  setSidecarStatus: (status: Partial<SidecarStatus>) => void;
}

export const useAppStore = create<AppState>((set) => ({
  sidecar: { connected: false, port: null, active_recordings: 0 },
  setSidecarStatus: (status) =>
    set((state) => ({ sidecar: { ...state.sidecar, ...status } })),
}));
```

Create `src/hooks/use-sidecar.ts`:

```typescript
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "@/stores/app-store";

export function useSidecar() {
  const { sidecar, setSidecarStatus } = useAppStore();

  useEffect(() => {
    const checkStatus = async () => {
      try {
        const status = await invoke<{ connected: boolean; port: number | null }>(
          "get_sidecar_status"
        );
        setSidecarStatus({ connected: status.connected, port: status.port });
      } catch {
        setSidecarStatus({ connected: false, port: null });
      }
    };

    checkStatus();
    const interval = setInterval(checkStatus, 5000);
    return () => clearInterval(interval);
  }, [setSidecarStatus]);

  return sidecar;
}
```

- [ ] **Step 4: Wire sidecar status into AppShell**

Update `src/components/layout/app-shell.tsx` — import and use the hook:

```tsx
import { useSidecar } from "@/hooks/use-sidecar";

export function AppShell() {
  const [currentPage, setCurrentPage] = useState<PageId>("dashboard");
  const sidecar = useSidecar();

  // ... rest same, but pass sidecar status to Sidebar:
  <Sidebar
    currentPage={currentPage}
    onNavigate={setCurrentPage}
    sidecarConnected={sidecar.connected}
    activeRecordings={sidecar.active_recordings}
  />
}
```

- [ ] **Step 5: Verify sidecar starts with app**

```bash
npm run tauri dev
```

Expected: App starts, sidecar process spawns (check via `ps aux | grep python`), sidebar shows "Sidecar: Connected" after a few seconds.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: add sidecar manager with auto-start/stop and connection status"
```

---

## Task 6: TikTok API Client — Live Status Check

**Files:**
- Create: `sidecar/src/tiktok/__init__.py`
- Create: `sidecar/src/tiktok/api.py`
- Create: `sidecar/src/tiktok/stream.py`
- Create: `sidecar/tests/test_tiktok_api.py`

- [ ] **Step 1: Write failing test for TikTok API client**

Create `sidecar/tests/test_tiktok_api.py`:

```python
import pytest
from unittest.mock import AsyncMock, patch
from src.tiktok.api import TikTokAPI


@pytest.mark.asyncio
async def test_check_live_status_returns_response():
    api = TikTokAPI()
    with patch.object(api, "_fetch_room_info", new_callable=AsyncMock) as mock:
        mock.return_value = {
            "LiveRoomInfo": {
                "status": 2,  # 2 = live
                "ownerInfo": {"uniqueId": "testuser"},
                "liveRoomStats": {"userCount": 1500},
            },
            "room_id": "12345",
        }
        result = await api.check_live_status("testuser")
        assert result.is_live is True
        assert result.room_id == "12345"
        assert result.viewer_count == 1500


@pytest.mark.asyncio
async def test_check_live_status_offline():
    api = TikTokAPI()
    with patch.object(api, "_fetch_room_info", new_callable=AsyncMock) as mock:
        mock.return_value = {
            "LiveRoomInfo": {"status": 4},
            "room_id": None,
        }
        result = await api.check_live_status("testuser")
        assert result.is_live is False
        assert result.room_id is None
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd sidecar
pytest tests/test_tiktok_api.py -v
```

Expected: FAIL — `ModuleNotFoundError: No module named 'src.tiktok'`

- [ ] **Step 3: Implement TikTok API client**

Create `sidecar/src/tiktok/__init__.py` (empty file).

Create `sidecar/src/tiktok/api.py`:

```python
from dataclasses import dataclass

import httpx


@dataclass
class LiveStatus:
    username: str
    is_live: bool
    room_id: str | None = None
    stream_url: str | None = None
    viewer_count: int | None = None
    title: str | None = None


class TikTokAPI:
    BASE_URL = "https://www.tiktok.com"
    API_LIVE_URL = "https://webcast.tiktok.com/webcast/room/check_alive/"

    def __init__(self, cookies: dict | None = None, proxy: str | None = None):
        self._cookies = cookies or {}
        self._proxy = proxy
        self._client: httpx.AsyncClient | None = None

    async def _get_client(self) -> httpx.AsyncClient:
        if self._client is None:
            self._client = httpx.AsyncClient(
                timeout=15.0,
                proxy=self._proxy,
                cookies=self._cookies,
                headers={
                    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
                    "Referer": "https://www.tiktok.com/",
                },
                follow_redirects=True,
            )
        return self._client

    async def check_live_status(self, username: str) -> LiveStatus:
        try:
            room_info = await self._fetch_room_info(username)
            live_room = room_info.get("LiveRoomInfo", {})
            status_code = live_room.get("status", 4)
            is_live = status_code == 2
            room_id = room_info.get("room_id")
            viewer_count = None
            if is_live:
                stats = live_room.get("liveRoomStats", {})
                viewer_count = stats.get("userCount")

            return LiveStatus(
                username=username,
                is_live=is_live,
                room_id=str(room_id) if room_id else None,
                viewer_count=viewer_count,
            )
        except Exception:
            return LiveStatus(username=username, is_live=False)

    async def _fetch_room_info(self, username: str) -> dict:
        client = await self._get_client()
        url = f"{self.BASE_URL}/@{username}/live"
        response = await client.get(url)
        response.raise_for_status()

        text = response.text
        room_id = self._extract_room_id(text)

        if not room_id:
            return {"LiveRoomInfo": {"status": 4}, "room_id": None}

        info_url = f"https://webcast.tiktok.com/webcast/room/info/?room_id={room_id}"
        info_response = await client.get(info_url)
        data = info_response.json().get("data", {})
        data["room_id"] = room_id
        return data

    def _extract_room_id(self, html: str) -> str | None:
        import re
        patterns = [
            r'"roomId":"(\d+)"',
            r"room_id=(\d+)",
            r'"id_str":"(\d+)"',
        ]
        for pattern in patterns:
            match = re.search(pattern, html)
            if match:
                return match.group(1)
        return None

    async def close(self):
        if self._client:
            await self._client.aclose()
            self._client = None
```

Create `sidecar/src/tiktok/stream.py`:

```python
import httpx


class StreamResolver:
    """Resolves the actual FLV/HLS stream URL for a TikTok live room."""

    def __init__(self, cookies: dict | None = None, proxy: str | None = None):
        self._cookies = cookies or {}
        self._proxy = proxy

    async def get_stream_url(self, room_id: str) -> str | None:
        async with httpx.AsyncClient(
            timeout=15.0,
            proxy=self._proxy,
            cookies=self._cookies,
            headers={
                "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            },
        ) as client:
            url = f"https://webcast.tiktok.com/webcast/room/info/?room_id={room_id}"
            response = await client.get(url)
            data = response.json().get("data", {})

            stream_url = data.get("stream_url", {})
            flv_pull = stream_url.get("flv_pull_url", {})
            if flv_pull:
                for quality in ["FULL_HD1", "HD1", "SD1", "SD2"]:
                    if quality in flv_pull:
                        return flv_pull[quality]

            hls_pull = stream_url.get("hls_pull_url_map", {})
            if hls_pull:
                for quality in ["FULL_HD1", "HD1", "SD1", "SD2"]:
                    if quality in hls_pull:
                        return hls_pull[quality]

            return stream_url.get("flv_pull_url") or stream_url.get("hls_pull_url")
```

- [ ] **Step 4: Run tests**

```bash
cd sidecar
pytest tests/test_tiktok_api.py -v
```

Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
cd ..
git add -A
git commit -m "feat: add TikTok API client for live status check and stream URL resolution"
```

---

## Task 7: Account Management — Backend + Frontend

**Files:**
- Create: `src-tauri/src/commands/mod.rs`, `src-tauri/src/commands/accounts.rs`
- Create: `sidecar/src/routes/accounts.py`
- Create: `src/lib/api.ts`
- Create: `src/stores/account-store.ts`
- Create: `src/components/accounts/account-list.tsx`, `src/components/accounts/account-form.tsx`, `src/components/accounts/account-badge.tsx`
- Modify: `src/pages/accounts.tsx`

- [ ] **Step 1: Create Rust account commands**

Create `src-tauri/src/commands/mod.rs`:

```rust
pub mod accounts;
```

Create `src-tauri/src/commands/accounts.rs`:

```rust
use crate::db::models::Account;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn list_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, username, display_name, avatar_url, type, tiktok_uid,
                    cookies_json, proxy_url, auto_record, auto_record_schedule,
                    priority, is_live, last_live_at, last_checked_at, notes,
                    created_at, updated_at
             FROM accounts ORDER BY priority DESC, username ASC",
        )
        .map_err(|e| e.to_string())?;

    let accounts = stmt
        .query_map([], |row| {
            Ok(Account {
                id: row.get(0)?,
                username: row.get(1)?,
                display_name: row.get(2)?,
                avatar_url: row.get(3)?,
                account_type: row.get(4)?,
                tiktok_uid: row.get(5)?,
                cookies_json: row.get(6)?,
                proxy_url: row.get(7)?,
                auto_record: row.get(8)?,
                auto_record_schedule: row.get(9)?,
                priority: row.get(10)?,
                is_live: row.get(11)?,
                last_live_at: row.get(12)?,
                last_checked_at: row.get(13)?,
                notes: row.get(14)?,
                created_at: row.get(15)?,
                updated_at: row.get(16)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(accounts)
}

#[derive(serde::Deserialize)]
pub struct CreateAccountInput {
    pub username: String,
    pub display_name: String,
    pub account_type: String,
    pub cookies_json: Option<String>,
    pub proxy_url: Option<String>,
    pub auto_record: bool,
    pub priority: i32,
    pub notes: Option<String>,
}

#[tauri::command]
pub fn create_account(state: State<'_, AppState>, input: CreateAccountInput) -> Result<Account, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO accounts (username, display_name, type, cookies_json, proxy_url, auto_record, priority, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            input.username,
            input.display_name,
            input.account_type,
            input.cookies_json,
            input.proxy_url,
            input.auto_record,
            input.priority,
            input.notes,
        ],
    )
    .map_err(|e| e.to_string())?;

    let id = conn.last_insert_rowid();

    let account = conn
        .query_row(
            "SELECT id, username, display_name, avatar_url, type, tiktok_uid,
                    cookies_json, proxy_url, auto_record, auto_record_schedule,
                    priority, is_live, last_live_at, last_checked_at, notes,
                    created_at, updated_at
             FROM accounts WHERE id = ?1",
            [id],
            |row| {
                Ok(Account {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    display_name: row.get(2)?,
                    avatar_url: row.get(3)?,
                    account_type: row.get(4)?,
                    tiktok_uid: row.get(5)?,
                    cookies_json: row.get(6)?,
                    proxy_url: row.get(7)?,
                    auto_record: row.get(8)?,
                    auto_record_schedule: row.get(9)?,
                    priority: row.get(10)?,
                    is_live: row.get(11)?,
                    last_live_at: row.get(12)?,
                    last_checked_at: row.get(13)?,
                    notes: row.get(14)?,
                    created_at: row.get(15)?,
                    updated_at: row.get(16)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    Ok(account)
}

#[tauri::command]
pub fn delete_account(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM accounts WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn update_account_live_status(
    state: State<'_, AppState>,
    id: i64,
    is_live: bool,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    if is_live {
        conn.execute(
            "UPDATE accounts SET is_live = 1, last_live_at = ?1, last_checked_at = ?1, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        )
    } else {
        conn.execute(
            "UPDATE accounts SET is_live = 0, last_checked_at = ?1, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        )
    }
    .map_err(|e| e.to_string())?;
    Ok(())
}
```

Register commands in `src-tauri/src/lib.rs`:

```rust
mod commands;

// In run():
.invoke_handler(tauri::generate_handler![
    get_sidecar_status,
    commands::accounts::list_accounts,
    commands::accounts::create_account,
    commands::accounts::delete_account,
    commands::accounts::update_account_live_status,
])
```

- [ ] **Step 2: Create frontend API client**

Create `src/lib/api.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";
import type { Account } from "@/types";

export async function listAccounts(): Promise<Account[]> {
  return invoke<Account[]>("list_accounts");
}

export async function createAccount(input: {
  username: string;
  display_name: string;
  account_type: "own" | "monitored";
  cookies_json?: string | null;
  proxy_url?: string | null;
  auto_record: boolean;
  priority: number;
  notes?: string | null;
}): Promise<Account> {
  return invoke<Account>("create_account", { input });
}

export async function deleteAccount(id: number): Promise<void> {
  return invoke("delete_account", { id });
}
```

- [ ] **Step 3: Create account store**

Create `src/stores/account-store.ts`:

```typescript
import { create } from "zustand";
import type { Account } from "@/types";
import * as api from "@/lib/api";

interface AccountState {
  accounts: Account[];
  loading: boolean;
  error: string | null;
  fetchAccounts: () => Promise<void>;
  addAccount: (input: Parameters<typeof api.createAccount>[0]) => Promise<void>;
  removeAccount: (id: number) => Promise<void>;
}

export const useAccountStore = create<AccountState>((set, get) => ({
  accounts: [],
  loading: false,
  error: null,

  fetchAccounts: async () => {
    set({ loading: true, error: null });
    try {
      const accounts = await api.listAccounts();
      set({ accounts, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  addAccount: async (input) => {
    await api.createAccount(input);
    await get().fetchAccounts();
  },

  removeAccount: async (id) => {
    await api.deleteAccount(id);
    await get().fetchAccounts();
  },
}));
```

- [ ] **Step 4: Create Account UI components**

Create `src/components/accounts/account-badge.tsx`:

```tsx
import { Badge } from "@/components/ui/badge";
import type { AccountStatus } from "@/types";

const statusConfig: Record<AccountStatus, { label: string; className: string }> = {
  live: { label: "Live", className: "bg-green-500/20 text-green-400 border-green-500/30" },
  offline: { label: "Offline", className: "bg-gray-500/20 text-gray-400 border-gray-500/30" },
  recording: { label: "Recording", className: "bg-red-500/20 text-red-400 border-red-500/30" },
};

export function AccountBadge({ status }: { status: AccountStatus }) {
  const config = statusConfig[status];
  return (
    <Badge variant="outline" className={config.className}>
      {status === "live" && <span className="w-1.5 h-1.5 rounded-full bg-green-400 mr-1.5 animate-pulse" />}
      {status === "recording" && <span className="w-1.5 h-1.5 rounded-full bg-red-400 mr-1.5 animate-pulse" />}
      {config.label}
    </Badge>
  );
}
```

Create `src/components/accounts/account-form.tsx`:

```tsx
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";

interface AccountFormProps {
  open: boolean;
  onClose: () => void;
  onSubmit: (data: {
    username: string;
    display_name: string;
    account_type: "own" | "monitored";
    auto_record: boolean;
    priority: number;
    proxy_url?: string;
    notes?: string;
  }) => void;
}

export function AccountForm({ open, onClose, onSubmit }: AccountFormProps) {
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [accountType, setAccountType] = useState<"own" | "monitored">("monitored");
  const [autoRecord, setAutoRecord] = useState(false);
  const [priority, setPriority] = useState(0);
  const [proxyUrl, setProxyUrl] = useState("");

  const handleSubmit = () => {
    if (!username.trim()) return;
    onSubmit({
      username: username.trim().replace("@", ""),
      display_name: displayName.trim() || username.trim(),
      account_type: accountType,
      auto_record: autoRecord,
      priority,
      proxy_url: proxyUrl.trim() || undefined,
    });
    setUsername("");
    setDisplayName("");
    setAccountType("monitored");
    setAutoRecord(false);
    setPriority(0);
    setProxyUrl("");
    onClose();
  };

  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="bg-[var(--color-surface)] border-[var(--color-border)] text-[var(--color-text)]">
        <DialogHeader>
          <DialogTitle className="text-white">Add Account</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <div>
            <Label>TikTok Username</Label>
            <Input
              placeholder="e.g. beauty_store_vn"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className="bg-[var(--color-bg)] border-[var(--color-border)]"
            />
          </div>
          <div>
            <Label>Display Name</Label>
            <Input
              placeholder="Optional display name"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              className="bg-[var(--color-bg)] border-[var(--color-border)]"
            />
          </div>
          <div>
            <Label>Type</Label>
            <div className="flex gap-2 mt-1">
              <Button
                variant={accountType === "own" ? "default" : "outline"}
                size="sm"
                onClick={() => setAccountType("own")}
              >
                My Account
              </Button>
              <Button
                variant={accountType === "monitored" ? "default" : "outline"}
                size="sm"
                onClick={() => setAccountType("monitored")}
              >
                Monitored
              </Button>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={autoRecord}
              onChange={(e) => setAutoRecord(e.target.checked)}
              className="rounded"
            />
            <Label>Auto-record when live</Label>
          </div>
          <div>
            <Label>Proxy URL (optional)</Label>
            <Input
              placeholder="http://proxy:port"
              value={proxyUrl}
              onChange={(e) => setProxyUrl(e.target.value)}
              className="bg-[var(--color-bg)] border-[var(--color-border)]"
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancel</Button>
          <Button onClick={handleSubmit} disabled={!username.trim()}>Add Account</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

Create `src/components/accounts/account-list.tsx`:

```tsx
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { useAccountStore } from "@/stores/account-store";
import { AccountBadge } from "./account-badge";
import { AccountForm } from "./account-form";
import type { AccountStatus } from "@/types";

export function AccountList() {
  const { accounts, loading, fetchAccounts, addAccount, removeAccount } = useAccountStore();
  const [showForm, setShowForm] = useState(false);

  useEffect(() => {
    fetchAccounts();
  }, [fetchAccounts]);

  const getStatus = (account: { is_live: boolean }): AccountStatus => {
    if (account.is_live) return "live";
    return "offline";
  };

  return (
    <div>
      <div className="flex justify-between items-center mb-4">
        <div>
          <span className="text-sm text-[var(--color-text-muted)]">{accounts.length} accounts</span>
        </div>
        <Button onClick={() => setShowForm(true)}>+ Add Account</Button>
      </div>

      <div className="rounded-lg border border-[var(--color-border)] overflow-hidden">
        <Table>
          <TableHeader>
            <TableRow className="border-[var(--color-border)] hover:bg-transparent">
              <TableHead className="text-[var(--color-text-muted)]">Username</TableHead>
              <TableHead className="text-[var(--color-text-muted)]">Type</TableHead>
              <TableHead className="text-[var(--color-text-muted)]">Status</TableHead>
              <TableHead className="text-[var(--color-text-muted)]">Auto-Record</TableHead>
              <TableHead className="text-[var(--color-text-muted)]">Priority</TableHead>
              <TableHead className="text-[var(--color-text-muted)]">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {accounts.map((account) => (
              <TableRow key={account.id} className="border-[var(--color-border)]">
                <TableCell className="font-medium text-white">@{account.username}</TableCell>
                <TableCell>{account.type === "own" ? "My Account" : "Monitored"}</TableCell>
                <TableCell><AccountBadge status={getStatus(account)} /></TableCell>
                <TableCell>{account.auto_record ? "On" : "Off"}</TableCell>
                <TableCell>{account.priority}</TableCell>
                <TableCell>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-red-400 hover:text-red-300"
                    onClick={() => removeAccount(account.id)}
                  >
                    Delete
                  </Button>
                </TableCell>
              </TableRow>
            ))}
            {accounts.length === 0 && !loading && (
              <TableRow>
                <TableCell colSpan={6} className="text-center text-[var(--color-text-muted)] py-8">
                  No accounts added yet. Click "Add Account" to get started.
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>

      <AccountForm
        open={showForm}
        onClose={() => setShowForm(false)}
        onSubmit={addAccount}
      />
    </div>
  );
}
```

- [ ] **Step 5: Wire up Accounts page**

Update `src/pages/accounts.tsx`:

```tsx
import { AccountList } from "@/components/accounts/account-list";

export function AccountsPage() {
  return <AccountList />;
}
```

- [ ] **Step 6: Verify account CRUD works**

```bash
npm run tauri dev
```

Expected: Navigate to Accounts page, click "Add Account", fill form, see account appear in table. Delete works.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add account management with CRUD, status badges, and form dialog"
```

---

## Task 8: Recording Engine — Worker Pool + FFmpeg

**Files:**
- Create: `sidecar/src/core/__init__.py`
- Create: `sidecar/src/core/worker.py`
- Create: `sidecar/src/core/recorder.py`
- Create: `sidecar/src/routes/recordings.py`
- Create: `sidecar/tests/test_worker.py`, `sidecar/tests/test_recorder.py`
- Modify: `sidecar/src/app.py`

- [ ] **Step 1: Write failing test for recording worker**

Create `sidecar/tests/test_worker.py`:

```python
import pytest
from unittest.mock import patch, AsyncMock, MagicMock
from src.core.worker import RecordingWorker


@pytest.mark.asyncio
async def test_worker_creates_ffmpeg_command():
    worker = RecordingWorker(
        recording_id="rec_001",
        stream_url="https://pull-flv.tiktok.com/stream/123.flv",
        output_dir="/tmp/recordings",
        username="testuser",
    )
    cmd = worker._build_ffmpeg_command()
    assert "ffmpeg" in cmd[0]
    assert "-i" in cmd
    assert "https://pull-flv.tiktok.com/stream/123.flv" in cmd


@pytest.mark.asyncio
async def test_worker_status_tracking():
    worker = RecordingWorker(
        recording_id="rec_001",
        stream_url="https://example.com/stream.flv",
        output_dir="/tmp/recordings",
        username="testuser",
    )
    assert worker.status == "pending"
    assert worker.duration_seconds == 0
    assert worker.file_size_bytes == 0
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd sidecar && pytest tests/test_worker.py -v
```

Expected: FAIL — `ModuleNotFoundError: No module named 'src.core'`

- [ ] **Step 3: Implement RecordingWorker**

Create `sidecar/src/core/__init__.py` (empty file).

Create `sidecar/src/core/worker.py`:

```python
import asyncio
import os
import time
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class RecordingWorker:
    recording_id: str
    stream_url: str
    output_dir: str
    username: str
    max_duration_seconds: int = 14400  # 4h
    status: str = "pending"
    duration_seconds: int = 0
    file_size_bytes: int = 0
    file_path: str | None = None
    error_message: str | None = None
    _process: asyncio.subprocess.Process | None = field(default=None, repr=False)
    _start_time: float = field(default=0.0, repr=False)
    _stop_requested: bool = field(default=False, repr=False)

    def _build_ffmpeg_command(self) -> list[str]:
        timestamp = time.strftime("%H%M%S")
        date = time.strftime("%Y-%m-%d")
        out_dir = Path(self.output_dir) / self.username / date
        out_dir.mkdir(parents=True, exist_ok=True)
        self.file_path = str(out_dir / f"{timestamp}.flv")

        return [
            "ffmpeg",
            "-y",
            "-i", self.stream_url,
            "-c", "copy",
            "-t", str(self.max_duration_seconds),
            self.file_path,
        ]

    async def start(self) -> None:
        cmd = self._build_ffmpeg_command()
        self.status = "recording"
        self._start_time = time.time()

        try:
            self._process = await asyncio.create_subprocess_exec(
                *cmd,
                stdout=asyncio.subprocess.DEVNULL,
                stderr=asyncio.subprocess.PIPE,
            )

            asyncio.create_task(self._monitor_progress())

            await self._process.wait()

            if self._stop_requested:
                self.status = "done"
            elif self._process.returncode == 0:
                self.status = "done"
            else:
                stderr = await self._process.stderr.read() if self._process.stderr else b""
                self.error_message = stderr.decode(errors="replace")[-500:]
                self.status = "error"
        except Exception as e:
            self.error_message = str(e)
            self.status = "error"

        self.duration_seconds = int(time.time() - self._start_time)
        self._update_file_size()

    async def stop(self) -> None:
        self._stop_requested = True
        if self._process and self._process.returncode is None:
            self._process.terminate()
            try:
                await asyncio.wait_for(self._process.wait(), timeout=10)
            except asyncio.TimeoutError:
                self._process.kill()

    async def _monitor_progress(self) -> None:
        while self._process and self._process.returncode is None:
            self.duration_seconds = int(time.time() - self._start_time)
            self._update_file_size()
            await asyncio.sleep(5)

    def _update_file_size(self) -> None:
        if self.file_path and os.path.exists(self.file_path):
            self.file_size_bytes = os.path.getsize(self.file_path)

    def to_dict(self) -> dict:
        return {
            "recording_id": self.recording_id,
            "username": self.username,
            "status": self.status,
            "duration_seconds": self.duration_seconds,
            "file_size_bytes": self.file_size_bytes,
            "file_path": self.file_path,
            "error_message": self.error_message,
        }
```

- [ ] **Step 4: Implement RecordingManager (worker pool)**

Create `sidecar/src/core/recorder.py`:

```python
import asyncio
import uuid
from .worker import RecordingWorker
from ..ws.manager import ws_manager
from ..config import settings


class RecordingManager:
    def __init__(self):
        self._workers: dict[str, RecordingWorker] = {}
        self._tasks: dict[str, asyncio.Task] = {}

    @property
    def active_count(self) -> int:
        return sum(1 for w in self._workers.values() if w.status == "recording")

    async def start_recording(
        self,
        account_id: int,
        username: str,
        stream_url: str,
        max_duration_seconds: int | None = None,
    ) -> str:
        if self.active_count >= settings.max_concurrent_recordings:
            raise RuntimeError(
                f"Max concurrent recordings ({settings.max_concurrent_recordings}) reached"
            )

        recording_id = f"rec_{uuid.uuid4().hex[:8]}"
        output_dir = str(settings.storage_path / "recordings")

        worker = RecordingWorker(
            recording_id=recording_id,
            stream_url=stream_url,
            output_dir=output_dir,
            username=username,
            max_duration_seconds=max_duration_seconds or settings.max_duration_hours * 3600,
        )

        self._workers[recording_id] = worker

        task = asyncio.create_task(self._run_worker(recording_id, account_id, worker))
        self._tasks[recording_id] = task

        return recording_id

    async def _run_worker(self, recording_id: str, account_id: int, worker: RecordingWorker):
        await ws_manager.broadcast("recording_started", {
            "recording_id": recording_id,
            "account_id": account_id,
            "username": worker.username,
        })

        progress_task = asyncio.create_task(
            self._broadcast_progress(recording_id, account_id, worker)
        )

        try:
            await worker.start()
        finally:
            progress_task.cancel()
            await ws_manager.broadcast("recording_finished", {
                "recording_id": recording_id,
                "account_id": account_id,
                **worker.to_dict(),
            })

    async def _broadcast_progress(self, recording_id: str, account_id: int, worker: RecordingWorker):
        try:
            while True:
                await asyncio.sleep(5)
                if worker.status == "recording":
                    await ws_manager.broadcast("recording_progress", {
                        "recording_id": recording_id,
                        "account_id": account_id,
                        **worker.to_dict(),
                    })
        except asyncio.CancelledError:
            pass

    async def stop_recording(self, recording_id: str) -> None:
        worker = self._workers.get(recording_id)
        if not worker:
            raise ValueError(f"Recording {recording_id} not found")
        await worker.stop()

    def get_status(self, recording_id: str) -> dict | None:
        worker = self._workers.get(recording_id)
        return worker.to_dict() if worker else None

    def get_all_status(self) -> list[dict]:
        return [w.to_dict() for w in self._workers.values()]

    async def stop_all(self) -> None:
        for worker in self._workers.values():
            if worker.status == "recording":
                await worker.stop()


recording_manager = RecordingManager()
```

- [ ] **Step 5: Create recording routes**

Create `sidecar/src/routes/recordings.py`:

```python
from fastapi import APIRouter, HTTPException
from ..core.recorder import recording_manager
from ..models.schemas import StartRecordingRequest, StopRecordingRequest, RecordingStatusResponse
from ..tiktok.stream import StreamResolver

router = APIRouter()


@router.post("/api/recording/start")
async def start_recording(req: StartRecordingRequest):
    stream_url = req.stream_url
    if not stream_url and req.room_id:
        resolver = StreamResolver(
            cookies=_parse_cookies(req.cookies_json),
            proxy=req.proxy_url,
        )
        stream_url = await resolver.get_stream_url(req.room_id)

    if not stream_url:
        raise HTTPException(status_code=400, detail="Could not resolve stream URL")

    try:
        recording_id = await recording_manager.start_recording(
            account_id=req.account_id,
            username=req.username,
            stream_url=stream_url,
            max_duration_seconds=req.max_duration_seconds,
        )
        return {"recording_id": recording_id, "status": "recording"}
    except RuntimeError as e:
        raise HTTPException(status_code=429, detail=str(e))


@router.post("/api/recording/stop")
async def stop_recording(req: StopRecordingRequest):
    try:
        await recording_manager.stop_recording(req.recording_id)
        return {"status": "stopped"}
    except ValueError as e:
        raise HTTPException(status_code=404, detail=str(e))


@router.get("/api/recording/status")
async def get_all_recording_status():
    return {"recordings": recording_manager.get_all_status()}


@router.get("/api/recording/status/{recording_id}")
async def get_recording_status(recording_id: str):
    status = recording_manager.get_status(recording_id)
    if not status:
        raise HTTPException(status_code=404, detail="Recording not found")
    return status


def _parse_cookies(cookies_json: str | None) -> dict | None:
    if not cookies_json:
        return None
    import json
    try:
        return json.loads(cookies_json)
    except json.JSONDecodeError:
        return None
```

- [ ] **Step 6: Register recording routes in app**

Update `sidecar/src/app.py`:

```python
from .routes import health, recordings

def create_app() -> FastAPI:
    # ... existing code ...
    app.include_router(health.router)
    app.include_router(recordings.router)
    # ... rest same ...
```

- [ ] **Step 7: Run tests**

```bash
cd sidecar && pytest tests/test_worker.py -v
```

Expected: 2 tests pass.

- [ ] **Step 8: Commit**

```bash
cd ..
git add -A
git commit -m "feat: add recording engine with FFmpeg worker pool, stream download, and REST API"
```

---

## Task 9: Account Watcher — Auto-Detect Live + Auto-Record

**Files:**
- Create: `sidecar/src/core/watcher.py`
- Create: `sidecar/src/routes/accounts.py`
- Create: `sidecar/tests/test_watcher.py`
- Modify: `sidecar/src/app.py`

- [ ] **Step 1: Write failing test for watcher**

Create `sidecar/tests/test_watcher.py`:

```python
import pytest
from unittest.mock import AsyncMock, patch, MagicMock
from src.core.watcher import AccountWatcher


@pytest.mark.asyncio
async def test_watcher_detects_live():
    watcher = AccountWatcher()
    mock_api = AsyncMock()
    mock_api.check_live_status = AsyncMock(return_value=MagicMock(
        is_live=True, room_id="12345", viewer_count=100, username="testuser"
    ))

    with patch("src.core.watcher.TikTokAPI", return_value=mock_api):
        result = await watcher.check_account("testuser")
        assert result["is_live"] is True
        assert result["room_id"] == "12345"


@pytest.mark.asyncio
async def test_watcher_handles_offline():
    watcher = AccountWatcher()
    mock_api = AsyncMock()
    mock_api.check_live_status = AsyncMock(return_value=MagicMock(
        is_live=False, room_id=None, viewer_count=None, username="testuser"
    ))

    with patch("src.core.watcher.TikTokAPI", return_value=mock_api):
        result = await watcher.check_account("testuser")
        assert result["is_live"] is False
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd sidecar && pytest tests/test_watcher.py -v
```

Expected: FAIL — `ModuleNotFoundError: No module named 'src.core.watcher'`

- [ ] **Step 3: Implement AccountWatcher**

Create `sidecar/src/core/watcher.py`:

```python
import asyncio
from dataclasses import dataclass, field

from ..config import settings
from ..tiktok.api import TikTokAPI
from ..tiktok.stream import StreamResolver
from ..ws.manager import ws_manager
from .recorder import recording_manager


@dataclass
class WatchedAccount:
    username: str
    account_id: int
    cookies_json: str | None = None
    proxy_url: str | None = None
    auto_record: bool = False
    was_live: bool = False


class AccountWatcher:
    def __init__(self):
        self._accounts: dict[int, WatchedAccount] = {}
        self._running = False
        self._task: asyncio.Task | None = None

    def add_account(self, account_id: int, username: str, cookies_json: str | None = None,
                    proxy_url: str | None = None, auto_record: bool = False):
        self._accounts[account_id] = WatchedAccount(
            username=username,
            account_id=account_id,
            cookies_json=cookies_json,
            proxy_url=proxy_url,
            auto_record=auto_record,
        )

    def remove_account(self, account_id: int):
        self._accounts.pop(account_id, None)

    def update_account(self, account_id: int, **kwargs):
        if account_id in self._accounts:
            for key, value in kwargs.items():
                if hasattr(self._accounts[account_id], key):
                    setattr(self._accounts[account_id], key, value)

    async def check_account(self, username: str, cookies_json: str | None = None,
                            proxy_url: str | None = None) -> dict:
        cookies = None
        if cookies_json:
            import json
            try:
                cookies = json.loads(cookies_json)
            except json.JSONDecodeError:
                pass

        api = TikTokAPI(cookies=cookies, proxy=proxy_url)
        try:
            status = await api.check_live_status(username)
            return {
                "username": username,
                "is_live": status.is_live,
                "room_id": status.room_id,
                "viewer_count": status.viewer_count,
            }
        finally:
            await api.close()

    def start(self):
        if not self._running:
            self._running = True
            self._task = asyncio.create_task(self._poll_loop())

    def stop(self):
        self._running = False
        if self._task:
            self._task.cancel()

    async def _poll_loop(self):
        while self._running:
            for acc in list(self._accounts.values()):
                if not self._running:
                    break
                try:
                    result = await self.check_account(
                        acc.username, acc.cookies_json, acc.proxy_url
                    )
                    is_live = result["is_live"]

                    if is_live and not acc.was_live:
                        await ws_manager.broadcast("account_live", {
                            "account_id": acc.account_id,
                            "username": acc.username,
                            "room_id": result["room_id"],
                            "viewer_count": result["viewer_count"],
                        })

                        if acc.auto_record and result.get("room_id"):
                            try:
                                resolver = StreamResolver(proxy=acc.proxy_url)
                                stream_url = await resolver.get_stream_url(result["room_id"])
                                if stream_url:
                                    await recording_manager.start_recording(
                                        account_id=acc.account_id,
                                        username=acc.username,
                                        stream_url=stream_url,
                                    )
                            except Exception:
                                pass

                    acc.was_live = is_live
                except Exception:
                    pass

            await asyncio.sleep(settings.poll_interval_seconds)


account_watcher = AccountWatcher()
```

- [ ] **Step 4: Create account status route**

Create `sidecar/src/routes/accounts.py`:

```python
from fastapi import APIRouter
from ..core.watcher import account_watcher
from ..models.schemas import AccountStatusRequest, AccountStatusResponse

router = APIRouter()


@router.post("/api/accounts/check-status", response_model=AccountStatusResponse)
async def check_account_status(req: AccountStatusRequest):
    result = await account_watcher.check_account(
        req.username, req.cookies_json, req.proxy_url
    )
    return AccountStatusResponse(**result)


@router.post("/api/accounts/watch")
async def add_watched_account(account_id: int, username: str,
                               auto_record: bool = False,
                               cookies_json: str | None = None,
                               proxy_url: str | None = None):
    account_watcher.add_account(
        account_id=account_id, username=username,
        auto_record=auto_record, cookies_json=cookies_json, proxy_url=proxy_url,
    )
    return {"status": "watching"}


@router.delete("/api/accounts/watch/{account_id}")
async def remove_watched_account(account_id: int):
    account_watcher.remove_account(account_id)
    return {"status": "removed"}
```

- [ ] **Step 5: Register routes and start watcher on app startup**

Update `sidecar/src/app.py`:

```python
from contextlib import asynccontextmanager
from .routes import health, recordings, accounts
from .core.watcher import account_watcher


@asynccontextmanager
async def lifespan(app: FastAPI):
    account_watcher.start()
    yield
    account_watcher.stop()


def create_app() -> FastAPI:
    app = FastAPI(title="TikClip Sidecar", version="0.1.0", lifespan=lifespan)
    # ... middleware ...
    app.include_router(health.router)
    app.include_router(recordings.router)
    app.include_router(accounts.router)
    # ... websocket endpoint ...
    return app
```

- [ ] **Step 6: Run tests**

```bash
cd sidecar && pytest tests/test_watcher.py -v
```

Expected: 2 tests pass.

- [ ] **Step 7: Commit**

```bash
cd ..
git add -A
git commit -m "feat: add account watcher with live detection polling and auto-record trigger"
```

---

## Task 10: Video Processing — Scene Detection + Clip Extraction

**Files:**
- Create: `sidecar/src/core/processor.py`
- Create: `sidecar/src/routes/clips.py`
- Create: `sidecar/tests/test_processor.py`
- Modify: `sidecar/pyproject.toml`
- Modify: `sidecar/src/app.py`

- [ ] **Step 1: Add processing dependencies**

Update `sidecar/pyproject.toml` dependencies:

```toml
dependencies = [
    "fastapi>=0.115",
    "uvicorn[standard]>=0.34",
    "websockets>=14.0",
    "httpx>=0.28",
    "pydantic>=2.10",
    "pydantic-settings>=2.7",
    "scenedetect[opencv]>=0.6",
]
```

```bash
cd sidecar && pip install -e ".[dev]"
```

- [ ] **Step 2: Write failing test for processor**

Create `sidecar/tests/test_processor.py`:

```python
import pytest
from src.core.processor import VideoProcessor


def test_processor_initializes():
    proc = VideoProcessor(
        recording_id=1,
        account_id=1,
        file_path="/tmp/test.flv",
        output_dir="/tmp/clips",
    )
    assert proc.status == "pending"
    assert proc.clips == []


def test_build_clip_path():
    proc = VideoProcessor(
        recording_id=1,
        account_id=1,
        file_path="/tmp/test.flv",
        output_dir="/tmp/clips",
        username="testuser",
    )
    path = proc._build_clip_path(1)
    assert "clip_001" in path
    assert path.endswith(".mp4")
```

- [ ] **Step 3: Run test to verify it fails**

```bash
pytest tests/test_processor.py -v
```

Expected: FAIL — import error.

- [ ] **Step 4: Implement VideoProcessor**

Create `sidecar/src/core/processor.py`:

```python
import asyncio
import os
import time
from dataclasses import dataclass, field
from pathlib import Path

from ..config import settings
from ..ws.manager import ws_manager


@dataclass
class ClipInfo:
    clip_number: int
    file_path: str
    thumbnail_path: str | None
    start_time: float
    end_time: float
    duration_seconds: float
    file_size_bytes: int = 0


@dataclass
class VideoProcessor:
    recording_id: int
    account_id: int
    file_path: str
    output_dir: str
    username: str = ""
    clip_min_duration: int = 15
    clip_max_duration: int = 90
    scene_threshold: float = 30.0
    status: str = "pending"
    clips: list[ClipInfo] = field(default_factory=list)
    error_message: str | None = None

    def _build_clip_path(self, clip_number: int) -> str:
        date = time.strftime("%Y-%m-%d")
        out_dir = Path(self.output_dir) / self.username / date
        out_dir.mkdir(parents=True, exist_ok=True)
        return str(out_dir / f"clip_{clip_number:03d}.mp4")

    def _build_thumbnail_path(self, clip_path: str) -> str:
        return clip_path.replace(".mp4", "_thumb.jpg")

    async def process(self) -> list[ClipInfo]:
        self.status = "processing"
        await ws_manager.broadcast("processing_progress", {
            "recording_id": self.recording_id,
            "status": "detecting_scenes",
            "progress": 0,
        })

        try:
            scenes = await self._detect_scenes()
            grouped = self._group_scenes(scenes)

            total = len(grouped)
            for i, (start, end) in enumerate(grouped):
                clip_num = i + 1
                clip_path = self._build_clip_path(clip_num)
                thumb_path = self._build_thumbnail_path(clip_path)

                await self._extract_clip(start, end, clip_path)
                await self._extract_thumbnail(start + (end - start) / 2, thumb_path)

                clip = ClipInfo(
                    clip_number=clip_num,
                    file_path=clip_path,
                    thumbnail_path=thumb_path,
                    start_time=start,
                    end_time=end,
                    duration_seconds=end - start,
                    file_size_bytes=os.path.getsize(clip_path) if os.path.exists(clip_path) else 0,
                )
                self.clips.append(clip)

                await ws_manager.broadcast("processing_progress", {
                    "recording_id": self.recording_id,
                    "status": "extracting_clips",
                    "progress": int((clip_num / total) * 100) if total > 0 else 100,
                    "current_clip": clip_num,
                    "total_clips": total,
                })

            self.status = "done"

            for clip in self.clips:
                await ws_manager.broadcast("clip_ready", {
                    "recording_id": self.recording_id,
                    "account_id": self.account_id,
                    "clip_number": clip.clip_number,
                    "file_path": clip.file_path,
                    "duration_seconds": clip.duration_seconds,
                })

            return self.clips

        except Exception as e:
            self.status = "error"
            self.error_message = str(e)
            raise

    async def _detect_scenes(self) -> list[tuple[float, float]]:
        """Run PySceneDetect in a thread to avoid blocking the event loop."""
        return await asyncio.to_thread(self._detect_scenes_sync)

    def _detect_scenes_sync(self) -> list[tuple[float, float]]:
        from scenedetect import open_video, SceneManager
        from scenedetect.detectors import ContentDetector

        video = open_video(self.file_path)
        scene_manager = SceneManager()
        scene_manager.add_detector(ContentDetector(threshold=self.scene_threshold))
        scene_manager.detect_scenes(video)
        scene_list = scene_manager.get_scene_list()

        return [(s[0].get_seconds(), s[1].get_seconds()) for s in scene_list]

    def _group_scenes(self, scenes: list[tuple[float, float]]) -> list[tuple[float, float]]:
        if not scenes:
            return []

        grouped: list[tuple[float, float]] = []
        current_start = scenes[0][0]
        current_end = scenes[0][1]

        for start, end in scenes[1:]:
            duration = end - current_start
            if duration <= self.clip_max_duration:
                current_end = end
            else:
                if current_end - current_start >= self.clip_min_duration:
                    grouped.append((current_start, current_end))
                current_start = start
                current_end = end

        if current_end - current_start >= self.clip_min_duration:
            grouped.append((current_start, current_end))

        return grouped

    async def _extract_clip(self, start: float, end: float, output_path: str) -> None:
        cmd = [
            "ffmpeg", "-y",
            "-i", self.file_path,
            "-ss", str(start),
            "-to", str(end),
            "-c", "copy",
            "-avoid_negative_ts", "make_zero",
            output_path,
        ]
        proc = await asyncio.create_subprocess_exec(
            *cmd, stdout=asyncio.subprocess.DEVNULL, stderr=asyncio.subprocess.PIPE
        )
        await proc.wait()

    async def _extract_thumbnail(self, timestamp: float, output_path: str) -> None:
        cmd = [
            "ffmpeg", "-y",
            "-i", self.file_path,
            "-ss", str(timestamp),
            "-vframes", "1",
            "-q:v", "2",
            output_path,
        ]
        proc = await asyncio.create_subprocess_exec(
            *cmd, stdout=asyncio.subprocess.DEVNULL, stderr=asyncio.subprocess.PIPE
        )
        await proc.wait()
```

- [ ] **Step 5: Create clips route**

Create `sidecar/src/routes/clips.py`:

```python
from fastapi import APIRouter, HTTPException, BackgroundTasks
from ..core.processor import VideoProcessor
from ..models.schemas import ProcessVideoRequest
from ..config import settings

router = APIRouter()
_active_processors: dict[int, VideoProcessor] = {}


@router.post("/api/video/process")
async def process_video(req: ProcessVideoRequest, background_tasks: BackgroundTasks):
    if req.recording_id in _active_processors:
        proc = _active_processors[req.recording_id]
        if proc.status == "processing":
            return {"status": "already_processing", "recording_id": req.recording_id}

    processor = VideoProcessor(
        recording_id=req.recording_id,
        account_id=req.account_id,
        file_path=req.file_path,
        output_dir=str(settings.storage_path / "clips"),
        clip_min_duration=req.clip_min_duration,
        clip_max_duration=req.clip_max_duration,
        scene_threshold=req.scene_threshold,
    )
    _active_processors[req.recording_id] = processor
    background_tasks.add_task(processor.process)

    return {"status": "processing_started", "recording_id": req.recording_id}


@router.get("/api/processing/status/{recording_id}")
async def get_processing_status(recording_id: int):
    proc = _active_processors.get(recording_id)
    if not proc:
        raise HTTPException(status_code=404, detail="No processing job found")
    return {
        "recording_id": recording_id,
        "status": proc.status,
        "clips_count": len(proc.clips),
        "error_message": proc.error_message,
    }
```

- [ ] **Step 6: Register clips route**

Update `sidecar/src/app.py` — add:

```python
from .routes import health, recordings, accounts, clips

# In create_app():
app.include_router(clips.router)
```

- [ ] **Step 7: Run tests**

```bash
cd sidecar && pytest tests/test_processor.py -v
```

Expected: 2 tests pass.

- [ ] **Step 8: Commit**

```bash
cd ..
git add -A
git commit -m "feat: add video processor with scene detection, clip extraction, and thumbnail generation"
```

---

## Task 11: Recording & Clips UI — Frontend Components

**Files:**
- Create: `src/lib/ws.ts`
- Create: `src/hooks/use-websocket.ts`
- Create: `src/stores/recording-store.ts`, `src/stores/clip-store.ts`
- Create: `src/components/recordings/recording-list.tsx`, `src/components/recordings/recording-progress.tsx`, `src/components/recordings/recording-controls.tsx`
- Create: `src/components/clips/clip-grid.tsx`, `src/components/clips/clip-card.tsx`
- Modify: `src/pages/recordings.tsx`, `src/pages/clips.tsx`
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Create WebSocket client**

Create `src/lib/ws.ts`:

```typescript
import type { WsEvent } from "@/types";

type EventHandler = (event: WsEvent) => void;

class WebSocketClient {
  private ws: WebSocket | null = null;
  private handlers: Map<string, Set<EventHandler>> = new Map();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private url: string = "";

  connect(port: number) {
    this.url = `ws://127.0.0.1:${port}/ws`;
    this._connect();
  }

  private _connect() {
    if (this.ws?.readyState === WebSocket.OPEN) return;

    this.ws = new WebSocket(this.url);

    this.ws.onmessage = (event) => {
      try {
        const parsed: WsEvent = JSON.parse(event.data);
        const typeHandlers = this.handlers.get(parsed.type);
        if (typeHandlers) {
          typeHandlers.forEach((handler) => handler(parsed));
        }
        const allHandlers = this.handlers.get("*");
        if (allHandlers) {
          allHandlers.forEach((handler) => handler(parsed));
        }
      } catch {
        // ignore parse errors
      }
    };

    this.ws.onclose = () => {
      this.reconnectTimer = setTimeout(() => this._connect(), 3000);
    };
  }

  disconnect() {
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    this.ws?.close();
    this.ws = null;
  }

  on(eventType: string, handler: EventHandler) {
    if (!this.handlers.has(eventType)) {
      this.handlers.set(eventType, new Set());
    }
    this.handlers.get(eventType)!.add(handler);
    return () => this.handlers.get(eventType)?.delete(handler);
  }
}

export const wsClient = new WebSocketClient();
```

- [ ] **Step 2: Create WebSocket hook**

Create `src/hooks/use-websocket.ts`:

```typescript
import { useEffect } from "react";
import { wsClient } from "@/lib/ws";
import type { WsEvent } from "@/types";

export function useWebSocket(eventType: string, handler: (event: WsEvent) => void) {
  useEffect(() => {
    const unsubscribe = wsClient.on(eventType, handler);
    return unsubscribe;
  }, [eventType, handler]);
}
```

- [ ] **Step 3: Extend API client with sidecar HTTP calls**

Add to `src/lib/api.ts`:

```typescript
let sidecarBaseUrl = "http://127.0.0.1:18321";

export function setSidecarPort(port: number) {
  sidecarBaseUrl = `http://127.0.0.1:${port}`;
}

async function sidecarFetch<T>(path: string, options?: RequestInit): Promise<T> {
  const response = await fetch(`${sidecarBaseUrl}${path}`, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });
  if (!response.ok) {
    const error = await response.json().catch(() => ({ detail: response.statusText }));
    throw new Error(error.detail || "Sidecar request failed");
  }
  return response.json();
}

export async function checkAccountStatus(username: string, cookies_json?: string, proxy_url?: string) {
  return sidecarFetch<{ username: string; is_live: boolean; room_id: string | null }>(
    "/api/accounts/check-status",
    { method: "POST", body: JSON.stringify({ username, cookies_json, proxy_url }) }
  );
}

export async function startRecording(params: {
  account_id: number;
  username: string;
  room_id?: string;
  stream_url?: string;
}) {
  return sidecarFetch<{ recording_id: string; status: string }>(
    "/api/recording/start",
    { method: "POST", body: JSON.stringify(params) }
  );
}

export async function stopRecording(recording_id: string) {
  return sidecarFetch<{ status: string }>(
    "/api/recording/stop",
    { method: "POST", body: JSON.stringify({ recording_id }) }
  );
}

export async function getRecordingStatus() {
  return sidecarFetch<{ recordings: Array<Record<string, unknown>> }>("/api/recording/status");
}
```

- [ ] **Step 4: Create recording store**

Create `src/stores/recording-store.ts`:

```typescript
import { create } from "zustand";

interface RecordingState {
  activeRecordings: Map<string, {
    recording_id: string;
    account_id: number;
    username: string;
    status: string;
    duration_seconds: number;
    file_size_bytes: number;
  }>;
  updateRecording: (id: string, data: Record<string, unknown>) => void;
  removeRecording: (id: string) => void;
}

export const useRecordingStore = create<RecordingState>((set) => ({
  activeRecordings: new Map(),

  updateRecording: (id, data) =>
    set((state) => {
      const newMap = new Map(state.activeRecordings);
      const existing = newMap.get(id) || {
        recording_id: id,
        account_id: 0,
        username: "",
        status: "recording",
        duration_seconds: 0,
        file_size_bytes: 0,
      };
      newMap.set(id, { ...existing, ...data } as typeof existing);
      return { activeRecordings: newMap };
    }),

  removeRecording: (id) =>
    set((state) => {
      const newMap = new Map(state.activeRecordings);
      newMap.delete(id);
      return { activeRecordings: newMap };
    }),
}));
```

- [ ] **Step 5: Create Recording UI components**

Create `src/components/recordings/recording-progress.tsx`:

```tsx
interface RecordingProgressProps {
  duration_seconds: number;
  file_size_bytes: number;
}

export function RecordingProgress({ duration_seconds, file_size_bytes }: RecordingProgressProps) {
  const formatDuration = (s: number) => {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = s % 60;
    return h > 0 ? `${h}h ${m}m ${sec}s` : `${m}m ${sec}s`;
  };

  const formatSize = (bytes: number) => {
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  };

  return (
    <div className="flex gap-4 text-xs text-[var(--color-text-muted)]">
      <span>{formatDuration(duration_seconds)}</span>
      <span>{formatSize(file_size_bytes)}</span>
    </div>
  );
}
```

Create `src/components/recordings/recording-controls.tsx`:

```tsx
import { Button } from "@/components/ui/button";
import * as api from "@/lib/api";

interface RecordingControlsProps {
  recording_id: string;
  status: string;
  onStopped?: () => void;
}

export function RecordingControls({ recording_id, status, onStopped }: RecordingControlsProps) {
  const handleStop = async () => {
    try {
      await api.stopRecording(recording_id);
      onStopped?.();
    } catch (e) {
      console.error("Failed to stop recording:", e);
    }
  };

  if (status !== "recording") return null;

  return (
    <Button variant="outline" size="sm" onClick={handleStop} className="text-red-400 border-red-400/30">
      ⏹ Stop
    </Button>
  );
}
```

Create `src/components/recordings/recording-list.tsx`:

```tsx
import { useCallback, useEffect } from "react";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { useRecordingStore } from "@/stores/recording-store";
import { useWebSocket } from "@/hooks/use-websocket";
import { RecordingProgress } from "./recording-progress";
import { RecordingControls } from "./recording-controls";
import type { WsEvent } from "@/types";

export function RecordingList() {
  const { activeRecordings, updateRecording, removeRecording } = useRecordingStore();

  const handleWsEvent = useCallback((event: WsEvent) => {
    const data = event.data as Record<string, unknown>;
    const id = data.recording_id as string;
    if (!id) return;

    switch (event.type) {
      case "recording_started":
        updateRecording(id, data);
        break;
      case "recording_progress":
        updateRecording(id, data);
        break;
      case "recording_finished":
        updateRecording(id, { ...data, status: "done" });
        break;
    }
  }, [updateRecording]);

  useWebSocket("recording_started", handleWsEvent);
  useWebSocket("recording_progress", handleWsEvent);
  useWebSocket("recording_finished", handleWsEvent);

  const recordings = Array.from(activeRecordings.values());

  return (
    <div className="rounded-lg border border-[var(--color-border)] overflow-hidden">
      <Table>
        <TableHeader>
          <TableRow className="border-[var(--color-border)] hover:bg-transparent">
            <TableHead className="text-[var(--color-text-muted)]">Account</TableHead>
            <TableHead className="text-[var(--color-text-muted)]">Status</TableHead>
            <TableHead className="text-[var(--color-text-muted)]">Progress</TableHead>
            <TableHead className="text-[var(--color-text-muted)]">Actions</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {recordings.map((rec) => (
            <TableRow key={rec.recording_id} className="border-[var(--color-border)]">
              <TableCell className="font-medium text-white">@{rec.username}</TableCell>
              <TableCell>
                <Badge variant={rec.status === "recording" ? "destructive" : "secondary"}>
                  {rec.status === "recording" && (
                    <span className="w-1.5 h-1.5 rounded-full bg-white mr-1.5 animate-pulse" />
                  )}
                  {rec.status.toUpperCase()}
                </Badge>
              </TableCell>
              <TableCell>
                <RecordingProgress
                  duration_seconds={rec.duration_seconds}
                  file_size_bytes={rec.file_size_bytes}
                />
              </TableCell>
              <TableCell>
                <RecordingControls recording_id={rec.recording_id} status={rec.status} />
              </TableCell>
            </TableRow>
          ))}
          {recordings.length === 0 && (
            <TableRow>
              <TableCell colSpan={4} className="text-center text-[var(--color-text-muted)] py-8">
                No active recordings. Start recording from the Accounts page or Dashboard.
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
    </div>
  );
}
```

- [ ] **Step 6: Create Clip UI components**

Create `src/components/clips/clip-card.tsx`:

```tsx
import { Badge } from "@/components/ui/badge";
import type { Clip } from "@/types";

const statusColors: Record<string, string> = {
  draft: "bg-gray-500/20 text-gray-400",
  ready: "bg-green-500/20 text-green-400",
  posted: "bg-blue-500/20 text-blue-400",
  archived: "bg-yellow-500/20 text-yellow-400",
};

export function ClipCard({ clip }: { clip: Clip }) {
  const formatDuration = (s: number) => {
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return `${m}:${sec.toString().padStart(2, "0")}`;
  };

  return (
    <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] overflow-hidden">
      <div className="aspect-video bg-[var(--color-bg)] flex items-center justify-center text-2xl">
        {clip.thumbnail_path ? (
          <img src={`asset://localhost/${clip.thumbnail_path}`} alt="" className="w-full h-full object-cover" />
        ) : (
          "🎬"
        )}
      </div>
      <div className="p-3">
        <div className="flex items-center justify-between mb-1">
          <span className="text-sm font-medium text-white truncate">
            {clip.title || `Clip #${clip.id}`}
          </span>
          <Badge variant="outline" className={statusColors[clip.status] || ""}>
            {clip.status}
          </Badge>
        </div>
        <div className="text-xs text-[var(--color-text-muted)]">
          {formatDuration(clip.duration_seconds)} • {clip.account_username && `@${clip.account_username}`}
        </div>
      </div>
    </div>
  );
}
```

Create `src/components/clips/clip-grid.tsx`:

```tsx
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ClipCard } from "./clip-card";
import type { Clip } from "@/types";

export function ClipGrid() {
  const [clips, setClips] = useState<Clip[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    invoke<Clip[]>("list_clips")
      .then(setClips)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  if (loading) return <p className="text-[var(--color-text-muted)]">Loading clips...</p>;

  if (clips.length === 0) {
    return (
      <div className="text-center py-12 text-[var(--color-text-muted)]">
        No clips yet. Clips will appear here after recordings are processed.
      </div>
    );
  }

  return (
    <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
      {clips.map((clip) => (
        <ClipCard key={clip.id} clip={clip} />
      ))}
    </div>
  );
}
```

- [ ] **Step 7: Wire up pages**

Update `src/pages/recordings.tsx`:

```tsx
import { RecordingList } from "@/components/recordings/recording-list";

export function RecordingsPage() {
  return <RecordingList />;
}
```

Update `src/pages/clips.tsx`:

```tsx
import { ClipGrid } from "@/components/clips/clip-grid";

export function ClipsPage() {
  return <ClipGrid />;
}
```

- [ ] **Step 8: Add clip commands to Rust backend**

Create `src-tauri/src/commands/clips.rs`:

```rust
use crate::db::models::Clip;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn list_clips(state: State<'_, AppState>) -> Result<Vec<Clip>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.recording_id, c.account_id, a.username,
                    c.title, c.file_path, c.thumbnail_path,
                    c.duration_seconds, c.file_size_bytes,
                    c.start_time, c.end_time, c.status,
                    c.quality_score, c.scene_type, c.ai_tags_json, c.notes,
                    c.created_at, c.updated_at
             FROM clips c
             LEFT JOIN accounts a ON a.id = c.account_id
             ORDER BY c.created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let clips = stmt
        .query_map([], |row| {
            Ok(Clip {
                id: row.get(0)?,
                recording_id: row.get(1)?,
                account_id: row.get(2)?,
                account_username: row.get(3)?,
                title: row.get(4)?,
                file_path: row.get(5)?,
                thumbnail_path: row.get(6)?,
                duration_seconds: row.get(7)?,
                file_size_bytes: row.get(8)?,
                start_time: row.get(9)?,
                end_time: row.get(10)?,
                status: row.get(11)?,
                quality_score: row.get(12)?,
                scene_type: row.get(13)?,
                ai_tags_json: row.get(14)?,
                notes: row.get(15)?,
                created_at: row.get(16)?,
                updated_at: row.get(17)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(clips)
}
```

Update `src-tauri/src/commands/mod.rs`:

```rust
pub mod accounts;
pub mod clips;
```

Register in `lib.rs`:

```rust
.invoke_handler(tauri::generate_handler![
    get_sidecar_status,
    commands::accounts::list_accounts,
    commands::accounts::create_account,
    commands::accounts::delete_account,
    commands::accounts::update_account_live_status,
    commands::clips::list_clips,
])
```

- [ ] **Step 9: Verify UI renders**

```bash
npm run tauri dev
```

Expected: Recordings page shows empty state. Clips page shows empty grid. Both ready to display data from sidecar.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat: add recording list, clip grid UI, WebSocket client, and sidecar API integration"
```

---

## Task 12: Dashboard + Notifications + System Tray

**Files:**
- Create: `src/components/dashboard/stat-cards.tsx`, `src/components/dashboard/active-recordings.tsx`, `src/components/dashboard/live-accounts.tsx`, `src/components/dashboard/recent-clips.tsx`
- Create: `src/components/notifications/notification-toast.tsx`
- Create: `src/stores/notification-store.ts`
- Create: `src-tauri/src/tray.rs`
- Modify: `src/pages/dashboard.tsx`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create notification store**

Create `src/stores/notification-store.ts`:

```typescript
import { create } from "zustand";

interface AppNotification {
  id: string;
  type: "live" | "recording" | "clip" | "error";
  title: string;
  message: string;
  timestamp: number;
  read: boolean;
}

interface NotificationState {
  notifications: AppNotification[];
  addNotification: (n: Omit<AppNotification, "id" | "timestamp" | "read">) => void;
  markRead: (id: string) => void;
  unreadCount: () => number;
}

export const useNotificationStore = create<NotificationState>((set, get) => ({
  notifications: [],

  addNotification: (n) =>
    set((state) => ({
      notifications: [
        {
          ...n,
          id: crypto.randomUUID(),
          timestamp: Date.now(),
          read: false,
        },
        ...state.notifications,
      ].slice(0, 100),
    })),

  markRead: (id) =>
    set((state) => ({
      notifications: state.notifications.map((n) =>
        n.id === id ? { ...n, read: true } : n
      ),
    })),

  unreadCount: () => get().notifications.filter((n) => !n.read).length,
}));
```

- [ ] **Step 2: Create dashboard stat cards**

Create `src/components/dashboard/stat-cards.tsx`:

```tsx
import { Card, CardContent } from "@/components/ui/card";

interface StatCardsProps {
  activeRecordings: number;
  totalAccounts: number;
  clipsToday: number;
  storageUsedGb: number;
  storageMaxGb: number;
}

export function StatCards({ activeRecordings, totalAccounts, clipsToday, storageUsedGb, storageMaxGb }: StatCardsProps) {
  const storagePercent = Math.round((storageUsedGb / storageMaxGb) * 100);

  const stats = [
    { label: "ACTIVE RECORDINGS", value: activeRecordings, color: "text-[var(--color-primary)]", sub: `${activeRecordings} running` },
    { label: "ACCOUNTS", value: totalAccounts, color: "text-[var(--color-accent)]", sub: "monitored" },
    { label: "CLIPS TODAY", value: clipsToday, color: "text-white", sub: "generated" },
    { label: "STORAGE USED", value: `${storageUsedGb.toFixed(1)}GB`, color: "text-white", sub: storagePercent > 80 ? `⚠ ${storagePercent}% of limit` : `${storagePercent}% of ${storageMaxGb}GB` },
  ];

  return (
    <div className="grid grid-cols-4 gap-3">
      {stats.map((stat) => (
        <Card key={stat.label} className="bg-[var(--color-surface)] border-[var(--color-border)]">
          <CardContent className="p-4">
            <div className="text-[10px] text-[var(--color-text-muted)] mb-1">{stat.label}</div>
            <div className={`text-2xl font-bold ${stat.color}`}>{stat.value}</div>
            <div className="text-[10px] text-[var(--color-text-muted)] mt-1">{stat.sub}</div>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}
```

- [ ] **Step 3: Create dashboard page**

Update `src/pages/dashboard.tsx`:

```tsx
import { useEffect, useState } from "react";
import { StatCards } from "@/components/dashboard/stat-cards";
import { useAccountStore } from "@/stores/account-store";
import { useRecordingStore } from "@/stores/recording-store";

export function DashboardPage() {
  const { accounts, fetchAccounts } = useAccountStore();
  const { activeRecordings } = useRecordingStore();

  useEffect(() => {
    fetchAccounts();
  }, [fetchAccounts]);

  const recordingCount = Array.from(activeRecordings.values()).filter(
    (r) => r.status === "recording"
  ).length;

  return (
    <div className="space-y-6">
      <StatCards
        activeRecordings={recordingCount}
        totalAccounts={accounts.length}
        clipsToday={0}
        storageUsedGb={0}
        storageMaxGb={100}
      />

      <div className="grid grid-cols-2 gap-4">
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <h3 className="text-sm font-semibold text-white mb-3">🔴 Active Recordings</h3>
          {recordingCount === 0 ? (
            <p className="text-xs text-[var(--color-text-muted)]">No active recordings</p>
          ) : (
            <div className="space-y-2">
              {Array.from(activeRecordings.values())
                .filter((r) => r.status === "recording")
                .map((r) => (
                  <div key={r.recording_id} className="flex items-center gap-3 p-2 rounded bg-[var(--color-bg)]">
                    <span className="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
                    <span className="text-sm text-white">@{r.username}</span>
                    <span className="ml-auto text-xs text-[var(--color-text-muted)]">
                      {Math.floor(r.duration_seconds / 60)}m
                    </span>
                  </div>
                ))}
            </div>
          )}
        </div>

        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <h3 className="text-sm font-semibold text-white mb-3">📡 Live Now</h3>
          {accounts.filter((a) => a.is_live).length === 0 ? (
            <p className="text-xs text-[var(--color-text-muted)]">No accounts are live right now</p>
          ) : (
            <div className="space-y-2">
              {accounts
                .filter((a) => a.is_live)
                .map((a) => (
                  <div key={a.id} className="flex items-center gap-3 p-2 rounded bg-[var(--color-bg)]">
                    <span className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
                    <span className="text-sm text-white">@{a.username}</span>
                  </div>
                ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Create system tray**

Create `src-tauri/src/tray.rs`:

```rust
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, Runtime,
};

pub fn setup_tray<R: Runtime>(app: &tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "Show TikClip", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("TikClip - Live Recorder")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    window.show().ok();
                    window.set_focus().ok();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
```

Wire it into `lib.rs` setup:

```rust
mod tray;

// In setup closure, after managing state:
tray::setup_tray(app).ok();
```

- [ ] **Step 5: Verify dashboard and tray**

```bash
npm run tauri dev
```

Expected: Dashboard shows stat cards (all zeros for now), two panels for active recordings and live accounts. System tray icon appears with Show/Quit menu.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: add dashboard with stat cards, notification store, and system tray"
```

---

## Task 13: Settings Page + WebSocket Integration Boot

**Files:**
- Create: `src-tauri/src/commands/settings.rs`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs`
- Modify: `src/pages/settings.tsx`
- Modify: `src/components/layout/app-shell.tsx` (connect WebSocket on sidecar connect)

- [ ] **Step 1: Create settings commands in Rust**

Create `src-tauri/src/commands/settings.rs`:

```rust
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn get_setting(state: State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let result = conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        [&key],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(val) => Ok(Some(val)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn set_setting(state: State<'_, AppState>, key: String, value: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')",
        rusqlite::params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
```

Update `src-tauri/src/commands/mod.rs`:

```rust
pub mod accounts;
pub mod clips;
pub mod settings;
```

Register in `lib.rs` invoke_handler.

- [ ] **Step 2: Create Settings page UI**

Update `src/pages/settings.tsx`:

```tsx
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export function SettingsPage() {
  const [maxConcurrent, setMaxConcurrent] = useState("5");
  const [pollInterval, setPollInterval] = useState("30");
  const [clipMinDuration, setClipMinDuration] = useState("15");
  const [clipMaxDuration, setClipMaxDuration] = useState("90");
  const [maxStorageGb, setMaxStorageGb] = useState("100");
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    const loadSettings = async () => {
      const keys = ["max_concurrent", "poll_interval", "clip_min_duration", "clip_max_duration", "max_storage_gb"];
      for (const key of keys) {
        const value = await invoke<string | null>("get_setting", { key });
        if (value !== null) {
          const setters: Record<string, (v: string) => void> = {
            max_concurrent: setMaxConcurrent,
            poll_interval: setPollInterval,
            clip_min_duration: setClipMinDuration,
            clip_max_duration: setClipMaxDuration,
            max_storage_gb: setMaxStorageGb,
          };
          setters[key]?.(value);
        }
      }
    };
    loadSettings();
  }, []);

  const handleSave = async () => {
    const settings = {
      max_concurrent: maxConcurrent,
      poll_interval: pollInterval,
      clip_min_duration: clipMinDuration,
      clip_max_duration: clipMaxDuration,
      max_storage_gb: maxStorageGb,
    };
    for (const [key, value] of Object.entries(settings)) {
      await invoke("set_setting", { key, value });
    }
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  return (
    <div className="space-y-6 max-w-2xl">
      <Card className="bg-[var(--color-surface)] border-[var(--color-border)]">
        <CardHeader><CardTitle className="text-white text-base">Recording</CardTitle></CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <Label>Max Concurrent Recordings</Label>
              <Input value={maxConcurrent} onChange={(e) => setMaxConcurrent(e.target.value)} type="number" className="bg-[var(--color-bg)] border-[var(--color-border)]" />
            </div>
            <div>
              <Label>Poll Interval (seconds)</Label>
              <Input value={pollInterval} onChange={(e) => setPollInterval(e.target.value)} type="number" className="bg-[var(--color-bg)] border-[var(--color-border)]" />
            </div>
          </div>
        </CardContent>
      </Card>

      <Card className="bg-[var(--color-surface)] border-[var(--color-border)]">
        <CardHeader><CardTitle className="text-white text-base">Clip Processing</CardTitle></CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <Label>Min Clip Duration (seconds)</Label>
              <Input value={clipMinDuration} onChange={(e) => setClipMinDuration(e.target.value)} type="number" className="bg-[var(--color-bg)] border-[var(--color-border)]" />
            </div>
            <div>
              <Label>Max Clip Duration (seconds)</Label>
              <Input value={clipMaxDuration} onChange={(e) => setClipMaxDuration(e.target.value)} type="number" className="bg-[var(--color-bg)] border-[var(--color-border)]" />
            </div>
          </div>
        </CardContent>
      </Card>

      <Card className="bg-[var(--color-surface)] border-[var(--color-border)]">
        <CardHeader><CardTitle className="text-white text-base">Storage</CardTitle></CardHeader>
        <CardContent>
          <div>
            <Label>Max Storage (GB)</Label>
            <Input value={maxStorageGb} onChange={(e) => setMaxStorageGb(e.target.value)} type="number" className="bg-[var(--color-bg)] border-[var(--color-border)]" />
          </div>
        </CardContent>
      </Card>

      <Button onClick={handleSave}>
        {saved ? "✓ Saved!" : "Save Settings"}
      </Button>
    </div>
  );
}
```

- [ ] **Step 3: Connect WebSocket when sidecar is ready**

Update `src/components/layout/app-shell.tsx` — add WebSocket connection:

```tsx
import { useEffect } from "react";
import { wsClient } from "@/lib/ws";
import { useNotificationStore } from "@/stores/notification-store";

// Inside AppShell component, after useSidecar():
useEffect(() => {
  if (sidecar.connected && sidecar.port) {
    wsClient.connect(sidecar.port);
    return () => wsClient.disconnect();
  }
}, [sidecar.connected, sidecar.port]);

// Subscribe to notification events
useEffect(() => {
  const unsub1 = wsClient.on("account_live", (event) => {
    const data = event.data as Record<string, unknown>;
    useNotificationStore.getState().addNotification({
      type: "live",
      title: `@${data.username} is live!`,
      message: `Viewers: ${data.viewer_count || "unknown"}`,
    });
  });
  const unsub2 = wsClient.on("recording_finished", (event) => {
    const data = event.data as Record<string, unknown>;
    useNotificationStore.getState().addNotification({
      type: "recording",
      title: "Recording complete",
      message: `@${data.username} - ${data.duration_seconds}s`,
    });
  });
  const unsub3 = wsClient.on("clip_ready", (event) => {
    const data = event.data as Record<string, unknown>;
    useNotificationStore.getState().addNotification({
      type: "clip",
      title: "New clip ready",
      message: `Clip #${data.clip_number} from recording`,
    });
  });
  return () => { unsub1(); unsub2(); unsub3(); };
}, []);
```

- [ ] **Step 4: Verify everything works end-to-end**

```bash
npm run tauri dev
```

Expected: Full app with Dashboard, Accounts, Recordings, Clips, Settings pages all working. Sidebar shows sidecar status. Settings save/load correctly.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add settings page, WebSocket boot integration, and notification system"
```

---

## Self-Review Checklist

### Spec Coverage

| Spec Requirement | Task |
|---|---|
| Tauri v2 + React + TypeScript | Task 1 |
| Dark theme, sidebar nav | Task 2 |
| SQLite database + migrations | Task 3 |
| Python sidecar FastAPI | Task 4 |
| Auto-start/stop sidecar | Task 5 |
| TikTok live status check | Task 6 |
| Account CRUD (own/monitored) | Task 7 |
| Cookie import, proxy config | Task 7 |
| Recording engine (FFmpeg) | Task 8 |
| Concurrent worker pool | Task 8 |
| Realtime progress (WebSocket) | Task 8, 11 |
| Auto-retry on disconnect | Task 8 (worker) |
| Account watcher (polling) | Task 9 |
| Auto-record when go live | Task 9 |
| Scene-based auto-split | Task 10 |
| Thumbnail generation | Task 10 |
| Clips list with status | Task 11 |
| OS notifications | Task 12 |
| System tray | Task 12 |
| Dashboard stat cards | Task 12 |
| Settings page | Task 13 |

### Type/Method Consistency

- `RecordingWorker.to_dict()` → matches `RecordingStatusResponse` schema
- `AccountWatcher.check_account()` → returns dict matching `AccountStatusResponse`
- `VideoProcessor.clips` → list of `ClipInfo` → maps to `Clip` DB model
- Rust `Account` model → matches TypeScript `Account` type
- WebSocket events → consistent naming across Python broadcast and TypeScript handlers
