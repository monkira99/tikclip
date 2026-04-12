# TikClip Phase 2 — Smart Clips & Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform TikClip into a daily-driver clip management workflow with video player, trim, product catalog, storage management, and realtime dashboard.

**Architecture:** Bottom-up approach — clip management foundation first (Rust commands + store + UI), then video player/trim on top, then product catalog, storage, and dashboard. Each layer builds on the previous. Frontend ↔ Rust via `invoke`, frontend ↔ sidecar via HTTP REST, realtime via WebSocket.

**Tech Stack:** Tauri v2 + React 19 + TypeScript + Zustand (frontend), Rust + rusqlite (desktop backend), Python + FastAPI + FFmpeg (sidecar)

**Spec:** `docs/superpowers/specs/2026-04-12-tikclip-phase2-smart-clips-design.md`

---

## File Structure

### New Files

| Path | Purpose |
|------|---------|
| `src-tauri/src/db/migrations/004_product_enhancements.sql` | Add `tiktok_url`, `updated_at` to products |
| `src-tauri/src/commands/products.rs` | Product CRUD + clip-product tagging |
| `src-tauri/src/commands/storage.rs` | Storage stats + manual cleanup |
| `src/components/clips/clip-toolbar.tsx` | Filters, search, view toggle, batch actions |
| `src/components/clips/clip-list.tsx` | Table/list view |
| `src/components/clips/clip-detail.tsx` | Detail view with player + metadata panel |
| `src/components/clips/video-player.tsx` | HTML5 video player with custom controls |
| `src/components/clips/trim-controls.tsx` | Range slider + time inputs + trim actions |
| `src/components/products/product-list.tsx` | Product grid with search |
| `src/components/products/product-card.tsx` | Single product card |
| `src/components/products/product-form.tsx` | Add/edit dialog with TikTok import tab |
| `src/components/products/product-picker.tsx` | Product picker for clip tagging |
| `src/components/dashboard/activity-feed.tsx` | Recent activity timeline |
| `src/pages/products.tsx` | Products page |
| `src/stores/product-store.ts` | Product state |
| `sidecar/src/tiktok/product_scraper.py` | TikTok URL → product data |
| `sidecar/src/core/cleanup.py` | Storage cleanup worker |
| `sidecar/src/routes/products.py` | Product fetch-from-url endpoint |
| `sidecar/src/routes/storage.py` | Storage stats endpoint |
| `sidecar/src/routes/trim.py` | Clip trim endpoint |

### Modified Files

| Path | Changes |
|------|---------|
| `src-tauri/src/commands/clips.rs` | Add filtered list, status updates, batch ops, get_clip_by_id, insert_trimmed_clip |
| `src-tauri/src/commands/mod.rs` | Register `products`, `storage` modules |
| `src-tauri/src/db/models.rs` | Add `Product` struct |
| `src-tauri/src/db/init.rs` | Run migration 004 |
| `src-tauri/src/lib.rs` | Register new commands in invoke_handler |
| `src/types/index.ts` | Add `Product`, `CreateProductInput`, `UpdateProductInput` types |
| `src/lib/api.ts` | Add product, storage, clip filter/update API wrappers |
| `src/stores/clip-store.ts` | Full rewrite: filters, view mode, selection, fetch |
| `src/stores/app-store.ts` | Add `dashboardRevision` |
| `src/pages/clips.tsx` | Orchestrate list/detail states |
| `src/pages/dashboard.tsx` | Add activity feed, watch dashboardRevision |
| `src/components/clips/clip-grid.tsx` | Add checkbox support, use store filters |
| `src/components/clips/clip-card.tsx` | Add checkbox overlay, context menu |
| `src/components/layout/sidebar.tsx` | Add "Products" nav item |
| `src/components/layout/app-shell.tsx` | Register Products page, new WS handlers, dashboard bumps |
| `src/components/dashboard/stat-cards.tsx` | Accept storage warning state |
| `src/pages/settings.tsx` | Add storage management section |
| `sidecar/src/app.py` | Register new routers, start cleanup worker |
| `sidecar/src/config.py` | Add cleanup settings |
| `sidecar/src/models/schemas.py` | Add trim, product, storage schemas |

---

### Task 1: Database Migration — Product Enhancements

**Files:**
- Create: `src-tauri/src/db/migrations/004_product_enhancements.sql`
- Modify: `src-tauri/src/db/init.rs`

- [ ] **Step 1: Create migration file**

Create `src-tauri/src/db/migrations/004_product_enhancements.sql`:

```sql
ALTER TABLE products ADD COLUMN tiktok_url TEXT;
ALTER TABLE products ADD COLUMN updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'));
```

- [ ] **Step 2: Register migration in init.rs**

In `src-tauri/src/db/init.rs`, add migration 004 to the migration list. Follow the same pattern as existing migrations (003). The init function reads and runs each SQL file in order, tracking which have been applied.

Find the migration list and append:

```rust
(4, include_str!("migrations/004_product_enhancements.sql")),
```

- [ ] **Step 3: Verify build**

Run from `src-tauri/`:
```bash
cargo check
```
Expected: compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/migrations/004_product_enhancements.sql src-tauri/src/db/init.rs
git commit -m "feat(db): add migration 004 for product enhancements (tiktok_url, updated_at)"
```

---

### Task 2: Rust — Clip Management Commands

**Files:**
- Modify: `src-tauri/src/commands/clips.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add `ListClipsFilteredInput` struct and `list_clips_filtered` command**

In `src-tauri/src/commands/clips.rs`, add below the existing imports:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ListClipsFilteredInput {
    pub status: Option<String>,
    pub account_id: Option<i64>,
    pub scene_type: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub search: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[tauri::command]
pub fn list_clips_filtered(
    state: State<'_, AppState>,
    input: ListClipsFilteredInput,
) -> Result<Vec<Clip>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let mut sql = String::from(
        "SELECT \
         c.id, c.recording_id, c.account_id, a.username, \
         c.title, c.file_path, c.thumbnail_path, c.duration_seconds, c.file_size_bytes, \
         c.start_time, c.end_time, c.status, c.quality_score, c.scene_type, c.ai_tags_json, \
         c.notes, c.created_at, c.updated_at \
         FROM clips c \
         INNER JOIN accounts a ON a.id = c.account_id \
         WHERE 1=1",
    );
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(ref status) = input.status {
        if status != "all" {
            sql.push_str(&format!(" AND c.status = ?{idx}"));
            params_vec.push(Box::new(status.clone()));
            idx += 1;
        }
    }
    if let Some(account_id) = input.account_id {
        sql.push_str(&format!(" AND c.account_id = ?{idx}"));
        params_vec.push(Box::new(account_id));
        idx += 1;
    }
    if let Some(ref scene_type) = input.scene_type {
        if scene_type != "all" {
            sql.push_str(&format!(" AND c.scene_type = ?{idx}"));
            params_vec.push(Box::new(scene_type.clone()));
            idx += 1;
        }
    }
    if let Some(ref date_from) = input.date_from {
        sql.push_str(&format!(" AND c.created_at >= ?{idx}"));
        params_vec.push(Box::new(date_from.clone()));
        idx += 1;
    }
    if let Some(ref date_to) = input.date_to {
        sql.push_str(&format!(" AND c.created_at <= ?{idx}"));
        params_vec.push(Box::new(format!("{date_to} 23:59:59")));
        idx += 1;
    }
    if let Some(ref search) = input.search {
        if !search.trim().is_empty() {
            let pattern = format!("%{}%", search.trim());
            sql.push_str(&format!(
                " AND (c.title LIKE ?{idx} OR c.notes LIKE ?{})",
                idx + 1
            ));
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
            idx += 2;
        }
    }

    let sort_col = match input.sort_by.as_deref() {
        Some("duration") => "c.duration_seconds",
        Some("file_size") => "c.file_size_bytes",
        Some("title") => "c.title",
        _ => "c.created_at",
    };
    let sort_dir = match input.sort_order.as_deref() {
        Some("asc") => "ASC",
        _ => "DESC",
    };
    sql.push_str(&format!(" ORDER BY {sort_col} {sort_dir}"));

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params_refs.as_slice(), map_clip_row)
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}
```

- [ ] **Step 2: Add `get_clip_by_id` command**

```rust
#[tauri::command]
pub fn get_clip_by_id(state: State<'_, AppState>, clip_id: i64) -> Result<Clip, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT \
         c.id, c.recording_id, c.account_id, a.username, \
         c.title, c.file_path, c.thumbnail_path, c.duration_seconds, c.file_size_bytes, \
         c.start_time, c.end_time, c.status, c.quality_score, c.scene_type, c.ai_tags_json, \
         c.notes, c.created_at, c.updated_at \
         FROM clips c \
         INNER JOIN accounts a ON a.id = c.account_id \
         WHERE c.id = ?1",
        [clip_id],
        map_clip_row,
    )
    .map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Add `update_clip_status` command**

