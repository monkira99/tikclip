import { invoke } from "@tauri-apps/api/core";

import { formatInvokeError } from "@/lib/invoke-error";
import type {
  Account,
  AccountType,
  AutoRecordSchedule,
  Clip,
  ClipFilters,
  CreateAccountInput,
  CreateProductInput,
  Product,
  SidecarRecordingStatus,
  UpdateProductInput,
} from "@/types";

/** Raw row from SQLite: schedule stored as JSON string. */
type AccountInvokeRow = Omit<Account, "auto_record_schedule" | "type"> & {
  type: AccountType;
  auto_record_schedule: string | AutoRecordSchedule | null;
};

let sidecarBaseUrl: string | null = null;

/** Called when the sidecar HTTP port is known (from `useSidecar` / app store). */
export function setSidecarPort(port: number | null): void {
  sidecarBaseUrl = port != null ? `http://127.0.0.1:${port}` : null;
}

export function getSidecarBaseUrl(): string | null {
  return sidecarBaseUrl;
}

function requireSidecarBase(): string {
  if (!sidecarBaseUrl) {
    throw new Error("Sidecar port not available yet");
  }
  return sidecarBaseUrl;
}

async function sidecarJson<T>(path: string, init?: RequestInit): Promise<T> {
  const base = requireSidecarBase();
  const res = await fetch(`${base}${path}`, {
    ...init,
    headers: {
      Accept: "application/json",
      "Content-Type": "application/json",
      ...(init?.headers ?? {}),
    },
  });
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(text || `Sidecar request failed: ${res.status}`);
  }
  return res.json() as Promise<T>;
}

