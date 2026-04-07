import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { ClipCard } from "@/components/clips/clip-card";
import { Button } from "@/components/ui/button";
import { listClips } from "@/lib/api";
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

export function ClipGrid() {
  const [clips, setClips] = useState<Clip[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const clipsRevision = useClipStore((s) => s.clipsRevision);
  const [openDates, setOpenDates] = useState<Set<string>>(() => new Set());
  const [openUsers, setOpenUsers] = useState<Set<string>>(() => new Set());
  const expansionInitRef = useRef(false);

  const grouped = useMemo(() => groupClipsByDateAndUser(clips), [clips]);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const rows = await listClips();
      setClips(rows);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setClips([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load, clipsRevision]);

  useEffect(() => {
    return () => {
      expansionInitRef.current = false;
    };
  }, []);

  useEffect(() => {
    if (clips.length === 0) {
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
  }, [clips.length, grouped]);

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

  if (loading) {
    return <p className="text-sm text-[var(--color-text-muted)]">Loading clips…</p>;
  }

  if (error) {
    return (
      <div className="space-y-2">
        <p className="text-sm text-red-400">{error}</p>
        <Button type="button" variant="outline" size="sm" onClick={() => void load()}>
          Retry
        </Button>
      </div>
    );
  }

  if (clips.length === 0) {
    return (
      <div className="space-y-2">
        <p className="text-sm text-[var(--color-text-muted)]">No clips in the database yet.</p>
        <Button type="button" variant="outline" size="sm" onClick={() => void load()}>
          Refresh
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex justify-end">
        <Button type="button" variant="outline" size="sm" onClick={() => void load()}>
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
              className="overflow-hidden rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)]"
            >
              <button
                type="button"
                onClick={() => toggleDate(block.dateKey)}
                className={cn(
                  "flex w-full items-center gap-2 px-4 py-3 text-left transition-colors",
                  "hover:bg-white/5",
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
                <div className="border-t border-[var(--color-border)] px-2 pb-3 pt-1">
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
                            "hover:bg-white/5",
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
                              <ClipCard key={c.id} clip={c} />
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