```rust
#[tauri::command]
pub fn update_clip_status(
    state: State<'_, AppState>,
    clip_id: i64,
    new_status: String,
) -> Result<(), String> {
    let valid = ["draft", "ready", "posted", "archived"];
    if !valid.contains(&new_status.as_str()) {
        return Err(format!("Invalid status: {new_status}"));
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let changed = conn
        .execute(
            &format!(
                "UPDATE clips SET status = ?1, updated_at = {} WHERE id = ?2",
                SQL_NOW_HCM
            ),
            params![&new_status, clip_id],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err(format!("Clip {clip_id} not found"));
    }
    Ok(())
}
```

- [ ] **Step 4: Add `update_clip_title` and `update_clip_notes` commands**

```rust
#[tauri::command]
pub fn update_clip_title(
    state: State<'_, AppState>,
    clip_id: i64,
    title: String,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        &format!(
            "UPDATE clips SET title = ?1, updated_at = {} WHERE id = ?2",
            SQL_NOW_HCM
        ),
        params![&title, clip_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn update_clip_notes(
    state: State<'_, AppState>,
    clip_id: i64,
    notes: String,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        &format!(
            "UPDATE clips SET notes = ?1, updated_at = {} WHERE id = ?2",
            SQL_NOW_HCM
        ),
        params![&notes, clip_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 5: Add `batch_update_clip_status` command**

```rust
#[tauri::command]
pub fn batch_update_clip_status(
    state: State<'_, AppState>,
    clip_ids: Vec<i64>,
    new_status: String,
) -> Result<(), String> {
    let valid = ["draft", "ready", "posted", "archived"];
    if !valid.contains(&new_status.as_str()) {
        return Err(format!("Invalid status: {new_status}"));
    }
    if clip_ids.is_empty() {
        return Ok(());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let placeholders: Vec<String> = (1..=clip_ids.len()).map(|i| format!("?{}", i + 1)).collect();
    let sql = format!(
        "UPDATE clips SET status = ?1, updated_at = {} WHERE id IN ({})",
        SQL_NOW_HCM,
        placeholders.join(", ")
    );
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    params_vec.push(Box::new(new_status));
    for id in &clip_ids {
        params_vec.push(Box::new(*id));
    }
    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, params_refs.as_slice())
        .map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 6: Add `batch_delete_clips` command**

```rust
#[tauri::command]
pub fn batch_delete_clips(state: State<'_, AppState>, clip_ids: Vec<i64>) -> Result<(), String> {
    if clip_ids.is_empty() {
        return Ok(());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let placeholders: Vec<String> = (1..=clip_ids.len()).map(|i| format!("?{i}")).collect();
    let sql = format!(
        "SELECT file_path, thumbnail_path FROM clips WHERE id IN ({})",
        placeholders.join(", ")
    );
    let params_refs: Vec<Box<dyn rusqlite::types::ToSql>> =
        clip_ids.iter().map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>).collect();
    let refs: Vec<&dyn rusqlite::types::ToSql> = params_refs.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let file_rows: Vec<(String, Option<String>)> = stmt
        .query_map(refs.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    for (fp, tp) in &file_rows {
        let _ = std::fs::remove_file(fp);
        if let Some(t) = tp {
            let _ = std::fs::remove_file(t);
        }
    }

    let del_sql = format!(
        "DELETE FROM clips WHERE id IN ({})",
        placeholders.join(", ")
    );
    conn.execute(&del_sql, refs.as_slice())
        .map_err(|e| e.to_string())?;

    Ok(())
}
```

- [ ] **Step 7: Register all new clip commands in `lib.rs`**

In `src-tauri/src/lib.rs`, add to the `invoke_handler` macro:

```rust
commands::clips::list_clips_filtered,
commands::clips::get_clip_by_id,
commands::clips::update_clip_status,
commands::clips::update_clip_title,
commands::clips::update_clip_notes,
commands::clips::batch_update_clip_status,
commands::clips::batch_delete_clips,
```

- [ ] **Step 8: Verify build**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/commands/clips.rs src-tauri/src/lib.rs
git commit -m "feat(clips): add filtered list, status update, batch ops, and delete commands"
```

---

### Task 3: Frontend — Types + API Wrappers for Clips

**Files:**
- Modify: `src/types/index.ts`
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Add Product and filter types to `src/types/index.ts`**

Append to the file:

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

export interface UpdateProductInput {
  name?: string;
  description?: string | null;
  sku?: string | null;
  image_url?: string | null;
  tiktok_shop_id?: string | null;
  tiktok_url?: string | null;
  price?: number | null;
  category?: string | null;
}

export interface ClipFilters {
  status: ClipStatus | "all";
  accountId: number | null;
  sceneType: SceneType | "all";
  dateFrom: string | null;
  dateTo: string | null;
  search: string;
  sortBy: "created_at" | "duration" | "file_size" | "title";
  sortOrder: "asc" | "desc";
}

export type ViewMode = "grid" | "list";
```

- [ ] **Step 2: Add clip management API wrappers to `src/lib/api.ts`**

Append to the file:

```typescript
import type { ClipFilters, Product, CreateProductInput, UpdateProductInput } from "@/types";

export async function listClipsFiltered(filters: ClipFilters): Promise<Clip[]> {
  return invoke<Clip[]>("list_clips_filtered", {
    input: {
      status: filters.status === "all" ? null : filters.status,
      account_id: filters.accountId,
      scene_type: filters.sceneType === "all" ? null : filters.sceneType,
      date_from: filters.dateFrom,
      date_to: filters.dateTo,
      search: filters.search || null,
      sort_by: filters.sortBy,
      sort_order: filters.sortOrder,
    },
  });
}

export async function getClipById(clipId: number): Promise<Clip> {
  return invoke<Clip>("get_clip_by_id", { clip_id: clipId });
}

export async function updateClipStatus(clipId: number, newStatus: string): Promise<void> {
  await invoke("update_clip_status", { clip_id: clipId, new_status: newStatus });
}

export async function updateClipTitle(clipId: number, title: string): Promise<void> {
  await invoke("update_clip_title", { clip_id: clipId, title });
}

export async function updateClipNotes(clipId: number, notes: string): Promise<void> {
  await invoke("update_clip_notes", { clip_id: clipId, notes });
}

export async function batchUpdateClipStatus(clipIds: number[], newStatus: string): Promise<void> {
  await invoke("batch_update_clip_status", { clip_ids: clipIds, new_status: newStatus });
}

export async function batchDeleteClips(clipIds: number[]): Promise<void> {
  await invoke("batch_delete_clips", { clip_ids: clipIds });
}
```

- [ ] **Step 3: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 4: Commit**

```bash
git add src/types/index.ts src/lib/api.ts
git commit -m "feat(clips): add clip filter types and API wrappers"
```

---

### Task 4: Frontend — Clip Store Expansion

**Files:**
- Modify: `src/stores/clip-store.ts`

- [ ] **Step 1: Rewrite clip-store.ts**

Replace the entire content of `src/stores/clip-store.ts`:

```typescript
import { create } from "zustand";
import { listClipsFiltered, batchUpdateClipStatus, batchDeleteClips } from "@/lib/api";
import type { Clip, ClipFilters, ClipStatus, ViewMode } from "@/types";

const DEFAULT_FILTERS: ClipFilters = {
  status: "all",
  accountId: null,
  sceneType: "all",
  dateFrom: null,
  dateTo: null,
  search: "",
  sortBy: "created_at",
  sortOrder: "desc",
};

type ClipStore = {
  clips: Clip[];
  filters: ClipFilters;
  viewMode: ViewMode;
  selectedClipIds: Set<number>;
  loading: boolean;
  activeClipId: number | null;
  clipsRevision: number;

  fetchClips: () => Promise<void>;
  setFilter: (partial: Partial<ClipFilters>) => void;
  resetFilters: () => void;
  setViewMode: (mode: ViewMode) => void;
  toggleSelect: (clipId: number) => void;
  selectAll: () => void;
  clearSelection: () => void;
  batchUpdateStatus: (status: ClipStatus) => Promise<void>;
  batchDelete: () => Promise<void>;
  setActiveClipId: (id: number | null) => void;
  bumpClipsRevision: () => void;
};

export const useClipStore = create<ClipStore>((set, get) => ({
  clips: [],
  filters: { ...DEFAULT_FILTERS },
  viewMode: "grid",
  selectedClipIds: new Set(),
  loading: false,
  activeClipId: null,
  clipsRevision: 0,

  fetchClips: async () => {
    set({ loading: true });
    try {
      const clips = await listClipsFiltered(get().filters);
      set({ clips, loading: false });
    } catch {
      set({ clips: [], loading: false });
    }
  },

  setFilter: (partial) => {
    set((s) => ({ filters: { ...s.filters, ...partial } }));
    void get().fetchClips();
  },

  resetFilters: () => {
    set({ filters: { ...DEFAULT_FILTERS } });
    void get().fetchClips();
  },

  setViewMode: (mode) => set({ viewMode: mode }),

  toggleSelect: (clipId) =>
    set((s) => {
      const next = new Set(s.selectedClipIds);
      if (next.has(clipId)) {
        next.delete(clipId);
      } else {
        next.add(clipId);
      }
      return { selectedClipIds: next };
    }),

  selectAll: () =>
    set((s) => ({
      selectedClipIds: new Set(s.clips.map((c) => c.id)),
    })),

  clearSelection: () => set({ selectedClipIds: new Set() }),

  batchUpdateStatus: async (status) => {
    const ids = Array.from(get().selectedClipIds);
    if (ids.length === 0) return;
    await batchUpdateClipStatus(ids, status);
    set({ selectedClipIds: new Set() });
    void get().fetchClips();
  },

  batchDelete: async () => {
    const ids = Array.from(get().selectedClipIds);
    if (ids.length === 0) return;
    await batchDeleteClips(ids);
    set({ selectedClipIds: new Set() });
    void get().fetchClips();
  },

  setActiveClipId: (id) => set({ activeClipId: id }),

  bumpClipsRevision: () => set((s) => ({ clipsRevision: s.clipsRevision + 1 })),
}));
```

- [ ] **Step 2: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 3: Commit**

```bash
git add src/stores/clip-store.ts
git commit -m "feat(clips): expand clip store with filters, selection, and batch operations"
```

---

### Task 5: Frontend — Clip Toolbar Component

**Files:**
- Create: `src/components/clips/clip-toolbar.tsx`

- [ ] **Step 1: Create the toolbar component**

Create `src/components/clips/clip-toolbar.tsx` with filter dropdowns, search, view toggle, and batch action bar. The component reads from and writes to the clip store.

Key elements:
- Status filter: select with options All / Draft / Ready / Posted / Archived
- Account filter: select populated from `useAccountStore`
- Search: debounced text input (300ms)
- View mode: two icon buttons (grid/list)
- Sort: select for sort_by + sort_order toggle
- Batch bar: shown when `selectedClipIds.size > 0`, with status dropdown + delete button

Use existing `Button`, `Badge` from `@/components/ui/`. Use native `<select>` styled with Tailwind for filter dropdowns (avoids adding new shadcn components).

- [ ] **Step 2: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 3: Commit**

```bash
git add src/components/clips/clip-toolbar.tsx
git commit -m "feat(clips): add clip toolbar with filters, search, view toggle, and batch actions"
```

---

### Task 6: Frontend — Clip List View (Table)

**Files:**
- Create: `src/components/clips/clip-list.tsx`

- [ ] **Step 1: Create the list/table view component**

Create `src/components/clips/clip-list.tsx`:
- Table with columns: checkbox, thumbnail (48×48 via `convertFileSrc`), title, account, duration, size, status badge, scene type, created_at
- Each row clickable → calls `setActiveClipId(clip.id)` from clip store
- Checkbox per row → calls `toggleSelect(clip.id)`
- Status badge uses the color scheme from spec: Draft=zinc, Ready=emerald, Posted=blue, Archived=zinc-darker
- Uses existing `Table` from `@/components/ui/table` if available, or plain `<table>` with Tailwind

- [ ] **Step 2: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 3: Commit**

```bash
git add src/components/clips/clip-list.tsx
git commit -m "feat(clips): add clip list/table view component"
```

---

### Task 7: Frontend — Upgrade ClipGrid + ClipCard

**Files:**
- Modify: `src/components/clips/clip-grid.tsx`
- Modify: `src/components/clips/clip-card.tsx`

- [ ] **Step 1: Update ClipGrid to use store**

Refactor `clip-grid.tsx`:
- Remove internal `clips` state, `load` function, and `clipsRevision` watcher
- Read `clips`, `loading`, `selectedClipIds` from `useClipStore`
- Keep the date/user grouping logic (it's useful)
- Call `fetchClips()` on mount if clips are empty
- Pass `selected` and `onToggleSelect` props to each `ClipCard`

- [ ] **Step 2: Update ClipCard with checkbox and click handler**

Modify `clip-card.tsx`:
- Add `selected?: boolean`, `onToggleSelect?: () => void`, `onClick?: () => void` props
- Render a checkbox overlay in the top-left corner (visible on hover or when selected)
- Clicking the card calls `onClick` (navigate to detail)
- Clicking the checkbox calls `onToggleSelect` (stops propagation)
- Add status badge (was commented out — uncomment and style per spec colors)

- [ ] **Step 3: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 4: Commit**

```bash
git add src/components/clips/clip-grid.tsx src/components/clips/clip-card.tsx
git commit -m "feat(clips): upgrade grid and card with selection, status badges, and store integration"
```

---

### Task 8: Frontend — Clips Page Orchestration

**Files:**
- Modify: `src/pages/clips.tsx`

- [ ] **Step 1: Rewrite clips page with list/detail states**

Replace `src/pages/clips.tsx`:

```typescript
import { useEffect } from "react";
import { ClipToolbar } from "@/components/clips/clip-toolbar";
import { ClipGrid } from "@/components/clips/clip-grid";
import { ClipList } from "@/components/clips/clip-list";
import { ClipDetail } from "@/components/clips/clip-detail";
import { useClipStore } from "@/stores/clip-store";

export function ClipsPage() {
  const activeClipId = useClipStore((s) => s.activeClipId);
  const viewMode = useClipStore((s) => s.viewMode);
  const fetchClips = useClipStore((s) => s.fetchClips);
  const clipsRevision = useClipStore((s) => s.clipsRevision);

  useEffect(() => {
    void fetchClips();
  }, [fetchClips, clipsRevision]);

  if (activeClipId != null) {
    return <ClipDetail clipId={activeClipId} />;
  }

  return (
    <div className="space-y-4">
      <ClipToolbar />
      {viewMode === "grid" ? <ClipGrid /> : <ClipList />}
    </div>
  );
}
```

- [ ] **Step 2: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 3: Commit**

```bash
git add src/pages/clips.tsx
git commit -m "feat(clips): orchestrate clips page with list/detail states and view modes"
```

---

### Task 9: Frontend — Video Player Component

**Files:**
- Create: `src/components/clips/video-player.tsx`

- [ ] **Step 1: Create the video player component**

Create `src/components/clips/video-player.tsx`:

Props: `src: string` (convertFileSrc URL), `onTimeUpdate?: (currentTime: number) => void`

Implementation:
- `<video>` element with `ref`
- Custom controls bar below video (don't use native controls):
  - Play/Pause button (toggle icon)
  - Current time display `MM:SS` / Total duration `MM:SS`
  - Seek bar: `<input type="range">` styled with Tailwind, bound to `video.currentTime`
  - Volume: small range slider + mute icon button
  - Speed selector: `<select>` with 0.5×, 1×, 1.5×, 2× options
  - Fullscreen button (uses `video.requestFullscreen()`)
- Expose `ref` with `seek(time: number)` and `playRange(start: number, end: number)` methods via `useImperativeHandle` (needed by trim controls)
- Call `onTimeUpdate` on the video's `timeupdate` event

- [ ] **Step 2: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 3: Commit**

```bash
git add src/components/clips/video-player.tsx
git commit -m "feat(clips): add video player component with custom controls"
```

---

### Task 10: Frontend — Clip Detail View

**Files:**
- Create: `src/components/clips/clip-detail.tsx`

- [ ] **Step 1: Create the clip detail component**

Create `src/components/clips/clip-detail.tsx`:

Props: `clipId: number`

Layout (two columns):
- Left (~65%): VideoPlayer + TrimControls (Task 12)
- Right (~35%): Metadata panel

Metadata panel:
- Back button → `setActiveClipId(null)` 
- Title: inline-editable text (click to edit, blur/Enter to save via `updateClipTitle`)
- Account: `@username` text
- Status: dropdown to change (calls `updateClipStatus`)
- Duration, File Size, Scene Type: read-only display
- Created at: formatted date
- Notes: inline-editable textarea (calls `updateClipNotes`)
- Products section: placeholder for Task 18 (just show "Products: coming soon" text)
- "Open in Finder" button → calls `openPathInSystem(clip.file_path)`
- "Delete" button → confirm dialog → `batchDeleteClips([clipId])` → `setActiveClipId(null)`

Fetch clip data via `getClipById(clipId)` on mount. Refetch when `clipsRevision` changes.

- [ ] **Step 2: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 3: Commit**

```bash
git add src/components/clips/clip-detail.tsx
git commit -m "feat(clips): add clip detail view with metadata panel and inline editing"
```

---

### Task 11: Sidecar — Trim API Endpoint

**Files:**
- Create: `sidecar/src/routes/trim.py`
- Modify: `sidecar/src/models/schemas.py`
- Modify: `sidecar/src/app.py`

- [ ] **Step 1: Add trim schemas to `models/schemas.py`**

Append to `sidecar/src/models/schemas.py`:

```python
class TrimClipRequest(BaseModel):
    source_path: str
    start_sec: float
    end_sec: float
    account_id: int
    recording_id: int


class TrimClipResponse(BaseModel):
    file_path: str
    thumbnail_path: str
    duration_sec: float
```

- [ ] **Step 2: Create trim route**

Create `sidecar/src/routes/trim.py`:

```python
import asyncio
import logging
import re
from pathlib import Path

from fastapi import APIRouter, HTTPException

from config import settings
from core.time_hcm import today_ymd_hcm
from models.schemas import TrimClipRequest, TrimClipResponse

logger = logging.getLogger(__name__)
router = APIRouter()

_CLIP_FILE = re.compile(r"^clip_(\d{3})(?:_trimmed)?\.mp4$", re.IGNORECASE)
_CLIP_THUMB = re.compile(r"^clip_(\d{3})(?:_trimmed)?\.jpg$", re.IGNORECASE)


def _next_trimmed_index(out_dir: Path) -> int:
    max_n = 0
    if not out_dir.is_dir():
        return 1
    for p in out_dir.iterdir():
        if not p.is_file():
            continue
        for pat in (_CLIP_FILE, _CLIP_THUMB):
            m = pat.match(p.name)
            if m:
                max_n = max(max_n, int(m.group(1)))
                break
    return max_n + 1


def _trim_sync(src: Path, dest: Path, start_sec: float, duration_sec: float) -> None:
    import subprocess

    dest.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        "ffmpeg", "-y",
        "-ss", str(start_sec),
        "-i", str(src),
        "-t", str(duration_sec),
        "-c", "copy",
        "-avoid_negative_ts", "make_zero",
        str(dest),
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False, timeout=600)
    if proc.returncode != 0:
        err = (proc.stderr or proc.stdout or "").strip()
        raise RuntimeError(f"ffmpeg trim failed ({proc.returncode}): {err[:2000]}")


def _extract_thumbnail_sync(video_path: Path, dest_jpg: Path, clip_duration_sec: float) -> None:
    import subprocess

    offset = min(1.0, max(0.0, clip_duration_sec / 2))
    cmd = [
        "ffmpeg", "-y",
        "-ss", str(offset),
        "-i", str(video_path),
        "-vframes", "1",
        "-q:v", "2",
        str(dest_jpg),
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False, timeout=300)
    if proc.returncode != 0:
        err = (proc.stderr or proc.stdout or "").strip()
        raise RuntimeError(f"ffmpeg thumbnail failed ({proc.returncode}): {err[:2000]}")


@router.post("/api/clips/trim", response_model=TrimClipResponse)
async def trim_clip(body: TrimClipRequest):
    src = Path(body.source_path).expanduser()
    if not src.is_file():
        raise HTTPException(status_code=400, detail="Source file not found")
    if body.end_sec <= body.start_sec:
        raise HTTPException(status_code=400, detail="end_sec must be greater than start_sec")

    duration = body.end_sec - body.start_sec

    parts = src.parts
    username = "unknown"
    for i, part in enumerate(parts):
        if part == "clips" and i + 1 < len(parts):
            username = parts[i + 1]
            break

    date_str = today_ymd_hcm()
    out_dir = settings.storage_path / "clips" / username / date_str
    out_dir.mkdir(parents=True, exist_ok=True)

    idx = _next_trimmed_index(out_dir)
    clip_path = out_dir / f"clip_{idx:03d}_trimmed.mp4"
    thumb_path = out_dir / f"clip_{idx:03d}_trimmed.jpg"

    await asyncio.to_thread(_trim_sync, src, clip_path, body.start_sec, duration)
    await asyncio.to_thread(_extract_thumbnail_sync, clip_path, thumb_path, duration)

    return TrimClipResponse(
        file_path=str(clip_path),
        thumbnail_path=str(thumb_path),
        duration_sec=duration,
    )
```

- [ ] **Step 3: Register trim router in `app.py`**

In `sidecar/src/app.py`, add:

```python
from routes import trim as trim_routes
```

And inside `create_app()`:

```python
app.include_router(trim_routes.router)
```

- [ ] **Step 4: Verify sidecar**

```bash
cd sidecar && uv run ruff check src && uv run ruff format --check src
```

- [ ] **Step 5: Commit**

```bash
git add sidecar/src/routes/trim.py sidecar/src/models/schemas.py sidecar/src/app.py
git commit -m "feat(sidecar): add clip trim endpoint with FFmpeg stream copy"
```

---

### Task 12: Rust — Insert Trimmed Clip Command

**Files:**
- Modify: `src-tauri/src/commands/clips.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add `InsertTrimmedClipInput` and command**

In `src-tauri/src/commands/clips.rs`:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InsertTrimmedClipInput {
    pub recording_id: i64,
    pub account_id: i64,
    pub file_path: String,
    pub thumbnail_path: String,
    pub duration_sec: f64,
    pub start_sec: f64,
    pub end_sec: f64,
}

#[tauri::command]
pub fn insert_trimmed_clip(
    state: State<'_, AppState>,
    input: InsertTrimmedClipInput,
) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let file_size: i64 = std::fs::metadata(&input.file_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    let duration = input.duration_sec.round().max(0.0) as i64;
    let thumb = input.thumbnail_path.trim();
    let thumb_opt = if thumb.is_empty() { None } else { Some(thumb) };

    conn.execute(
        &format!(
            "INSERT INTO clips (\
               recording_id, account_id, title, file_path, thumbnail_path, \
               duration_seconds, file_size_bytes, start_time, end_time, status, created_at, updated_at\
             ) VALUES (?1, ?2, 'Trimmed clip', ?3, ?4, ?5, ?6, ?7, ?8, 'draft', {}, {})",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![
            input.recording_id,
            input.account_id,
            &input.file_path,
            thumb_opt,
            duration,
            file_size,
            input.start_sec,
            input.end_sec,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}
```

- [ ] **Step 2: Register in `lib.rs`**

Add `commands::clips::insert_trimmed_clip` to invoke_handler.

- [ ] **Step 3: Verify build**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/clips.rs src-tauri/src/lib.rs
git commit -m "feat(clips): add insert_trimmed_clip command"
```

---

### Task 13: Frontend — Trim Controls + API Integration

**Files:**
- Create: `src/components/clips/trim-controls.tsx`
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Add trim API wrapper to `api.ts`**

```typescript
export async function trimClip(body: {
  source_path: string;
  start_sec: number;
  end_sec: number;
  account_id: number;
  recording_id: number;
}): Promise<{ file_path: string; thumbnail_path: string; duration_sec: number }> {
  return sidecarJson("/api/clips/trim", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function insertTrimmedClip(input: {
  recording_id: number;
  account_id: number;
  file_path: string;
  thumbnail_path: string;
  duration_sec: number;
  start_sec: number;
  end_sec: number;
}): Promise<number> {
  return invoke<number>("insert_trimmed_clip", { input });
}
```

- [ ] **Step 2: Create trim controls component**

Create `src/components/clips/trim-controls.tsx`:

Props: `clip: Clip`, `playerRef: RefObject<VideoPlayerHandle>`, `onTrimComplete: (newClipId: number) => void`

Features:
- Two range inputs (start/end) or a dual-thumb range slider
- Time display inputs (`MM:SS.s` format) for precise values
- "Preview" button: calls `playerRef.current.playRange(startSec, endSec)`
- "Create trimmed clip" button: calls `trimClip()` → `insertTrimmedClip()` → `onTrimComplete(newId)` → `bumpClipsRevision()`
- Loading state while trimming
- Start defaults to 0, end defaults to clip duration

- [ ] **Step 3: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 4: Commit**

```bash
git add src/components/clips/trim-controls.tsx src/lib/api.ts
git commit -m "feat(clips): add trim controls component and trim API integration"
```

---

### Task 14: Rust — Product CRUD Commands

**Files:**
- Create: `src-tauri/src/commands/products.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/db/models.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add `Product` model to `models.rs`**

Append to `src-tauri/src/db/models.rs`:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Product {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub sku: Option<String>,
    pub image_url: Option<String>,
    pub tiktok_shop_id: Option<String>,
    pub tiktok_url: Option<String>,
    pub price: Option<f64>,
    pub category: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

- [ ] **Step 2: Create `products.rs` with all CRUD + tagging commands**

Create `src-tauri/src/commands/products.rs`:

```rust
use crate::db::models::Product;
use crate::time_hcm::SQL_NOW_HCM;
use crate::AppState;
use rusqlite::{params, Row};
use serde::Deserialize;
use tauri::State;

fn map_product_row(row: &Row) -> rusqlite::Result<Product> {
    Ok(Product {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        sku: row.get(3)?,
        image_url: row.get(4)?,
        tiktok_shop_id: row.get(5)?,
        tiktok_url: row.get(6)?,
        price: row.get(7)?,
        category: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

const PRODUCT_COLS: &str = "id, name, description, sku, image_url, tiktok_shop_id, tiktok_url, price, category, created_at, updated_at";

#[tauri::command]
pub fn list_products(state: State<'_, AppState>) -> Result<Vec<Product>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(&format!("SELECT {PRODUCT_COLS} FROM products ORDER BY created_at DESC"))
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], map_product_row).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub fn get_product_by_id(state: State<'_, AppState>, product_id: i64) -> Result<Product, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        &format!("SELECT {PRODUCT_COLS} FROM products WHERE id = ?1"),
        [product_id],
        map_product_row,
    )
    .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateProductInput {
    pub name: String,
    pub description: Option<String>,
    pub sku: Option<String>,
    pub image_url: Option<String>,
    pub tiktok_shop_id: Option<String>,
    pub tiktok_url: Option<String>,
    pub price: Option<f64>,
    pub category: Option<String>,
}

#[tauri::command]
pub fn create_product(state: State<'_, AppState>, input: CreateProductInput) -> Result<i64, String> {
    if input.name.trim().is_empty() {
        return Err("Product name is required".to_string());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        &format!(
            "INSERT INTO products (name, description, sku, image_url, tiktok_shop_id, tiktok_url, price, category, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, {}, {})",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![
            input.name.trim(),
            input.description,
            input.sku,
            input.image_url,
            input.tiktok_shop_id,
            input.tiktok_url,
            input.price,
            input.category,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateProductInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub sku: Option<String>,
    pub image_url: Option<String>,
    pub tiktok_shop_id: Option<String>,
    pub tiktok_url: Option<String>,
    pub price: Option<f64>,
    pub category: Option<String>,
}

#[tauri::command]
pub fn update_product(
    state: State<'_, AppState>,
    product_id: i64,
    input: UpdateProductInput,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut sets = Vec::new();
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    macro_rules! add_field {
        ($field:expr, $col:expr) => {
            if let Some(ref val) = $field {
                sets.push(format!("{} = ?{idx}", $col));
                params_vec.push(Box::new(val.clone()));
                idx += 1;
            }
        };
    }
    add_field!(input.name, "name");
    add_field!(input.description, "description");
    add_field!(input.sku, "sku");
    add_field!(input.image_url, "image_url");
    add_field!(input.tiktok_shop_id, "tiktok_shop_id");
    add_field!(input.tiktok_url, "tiktok_url");
    add_field!(input.category, "category");
    if let Some(price) = input.price {
        sets.push(format!("price = ?{idx}"));
        params_vec.push(Box::new(price));
        idx += 1;
    }

    if sets.is_empty() {
        return Ok(());
    }
    sets.push(format!("updated_at = {SQL_NOW_HCM}"));

    let sql = format!("UPDATE products SET {} WHERE id = ?{idx}", sets.join(", "));
    params_vec.push(Box::new(product_id));

    let refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, refs.as_slice()).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_product(state: State<'_, AppState>, product_id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM clip_products WHERE product_id = ?1", [product_id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM products WHERE id = ?1", [product_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn list_clip_products(state: State<'_, AppState>, clip_id: i64) -> Result<Vec<Product>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.name, p.description, p.sku, p.image_url, p.tiktok_shop_id, \
             p.tiktok_url, p.price, p.category, p.created_at, p.updated_at \
             FROM products p \
             INNER JOIN clip_products cp ON cp.product_id = p.id \
             WHERE cp.clip_id = ?1 \
             ORDER BY p.name",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([clip_id], map_product_row).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub fn tag_clip_product(
    state: State<'_, AppState>,
    clip_id: i64,
    product_id: i64,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO clip_products (clip_id, product_id) VALUES (?1, ?2)",
        params![clip_id, product_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn untag_clip_product(
    state: State<'_, AppState>,
    clip_id: i64,
    product_id: i64,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM clip_products WHERE clip_id = ?1 AND product_id = ?2",
        params![clip_id, product_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn batch_tag_clip_products(
    state: State<'_, AppState>,
    clip_ids: Vec<i64>,
    product_id: i64,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    for clip_id in &clip_ids {
        conn.execute(
            "INSERT OR IGNORE INTO clip_products (clip_id, product_id) VALUES (?1, ?2)",
            params![clip_id, product_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
```

- [ ] **Step 3: Register module in `commands/mod.rs`**

Add to `src-tauri/src/commands/mod.rs`:

```rust
pub mod products;
```

- [ ] **Step 4: Register all product commands in `lib.rs`**

Add to invoke_handler in `src-tauri/src/lib.rs`:

```rust
commands::products::list_products,
commands::products::get_product_by_id,
commands::products::create_product,
commands::products::update_product,
commands::products::delete_product,
commands::products::list_clip_products,
commands::products::tag_clip_product,
commands::products::untag_clip_product,
commands::products::batch_tag_clip_products,
```

- [ ] **Step 5: Verify build**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/products.rs src-tauri/src/commands/mod.rs src-tauri/src/db/models.rs src-tauri/src/lib.rs
git commit -m "feat(products): add product CRUD and clip-product tagging commands"
```

---

### Task 15: Sidecar — TikTok Product Scraper + Route

**Files:**
- Create: `sidecar/src/tiktok/product_scraper.py`
- Create: `sidecar/src/routes/products.py`
- Modify: `sidecar/src/models/schemas.py`
- Modify: `sidecar/src/app.py`

- [ ] **Step 1: Add product schemas to `models/schemas.py`**

Append:

```python
class FetchProductRequest(BaseModel):
    url: str
    cookies_json: str | None = None


class FetchedProductData(BaseModel):
    name: str | None = None
    description: str | None = None
    price: float | None = None
    image_url: str | None = None
    category: str | None = None
    tiktok_shop_id: str | None = None


class FetchProductResponse(BaseModel):
    success: bool
    incomplete: bool = False
    data: FetchedProductData | None = None
    error: str | None = None
```

- [ ] **Step 2: Create product scraper module**

Create `sidecar/src/tiktok/product_scraper.py`:

```python
"""Fetch product info from a TikTok Shop URL via OG tags and JSON-LD."""

from __future__ import annotations

import json
import logging
import re
from dataclasses import dataclass

import httpx

logger = logging.getLogger(__name__)

_OG_PATTERN = re.compile(
    r'<meta\s+(?:property|name)=["\']og:(\w+)["\']\s+content=["\']([^"\']*)["\']',
    re.IGNORECASE,
)
_JSON_LD_PATTERN = re.compile(
    r'<script\s+type=["\']application/ld\+json["\']\s*>(.*?)</script>',
    re.DOTALL | re.IGNORECASE,
)


@dataclass
class ScrapedProduct:
    name: str | None = None
    description: str | None = None
    price: float | None = None
    image_url: str | None = None
    category: str | None = None
    tiktok_shop_id: str | None = None
    incomplete: bool = True


async def fetch_product_from_url(
    url: str, cookies_json: str | None = None
) -> ScrapedProduct:
    headers = {
        "User-Agent": (
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
            "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
        ),
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "Accept-Language": "en-US,en;q=0.9,vi;q=0.8",
    }

    cookies: dict[str, str] = {}
    if cookies_json:
        try:
            parsed = json.loads(cookies_json)
            if isinstance(parsed, dict):
                cookies = {k: str(v) for k, v in parsed.items()}
            elif isinstance(parsed, list):
                for c in parsed:
                    if isinstance(c, dict) and "name" in c and "value" in c:
                        cookies[c["name"]] = str(c["value"])
        except (json.JSONDecodeError, TypeError):
            pass

    product = ScrapedProduct()

    try:
        async with httpx.AsyncClient(
            follow_redirects=True, timeout=15.0, cookies=cookies
        ) as client:
            resp = await client.get(url, headers=headers)
            resp.raise_for_status()
            html = resp.text
    except Exception as exc:
        logger.warning("Failed to fetch product URL %s: %s", url, exc)
        product.incomplete = True
        return product

    og_tags: dict[str, str] = {}
    for m in _OG_PATTERN.finditer(html):
        og_tags[m.group(1).lower()] = m.group(2)

    product.name = og_tags.get("title")
    product.description = og_tags.get("description")
    product.image_url = og_tags.get("image")

    for m in _JSON_LD_PATTERN.finditer(html):
        try:
            data = json.loads(m.group(1))
            if isinstance(data, dict) and data.get("@type") == "Product":
                product.name = product.name or data.get("name")
                product.description = product.description or data.get("description")
                product.image_url = product.image_url or data.get("image")
                product.category = data.get("category")
                offers = data.get("offers", {})
                if isinstance(offers, dict) and "price" in offers:
                    try:
                        product.price = float(offers["price"])
                    except (ValueError, TypeError):
                        pass
                sku = data.get("sku")
                if sku:
                    product.tiktok_shop_id = str(sku)
                break
        except (json.JSONDecodeError, TypeError):
            continue

    has_required = product.name is not None
    product.incomplete = not has_required

    return product
```

- [ ] **Step 3: Create products route**

Create `sidecar/src/routes/products.py`:

```python
import logging

from fastapi import APIRouter

from models.schemas import (
    FetchedProductData,
    FetchProductRequest,
    FetchProductResponse,
)
from tiktok.product_scraper import fetch_product_from_url

logger = logging.getLogger(__name__)
router = APIRouter()


@router.post("/api/products/fetch-from-url", response_model=FetchProductResponse)
async def fetch_product(body: FetchProductRequest):
    url = body.url.strip()
    if not url:
        return FetchProductResponse(success=False, error="URL is required")

    try:
        result = await fetch_product_from_url(url, cookies_json=body.cookies_json)
    except Exception as exc:
        logger.exception("Product fetch failed for %s", url)
        return FetchProductResponse(success=False, error=str(exc))

    data = FetchedProductData(
        name=result.name,
        description=result.description,
        price=result.price,
        image_url=result.image_url,
        category=result.category,
        tiktok_shop_id=result.tiktok_shop_id,
    )

    return FetchProductResponse(
        success=result.name is not None,
        incomplete=result.incomplete,
        data=data,
    )
```

- [ ] **Step 4: Register in `app.py`**

Add import and include_router:

```python
from routes import products as product_routes
```

```python
app.include_router(product_routes.router, tags=["products"])
```

- [ ] **Step 5: Verify sidecar lint**

```bash
cd sidecar && uv run ruff check src && uv run ruff format --check src
```

- [ ] **Step 6: Commit**

```bash
git add sidecar/src/tiktok/product_scraper.py sidecar/src/routes/products.py sidecar/src/models/schemas.py sidecar/src/app.py
git commit -m "feat(sidecar): add TikTok product scraper and fetch-from-url endpoint"
```

---

### Task 16: Frontend — Product Store + API Wrappers

**Files:**
- Create: `src/stores/product-store.ts`
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Add product API wrappers to `api.ts`**

Append to `src/lib/api.ts`:

```typescript
export async function listProducts(): Promise<Product[]> {
  return invoke<Product[]>("list_products");
}

export async function getProductById(productId: number): Promise<Product> {
  return invoke<Product>("get_product_by_id", { product_id: productId });
}

export async function createProduct(input: CreateProductInput): Promise<number> {
  return invoke<number>("create_product", { input });
}

export async function updateProduct(productId: number, input: UpdateProductInput): Promise<void> {
  await invoke("update_product", { product_id: productId, input });
}

export async function deleteProduct(productId: number): Promise<void> {
  await invoke("delete_product", { product_id: productId });
}

export async function listClipProducts(clipId: number): Promise<Product[]> {
  return invoke<Product[]>("list_clip_products", { clip_id: clipId });
}

export async function tagClipProduct(clipId: number, productId: number): Promise<void> {
  await invoke("tag_clip_product", { clip_id: clipId, product_id: productId });
}

export async function untagClipProduct(clipId: number, productId: number): Promise<void> {
  await invoke("untag_clip_product", { clip_id: clipId, product_id: productId });
}

export async function batchTagClipProducts(clipIds: number[], productId: number): Promise<void> {
  await invoke("batch_tag_clip_products", { clip_ids: clipIds, product_id: productId });
}

export async function fetchProductFromUrl(
  url: string,
  cookiesJson?: string | null,
): Promise<{
  success: boolean;
  incomplete: boolean;
  data: {
    name: string | null;
    description: string | null;
    price: number | null;
    image_url: string | null;
    category: string | null;
    tiktok_shop_id: string | null;
  } | null;
  error: string | null;
}> {
  return sidecarJson("/api/products/fetch-from-url", {
    method: "POST",
    body: JSON.stringify({ url, cookies_json: cookiesJson ?? null }),
  });
}
```

- [ ] **Step 2: Create product store**

Create `src/stores/product-store.ts`:

```typescript
import { create } from "zustand";
import { listProducts } from "@/lib/api";
import type { Product } from "@/types";

type ProductStore = {
  products: Product[];
  loading: boolean;
  searchQuery: string;
  fetchProducts: () => Promise<void>;
  setSearchQuery: (q: string) => void;
};

export const useProductStore = create<ProductStore>((set) => ({
  products: [],
  loading: false,
  searchQuery: "",

  fetchProducts: async () => {
    set({ loading: true });
    try {
      const products = await listProducts();
      set({ products, loading: false });
    } catch {
      set({ products: [], loading: false });
    }
  },

  setSearchQuery: (q) => set({ searchQuery: q }),
}));
```

- [ ] **Step 3: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 4: Commit**

```bash
git add src/stores/product-store.ts src/lib/api.ts
git commit -m "feat(products): add product store and API wrappers"
```

---

### Task 17: Frontend — Products Page + Form

**Files:**
- Create: `src/pages/products.tsx`
- Create: `src/components/products/product-card.tsx`
- Create: `src/components/products/product-list.tsx`
- Create: `src/components/products/product-form.tsx`

- [ ] **Step 1: Create product card component**

Create `src/components/products/product-card.tsx`:
- Displays product image (or placeholder), name, price formatted, category badge
- Edit button → opens product form dialog
- Delete button → confirm → `deleteProduct()`

- [ ] **Step 2: Create product form dialog**

Create `src/components/products/product-form.tsx`:
- Dialog component with two tabs:
  - "Import from TikTok" tab: URL input + "Fetch" button. On fetch: calls `fetchProductFromUrl()`, fills form fields. Shows error/incomplete warnings.
  - "Manual" tab: form fields for name (required), description, SKU, price, category, image URL
- Props: `open`, `onClose`, `product?: Product` (null = create, non-null = edit)
- On save: calls `createProduct()` or `updateProduct()` → refreshes product store

- [ ] **Step 3: Create product list component**

Create `src/components/products/product-list.tsx`:
- Grid layout of product cards
- Search input (filters products client-side by name/SKU)
- "Add Product" button → opens product form

- [ ] **Step 4: Create products page**

Create `src/pages/products.tsx`:

```typescript
import { useEffect } from "react";
import { ProductList } from "@/components/products/product-list";
import { useProductStore } from "@/stores/product-store";

export function ProductsPage() {
  const fetchProducts = useProductStore((s) => s.fetchProducts);

  useEffect(() => {
    void fetchProducts();
  }, [fetchProducts]);

  return (
    <div className="space-y-6">
      <ProductList />
    </div>
  );
}
```

- [ ] **Step 5: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 6: Commit**

```bash
git add src/pages/products.tsx src/components/products/
git commit -m "feat(products): add products page with card grid, form dialog, and TikTok import"
```

---

### Task 18: Frontend — Product Picker + Clip Tagging

**Files:**
- Create: `src/components/products/product-picker.tsx`
- Modify: `src/components/clips/clip-detail.tsx`

- [ ] **Step 1: Create product picker dialog**

Create `src/components/products/product-picker.tsx`:
- Props: `clipId: number`, `open: boolean`, `onClose: () => void`
- Fetches all products via `listProducts()` and tagged products via `listClipProducts(clipId)`
- Searchable list with checkboxes showing tagged state
- Click item: toggle tag/untag via `tagClipProduct()` / `untagClipProduct()`
- "Quick create" link → opens product form inline

- [ ] **Step 2: Update clip detail to show products**

In `src/components/clips/clip-detail.tsx`, replace the "Products: coming soon" placeholder:
- Fetch products for clip on mount: `listClipProducts(clipId)`
- Display tagged products as small pills (image + name + × remove button)
- "Add Product" button → opens `ProductPicker` dialog
- On tag/untag: refetch product list

- [ ] **Step 3: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 4: Commit**

```bash
git add src/components/products/product-picker.tsx src/components/clips/clip-detail.tsx
git commit -m "feat(products): add product picker and clip tagging in detail view"
```

---

### Task 19: Sidebar + App Shell — Navigation Wiring

**Files:**
- Modify: `src/components/layout/sidebar.tsx`
- Modify: `src/components/layout/app-shell.tsx`

- [ ] **Step 1: Add Products to sidebar navigation**

In `src/components/layout/sidebar.tsx`, add to `navItems` array after "clips":

```typescript
{ id: "products", label: "Products", icon: "📦" },
```

- [ ] **Step 2: Update app-shell.tsx**

In `src/components/layout/app-shell.tsx`:

Add import:
```typescript
import { ProductsPage } from "@/pages/products";
```

Add `"products"` to the `PageId` type union.

Add to `pageMeta`:
```typescript
products: { title: "Products", subtitle: "Product catalog and tagging" },
```

Add to `pageComponents`:
```typescript
products: ProductsPage,
```

- [ ] **Step 3: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 4: Commit**

```bash
git add src/components/layout/sidebar.tsx src/components/layout/app-shell.tsx
git commit -m "feat(nav): add Products page to sidebar navigation"
```

---

### Task 20: Sidecar — Storage Stats API + Cleanup Worker

**Files:**
- Create: `sidecar/src/routes/storage.py`
- Create: `sidecar/src/core/cleanup.py`
- Modify: `sidecar/src/models/schemas.py`
- Modify: `sidecar/src/config.py`
- Modify: `sidecar/src/app.py`

- [ ] **Step 1: Add storage + cleanup settings to `config.py`**

Add to `Settings` class in `sidecar/src/config.py`:

```python
archive_retention_days: int = 30
storage_warn_percent: int = 80
storage_cleanup_percent: int = 95
cleanup_interval_minutes: int = 30
```

- [ ] **Step 2: Add storage schemas to `models/schemas.py`**

Append:

```python
class StorageStatsResponse(BaseModel):
    recordings_bytes: int = 0
    recordings_count: int = 0
    clips_bytes: int = 0
    clips_count: int = 0
    products_bytes: int = 0
    total_bytes: int = 0
    quota_bytes: int | None = None
    usage_percent: float = 0.0
```

- [ ] **Step 3: Create storage route**

Create `sidecar/src/routes/storage.py`:

```python
import logging
from pathlib import Path

from fastapi import APIRouter

from config import settings
from models.schemas import StorageStatsResponse

logger = logging.getLogger(__name__)
router = APIRouter()


def _dir_size(path: Path) -> tuple[int, int]:
    """Return (total_bytes, file_count) for a directory tree."""
    total = 0
    count = 0
    if not path.is_dir():
        return 0, 0
    for f in path.rglob("*"):
        if f.is_file():
            try:
                total += f.stat().st_size
                count += 1
            except OSError:
                pass
    return total, count


@router.get("/api/storage/stats", response_model=StorageStatsResponse)
async def storage_stats():
    root = settings.storage_path

    rec_bytes, rec_count = _dir_size(root / "recordings")
    clip_bytes, clip_count = _dir_size(root / "clips")
    prod_bytes, _ = _dir_size(root / "products")

    total = rec_bytes + clip_bytes + prod_bytes
    quota = int(settings.storage_quota_gb * 1_073_741_824) if settings.storage_quota_gb else None
    usage_pct = (total / quota * 100) if quota and quota > 0 else 0.0

    return StorageStatsResponse(
        recordings_bytes=rec_bytes,
        recordings_count=rec_count,
        clips_bytes=clip_bytes,
        clips_count=clip_count,
        products_bytes=prod_bytes,
        total_bytes=total,
        quota_bytes=quota,
        usage_percent=round(usage_pct, 1),
    )
```

- [ ] **Step 4: Create cleanup worker**

Create `sidecar/src/core/cleanup.py`:

```python
"""Periodic storage cleanup: retention-based + quota-based."""

from __future__ import annotations

import asyncio
import logging
import time
from pathlib import Path

from config import settings
from ws.manager import ws_manager

logger = logging.getLogger(__name__)


def _file_age_days(path: Path) -> float:
    try:
        mtime = path.stat().st_mtime
        return (time.time() - mtime) / 86400
    except OSError:
        return 0.0


def _dir_total_bytes(path: Path) -> int:
    total = 0
    if not path.is_dir():
        return 0
    for f in path.rglob("*"):
        if f.is_file():
            try:
                total += f.stat().st_size
            except OSError:
                pass
    return total


def _delete_old_recordings(root: Path, retention_days: int) -> tuple[int, int]:
    """Delete recording files older than retention_days. Returns (count, freed_bytes)."""
    if retention_days <= 0:
        return 0, 0
    rec_dir = root / "recordings"
    if not rec_dir.is_dir():
        return 0, 0
    count = 0
    freed = 0
    for f in rec_dir.rglob("*"):
        if f.is_file() and f.suffix.lower() in (".flv", ".mp4", ".ts"):
            age = _file_age_days(f)
            if age > retention_days:
                try:
                    size = f.stat().st_size
                    f.unlink()
                    freed += size
                    count += 1
                except OSError as e:
                    logger.warning("Failed to delete recording %s: %s", f, e)
    return count, freed


def _delete_old_archived_clips(root: Path, retention_days: int) -> tuple[int, int]:
    """Delete clip files older than retention_days in the clips directory.

    Note: This deletes based on file age. The actual status check (archived only)
    should be coordinated with the DB, but since sidecar doesn't have DB access,
    we delete by age only. The Tauri frontend should mark old archived clips for
    deletion and call the manual cleanup, or the sidecar can rely on file age as
    a reasonable proxy.
    """
    if retention_days <= 0:
        return 0, 0
    clips_dir = root / "clips"
    if not clips_dir.is_dir():
        return 0, 0
    count = 0
    freed = 0
    for f in clips_dir.rglob("*"):
        if f.is_file() and _file_age_days(f) > retention_days:
            try:
                size = f.stat().st_size
                f.unlink()
                freed += size
                count += 1
            except OSError as e:
                logger.warning("Failed to delete clip file %s: %s", f, e)
    return count, freed


class StorageCleanupWorker:
    def __init__(self) -> None:
        self._task: asyncio.Task[None] | None = None
        self._running = False

    async def start(self) -> None:
        if self._running:
            return
        self._running = True
        self._task = asyncio.create_task(self._loop())
        logger.info("StorageCleanupWorker started (interval=%dm)", settings.cleanup_interval_minutes)

    async def stop(self) -> None:
        self._running = False
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass
            self._task = None
        logger.info("StorageCleanupWorker stopped")

    async def run_once(self) -> dict:
        """Run cleanup cycle once. Returns summary."""
        root = settings.storage_path
        total_deleted_rec = 0
        total_deleted_clips = 0
        total_freed = 0

        rec_count, rec_freed = await asyncio.to_thread(
            _delete_old_recordings, root, settings.raw_retention_days
        )
        total_deleted_rec += rec_count
        total_freed += rec_freed

        clip_count, clip_freed = await asyncio.to_thread(
            _delete_old_archived_clips, root, settings.archive_retention_days
        )
        total_deleted_clips += clip_count
        total_freed += clip_freed

        if settings.storage_quota_gb and settings.storage_quota_gb > 0:
            quota_bytes = int(settings.storage_quota_gb * 1_073_741_824)
            current = _dir_total_bytes(root)
            usage_pct = current / quota_bytes * 100 if quota_bytes > 0 else 0

            if usage_pct > settings.storage_cleanup_percent:
                await ws_manager.broadcast("storage_warning", {
                    "usage_percent": round(usage_pct, 1),
                    "quota_bytes": quota_bytes,
                    "total_bytes": current,
                })

            if usage_pct > settings.storage_warn_percent:
                await ws_manager.broadcast("storage_warning", {
                    "usage_percent": round(usage_pct, 1),
                    "quota_bytes": quota_bytes,
                    "total_bytes": current,
                })

        summary = {
            "deleted_recordings": total_deleted_rec,
            "deleted_clips": total_deleted_clips,
            "freed_bytes": total_freed,
        }

        if total_freed > 0:
            await ws_manager.broadcast("cleanup_completed", summary)

        return summary

    async def _loop(self) -> None:
        while self._running:
            try:
                await self.run_once()
            except Exception:
                logger.exception("Cleanup cycle failed")
            await asyncio.sleep(settings.cleanup_interval_minutes * 60)


cleanup_worker = StorageCleanupWorker()
```

- [ ] **Step 5: Register in `app.py`**

In `sidecar/src/app.py`:

Add imports:
```python
from core.cleanup import cleanup_worker
from routes import storage as storage_routes
```

In `lifespan()`, add after `await account_watcher.start()`:
```python
await cleanup_worker.start()
```

Before `yield`, nothing else needed. After `yield` (shutdown), add before `await account_watcher.stop()`:
```python
await cleanup_worker.stop()
```

Register router:
```python
app.include_router(storage_routes.router, tags=["storage"])
```

- [ ] **Step 6: Verify sidecar lint**

```bash
cd sidecar && uv run ruff check src && uv run ruff format --check src
```

- [ ] **Step 7: Commit**

```bash
git add sidecar/src/routes/storage.py sidecar/src/core/cleanup.py sidecar/src/config.py sidecar/src/models/schemas.py sidecar/src/app.py
git commit -m "feat(sidecar): add storage stats API and cleanup worker with retention policies"
```

---

### Task 21: Rust — Storage Commands

**Files:**
- Create: `src-tauri/src/commands/storage.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `storage.rs`**

Create `src-tauri/src/commands/storage.rs`:

```rust
use crate::AppState;
use rusqlite::params;
use tauri::State;

#[tauri::command]
pub fn delete_recording_files(state: State<'_, AppState>, recording_id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let file_path: Option<String> = conn
        .query_row(
            "SELECT file_path FROM recordings WHERE id = ?1",
            [recording_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if let Some(ref path) = file_path {
        let _ = std::fs::remove_file(path);
    }

    conn.execute(
        "UPDATE recordings SET file_path = NULL, file_size_bytes = 0 WHERE id = ?1",
        [recording_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn list_recordings_for_cleanup(
    state: State<'_, AppState>,
    retention_days: i64,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, account_id, file_path, file_size_bytes, ended_at \
             FROM recordings \
             WHERE status = 'done' \
               AND file_path IS NOT NULL \
               AND ended_at IS NOT NULL \
               AND julianday('now', '+7 hours') - julianday(ended_at) > ?1 \
             ORDER BY ended_at ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![retention_days], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "account_id": row.get::<_, i64>(1)?,
                "file_path": row.get::<_, Option<String>>(2)?,
                "file_size_bytes": row.get::<_, i64>(3)?,
                "ended_at": row.get::<_, Option<String>>(4)?,
            }))
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub fn list_activity_feed(state: State<'_, AppState>, limit: i64) -> Result<Vec<serde_json::Value>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, type, title, message, account_id, recording_id, clip_id, created_at \
             FROM notifications \
             ORDER BY created_at DESC \
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "type": row.get::<_, String>(1)?,
                "title": row.get::<_, String>(2)?,
                "message": row.get::<_, String>(3)?,
                "account_id": row.get::<_, Option<i64>>(4)?,
                "recording_id": row.get::<_, Option<i64>>(5)?,
                "clip_id": row.get::<_, Option<i64>>(6)?,
                "created_at": row.get::<_, String>(7)?,
            }))
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}
```

- [ ] **Step 2: Register module and commands**

Add to `commands/mod.rs`:
```rust
pub mod storage;
```

Add to `lib.rs` invoke_handler:
```rust
commands::storage::delete_recording_files,
commands::storage::list_recordings_for_cleanup,
commands::storage::list_activity_feed,
```

- [ ] **Step 3: Verify build**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/storage.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(storage): add storage cleanup commands and activity feed query"
```

---

### Task 22: Frontend — Storage Management UI in Settings

**Files:**
- Modify: `src/lib/api.ts`
- Modify: `src/pages/settings.tsx`

- [ ] **Step 1: Add storage API wrappers to `api.ts`**

Append:

```typescript
export type StorageStats = {
  recordings_bytes: number;
  recordings_count: number;
  clips_bytes: number;
  clips_count: number;
  products_bytes: number;
  total_bytes: number;
  quota_bytes: number | null;
  usage_percent: number;
};

export async function getStorageStats(): Promise<StorageStats> {
  return sidecarJson<StorageStats>("/api/storage/stats");
}

export async function deleteRecordingFiles(recordingId: number): Promise<void> {
  await invoke("delete_recording_files", { recording_id: recordingId });
}

export async function listRecordingsForCleanup(retentionDays: number): Promise<unknown[]> {
  return invoke<unknown[]>("list_recordings_for_cleanup", { retention_days: retentionDays });
}
```

- [ ] **Step 2: Add storage management section to `settings.tsx`**

Add a new card/section to `settings.tsx` with:
- Storage overview: progress bar + breakdown (recordings/clips/other bytes + counts)
- Color logic: green < 80%, yellow 80-95%, red > 95%
- "Scan Now" button → refetch `getStorageStats()`
- Retention settings: raw recording retention days input, archived clip retention days input
- Threshold settings: warning % and cleanup % inputs
- Save button → `setSetting()` for each key
- "Clean Up Now" button → call sidecar cleanup endpoint (or trigger via settings)

Settings keys: `TIKCLIP_RAW_RETENTION_DAYS`, `TIKCLIP_ARCHIVE_RETENTION_DAYS`, `TIKCLIP_STORAGE_WARN_PERCENT`, `TIKCLIP_STORAGE_CLEANUP_PERCENT`

- [ ] **Step 3: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 4: Commit**

```bash
git add src/lib/api.ts src/pages/settings.tsx
git commit -m "feat(settings): add storage management section with usage display and retention policies"
```

---

### Task 23: Frontend — Dashboard Realtime + Activity Feed

**Files:**
- Modify: `src/stores/app-store.ts`
- Create: `src/components/dashboard/activity-feed.tsx`
- Modify: `src/pages/dashboard.tsx`
- Modify: `src/lib/api.ts`
- Modify: `src/components/layout/app-shell.tsx`

- [ ] **Step 1: Add dashboardRevision to app-store.ts**

In `src/stores/app-store.ts`, add to the store type and implementation:

```typescript
dashboardRevision: number;
bumpDashboardRevision: () => void;
```

Initialize: `dashboardRevision: 0`

Implementation: `bumpDashboardRevision: () => set((s) => ({ dashboardRevision: s.dashboardRevision + 1 }))`

- [ ] **Step 2: Add activity feed API wrapper**

Append to `src/lib/api.ts`:

```typescript
export type ActivityFeedItem = {
  id: number;
  type: string;
  title: string;
  message: string;
  account_id: number | null;
  recording_id: number | null;
  clip_id: number | null;
  created_at: string;
};

export async function listActivityFeed(limit = 10): Promise<ActivityFeedItem[]> {
  return invoke<ActivityFeedItem[]>("list_activity_feed", { limit });
}
```

- [ ] **Step 3: Create activity feed component**

Create `src/components/dashboard/activity-feed.tsx`:
- Fetches data via `listActivityFeed(10)` on mount + when `dashboardRevision` changes
- Each item: icon based on `type` (🔴 account_live, 🎬 recording_finished, ✂️ clip_ready, 📦 product_created, 🧹 cleanup_completed, ⚠️ storage_warning), message text, relative time
- Items clickable: if `clip_id` → `setActiveClipId(clip_id)` + navigate to clips page. Implementation: accept an `onNavigate` prop.
- "View All →" link at the bottom

- [ ] **Step 4: Update dashboard.tsx**

In `src/pages/dashboard.tsx`:
- Import and use `ActivityFeed` component
- Watch `dashboardRevision` from `useAppStore` alongside `clipsRevision` to trigger stats refetch
- Insert `<ActivityFeed />` between stat cards and active recordings grid

- [ ] **Step 5: Wire WebSocket events to bumpDashboardRevision in app-shell.tsx**

In `src/components/layout/app-shell.tsx`, inside the WebSocket setup effect:

After existing `clip_ready` handler, add:
```typescript
useAppStore.getState().bumpDashboardRevision();
```

Do the same for `recording_finished`, `account_live`, `account_status` handlers.

Add new WS subscriptions for `cleanup_completed` and `storage_warning`:
```typescript
const unsubCleanup = wsClient.on("cleanup_completed", (data) => {
  dispatchSidecarNotification("cleanup_completed", data);
  useAppStore.getState().bumpDashboardRevision();
});
const unsubStorageWarn = wsClient.on("storage_warning", (data) => {
  dispatchSidecarNotification("storage_warning", data);
  useAppStore.getState().bumpDashboardRevision();
});
```

Remember to call `unsubCleanup()` and `unsubStorageWarn()` in the cleanup function.

- [ ] **Step 6: Insert notification DB rows for new event types**

In the same WS handlers in `app-shell.tsx`, when receiving `clip_ready`, `recording_finished`, etc. — ensure `insertNotificationDb()` is called so the activity feed has data. Check existing handlers — some already call `dispatchSidecarNotification` which may already insert. If not, add explicit inserts for:
- `cleanup_completed`: title "Dọn dẹp hoàn tất", message with freed_bytes
- `storage_warning`: title "Cảnh báo dung lượng", message with usage_percent

- [ ] **Step 7: Verify lint**

```bash
npm run lint:js
```

- [ ] **Step 8: Commit**

```bash
git add src/stores/app-store.ts src/components/dashboard/activity-feed.tsx src/pages/dashboard.tsx src/lib/api.ts src/components/layout/app-shell.tsx
git commit -m "feat(dashboard): add realtime refresh and activity feed with WebSocket event wiring"
```

---

### Task 24: Final Verification

- [ ] **Step 1: Rust checks**

```bash
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings
```

Fix any issues.

- [ ] **Step 2: Frontend checks**

```bash
npm run lint:js
```

Fix any issues.

- [ ] **Step 3: Sidecar checks**

```bash
cd sidecar && uv run ruff check src tests && uv run ruff format --check src tests && uv run pytest tests/ -q
```

Fix any issues.

- [ ] **Step 4: Final commit if any fixes**

```bash
git add -A && git commit -m "fix: address lint and format issues from Phase 2 implementation"
```
