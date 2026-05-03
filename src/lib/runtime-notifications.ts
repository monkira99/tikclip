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

function pickNotificationId(data: Record<string, unknown>): string | undefined {
  const raw = data.notification_id;
  if (typeof raw === "number" && Number.isFinite(raw)) {
    return String(raw);
  }
  if (typeof raw === "string" && raw.trim()) {
    return raw.trim();
  }
  return undefined;
}

function formatRuntimeMessage(
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
    case "cleanup_completed": {
      const freed = data.freed_bytes;
      const n =
        typeof freed === "number" && Number.isFinite(freed)
          ? freed
          : typeof freed === "string"
            ? Number(freed)
            : 0;
      const mb = n > 0 ? (n / (1024 * 1024)).toFixed(1) : "0";
      const rec = data.deleted_recordings;
      const clips = data.deleted_clips;
      const recN = typeof rec === "number" ? rec : Number(rec) || 0;
      const clipN = typeof clips === "number" ? clips : Number(clips) || 0;
      return {
        kind: "cleanup_completed",
        title: "Dọn dẹp hoàn tất",
        body: `Đã xóa ${recN} file ghi, ${clipN} clip; giải phóng ~${mb} MB`,
      };
    }
    case "storage_warning": {
      const pct = data.usage_percent;
      const p =
        typeof pct === "number" && Number.isFinite(pct)
          ? pct
          : typeof pct === "string"
            ? Number(pct)
            : 0;
      const critical = data.critical === true;
      return {
        kind: "storage_warning",
        title: critical ? "Dung lượng gần đầy" : "Cảnh báo dung lượng",
        body:
          Number.isFinite(p) && p > 0
            ? `Đang dùng khoảng ${p.toFixed(1)}% quota cấu hình`
            : "Kiểm tra mục Cài đặt → Storage",
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
export function dispatchRuntimeNotification(
  eventType: string,
  data: Record<string, unknown>,
): void {
  void (async () => {
    const { kind, title, body } = formatRuntimeMessage(eventType, data);
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

/**
 * Rust-owned events are already persisted before emission; only mirror them into UI surfaces.
 */
export function displayRuntimeNotification(
  eventType: string,
  data: Record<string, unknown>,
): void {
  void (async () => {
    const { kind, title, body } = formatRuntimeMessage(eventType, data);
    useNotificationStore.getState().addNotification({
      kind,
      title,
      body,
      id: pickNotificationId(data),
      createdAt: Date.now(),
    });

    toast(title, {
      description: body || undefined,
      duration: 6500,
    });

    await trySendOsNotification(title, body);
  })();
}
