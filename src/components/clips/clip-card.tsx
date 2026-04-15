import { useEffect, useMemo, useState } from "react";
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import { ClipStatusBadge } from "@/components/clips/clip-status-badge";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardFooter, CardHeader } from "@/components/ui/card";
import { cn } from "@/lib/utils";
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

interface ClipCardProps {
  clip: Clip;
  selected?: boolean;
  onToggleSelect?: () => void;
  onOpen?: () => void;
  selectable?: boolean;
}

export function ClipCard({
  clip,
  selected = false,
  onToggleSelect,
  onOpen,
  selectable = true,
}: ClipCardProps) {
  const [thumbFailed, setThumbFailed] = useState(false);

  useEffect(() => {
    setThumbFailed(false);
  }, [clip.id, clip.thumbnail_path]);

  const thumbSrc = useMemo(() => {
    if (thumbFailed) {
      return null;
    }
    const path = clip.thumbnail_path?.trim();
    if (!path) {
      return null;
    }
    if (!isTauri()) {
      return null;
    }
    try {
      return convertFileSrc(path);
    } catch {
      return null;
    }
  }, [clip.thumbnail_path, thumbFailed]);

  const videoSrc = useMemo(() => {
    if (!isTauri() || !clip.file_path?.trim()) {
      return null;
    }
    try {
      return convertFileSrc(clip.file_path.trim());
    } catch {
      return null;
    }
  }, [clip.file_path]);

  const showVideoPoster = thumbFailed || !thumbSrc;

  return (
    <Card
      className={cn(
        "relative cursor-pointer overflow-hidden transition-opacity hover:opacity-95",
        selectable &&
          selected &&
          "border-[color-mix(in_oklab,var(--color-accent)_30%,var(--color-border))] shadow-[0_0_0_1px_rgba(85,179,255,0.18),inset_0_1px_0_rgba(255,255,255,0.04),0_18px_40px_rgba(0,0,0,0.24)]",
      )}
      onClick={() => onOpen?.()}
      onKeyDown={(e) => {
        if (!onOpen) return;
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onOpen();
        }
      }}
      role={onOpen ? "button" : undefined}
      tabIndex={onOpen ? 0 : undefined}
    >
      {selectable ? (
        <div
          className="absolute left-2 top-2 z-10"
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
        >
          <input
            type="checkbox"
            className="size-4 rounded border border-white/20 bg-black/50 shadow-sm"
            checked={selected}
            onChange={() => onToggleSelect?.()}
            aria-label={`Select clip ${clip.id}`}
          />
        </div>
      ) : null}

      <div className="relative aspect-video w-full bg-black/40">
        {thumbSrc && !thumbFailed ? (
          <img
            src={thumbSrc}
            alt=""
            className="h-full w-full object-cover"
            onError={() => setThumbFailed(true)}
          />
        ) : showVideoPoster && videoSrc ? (
          <video
            src={videoSrc}
            muted
            playsInline
            preload="metadata"
            className="h-full w-full object-cover"
            aria-label="Xem trước clip"
          />
        ) : (
          <div className="flex h-full w-full items-center justify-center text-4xl opacity-40">
            🎞️
          </div>
        )}
        <div className="app-keycap pointer-events-none absolute bottom-2 right-2 rounded-md px-2 py-0.5 text-[10px] text-white">
          {formatDuration(clip.duration_seconds)}
        </div>
        <div className="pointer-events-none absolute bottom-2 left-2">
          <ClipStatusBadge status={clip.status} />
        </div>
      </div>
      <CardHeader className="space-y-1 pb-2">
        <div className="line-clamp-2 text-sm font-semibold text-[var(--color-text)]">
          {clip.title?.trim() || `Clip #${clip.id}`}
        </div>
        {clip.account_username && (
          <div className="text-xs text-[var(--color-text-muted)]">@{clip.account_username}</div>
        )}
      </CardHeader>
      <CardContent className="pb-2">
        {clip.scene_type && (
          <Badge variant="secondary" className="text-[10px]">
            {clip.scene_type}
          </Badge>
        )}
      </CardContent>
      <CardFooter className="text-[10px] text-[var(--color-text-muted)]">
        {formatBytes(clip.file_size_bytes)} · rec {clip.recording_id}
      </CardFooter>
    </Card>
  );
}
