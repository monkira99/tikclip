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
import { useClipStore } from "@/stores/clip-store";
import type { Clip } from "@/types";

function formatDuration(totalSeconds: number): string {
  const m = Math.floor(totalSeconds / 60);
  const s = Math.floor(totalSeconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function formatBytes(n: number): string {
  if (n < 1024) {
    return `${n} B`;
  }
  if (n < 1024 * 1024) {
    return `${(n / 1024).toFixed(1)} KB`;
  }
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

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

export function ClipList() {
  const clips = useClipStore((s) => s.clips);
  const loading = useClipStore((s) => s.loading);
  const selectedClipIds = useClipStore((s) => s.selectedClipIds);
  const toggleSelect = useClipStore((s) => s.toggleSelect);
  const selectAll = useClipStore((s) => s.selectAll);
  const clearSelection = useClipStore((s) => s.clearSelection);
  const setActiveClipId = useClipStore((s) => s.setActiveClipId);
  const fetchClips = useClipStore((s) => s.fetchClips);

  const headerRef = useRef<HTMLInputElement>(null);

  const allSelected =
    clips.length > 0 && clips.every((c) => selectedClipIds.has(c.id));
  const someSelected = clips.some((c) => selectedClipIds.has(c.id));

  useEffect(() => {
    const el = headerRef.current;
    if (el) {
      el.indeterminate = someSelected && !allSelected;
    }
  }, [someSelected, allSelected]);

  if (loading) {
    return <p className="text-sm text-[var(--color-text-muted)]">Loading clips…</p>;
  }

  if (clips.length === 0) {
    return (
      <div className="space-y-2">
        <p className="text-sm text-[var(--color-text-muted)]">No clips match the current filters.</p>
        <Button type="button" variant="outline" size="sm" onClick={() => void fetchClips()}>
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
            <input
              ref={headerRef}
              type="checkbox"
              className="size-4 rounded border-input"
              checked={allSelected}
              onChange={() => {
                if (allSelected) {
                  clearSelection();
                } else {
                  selectAll();
                }
              }}
              aria-label="Select all clips"
            />
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
        {clips.map((clip) => (
          <ClipTableRow
            key={clip.id}
            clip={clip}
            selected={selectedClipIds.has(clip.id)}
            onToggleSelect={() => toggleSelect(clip.id)}
            onOpen={() => setActiveClipId(clip.id)}
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
}: {
  clip: Clip;
  selected: boolean;
  onToggleSelect: () => void;
  onOpen: () => void;
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
        <input
          type="checkbox"
          className="size-4 rounded border-input"
          checked={selected}
          onChange={() => onToggleSelect()}
          aria-label={`Select clip ${clip.id}`}
        />
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
