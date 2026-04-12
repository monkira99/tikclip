import { invoke } from "@tauri-apps/api/core";
import type {
  Account,
  AccountType,
  AutoRecordSchedule,
  Clip,
  ClipFilters,
  CreateAccountInput,
  SidecarRecordingStatus,
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
  await invoke("update_account_live_status", { id, is_live: isLive });
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
    today_ymd: hcmDateYmd(),
  });
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
