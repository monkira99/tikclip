import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { ClipCard } from "@/components/clips/clip-card";
import { Button } from "@/components/ui/button";
import {
  groupClipsByDateAndUser,
  userRowKey,
  type DateClipGroup,
} from "@/lib/group-clips";
import { cn } from "@/lib/utils";
import { useClipStore } from "@/stores/clip-store";
import type { Clip } from "@/types";

function applyFirstDateDefaults(grouped: DateClipGroup[]): {
  dates: Set<string>;
  users: Set<string>;
} {
  if (grouped.length === 0) {
    return { dates: new Set(), users: new Set() };
  }
  const first = grouped[0].dateKey;
  const users = new Set<string>();
  for (const u of grouped[0].users) {
    users.add(userRowKey(first, u.username));
  }
  return { dates: new Set([first]), users };
}

type ClipGridProps = {
  clips?: Clip[];
  loading?: boolean;
  onRefresh?: () => void | Promise<void>;
  emptyMessage?: string;
  queueTitle?: string;
  queueDescription?: string;
  selectable?: boolean;
};

export function ClipGrid({
  clips,
  loading,
  onRefresh,
  emptyMessage = "No clips match the current filters.",
  queueTitle = "Review queue",
  queueDescription = "Browse clips by day and account to keep review sessions compact.",
  selectable = true,
}: ClipGridProps = {}) {
  const clipsFromStore = useClipStore((s) => s.clips);
  const loadingFromStore = useClipStore((s) => s.loading);
  const selectedClipIds = useClipStore((s) => s.selectedClipIds);
  const toggleSelect = useClipStore((s) => s.toggleSelect);
  const setActiveClipId = useClipStore((s) => s.setActiveClipId);
  const fetchClips = useClipStore((s) => s.fetchClips);

  const data = clips ?? clipsFromStore;
  const isLoading = loading ?? loadingFromStore;
  const handleRefresh = () => {
    if (onRefresh) {
      return onRefresh();
    }
    return fetchClips();
  };

  const [openDates, setOpenDates] = useState<Set<string>>(() => new Set());
  const [openUsers, setOpenUsers] = useState<Set<string>>(() => new Set());
  const expansionInitRef = useRef(false);

  const grouped = useMemo(() => groupClipsByDateAndUser(data), [data]);

  useEffect(() => {
    return () => {
      expansionInitRef.current = false;
    };
  }, []);

  useEffect(() => {
    if (data.length === 0) {
      expansionInitRef.current = false;
      setOpenDates(new Set());
      setOpenUsers(new Set());
      return;
    }
    if (!expansionInitRef.current) {
      const { dates, users } = applyFirstDateDefaults(grouped);
      setOpenDates(dates);
      setOpenUsers(users);
      expansionInitRef.current = true;
    }
  }, [data.length, grouped]);

  const toggleDate = useCallback((dateKey: string) => {
    setOpenDates((prev) => {
      const next = new Set(prev);
      if (next.has(dateKey)) {
        next.delete(dateKey);
      } else {
        next.add(dateKey);
      }
      return next;
    });
  }, []);

  const toggleUser = useCallback((dateKey: string, username: string) => {
    const key = userRowKey(dateKey, username);
    setOpenUsers((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }, []);

  if (isLoading) {
    return <p className="text-sm text-[var(--color-text-muted)]">Loading clips…</p>;
  }

  if (data.length === 0) {
    return (
      <div className="space-y-2">
        <p className="text-sm text-[var(--color-text-muted)]">{emptyMessage}</p>
        <Button type="button" variant="outline" size="sm" onClick={() => void handleRefresh()}>
          Refresh
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="app-panel-subtle flex items-center justify-between rounded-2xl px-4 py-4">
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--color-text-muted)]">
            {queueTitle}
          </p>
          <p className="mt-1 text-sm text-[var(--color-text-soft)]">
            {queueDescription}
          </p>
        </div>
        <Button type="button" variant="outline" size="sm" onClick={() => void handleRefresh()}>
          Refresh
        </Button>
      </div>

      <div className="flex flex-col gap-1">
        {grouped.map((block) => {
          const dateOpen = openDates.has(block.dateKey);
          const clipCount = block.users.reduce((n, u) => n + u.clips.length, 0);
          return (
            <section
              key={block.dateKey}
              className="app-panel-subtle overflow-hidden rounded-2xl"
            >
              <button
                type="button"
                onClick={() => toggleDate(block.dateKey)}
                className={cn(
                  "flex w-full items-center gap-2 px-4 py-3 text-left transition-colors",
                  "hover:bg-white/[0.03]",
                )}
              >
                {dateOpen ? (
                  <ChevronDown className="size-4 shrink-0 text-[var(--color-text-muted)]" />
                ) : (
                  <ChevronRight className="size-4 shrink-0 text-[var(--color-text-muted)]" />
                )}
                <span className="min-w-0 flex-1 font-medium text-[var(--color-text)]">
                  {block.label}
                </span>
                <span className="shrink-0 text-xs text-[var(--color-text-muted)]">
                  {clipCount} clip{clipCount === 1 ? "" : "s"}
                </span>
              </button>

              {dateOpen ? (
                <div className="border-t border-white/6 px-2 pb-3 pt-1">
                  {block.users.map((ug) => {
                    const uKey = userRowKey(block.dateKey, ug.username);
                    const userOpen = openUsers.has(uKey);
                    return (
                      <div key={uKey} className="mt-2 first:mt-0">
                        <button
                          type="button"
                          onClick={() => toggleUser(block.dateKey, ug.username)}
                          className={cn(
                            "flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm",
                            "hover:bg-white/[0.03]",
                          )}
                        >
                          {userOpen ? (
                            <ChevronDown className="size-3.5 shrink-0 text-[var(--color-text-muted)]" />
                          ) : (
                            <ChevronRight className="size-3.5 shrink-0 text-[var(--color-text-muted)]" />
                          )}
                          <span className="font-medium text-[var(--color-text)]">
                            {ug.username.startsWith("account-")
                              ? ug.username
                              : `@${ug.username}`}
                          </span>
                          <span className="text-xs text-[var(--color-text-muted)]">
                            {ug.clips.length}
                          </span>
                        </button>
                        {userOpen ? (
                          <div className="mt-2 grid gap-4 px-1 sm:grid-cols-2 lg:grid-cols-3">
                            {ug.clips.map((c) => (
                              <ClipCard
                                key={c.id}
                                clip={c}
                                selected={selectable ? selectedClipIds.has(c.id) : false}
                                onToggleSelect={selectable ? () => toggleSelect(c.id) : undefined}
                                onOpen={() => setActiveClipId(c.id)}
                                selectable={selectable}
                              />
                            ))}
                          </div>
                        ) : null}
                      </div>
                    );
                  })}
                </div>
              ) : null}
            </section>
          );
        })}
      </div>
    </div>
  );
}
