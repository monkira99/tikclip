import type { Clip } from "@/types";

export type UserClipGroup = {
  username: string;
  clips: Clip[];
};

export type DateClipGroup = {
  dateKey: string;
  label: string;
  users: UserClipGroup[];
};

/** `created_at` from SQLite → `YYYY-MM-DD` for grouping. */
export function clipDateKey(createdAt: string): string {
  const s = createdAt.trim();
  if (s.length >= 10) {
    return s.slice(0, 10);
  }
  const d = new Date(s);
  if (!Number.isNaN(d.getTime())) {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${day}`;
  }
  return "unknown";
}

function formatDateLabel(dateKey: string): string {
  if (dateKey === "unknown") {
    return "Không rõ ngày";
  }
  try {
    const [y, m, d] = dateKey.split("-").map(Number);
    const dt = new Date(y, m - 1, d);
    return dt.toLocaleDateString("vi-VN", {
      weekday: "long",
      year: "numeric",
      month: "long",
      day: "numeric",
    });
  } catch {
    return dateKey;
  }
}

function userLabel(clip: Clip): string {
  return clip.account_username?.trim() || `account-${clip.account_id}`;
}

/** Newest dates first; users A–Z; clips newest first by id. */
export function groupClipsByDateAndUser(clips: Clip[]): DateClipGroup[] {
  const byDate = new Map<string, Map<string, Clip[]>>();

  for (const c of clips) {
    const dk = clipDateKey(c.created_at);
    if (!byDate.has(dk)) {
      byDate.set(dk, new Map());
    }
    const users = byDate.get(dk)!;
    const u = userLabel(c);
    if (!users.has(u)) {
      users.set(u, []);
    }
    users.get(u)!.push(c);
  }

  const dateKeys = Array.from(byDate.keys()).sort((a, b) => b.localeCompare(a));

  return dateKeys.map((dateKey) => {
    const usersMap = byDate.get(dateKey)!;
    const users = Array.from(usersMap.entries())
      .sort(([a], [b]) => a.localeCompare(b, "vi"))
      .map(([username, list]) => ({
        username,
        clips: [...list].sort((x, y) => y.id - x.id),
      }));
    return {
      dateKey,
      label: formatDateLabel(dateKey),
      users,
    };
  });
}

export function userRowKey(dateKey: string, username: string): string {
  return `${dateKey}\u0001${username}`;
}
