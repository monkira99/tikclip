import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import { FolderOpen, Trash2 } from "lucide-react";
import { ClipStatusBadge } from "@/components/clips/clip-status-badge";
import { TrimControls } from "@/components/clips/trim-controls";
import { VideoPlayer, type VideoPlayerHandle } from "@/components/clips/video-player";
import { Button } from "@/components/ui/button";
import {
  batchDeleteClips,
  getClipById,
  openPathInSystem,
  updateClipNotes,
  updateClipStatus,
  updateClipTitle,
} from "@/lib/api";
import { useClipStore } from "@/stores/clip-store";
import type { Clip, ClipStatus } from "@/types";

function formatBytes(n: number): string {
  if (n < 1024) {
    return `${n} B`;
  }
  if (n < 1024 * 1024) {
    return `${(n / 1024).toFixed(1)} KB`;
  }
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDuration(totalSeconds: number): string {
  const m = Math.floor(totalSeconds / 60);
  const s = Math.floor(totalSeconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

const STATUS_OPTIONS: ClipStatus[] = ["draft", "ready", "posted", "archived"];

export function ClipDetail({ clipId }: { clipId: number }) {
  const setActiveClipId = useClipStore((s) => s.setActiveClipId);
  const clipsRevision = useClipStore((s) => s.clipsRevision);
  const bumpClipsRevision = useClipStore((s) => s.bumpClipsRevision);

  const [clip, setClip] = useState<Clip | null>(null);
  const [titleEdit, setTitleEdit] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const [notesDraft, setNotesDraft] = useState("");
  const [videoDuration, setVideoDuration] = useState<number | null>(null);
  const playerRef = useRef<VideoPlayerHandle | null>(null);

  useEffect(() => {
    setVideoDuration(null);
  }, [clipId]);

  useEffect(() => {
    let cancelled = false;
    void getClipById(clipId)
      .then((row) => {
        if (!cancelled) {
          setClip(row);
          setTitleDraft(row.title?.trim() || `Clip #${clipId}`);
          setNotesDraft(row.notes ?? "");
        }
      })
      .catch(() => {
        if (!cancelled) {
          setClip(null);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [clipId, clipsRevision]);

  const videoSrc = useMemo(() => {
    if (!isTauri() || !clip?.file_path?.trim()) {
      return null;
    }
    try {
      return convertFileSrc(clip.file_path.trim());
    } catch {
      return null;
    }
  }, [clip?.file_path]);

  const durationForTrim = videoDuration ?? (clip ? Math.max(clip.duration_seconds, 0.001) : 0.001);

  const saveTitle = useCallback(async () => {
    if (!clip) {
      return;
    }
    const next = titleDraft.trim() || `Clip #${clip.id}`;
    await updateClipTitle(clip.id, next);
    setClip((c) => (c ? { ...c, title: next } : c));
    setTitleEdit(false);
    bumpClipsRevision();
  }, [clip, titleDraft, bumpClipsRevision]);

  const saveNotes = useCallback(async () => {
    if (!clip) {
      return;
    }
    await updateClipNotes(clip.id, notesDraft);
    setClip((c) => (c ? { ...c, notes: notesDraft } : c));
    bumpClipsRevision();
  }, [clip, notesDraft, bumpClipsRevision]);

  const onStatusChange = async (status: ClipStatus) => {
    if (!clip) {
      return;
    }
    await updateClipStatus(clip.id, status);
    setClip((c) => (c ? { ...c, status } : c));
    bumpClipsRevision();
  };

  const onDelete = async () => {
    if (!clip) {
      return;
    }
    if (!window.confirm(`Delete this clip permanently? This cannot be undone.`)) {
      return;
    }
    await batchDeleteClips([clip.id]);
    bumpClipsRevision();
    setActiveClipId(null);
  };

  const onTrimComplete = (newId: number) => {
    bumpClipsRevision();
    setActiveClipId(newId);
  };

  if (!clip) {
    return (
      <div className="space-y-4">
        <Button type="button" variant="outline" size="sm" onClick={() => setActiveClipId(null)}>
          Back
        </Button>
        <p className="text-sm text-[var(--color-text-muted)]">Loading clip…</p>
      </div>
    );
  }

  const displayTitle = clip.title?.trim() || `Clip #${clip.id}`;
  const user = clip.account_username?.trim() ? `@${clip.account_username.replace(/^@/, "")}` : "—";

  return (
    <div className="space-y-4">
      <Button type="button" variant="outline" size="sm" onClick={() => setActiveClipId(null)}>
        Back
      </Button>

      <div className="flex flex-col gap-6 lg:flex-row">
        <div className="min-w-0 flex-[1_1_65%] space-y-4">
          <VideoPlayer
            ref={playerRef}
            src={videoSrc}
            onDurationChange={(d) => setVideoDuration(d)}
          />
          <TrimControls
            clip={clip}
            playerRef={playerRef}
            durationSec={durationForTrim}
            onTrimComplete={onTrimComplete}
          />
        </div>

        <div className="min-w-0 flex-[1_1_35%] space-y-4 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <div className="space-y-1">
            {titleEdit ? (
              <input
                autoFocus
                value={titleDraft}
                onChange={(e) => setTitleDraft(e.target.value)}
                onBlur={() => void saveTitle()}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    void saveTitle();
                  }
                  if (e.key === "Escape") {
                    setTitleDraft(displayTitle);
                    setTitleEdit(false);
                  }
                }}
                className="w-full rounded-md border border-[var(--color-border)] bg-background px-2 py-1 text-lg font-semibold text-[var(--color-text)]"
              />
            ) : (
              <h2
                className="cursor-text text-lg font-semibold text-[var(--color-text)]"
                onClick={() => {
                  setTitleDraft(displayTitle);
                  setTitleEdit(true);
                }}
              >
                {displayTitle}
              </h2>
            )}
            <p className="text-sm text-[var(--color-text-muted)]">{user}</p>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs text-[var(--color-text-muted)]">Status</span>
            <ClipStatusBadge status={clip.status} />
            <select
              value={clip.status}
              onChange={(e) => void onStatusChange(e.target.value as ClipStatus)}
              className="rounded-md border border-[var(--color-border)] bg-background px-2 py-1 text-sm capitalize text-[var(--color-text)]"
            >
              {STATUS_OPTIONS.map((s) => (
                <option key={s} value={s}>
                  {s}
                </option>
              ))}
            </select>
          </div>

          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
            <dt className="text-[var(--color-text-muted)]">Duration</dt>
            <dd className="font-mono text-[var(--color-text)]">{formatDuration(clip.duration_seconds)}</dd>
            <dt className="text-[var(--color-text-muted)]">File size</dt>
            <dd className="font-mono text-[var(--color-text)]">{formatBytes(clip.file_size_bytes)}</dd>
            <dt className="text-[var(--color-text-muted)]">Scene</dt>
            <dd className="text-[var(--color-text)]">{clip.scene_type ?? "—"}</dd>
            <dt className="text-[var(--color-text-muted)]">Created</dt>
            <dd className="text-[var(--color-text)]">
              {(() => {
                const raw = clip.created_at.trim();
                const d = new Date(`${raw.replace(" ", "T")}+07:00`);
                return Number.isNaN(d.getTime()) ? raw : d.toLocaleString();
              })()}
            </dd>
          </dl>

          <label className="flex flex-col gap-1 text-sm">
            <span className="text-[var(--color-text-muted)]">Notes</span>
            <textarea
              value={notesDraft}
              onChange={(e) => setNotesDraft(e.target.value)}
              onBlur={() => void saveNotes()}
              rows={4}
              className="resize-y rounded-md border border-[var(--color-border)] bg-background px-2 py-2 text-[var(--color-text)]"
            />
          </label>

          <div className="rounded-md border border-dashed border-[var(--color-border)] px-3 py-2 text-sm text-[var(--color-text-muted)]">
            Products: coming soon
          </div>

          <div className="flex flex-wrap gap-2 pt-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => void openPathInSystem(clip.file_path)}
            >
              <FolderOpen className="mr-1 h-4 w-4" />
              Open in Finder
            </Button>
            <Button type="button" variant="destructive" size="sm" onClick={() => void onDelete()}>
              <Trash2 className="mr-1 h-4 w-4" />
              Delete
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
