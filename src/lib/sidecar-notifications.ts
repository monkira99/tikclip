import { isTauri } from "@tauri-apps/api/core";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { toast } from "sonner";
import { insertNotificationDb } from "@/lib/api";
import {
  type NotificationKind,
  useNotificationStore,
} from "@/stores/notification-store";

function pickAccountId(data: Record<string, unknown>): number | null {
  const raw = data.account_id;
  if (typeof raw === "number" && Number.isFinite(raw)) {
    return raw;
  }
  if (typeof raw === "string") {
    const n = Number(raw);
    return Number.isFinite(n) ? n : null;
  }
  return null;
}

function formatSidecarMessage(
  eventType: string,
  data: Record<string, unknown>,
): { kind: NotificationKind; title: string; body: string } {
  const username =
    typeof data.username === "string"
      ? data.username
      : typeof data.account_username === "string"
        ? data.account_username
        : null;

  switch (eventType) {
    case "account_live": {
      const u = username ?? "Account";
      return {
        kind: "account_live",
        title: "Live now",
        body: `${u} is live`,
      };
    }
    case "recording_finished": {
      const u = username ?? "Recording";
      const rid =
        typeof data.recording_id === "string"
          ? data.recording_id
          : data.recording_id != null
            ? String(data.recording_id)
            : "";
      return {
        kind: "recording_finished",
        title: "Recording finished",
        body: rid ? `${u} — ${rid}` : `${u} finished`,
      };
    }
    case "clip_ready": {
      const title =
        typeof data.title === "string" && data.title.length > 0
          ? data.title
          : "New clip";
      return {
        kind: "clip_ready",
        title: "Clip ready",
        body: username ? `${title} (${username})` : title,
      };
    }
    default:
      return {
        kind: "info",
        title: eventType,
        body: "",
      };
  }
}

async function trySendOsNotification(title: string, body: string): Promise<void> {
  if (!isTauri()) {
    return;
  }
  let granted = await isPermissionGranted();
  if (!granted) {
    const perm = await requestPermission();
    granted = perm === "granted";
  }
  if (!granted) {
    if (import.meta.env.DEV) {
      console.warn("[TikClip] OS notifications: permission not granted");
    }
    return;
  }
  try {
    sendNotification({ title, body: body || undefined });
  } catch (e) {
    if (import.meta.env.DEV) {
      console.warn("[TikClip] sendNotification failed", e);
    }
  }
}

/**
 * Persist to SQLite, in-app store + toast, and OS notification (Tauri) after permission check.
 */
export function dispatchSidecarNotification(
  eventType: string,
  data: Record<string, unknown>,
): void {
  void (async () => {
    const { kind, title, body } = formatSidecarMessage(eventType, data);
    const accountId = pickAccountId(data);
    let idStr: string | undefined;
    const createdAt = Date.now();

    if (isTauri()) {
      try {
        const rowId = await insertNotificationDb({
          notificationType: kind,
          title,
          message: body,
          accountId,
          recordingId: null,
          clipId: null,
        });
        idStr = String(rowId);
      } catch (e) {
        if (import.meta.env.DEV) {
          console.warn("[TikClip] insertNotificationDb failed", e);
        }
      }
    }

    useNotificationStore.getState().addNotification({
      kind,
      title,
      body,
      id: idStr,
      createdAt,
    });

    const description = body || undefined;
    toast(title, {
      description,
      duration: 6500,
    });

    await trySendOsNotification(title, body);
  })();
}
