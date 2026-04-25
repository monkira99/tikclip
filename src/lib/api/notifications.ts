import { invoke } from "@tauri-apps/api/core";

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