export async function startRecording(body: {
  account_id: number;
  username: string;
  room_id?: string | null;
  stream_url?: string | null;
  cookies_json?: string | null;
  proxy_url?: string | null;
  max_duration_seconds?: number | null;
}): Promise<SidecarRecordingStatus> {
  return sidecarJson<SidecarRecordingStatus>("/api/recording/start", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function stopRecording(recordingId: string): Promise<SidecarRecordingStatus> {
  return sidecarJson<SidecarRecordingStatus>("/api/recording/stop", {
    method: "POST",
    body: JSON.stringify({ recording_id: recordingId }),
  });
}

export async function getRecordingStatus(): Promise<SidecarRecordingStatus[]> {
  return sidecarJson<SidecarRecordingStatus[]>("/api/recording/status");
}

export async function getRecordingStatusOne(recordingId: string): Promise<SidecarRecordingStatus> {
  return sidecarJson<SidecarRecordingStatus>(`/api/recording/status/${encodeURIComponent(recordingId)}`);
}

export async function checkAccountStatus(body: {
  username: string;
  cookies_json?: string | null;
  proxy_url?: string | null;
}): Promise<{
  username: string;
  is_live: boolean;
  room_id: string | null;
  stream_url: string | null;
  viewer_count: number | null;
}> {
  return sidecarJson("/api/accounts/check-status", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export type WatchAccountBody = {
  account_id: number;
  username: string;
  auto_record: boolean;
  cookies_json?: string | null;
  proxy_url?: string | null;
};

function normalizeUsername(username: string): string {
  return username.trim().replace(/^@/, "");
}

/** Register account with sidecar poller (required for live checks + auto-record). */
export async function watchAccount(body: WatchAccountBody): Promise<void> {
  await sidecarJson<{ ok: boolean }>("/api/accounts/watch", {
    method: "POST",
    body: JSON.stringify({
      account_id: body.account_id,
      username: normalizeUsername(body.username),
      auto_record: body.auto_record,
      cookies_json: body.cookies_json ?? null,
      proxy_url: body.proxy_url ?? null,
    }),
  });
}

export async function unwatchAccount(accountId: number): Promise<void> {
  const base = getSidecarBaseUrl();
  if (!base) {
    return;
  }
  const res = await fetch(`${base}/api/accounts/watch/${accountId}`, {
    method: "DELETE",
  });
  if (!res.ok && res.status !== 404) {
    const text = await res.text().catch(() => "");
    throw new Error(text || `unwatch failed: ${res.status}`);
  }
}

/** Persist live flag from sidecar into app SQLite (drives Accounts UI). */
export async function updateAccountLiveStatus(id: number, isLive: boolean): Promise<void> {
  if (import.meta.env.DEV) {
    console.debug("[TikClip] invoke update_account_live_status", { id, isLive });
  }
  await invoke("update_account_live_status", { id, isLive });
}

/** Single transaction — avoids N× invoke racing with list_accounts (StrictMode / duplicate fetches). */
export async function syncAccountsLiveStatus(
  rows: { account_id: number; is_live: boolean }[],
): Promise<void> {
  if (rows.length === 0) {
    return;
  }
  if (import.meta.env.DEV) {
    console.debug("[TikClip] invoke sync_accounts_live_status", { count: rows.length, rows });
  }
  await invoke("sync_accounts_live_status", { rows });
}

/** Last poller snapshot (HTTP); works when WebSocket from sidecar → webview is blocked. */
export type LiveOverviewAccount = {
  account_id: number;
  username: string;
  is_live: boolean;
};

export async function getLiveOverview(): Promise<LiveOverviewAccount[]> {
  const data = await sidecarJson<{ accounts: LiveOverviewAccount[] }>("/api/accounts/live-overview");
  return data.accounts;
}

/** Force an immediate poll of all watched accounts and return fresh live flags. */
export async function pollNow(): Promise<LiveOverviewAccount[]> {
  const data = await sidecarJson<{ accounts: LiveOverviewAccount[] }>("/api/accounts/poll-now", {
    method: "POST",
  });
  return data.accounts;
}

/** Re-register every DB account with the sidecar after connect/restart. */
export async function syncWatcherForAccounts(
  accounts: {
    id: number;
    username: string;
    auto_record: boolean;
    cookies_json: string | null;
    proxy_url: string | null;
  }[],
): Promise<void> {
  await Promise.all(
    accounts.map((a) =>
      watchAccount({
        account_id: a.id,
        username: a.username,
        auto_record: a.auto_record,
        cookies_json: a.cookies_json,
        proxy_url: a.proxy_url,
      }).catch(() => {
        /* sidecar may be mid-restart */
      }),
    ),
  );
}

export async function listClips(): Promise<Clip[]> {
  return invoke<Clip[]>("list_clips");
}

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
  return invoke<Clip>("get_clip_by_id", { clipId });
}

export async function updateClipStatus(clipId: number, newStatus: string): Promise<void> {
  await invoke("update_clip_status", { clipId, newStatus });
}

export async function updateClipTitle(clipId: number, title: string): Promise<void> {
  await invoke("update_clip_title", { clipId, title });
}

export async function updateClipNotes(clipId: number, notes: string): Promise<void> {
  await invoke("update_clip_notes", { clipId, notes });
}

export async function batchUpdateClipStatus(clipIds: number[], newStatus: string): Promise<void> {
  await invoke("batch_update_clip_status", { clipIds, newStatus });
}

export async function batchDeleteClips(clipIds: number[]): Promise<void> {
  await invoke("batch_delete_clips", { clipIds });
}

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

export async function listProducts(): Promise<Product[]> {
  return invoke<Product[]>("list_products");
}

export async function getProductById(productId: number): Promise<Product> {
  return invoke<Product>("get_product_by_id", { productId });
}

export async function createProduct(input: CreateProductInput): Promise<number> {
  return invoke<number>("create_product", { input });
}

export async function updateProduct(productId: number, input: UpdateProductInput): Promise<void> {
  await invoke("update_product", { productId, input });
}

export async function deleteProduct(productId: number): Promise<void> {
  const id = Number(productId);
  if (!Number.isInteger(id) || id < 1) {
    throw new Error(`Invalid product id: ${String(productId)}`);
  }
  try {
    await invoke("delete_product", { productId: id });
  } catch (e) {
    throw new Error(formatInvokeError(e));
  }
}

export async function listClipProducts(clipId: number): Promise<Product[]> {
  return invoke<Product[]>("list_clip_products", { clipId });
}

export async function tagClipProduct(clipId: number, productId: number): Promise<void> {
  await invoke("tag_clip_product", { clipId, productId });
}

export async function untagClipProduct(clipId: number, productId: number): Promise<void> {
  await invoke("untag_clip_product", { clipId, productId });
}

export async function batchTagClipProducts(clipIds: number[], productId: number): Promise<void> {
  await invoke("batch_tag_clip_products", { clipIds, productId });
}

export type FetchedProductMediaFile = {
  kind: "image" | "video";
  path: string;
  source_url: string;
};

export type FetchProductFromUrlResult = {
  success: boolean;
  incomplete: boolean;
  data: {
    name: string | null;
    description: string | null;
    price: number | null;
    image_url: string | null;
    category: string | null;
    tiktok_shop_id: string | null;
    image_urls: string[];
    video_urls: string[];
    media_files: FetchedProductMediaFile[];
  } | null;
  error: string | null;
};

export async function fetchProductFromUrl(
  url: string,
  cookiesJson?: string | null,
  options?: { downloadMedia?: boolean },
): Promise<FetchProductFromUrlResult> {
  return sidecarJson<FetchProductFromUrlResult>("/api/products/fetch-from-url", {
    method: "POST",
    body: JSON.stringify({
      url,
      cookies_json: cookiesJson ?? null,
      download_media: options?.downloadMedia !== false,
    }),
  });
}

export type ProductEmbeddingMediaItem = {
  kind: "image" | "video";
  path: string;
  source_url?: string;
};

export type IndexProductEmbeddingsResult = {
  indexed: number;
  skipped: number;
  errors: string[];
  message: string | null;
};

export async function indexProductEmbeddings(
  productId: number,
  body: {
    product_name: string;
    product_description?: string;
    items: ProductEmbeddingMediaItem[];
  },
): Promise<IndexProductEmbeddingsResult> {
  return sidecarJson<IndexProductEmbeddingsResult>("/api/products/embeddings/index", {
    method: "POST",
    body: JSON.stringify({
      product_id: productId,
      product_name: body.product_name,
      product_description: body.product_description ?? "",
      items: body.items.map((x) => ({
        kind: x.kind,
        path: x.path,
        source_url: x.source_url ?? "",
      })),
    }),
  });
}

export async function deleteProductEmbeddings(productId: number): Promise<void> {
  await sidecarJson<{ ok: boolean }>("/api/products/embeddings/delete", {
    method: "POST",
    body: JSON.stringify({ product_id: productId }),
  });
}

export type ClipSuggestFrameRow = {
  index: number;
  source: "thumbnail" | "extracted";
  media_relative_path: string;
  outcome: "hit" | "no_hit" | "error";
  error: string | null;
  top_product_id: number | null;
  top_score: number | null;
  top_product_name: string | null;
};

export type ClipSuggestVoteRow = {
  product_id: number;
  vote_count: number;
};

export type ClipSuggestTextHit = {
  product_id: number;
  score: number;
  product_name: string | null;
};

export type ClipSuggestProductResult = {
  matched: boolean;
  product_id: number | null;
  product_name: string | null;
  best_score: number | null;
  frames_used: number;
  skipped_reason: string | null;
  video_relative_path: string | null;
  thumbnail_used: boolean;
  extracted_frame_count: number;
  frames_searched: number;
  config_target_extracted_frames: number;
  config_max_score_threshold: number;
  suggest_weight_image: number;
  suggest_weight_text: number;
  suggest_min_fused_score: number;
  pick_method: "majority_vote" | "min_distance_tiebreak" | "weighted_fusion" | null;
  votes_by_product: ClipSuggestVoteRow[];
  candidate_product_id: number | null;
  candidate_product_name: string | null;
  candidate_score: number | null;
  frame_rows: ClipSuggestFrameRow[];
  text_search_hits: ClipSuggestTextHit[];
  text_search_used: boolean;
  fusion_method: string | null;
};

export async function suggestProductForClip(body: {
  video_path: string;
  thumbnail_path?: string | null;
  transcript_text?: string | null;
}): Promise<ClipSuggestProductResult> {
  return sidecarJson<ClipSuggestProductResult>("/api/clips/suggest-product", {
    method: "POST",
    body: JSON.stringify({
      video_path: body.video_path,
      thumbnail_path: body.thumbnail_path ?? null,
      transcript_text: body.transcript_text ?? null,
    }),
  });
}

function normalizeAccount(row: AccountInvokeRow): Account {
  const raw = row.auto_record_schedule;
  let auto_record_schedule: AutoRecordSchedule | null = null;
  if (typeof raw === "string" && raw.length > 0) {
    try {
      auto_record_schedule = JSON.parse(raw) as AutoRecordSchedule;
    } catch {
      auto_record_schedule = null;
    }
  } else if (raw && typeof raw === "object") {
    auto_record_schedule = raw;
  }
  return {
    ...row,
    auto_record_schedule,
  };
}

export async function listAccounts(): Promise<Account[]> {
  const rows = await invoke<AccountInvokeRow[]>("list_accounts");
  return rows.map(normalizeAccount);
}

export async function createAccount(input: CreateAccountInput): Promise<number> {
  return invoke<number>("create_account", {
    input: {
      username: input.username,
      display_name: input.display_name,
      type: input.type,
      cookies_json: input.cookies_json ?? null,
      proxy_url: input.proxy_url ?? null,
      auto_record: input.auto_record,
      priority: input.priority,
      notes: input.notes ?? null,
    },
  });
}

export async function deleteAccount(id: number): Promise<void> {
  await invoke("delete_account", { id });
}

export async function getSetting(key: string): Promise<string | null> {
  const value = await invoke<string | null>("get_setting", { key });
  return value ?? null;
}

export async function setSetting(key: string, value: string): Promise<void> {
  await invoke("set_setting", { key, value });
}

/** Reload Python sidecar so it picks up SQLite-backed `TIKCLIP_*` settings. */
export async function restartSidecar(): Promise<number> {
  return invoke<number>("restart_sidecar");
}

export type AppDataPaths = {
  storage_root: string;
  data_dir: string;
  clips_dir: string;
  records_dir: string;
};

export async function getAppDataPaths(): Promise<AppDataPaths> {
  return invoke<AppDataPaths>("get_app_data_paths");
}

const HCM_TIMEZONE = "Asia/Ho_Chi_Minh";

/** Calendar date in GMT+7 (Vietnam), `YYYY-MM-DD` — matches DB wall clock and clip paths. */
export function hcmDateYmd(): string {
  const parts = new Intl.DateTimeFormat("en-CA", {
    timeZone: HCM_TIMEZONE,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).formatToParts(new Date());
  const y = parts.find((p) => p.type === "year")?.value;
  const m = parts.find((p) => p.type === "month")?.value;
  const d = parts.find((p) => p.type === "day")?.value;
  if (y && m && d) {
    return `${y}-${m}-${d}`;
  }
  const d0 = new Date();
  const yy = d0.getFullYear();
  const mm = String(d0.getMonth() + 1).padStart(2, "0");
  const dd = String(d0.getDate()).padStart(2, "0");
  return `${yy}-${mm}-${dd}`;
}

/** @deprecated Use `hcmDateYmd` — kept for any external imports. */
export const localDateYmd = hcmDateYmd;

export type DashboardStats = {
  clipsToday: number;
  storageUsedBytes: number;
  storageQuotaGb: number | null;
};

export async function getDashboardStats(): Promise<DashboardStats> {
  return invoke<DashboardStats>("get_dashboard_stats", {
    todayYmd: hcmDateYmd(),
  });
}

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

export type StorageCleanupSummary = {
  deleted_recordings: number;
  deleted_clips: number;
  freed_bytes: number;
};

export async function runStorageCleanupNow(input: {
  raw_retention_days: number;
  archive_retention_days: number;
}): Promise<StorageCleanupSummary> {
  return sidecarJson<StorageCleanupSummary>("/api/storage/cleanup-run", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export async function deleteRecordingFiles(recordingId: number): Promise<void> {
  await invoke("delete_recording_files", { recordingId });
}

export async function listRecordingsForCleanup(retentionDays: number): Promise<unknown[]> {
  return invoke<unknown[]>("list_recordings_for_cleanup", { retentionDays });
}

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

/** Row from `notifications` table (camelCase from Tauri). */
export type DbNotificationRow = {
  id: number;
  kind: string;
  title: string;
  body: string;
  read: boolean;
  createdAt: string;
};

export async function insertNotificationDb(input: {
  notificationType: string;
  title: string;
  message: string;
  accountId?: number | null;
  recordingId?: number | null;
  clipId?: number | null;
}): Promise<number> {
  return invoke<number>("insert_notification", {
    input: {
      notificationType: input.notificationType,
      title: input.title,
      message: input.message,
      accountId: input.accountId ?? null,
      recordingId: input.recordingId ?? null,
      clipId: input.clipId ?? null,
    },
  });
}

export async function listNotificationsDb(limit = 200): Promise<DbNotificationRow[]> {
  return invoke<DbNotificationRow[]>("list_notifications", { limit });
}

export async function markNotificationReadDb(id: number): Promise<void> {
  await invoke("mark_notification_read", { id });
}

export async function markAllNotificationsReadDb(): Promise<void> {
  await invoke("mark_all_notifications_read");
}

/** Parse SQLite `YYYY-MM-DD HH:MM:SS` stored as GMT+7 wall clock. */
export function dbNotificationCreatedAtToMs(createdAt: string): number {
  const t = createdAt.trim().replace(" ", "T");
  const d = new Date(`${t}+07:00`);
  return Number.isNaN(d.getTime()) ? Date.now() : d.getTime();
}

/** Open a folder or file in Finder / Explorer / file manager (native backend). */
export async function openPathInSystem(path: string): Promise<void> {
  await invoke("open_path", { path });
}

export async function storageRootIsCustom(): Promise<boolean> {
  return invoke<boolean>("storage_root_is_custom");
}

/** Native folder picker; requires a WebviewWindow (invoked from the app UI). */
export async function pickStorageRootFolder(): Promise<string | null> {
  return invoke<string | null>("pick_storage_root_folder");
}

/** Persists root and restarts the app (invoke may not return). */
export async function applyStorageRoot(path: string): Promise<void> {
  await invoke("apply_storage_root", { path });
}

/** Removes custom root config and restarts (back to ~/.tikclip rules). */
export async function resetStorageRootDefault(): Promise<void> {
  await invoke("reset_storage_root_default");
}
