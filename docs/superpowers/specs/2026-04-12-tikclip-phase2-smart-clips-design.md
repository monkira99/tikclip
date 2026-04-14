# TikClip Phase 2 — Smart Clips & Polish

**Date:** 2026-04-12
**Status:** Approved
**Builds on:** Phase 1 MVP (2026-04-05-tikclip-desktop-app-design.md)

---

## 1. Overview

Phase 2 transforms TikClip from a functional recording tool into a daily-driver clip management workflow. It adds a video player with trim, full clip management (filters, views, batch ops, status workflow), a product catalog with TikTok Shop auto-import, storage management with auto-cleanup, and a realtime dashboard with activity feed.

### Goals

- Smooth daily workflow for reviewing, trimming, tagging, and managing clips
- Product catalog with auto-import from TikTok links + manual fallback
- Storage sustainability via configurable retention and auto-cleanup
- Realtime awareness of what's happening across the app

### Implementation Order

1. Clip Management + Status Workflow (foundation for everything else)
2. Video Player + Trim (depends on clip management for navigation/save)
3. Product Catalog + TikTok Import + Tagging (depends on clip management for tagging UI)
4. Storage Management + Auto-cleanup (parallel-safe with product catalog)
5. Dashboard Realtime + Activity Feed (benefits from all prior events)

---

## 2. Clip Management + Status Workflow

### 2.1 Rust Commands

| Command | Input | Output | Description |
|---------|-------|--------|-------------|
| `list_clips_filtered` | `status?`, `account_id?`, `scene_type?`, `date_from?`, `date_to?`, `search?`, `sort_by?`, `sort_order?` | `Vec<Clip>` | Replaces `list_clips`. All params optional; omit = no filter. |
| `update_clip_status` | `clip_id`, `new_status` | `()` | Validates transition rules. |
| `update_clip_title` | `clip_id`, `title` | `()` | Rename clip. |
| `update_clip_notes` | `clip_id`, `notes` | `()` | Edit notes. |
| `batch_update_clip_status` | `Vec<clip_id>`, `new_status` | `()` | Single transaction. |
| `batch_delete_clips` | `Vec<clip_id>` | `()` | Deletes DB rows + physical files (clip + thumbnail). |
| `get_clip_by_id` | `clip_id` | `Clip` | Single clip for detail view. |

### 2.2 Status Workflow Rules

```
draft ──→ ready ──→ posted ──→ archived
  ↑                                │
  └────────────────────────────────┘
           (reset / unarchive)
```

- `draft` → `ready`: user confirms clip is good
- `ready` → `posted`: user posted to TikTok
- `posted` → `archived`: manual or auto after retention
- Any status → `draft`: reset
- `archived` → `draft`: unarchive
- Delete on `posted` clips requires explicit confirmation dialog (warning that clip may still be live on TikTok)

### 2.3 Frontend — Clip Store

Expand `clip-store.ts` from simple revision counter to full state management:

```typescript
type ClipFilters = {
  status: ClipStatus | "all";
  accountId: number | null;
  sceneType: SceneType | "all";
  dateFrom: string | null;
  dateTo: string | null;
  search: string;
  sortBy: "created_at" | "duration" | "file_size" | "title";
  sortOrder: "asc" | "desc";
};

type ViewMode = "grid" | "list";

type ClipStore = {
  clips: Clip[];
  filters: ClipFilters;
  viewMode: ViewMode;
  selectedClipIds: Set<number>;
  loading: boolean;

  fetchClips: () => Promise<void>;
  setFilter: (partial: Partial<ClipFilters>) => void;
  setViewMode: (mode: ViewMode) => void;
  toggleSelect: (clipId: number) => void;
  selectAll: () => void;
  clearSelection: () => void;
  batchUpdateStatus: (status: ClipStatus) => Promise<void>;
  batchDelete: () => Promise<void>;
};
```

### 2.4 Frontend — UI

**Clips page toolbar:**
- View mode toggle (grid / list icons)
- Status filter dropdown (All / Draft / Ready / Posted / Archived)
- Account filter dropdown (populated from accounts store)
- Scene type filter dropdown
- Search input (debounced, searches title/notes)
- Sort dropdown (Date / Duration / Size)
- Batch action bar (appears when items selected): "Set Status →" dropdown, Delete button, selection count indicator

