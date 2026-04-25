import { useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import { ClipStatusBadge } from "@/components/clips/clip-status-badge";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatBytes, formatDuration } from "@/lib/format";
import { useClipStore } from "@/stores/clip-store";
import type { Clip } from "@/types";

function ClipRowThumb({ clip }: { clip: Clip }) {
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setFailed(false);
  }, [clip.id, clip.thumbnail_path]);

  const thumbSrc = useMemo(() => {
    if (failed) return null;
    const path = clip.thumbnail_path?.trim();
    if (!path || !isTauri()) return null;
    try {
      return convertFileSrc(path);
    } catch {
      return null;
    }
  }, [clip.thumbnail_path, failed]);

  if (thumbSrc && !failed) {
    return (
      <img
        src={thumbSrc}
        alt=""
        width={48}
        height={48}
        className="size-12 rounded-md object-cover"
        onError={() => setFailed(true)}
      />
    );
  }

  return (
    <div className="flex size-12 items-center justify-center rounded-md bg-black/30 text-lg opacity-50">
      🎞️
    </div>
  );
}

type ClipListProps = {
  clips?: Clip[];
  loading?: boolean;
  onRefresh?: () => void | Promise<void>;
  emptyMessage?: string;
  selectable?: boolean;
};

export function ClipList({
  clips,
  loading,
  onRefresh,
  emptyMessage = "No clips match the current filters.",
  selectable = true,
}: ClipListProps = {}) {
  const clipsFromStore = useClipStore((s) => s.clips);
  const loadingFromStore = useClipStore((s) => s.loading);
  const selectedClipIds = useClipStore((s) => s.selectedClipIds);
  const toggleSelect = useClipStore((s) => s.toggleSelect);
  const selectAll = useClipStore((s) => s.selectAll);
  const clearSelection = useClipStore((s) => s.clearSelection);
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

  const headerRef = useRef<HTMLInputElement>(null);

  const allSelected =
    selectable && data.length > 0 && data.every((c) => selectedClipIds.has(c.id));
  const someSelected = selectable && data.some((c) => selectedClipIds.has(c.id));

  useEffect(() => {
    const el = headerRef.current;
    if (el) {
      el.indeterminate = someSelected && !allSelected;
    }
  }, [someSelected, allSelected]);

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
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead className="w-10">
            {selectable ? (
              <input
                ref={headerRef}
                type="checkbox"
                className="size-4 rounded border-input"
                checked={allSelected}
                onChange={() => {
                  if (allSelected) {
                    if (clips) {
                      data.forEach((clip) => {
                        if (selectedClipIds.has(clip.id)) {
                          toggleSelect(clip.id);
                        }
                      });
                    } else {
                      clearSelection();
                    }
                  } else {
                    if (clips) {
                      data.forEach((clip) => {
                        if (!selectedClipIds.has(clip.id)) {
                          toggleSelect(clip.id);
                        }
                      });
                    } else {
                      selectAll();
                    }
                  }
                }}
                aria-label="Select all clips"
              />
            ) : null}
          </TableHead>
          <TableHead className="w-14" />
          <TableHead>Title</TableHead>
          <TableHead>Account</TableHead>
          <TableHead>Duration</TableHead>
          <TableHead>Size</TableHead>
          <TableHead>Status</TableHead>
          <TableHead>Scene</TableHead>
          <TableHead>Created</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {data.map((clip) => (
          <ClipTableRow
            key={clip.id}
            clip={clip}
            selected={selectable ? selectedClipIds.has(clip.id) : false}
            onToggleSelect={selectable ? () => toggleSelect(clip.id) : undefined}
            onOpen={() => setActiveClipId(clip.id)}
            selectable={selectable}
          />
        ))}
      </TableBody>
    </Table>
  );
}

function ClipTableRow({
  clip,
  selected,
  onToggleSelect,
  onOpen,
  selectable,
}: {
  clip: Clip;
  selected: boolean;
  onToggleSelect?: () => void;
  onOpen: () => void;
  selectable: boolean;
}) {
  const uname = clip.account_username?.trim();
  const accountLabel = uname
    ? uname.startsWith("account-")
      ? uname
      : `@${uname}`
    : "—";

  return (
    <TableRow
      className="cursor-pointer"
      onClick={onOpen}
      data-state={selected ? "selected" : undefined}
    >
      <TableCell
        onClick={(e) => {
          e.stopPropagation();
        }}
      >
        {selectable ? (
          <input
            type="checkbox"
            className="size-4 rounded border-input"
            checked={selected}
            onChange={() => onToggleSelect?.()}
            aria-label={`Select clip ${clip.id}`}
          />
        ) : null}
      </TableCell>
      <TableCell>
        <ClipRowThumb clip={clip} />
      </TableCell>
      <TableCell className="max-w-[200px] truncate font-medium text-[var(--color-text)]">
        {clip.title?.trim() || `Clip #${clip.id}`}
      </TableCell>
      <TableCell className="text-[var(--color-text-muted)]">{accountLabel}</TableCell>
      <TableCell className="font-mono text-xs">{formatDuration(clip.duration_seconds)}</TableCell>
      <TableCell className="text-xs text-[var(--color-text-muted)]">
        {formatBytes(clip.file_size_bytes)}
      </TableCell>
      <TableCell>
        <ClipStatusBadge status={clip.status} />
      </TableCell>
      <TableCell>
        {clip.scene_type ? (
          <Badge variant="outline" className="text-[10px]">
            {clip.scene_type}
          </Badge>
        ) : (
          "—"
        )}
      </TableCell>
      <TableCell className="text-xs text-[var(--color-text-muted)]">
        {clip.created_at}
      </TableCell>
    </TableRow>
  );
}
