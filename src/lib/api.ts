import { invoke } from "@tauri-apps/api/core";

import { formatInvokeError } from "@/lib/invoke-error";
import type {
  Account,
  AccountType,
  AutoRecordSchedule,
  Clip,
  ClipCaptionStatus,
  ClipFilters,
  CreateFlowInput,
  CreateAccountInput,
  FlowEditorPayload,
  FlowNodeConfig,
  FlowNodeKey,
  FlowRuntimeLogEntry,
  FlowRuntimeSnapshot,
  FlowStatus,
  FlowSummary,
  PublishFlowResult,
  CreateProductInput,
  Product,
  ActiveRecordingStatus,
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

/** Persist live flag into app SQLite (drives Accounts UI). */
export async function updateAccountLiveStatus(id: number, isLive: boolean): Promise<void> {
  if (import.meta.env.DEV) {
    console.debug("[TikClip] invoke update_account_live_status", { id, isLive });
  }
  await invoke("update_account_live_status", { id, isLive });
}

/** Batch persist live flags into SQLite. */
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

export async function updateClipCaption(
  clipId: number,
  captionText: string | null,
  captionStatus: ClipCaptionStatus,
  captionError?: string | null,
): Promise<void> {
  await invoke("update_clip_caption", {
    clipId,
    captionText,
    captionStatus,
    captionError: captionError ?? null,
  });
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

export type FlowRecording = {
  id: number;
  account_id: number;
  account_username: string;
  room_id: string | null;
  status: string;
  started_at: string;
  ended_at: string | null;
  duration_seconds: number;
  file_path: string | null;
  file_size_bytes: number;
  sidecar_recording_id: string | null;
  error_message: string | null;
  flow_id: number | null;
  created_at: string;
};

export type UpdateFlowInput = {
  name?: string;
  status?: FlowStatus;
  current_node?: FlowNodeKey | null;
  last_live_at?: string | null;
  last_run_at?: string | null;
  last_error?: string | null;
};

export type UpdateFlowRuntimeByAccountInput = {
  status?: FlowStatus;
  current_node?: FlowNodeKey | null;
  last_live_at?: string | null;
  last_run_at?: string | null;
  last_error?: string | null;
};

export async function listFlows(): Promise<FlowSummary[]> {
  return invoke<FlowSummary[]>("list_flows");
}

export async function getFlowDefinition(flowId: number): Promise<FlowEditorPayload> {
  return invoke<FlowEditorPayload>("get_flow_definition", { flowId });
}

export async function listLiveRuntimeSessions(): Promise<FlowRuntimeSnapshot[]> {
  return invoke<FlowRuntimeSnapshot[]>("list_live_runtime_sessions");
}

export async function listLiveRuntimeLogs(
  flowId?: number,
  limit?: number,
): Promise<FlowRuntimeLogEntry[]> {
  return invoke<FlowRuntimeLogEntry[]>("list_live_runtime_logs", { flowId, limit });
}

export async function listActiveRustRecordings(): Promise<ActiveRecordingStatus[]> {
  return invoke<ActiveRecordingStatus[]>("list_active_rust_recordings");
}

export async function stopRustRecording(recordingId: string): Promise<void> {
  await invoke("stop_rust_recording", { recordingId });
}

export async function createFlow(input: CreateFlowInput): Promise<number> {
  return invoke<number>("create_flow", { input });
}

export async function deleteFlow(flowId: number): Promise<void> {
  await invoke("delete_flow", { flowId });
}

export async function saveFlowNodeDraft(input: {
  flow_id: number;
  node_key: FlowNodeKey;
  draft_config_json: string;
}): Promise<void> {
  await invoke("save_flow_node_draft", {
    flowId: input.flow_id,
    nodeKey: input.node_key,
    draftConfigJson: input.draft_config_json,
  });
}

export async function publishFlowDefinition(flowId: number): Promise<PublishFlowResult> {
  return invoke<PublishFlowResult>("publish_flow_definition", { flowId });
}

export type RestartFlowRunResult = {
  flowId: number;
  newRunId: number;
};

export async function restartFlowRun(flowId: number): Promise<RestartFlowRunResult> {
  return invoke<RestartFlowRunResult>("restart_flow_run", { flowId });
}

export async function updateFlow(flowId: number, input: UpdateFlowInput): Promise<void> {
  await invoke("update_flow", {
    flowId,
    input: {
      name: input.name,
      status: input.status,
      current_node:
        input.current_node === undefined
          ? undefined
          : input.current_node === null
            ? ""
            : input.current_node,
      last_live_at:
        input.last_live_at === undefined
          ? undefined
          : input.last_live_at === null
            ? ""
            : input.last_live_at,
      last_run_at:
        input.last_run_at === undefined
          ? undefined
          : input.last_run_at === null
            ? ""
            : input.last_run_at,
      last_error:
        input.last_error === undefined
          ? undefined
          : input.last_error === null
            ? ""
            : input.last_error,
    },
  });
}

export type SidecarFlowRuntimeHint =
  | "clip_ready"
  | "caption_ready";

/** Rust maps `hint` to `flows` / Start-node telemetry (no transition logic in JS). */
export async function applySidecarFlowRuntimeHint(input: {
  account_id: number;
  hint: SidecarFlowRuntimeHint;
  /** Desktop SQLite `clips.id` for `clip_ready` / `caption_ready` pipeline node runs. */
  clip_id?: number | null;
}): Promise<void> {
  await invoke("apply_sidecar_flow_runtime_hint", {
    input: {
      account_id: input.account_id,
      hint: input.hint,
      clip_id:
        input.clip_id === undefined || input.clip_id === null ? undefined : input.clip_id,
    },
  });
}

export async function finalizeRustRecordingRuntime(input: {
  external_recording_id: string;
  worker_status: string;
  room_id?: string | null;
  error_message?: string | null;
}): Promise<void> {
  await invoke("finalize_rust_recording_runtime", {
    input: {
      external_recording_id: input.external_recording_id,
      worker_status: input.worker_status,
      room_id: input.room_id === undefined || input.room_id === null ? undefined : input.room_id,
      error_message:
        input.error_message === undefined || input.error_message === null
          ? undefined
          : input.error_message,
    },
  });
}

export async function updateFlowRuntimeByAccount(
  accountId: number,
  input: UpdateFlowRuntimeByAccountInput,
): Promise<void> {
  await invoke("update_flow_runtime_by_account", {
    accountId,
    input: {
      status: input.status,
      current_node:
        input.current_node === undefined
          ? undefined
          : input.current_node === null
            ? ""
            : input.current_node,
      last_live_at:
        input.last_live_at === undefined
          ? undefined
          : input.last_live_at === null
            ? ""
            : input.last_live_at,
      last_run_at:
        input.last_run_at === undefined
          ? undefined
          : input.last_run_at === null
            ? ""
            : input.last_run_at,
      last_error:
        input.last_error === undefined
          ? undefined
          : input.last_error === null
            ? ""
            : input.last_error,
    },
  });
}

export async function setFlowEnabled(flowId: number, enabled: boolean): Promise<void> {
  await invoke("set_flow_enabled", { flowId, enabled });
}

export async function saveFlowNodeConfig(input: {
  flow_id: number;
  node_key: FlowNodeKey;
  config_json: string;
}): Promise<FlowNodeConfig> {
  return invoke<FlowNodeConfig>("save_flow_node_config", { input });
}

export async function listRecordingsByFlow(flowId: number): Promise<FlowRecording[]> {
  return invoke<FlowRecording[]>("list_recordings_by_flow", { flowId });
}

export async function listClipsByFlow(flowId: number): Promise<Clip[]> {
  return invoke<Clip[]>("list_clips_by_flow", { flowId });
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

export type ProductEmbeddingIndexedProductRow = {
  product_id: number;
  image_doc_count: number;
  video_doc_count: number;
  text_doc_count: number;
  product_name: string | null;
};

export type ProductEmbeddingsIndexedSummary = {
  product_vector_enabled: boolean;
  store_ready: boolean;
  vector_store_relative: string;
  total_documents_scanned: number;
  scan_truncated: boolean;
  product_count: number;
  products: ProductEmbeddingIndexedProductRow[];
  message: string | null;
};

export async function getProductEmbeddingsIndexedSummary(options?: {
  maxDocs?: number;
}): Promise<ProductEmbeddingsIndexedSummary> {
  const max = options?.maxDocs ?? 100_000;
  const q = new URLSearchParams({ max_docs: String(max) });
  return sidecarJson<ProductEmbeddingsIndexedSummary>(
    `/api/products/embeddings/indexed-summary?${q.toString()}`,
    { method: "GET" },
  );
}

export type ClipSuggestImageEvidenceHit = {
  product_id: number;
  score: number;
  product_name: string | null;
  product_description: string | null;
  /** Ảnh/video catalog đã index, đường dẫn tương đối storage (cặp với query = media_relative_path của frame row). */
  catalog_media_relative_path?: string | null;
  catalog_source_url?: string | null;
  catalog_modality?: "image" | "video" | null;
};

export type ClipSuggestFrameRow = {
  index: number;
  source: "thumbnail" | "extracted";
  media_relative_path: string;
  outcome: "hit" | "no_hit" | "error";
  error: string | null;
  top_product_id: number | null;
  top_score: number | null;
  top_product_name: string | null;
  matched_product_description?: string | null;
  image_evidence_hits?: ClipSuggestImageEvidenceHit[];
};

export type ClipSuggestVoteRow = {
  product_id: number;
  vote_count: number;
};

export type ClipSuggestTextHit = {
  product_id: number;
  score: number;
  product_name: string | null;
  product_description?: string | null;
};

export type ClipSuggestTranscriptSegmentRow = {
  segment_index: number;
  segment_text: string;
  outcome: "hit" | "no_hit" | "error";
  error: string | null;
  best_product_id: number | null;
  best_score: number | null;
  best_product_name: string | null;
  matched_product_description: string | null;
};

export type ClipSuggestProductRankRow = {
  product_id: number;
  product_name: string | null;
  frame_hit_count: number;
  /** Mean best-per-frame cosine distance (lower = closer). */
  avg_frame_distance: number | null;
  /** 1 - avg_frame_distance; 0 khi không có frame hit. [0,1] */
  image_score: number;
  /** Raw score từ full-transcript text search (higher = better). */
  transcript_text_score: number | null;
  /** = transcript_text_score hoặc 0. [0,1] */
  text_score: number;
  /** w_img * image_score + w_txt * text_score. Xếp hạng giảm dần. */
  final_score: number;
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
  /** Prompt đang dùng kèm ảnh khi embed frame (echo từ cấu hình). */
  suggest_image_embed_focus_prompt?: string;
  pick_method: "majority_vote" | "min_distance_tiebreak" | "weighted_fusion" | "unified_score" | null;
  votes_by_product: ClipSuggestVoteRow[];
  product_ranks?: ClipSuggestProductRankRow[];
  transcript_segment_evidence?: ClipSuggestTranscriptSegmentRow[];
  candidate_product_id: number | null;
  candidate_product_name: string | null;
  candidate_score: number | null;
  frame_rows: ClipSuggestFrameRow[];
  text_search_hits: ClipSuggestTextHit[];
  text_search_used: boolean;
  fusion_method: string | null;
  /** Khi bật lưu frame debug: đường dẫn tương đối từ thư mục lưu trữ tới thư mục chứa frame_*.jpg */
  debug_extracted_frames_dir?: string | null;
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

export type GenerateCaptionResult = {
  clip_id: number;
  caption_text: string;
};

export async function generateCaptionForClip(body: {
  clip_id: number;
  username: string;
  transcript_text?: string | null;
  clip_title?: string | null;
}): Promise<GenerateCaptionResult> {
  return sidecarJson<GenerateCaptionResult>("/api/captions/generate", {
    method: "POST",
    body: JSON.stringify({
      clip_id: body.clip_id,
      username: body.username,
      transcript_text: body.transcript_text ?? null,
      clip_title: body.clip_title ?? null,
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