**Grid view** (upgrade existing `ClipGrid`):
- Keep date/user grouping layout
- Add checkbox overlay on each card for multi-select
- Click card → navigate to clip detail view (Section 3)
- Context menu ("..." button) on card: Set Status, Edit Title, Delete, Open in Finder

**List view** (new `ClipList` component):
- Table: checkbox, thumbnail (48×48), title, account, duration, size, status badge, scene type, created_at
- Sortable column headers
- Click row → navigate to clip detail view

**Status badge colors:**
- Draft: `bg-zinc-700 text-zinc-300`
- Ready: `bg-emerald-900 text-emerald-300`
- Posted: `bg-blue-900 text-blue-300`
- Archived: `bg-zinc-800 text-zinc-500`

---

## 3. Video Player + Trim

### 3.1 Navigation

Clips page has two states (not separate pages):
- **List state:** toolbar + grid/list (Section 2)
- **Detail state:** video player + clip info + actions, with Back button

Reason: video player needs space; a modal would be too cramped.

### 3.2 Video Player Component

Uses native HTML5 `<video>` via `convertFileSrc` — no external library needed for local MP4 playback.

**Controls:**
- Play / Pause
- Seek bar (click + drag)
- Current time / Total duration
- Volume slider + mute toggle
- Playback speed (0.5×, 1×, 1.5×, 2×)
- Fullscreen toggle

### 3.3 Clip Detail Layout

```
┌─────────────────────────────────────────┐
│ ← Back to Clips          Clip #42       │
├───────────────────────┬─────────────────┤
│                       │ Title: [edit]   │
│   ┌───────────────┐   │ Account: @user  │
│   │  Video Player │   │ Status: [Ready▼]│
│   └───────────────┘   │ Duration: 1:23  │
│   [controls + seek]   │ Size: 12.5 MB   │
│                       │ Scene: highlight│
│   ┌─ Trim ───────┐   │ Created: 12/04  │
│   │ [==|====|==]  │   │ Notes: [edit]   │
│   │ Start: 00:05  │   │                 │
│   │ End:   01:10  │   │ Products: [tag] │
│   │ [Preview][Cut]│   │                 │
│   └───────────────┘   │ [Open in Finder]│
│                       │ [Delete Clip]   │
└───────────────────────┴─────────────────┘
```

Left panel (~65%): video + controls + trim.
Right panel (~35%): metadata + actions.

### 3.4 Trim Feature

**UI:**
- Range slider on timeline (two handles: start + end)
- Precise time inputs (`MM:SS.ms` format)
- "Preview trim" button: play from start to end then pause
- "Create trimmed clip" button: calls sidecar

**Trim flow:**
1. User drags handles or types exact times
2. "Preview" → video plays trim region
3. "Create trimmed clip" → calls sidecar API
4. Sidecar runs FFmpeg stream copy (no re-encode, near-instant)
5. New clip inserted into DB with same `recording_id`, status `draft`
6. Toast notification + option to view new clip

**Sidecar API:**

```
POST /api/clips/trim
Body: {
  "source_path": "/path/to/clip.mp4",
  "start_sec": 5.0,
  "end_sec": 70.0,
  "account_id": 1,
  "recording_id": 3
}
Response: {
  "file_path": "/path/to/new_clip.mp4",
  "thumbnail_path": "/path/to/thumb.jpg",
  "duration_sec": 65.0
}
```

New file named `clip_{NNN}_trimmed.mp4` in the same clips directory (auto-incrementing index). Thumbnail generated for new clip.

**Rust command:** `insert_trimmed_clip` — receives sidecar trim output, inserts into SQLite, returns new clip ID.

**Out of scope for Phase 2:**
- Waveform visualization
- Frame-by-frame stepping
- Multi-segment trim
- Re-encode / resolution / bitrate adjustment

---

## 4. Product Catalog + TikTok Import + Tagging

### 4.1 Database Migration

```sql
-- 004_product_enhancements.sql
ALTER TABLE products ADD COLUMN tiktok_url TEXT;
ALTER TABLE products ADD COLUMN updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'));
```

