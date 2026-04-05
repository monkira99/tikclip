import { isTauri } from "@tauri-apps/api/core";
import { sendNotification } from "@tauri-apps/plugin-notification";
import {
  type NotificationKind,
  useNotificationStore,
} from "@/stores/notification-store";

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

/**
 * Queue in the notification store and show an OS notification when running inside Tauri.
 */
export function dispatchSidecarNotification(
  eventType: string,
  data: Record<string, unknown>,
): void {
  const { kind, title, body } = formatSidecarMessage(eventType, data);
  useNotificationStore.getState().addNotification({ kind, title, body });

  if (!isTauri()) {
    return;
  }
  try {
    void sendNotification({ title, body: body || undefined });
  } catch {
    /* OS notification unavailable or permission denied */
  }
}
