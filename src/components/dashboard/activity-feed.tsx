import { useCallback, useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  dbNotificationCreatedAtToMs,
  listActivityFeed,
  type ActivityFeedItem,
} from "@/lib/api";
import { useAppStore } from "@/stores/app-store";
import { useClipStore } from "@/stores/clip-store";

function iconForType(t: string): string {
  switch (t) {
    case "account_live":
      return "🔴";
    case "recording_finished":
      return "🎬";
    case "clip_ready":
      return "✂️";
    case "product_created":
      return "📦";
    case "cleanup_completed":
      return "🧹";
    case "storage_warning":
      return "⚠️";
    default:
      return "•";
  }
}

function relativeTime(createdAt: string): string {
  const ms = dbNotificationCreatedAtToMs(createdAt);
  const diff = Date.now() - ms;
  const s = Math.floor(diff / 1000);
  if (s < 60) return "vừa xong";
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} phút trước`;
  const h = Math.floor(m / 60);
  if (h < 48) return `${h} giờ trước`;
  const d = Math.floor(h / 24);
  return `${d} ngày trước`;
}

type ActivityFeedProps = {
  dashboardRevision: number;
};

export function ActivityFeed({ dashboardRevision }: ActivityFeedProps) {
  const [items, setItems] = useState<ActivityFeedItem[]>([]);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    if (!isTauri()) {
      setItems([]);
      setLoading(false);
      return;
    }
    setLoading(true);
    try {
      const rows = await listActivityFeed(10);
      setItems(rows);
    } catch (e) {
      if (import.meta.env.DEV) {
        console.warn("[TikClip] listActivityFeed failed", e);
      }
      setItems([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load, dashboardRevision]);

  const onItemClick = (row: ActivityFeedItem) => {
    if (row.clip_id != null && Number.isFinite(row.clip_id)) {
      useClipStore.getState().setActiveClipId(row.clip_id);
      useAppStore.getState().requestNavigation({ page: "clips", clipId: row.clip_id });
    }
  };

  return (
    <Card className="bg-[var(--color-bg-elevated)]">
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-base">Hoạt động gần đây</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2">
        {loading ? (
          <p className="text-sm text-[var(--color-text-muted)]">Đang tải…</p>
        ) : items.length === 0 ? (
          <p className="text-sm text-[var(--color-text-muted)]">Chưa có sự kiện.</p>
        ) : (
          <ul className="space-y-2">
            {items.map((row) => {
              const clickable = row.clip_id != null;
              return (
                <li key={row.id}>
                  <button
                    type="button"
                    disabled={!clickable}
                    onClick={() => onItemClick(row)}
                    className={`flex w-full gap-2 rounded-md border border-transparent px-2 py-1.5 text-left text-sm transition-colors ${
                      clickable
                        ? "cursor-pointer hover:border-[var(--color-border)] hover:bg-[var(--color-bg)]"
                        : "cursor-default opacity-95"
                    }`}
                  >
                    <span className="shrink-0 pt-0.5" aria-hidden>
                      {iconForType(row.type)}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="font-medium text-[var(--color-text)]">{row.title}</span>
                      {row.message ? (
                        <span className="mt-0.5 block text-[var(--color-text-muted)]">
                          {row.message}
                        </span>
                      ) : null}
                    </span>
                    <span className="shrink-0 text-xs text-[var(--color-text-muted)] tabular-nums">
                      {relativeTime(row.created_at)}
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
        <p className="border-t border-[var(--color-border)] pt-3 text-xs text-[var(--color-text-muted)]">
          Xem thêm trong hộp thông báo trên thanh tiêu đề.
        </p>
      </CardContent>
    </Card>
  );
}