### 4.2 TikTok Product Scraper (Sidecar)

New module: `sidecar/src/tiktok/product_scraper.py`

**Scraping strategy (in order):**
1. Parse Open Graph meta tags (`og:title`, `og:image`, `og:description`) — least likely to be blocked
2. Parse JSON-LD structured data in page
3. If still incomplete → return partial data with `incomplete: true` flag

**Sidecar API:**

```
POST /api/products/fetch-from-url
Body: { "url": "https://...", "cookies_json": "..." }
Response: {
  "success": true,
  "incomplete": false,
  "data": {
    "name": "...",
    "description": "...",
    "price": 299000,
    "image_url": "https://...",
    "category": "...",
    "tiktok_shop_id": "..."
  },
  "error": null
}
```

Frontend receives data (full or partial), fills form, user edits and saves.

### 4.3 Rust Commands (Products)

| Command | Input | Output | Description |
|---------|-------|--------|-------------|
| `list_products` | — | `Vec<Product>` | Order by `created_at DESC`. |
| `create_product` | `CreateProductInput` | `i64` (product ID) | Name required, rest optional. |
| `update_product` | `product_id`, `UpdateProductInput` | `()` | Partial update. |
| `delete_product` | `product_id` | `()` | Cascade deletes `clip_products` rows. |
| `get_product_by_id` | `product_id` | `Product` | Single product. |
| `list_clip_products` | `clip_id` | `Vec<Product>` | Products linked to a clip. |
| `tag_clip_product` | `clip_id`, `product_id` | `()` | Insert into `clip_products`. Idempotent. |
| `untag_clip_product` | `clip_id`, `product_id` | `()` | Remove link. |
| `batch_tag_clip_products` | `Vec<clip_id>`, `product_id` | `()` | Tag one product to many clips. |

### 4.4 Frontend Types

```typescript
export interface Product {
  id: number;
  name: string;
  description: string | null;
  sku: string | null;
  image_url: string | null;
  tiktok_shop_id: string | null;
  tiktok_url: string | null;
  price: number | null;
  category: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateProductInput {
  name: string;
  description?: string | null;
  sku?: string | null;
  image_url?: string | null;
  tiktok_shop_id?: string | null;
  tiktok_url?: string | null;
  price?: number | null;
  category?: string | null;
}
```

### 4.5 Frontend — Products Page

New page in sidebar navigation (between Clips and Statistics).

**Product list:**
- Grid of product cards (image, name, price, category, linked clip count)
- Search bar (name / SKU)
- "Add Product" button → opens dialog

**Add/Edit Product Dialog (two tabs):**
- Tab "Import from TikTok": paste URL field → "Fetch" button → loading → auto-fill form. On failure/incomplete: show message, let user fix manually.
- Tab "Manual": form fields (name*, description, SKU, price, category, image URL)
- Save button → `create_product` / `update_product`

### 4.6 Frontend — Product Tagging on Clips

In Clip Detail view (Section 3, right panel):

**Products section:**
- List of tagged products (name + small image + remove button)
- "Add Product" button → opens product picker dialog
- Picker: searchable list of all products, click to toggle tag/untag
- "Quick create" link in picker to create new product inline

**Batch tagging from clip list:**
- Select multiple clips → batch action "Tag Product" → picker → select product → `batch_tag_clip_products`

### 4.7 Sidebar Navigation Update

```
Dashboard
Accounts
Recordings
Clips
Products    ← NEW
Statistics
Settings
```

---

## 5. Storage Management + Auto-cleanup

### 5.1 Storage Stats API (Sidecar)

```
GET /api/storage/stats
Response: {
  "recordings_bytes": 52428800000,
  "recordings_count": 42,
  "clips_bytes": 10485760000,
  "clips_count": 356,
  "products_bytes": 5242880,
  "total_bytes": 62919742880,
  "quota_bytes": 107374182400,
  "usage_percent": 58.6
}
```

Scans actual filesystem (not DB) for accuracy.

### 5.2 Retention Policies

Stored in `app_settings`:

