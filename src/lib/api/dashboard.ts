import { invoke } from "@tauri-apps/api/core";

const HCM_TIMEZONE = "Asia/Ho_Chi_Minh";

/** Calendar date in GMT+7 (Vietnam), `YYYY-MM-DD` — matches DB wall clock and clip paths. */
function hcmDateYmd(): string {
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
