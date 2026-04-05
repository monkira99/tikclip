import { invoke } from "@tauri-apps/api/core";
import type {
  Account,
  AccountType,
  AutoRecordSchedule,
  Clip,
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

export async function listClips(): Promise<Clip[]> {
  return invoke<Clip[]>("list_clips");
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
    username: input.username,
    display_name: input.display_name,
    type: input.type,
    cookies_json: input.cookies_json ?? null,
    proxy_url: input.proxy_url ?? null,
    auto_record: input.auto_record,
    priority: input.priority,
    notes: input.notes ?? null,
  });
}

export async function deleteAccount(id: number): Promise<void> {
  await invoke("delete_account", { id });
}