| Key | Default | Description |
|-----|---------|-------------|
| `TIKCLIP_RAW_RETENTION_DAYS` | `7` | Delete processed recordings after N days. 0 = keep forever. |
| `TIKCLIP_ARCHIVE_RETENTION_DAYS` | `30` | Delete archived clips after N days. 0 = keep forever. |
| `TIKCLIP_STORAGE_WARN_PERCENT` | `80` | Show warning above this usage. |
| `TIKCLIP_STORAGE_CLEANUP_PERCENT` | `95` | Trigger auto-cleanup above this usage. |

### 5.3 Cleanup Worker (Sidecar)

New module: `sidecar/src/core/cleanup.py`

**`StorageCleanupWorker`:**
- Runs periodic check every 30 minutes
- Starts with sidecar lifespan (like `AccountWatcher`)

**Cleanup logic (priority order):**

1. **Retention-based** (always runs):
   - Recordings with `status = 'done'` + `ended_at` older than retention → delete file + update DB
   - Clips with `status = 'archived'` + `updated_at` older than retention → delete file + thumbnail + DB row
   - Broadcast `cleanup_completed` via WebSocket

2. **Quota-based** (only when usage > cleanup threshold):
   - Delete oldest processed recordings first
   - If still over → delete oldest archived clips
   - Stop when usage < warning threshold
   - Broadcast `storage_warning` at start, `cleanup_completed` when done

**Never auto-deletes:**
- Recordings with status `recording` or `processing`
- Clips with status `draft`, `ready`, or `posted`

### 5.4 Rust Commands

| Command | Description |
|---------|-------------|
| `get_storage_stats` | Calls sidecar `/api/storage/stats`, merges with DB counts. |
| `delete_recording_files` | Manual cleanup: delete physical files + update DB. |
| `list_recordings_for_cleanup` | Preview: recordings eligible for cleanup (done + past retention). |

### 5.5 Frontend — Settings Page Addition

New "Storage Management" section in `settings.tsx`:

**Storage overview card:**
- Progress bar showing usage percent (green < 80%, yellow 80-95%, red > 95%)
- Breakdown: Recordings / Clips / Other with sizes and counts
- "Scan Now" button to refresh

**Retention policies form:**
- Raw recording retention: number input (days), 0 = keep forever
- Archived clip retention: number input (days), 0 = keep forever
- Warning threshold: slider 50–100%
- Auto-cleanup threshold: slider 50–100% (must be > warning)
- Save → `set_setting` per key

**Manual cleanup:**
- "Clean up now" button → triggers immediate sidecar cleanup
- Shows list of files to be deleted before confirmation

### 5.6 WebSocket Events

| Event | Data | When |
|-------|------|------|
| `storage_warning` | `{ usage_percent, quota_bytes, total_bytes }` | Usage exceeds warning threshold |
| `cleanup_started` | `{ reason: "retention" \| "quota" }` | Cleanup begins |
| `cleanup_completed` | `{ deleted_recordings, deleted_clips, freed_bytes }` | Cleanup finished |

Frontend receives `storage_warning` → toast + badge on Settings sidebar icon.

---

## 6. Dashboard Realtime + Activity Feed

### 6.1 Realtime Refresh

Add `dashboardRevision` counter to `app-store.ts`. WebSocket events that trigger a bump:

- `clip_ready` → refresh "Clips Today"
- `recording_finished` → refresh "Active Recordings"
- `account_live` / `account_status` → refresh "Live Accounts"
- `cleanup_completed` → refresh "Storage Used"
- `storage_warning` → refresh storage + show indicator

`DashboardPage` watches `dashboardRevision` and refetches stats on change. Simple and reliable — no live WebSocket streaming for stats.

### 6.2 Activity Feed

Uses existing `notifications` table as data source. No new table needed.

**New notification types to auto-insert on events:**

| Type | Trigger | Example |
|------|---------|---------|
| `account_live` | Already exists | @username đang live |
| `recording_started` | Already exists | Bắt đầu ghi @username |
| `recording_finished` | Already exists | Ghi xong @username — 45 phút |
| `clip_ready` | Already exists | Clip mới từ @username |
| `clip_status_changed` | New | Clip #42 → Ready |
| `product_created` | New | Sản phẩm "Áo thun XYZ" đã tạo |
| `cleanup_completed` | New | Dọn dẹp: giải phóng 2.3 GB |
| `storage_warning` | New | Cảnh báo: dung lượng đạt 85% |

