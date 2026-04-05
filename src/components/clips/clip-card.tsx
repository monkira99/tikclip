import { useMemo } from "react";
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardFooter, CardHeader } from "@/components/ui/card";
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
}

export function ClipCard({ clip }: ClipCardProps) {
  const thumbSrc = useMemo(() => {
    const path = clip.thumbnail_path;
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
  }, [clip.thumbnail_path]);

  return (
    <Card className="overflow-hidden border-[var(--color-border)] bg-[var(--color-surface)]">
      <div className="relative aspect-video w-full bg-black/40">
        {thumbSrc ? (
          <img src={thumbSrc} alt="" className="h-full w-full object-cover" />
        ) : (
          <div className="flex h-full w-full items-center justify-center text-4xl opacity-40">
            🎞️
          </div>
        )}
        <div className="absolute bottom-2 right-2 rounded bg-black/60 px-2 py-0.5 font-mono text-[10px] text-white">
          {formatDuration(clip.duration_seconds)}
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
        <Badge variant="outline" className="text-[10px]">
          {clip.status}
        </Badge>
        {clip.scene_type && (
          <Badge variant="secondary" className="ml-2 text-[10px]">
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
