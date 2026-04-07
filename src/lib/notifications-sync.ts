import { isTauri } from "@tauri-apps/api/core";
import {
  dbNotificationCreatedAtToMs,
  listNotificationsDb,
} from "@/lib/api";
import {
  type NotificationKind,
  useNotificationStore,
} from "@/stores/notification-store";

function coerceKind(s: string): NotificationKind {
  if (
    s === "account_live" ||
    s === "recording_finished" ||
    s === "clip_ready" ||
    s === "info"
  ) {
    return s;
  }
  return "info";
}

/** Load `notifications` from SQLite into the in-memory store (Tauri only). */
export async function hydrateNotificationsFromDb(): Promise<void> {
  if (!isTauri()) {
    return;
  }
  try {
    const rows = await listNotificationsDb(200);
    useNotificationStore.getState().setNotifications(
      rows.map((r) => ({
        id: String(r.id),
        kind: coerceKind(r.kind),
        title: r.title,
        body: r.body,
        read: r.read,
        createdAt: dbNotificationCreatedAtToMs(r.createdAt),
      })),
    );
  } catch (e) {
    if (import.meta.env.DEV) {
      console.warn("[TikClip] hydrateNotificationsFromDb failed", e);
    }
  }
}