### 6.3 ActivityFeed Component

```
Activity Feed                    [View All →]
─────────────────────────────────────────────
🔴 @username đang live                  2m ago
🎬 Ghi xong @user2 — 45 phút          15m ago
✂️  3 clips mới từ @user2              14m ago
📦 Sản phẩm "Áo thun" đã tạo          1h ago
🧹 Dọn dẹp: giải phóng 2.3 GB        3h ago
```

- Shows 10 most recent items on dashboard
- "View All →" navigates to notification menu / full list
- Each item: icon by type, message, relative timestamp
- Items are clickable: clip events → clip detail, recording events → recordings page
- Auto-updates via `dashboardRevision`

### 6.4 Rust Command

| Command | Description |
|---------|-------------|
| `list_activity_feed` | Query `notifications` table, limit 10, order by `created_at DESC`. Returns rows with navigation metadata (clip_id, recording_id, account_id). |

Separate from `list_notifications` because: no `is_read` filter, different shape, dedicated limit.

### 6.5 Dashboard Layout

```
┌───────────────────────────────────────────────┐
│ [Active Rec] [Accounts] [Clips Today] [Storage] │  stat cards (auto-refresh)
├───────────────────────────────────────────────┤
│ Activity Feed                     [View All →] │  NEW
│ 🔴 @username đang live                 2m ago  │
│ 🎬 Ghi xong @user2                   15m ago  │
│ ...                                            │
├───────────────────────────────────────────────┤
│ Active Recordings                (unchanged)   │
├───────────────────────────────────────────────┤
│ Live Now                         (unchanged)   │
└───────────────────────────────────────────────┘
```

---

## 7. New Files Summary

### Frontend (`src/`)

| Path | Purpose |
|------|---------|
| `components/clips/clip-list.tsx` | Table/list view for clips |
| `components/clips/clip-detail.tsx` | Clip detail view with player + metadata |
| `components/clips/clip-toolbar.tsx` | Filters, search, view toggle, batch actions |
| `components/clips/video-player.tsx` | HTML5 video player with custom controls |
| `components/clips/trim-controls.tsx` | Range slider + time inputs + preview/cut buttons |
| `components/products/product-list.tsx` | Product grid with search |
| `components/products/product-card.tsx` | Single product card |
| `components/products/product-form.tsx` | Add/edit dialog with TikTok import tab |
| `components/products/product-picker.tsx` | Product picker dialog for tagging |
| `components/dashboard/activity-feed.tsx` | Recent activity timeline |
| `pages/products.tsx` | Products page |
| `stores/product-store.ts` | Product state management |
| `types/index.ts` | Add `Product`, `CreateProductInput` types |

### Sidecar (`sidecar/src/`)

| Path | Purpose |
|------|---------|
| `tiktok/product_scraper.py` | TikTok Shop URL → product data |
| `core/cleanup.py` | StorageCleanupWorker |
| `routes/clips.py` | Trim endpoint (separate from existing `clips.py` or extend) |
| `routes/products.py` | Product fetch-from-url endpoint |
| `routes/storage.py` | Storage stats endpoint |

### Rust (`src-tauri/src/`)

| Path | Purpose |
|------|---------|
| `commands/products.rs` | Product CRUD + clip tagging commands |
| `db/migrations/004_product_enhancements.sql` | Add `tiktok_url`, `updated_at` to products |
| `commands/clips.rs` | Extend with filtered list, status update, batch ops, trim insert |
| `commands/storage.rs` | Storage stats, manual cleanup commands |

---

## 8. Non-Goals (Phase 2)

- AI-powered clip splitting or quality scoring (Phase 3)
- Auto-upload to TikTok (Phase 4)
- Statistics page with charts (Phase 3)
- Video re-encoding, resolution/bitrate adjustment
- Waveform visualization, frame-by-frame stepping
- Multi-segment trim
- Team collaboration
