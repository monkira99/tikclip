import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import { FolderOpen, Plus, Trash2, X } from "lucide-react";
import { ClipStatusBadge } from "@/components/clips/clip-status-badge";
import { TrimControls } from "@/components/clips/trim-controls";
import { VideoPlayer, type VideoPlayerHandle } from "@/components/clips/video-player";
import { Button } from "@/components/ui/button";
import { ProductMediaThumb } from "@/components/products/product-media-thumb";
import { ProductPicker } from "@/components/products/product-picker";
import {
  batchDeleteClips,
  getClipById,
  listClipProducts,
  openPathInSystem,
  untagClipProduct,
  updateClipNotes,
  updateClipStatus,
  updateClipTitle,
} from "@/lib/api";
import { formatBytes, formatDuration } from "@/lib/format";
import { useClipStore } from "@/stores/clip-store";
import type { Clip, ClipStatus, Product } from "@/types";

const STATUS_OPTIONS: ClipStatus[] = ["draft", "ready", "posted", "archived"];

type ClipDetailProps = {
  clipId: number;
  onBack?: () => void;
  backLabel?: string;
  onMutated?: () => void;
};

export function ClipDetail({ clipId, onBack, backLabel = "Back", onMutated }: ClipDetailProps) {
  const setActiveClipId = useClipStore((s) => s.setActiveClipId);
  const clipsRevision = useClipStore((s) => s.clipsRevision);
  const bumpClipsRevision = useClipStore((s) => s.bumpClipsRevision);
  const clipFromList = useClipStore((s) => s.clips.find((c) => c.id === clipId) ?? null);

  const [remoteClip, setRemoteClip] = useState<Clip | null>(null);
  const [fetchDone, setFetchDone] = useState(false);
  const [fetchError, setFetchError] = useState<string | null>(null);

  const clip = remoteClip ?? clipFromList;
  const [titleEdit, setTitleEdit] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const [notesDraft, setNotesDraft] = useState("");
  const [videoDuration, setVideoDuration] = useState<number | null>(null);
  const playerRef = useRef<VideoPlayerHandle | null>(null);
  const [clipProducts, setClipProducts] = useState<Product[]>([]);
  const [productPickerOpen, setProductPickerOpen] = useState(false);

  const handleBack = useCallback(() => {
    if (onBack) {
      onBack();
      return;
    }
    setActiveClipId(null);
  }, [onBack, setActiveClipId]);

  const refreshClipProducts = useCallback(async () => {
    try {
      const rows = await listClipProducts(clipId);
      setClipProducts(rows);
    } catch {
      setClipProducts([]);
    }
  }, [clipId]);

  useEffect(() => {
    setVideoDuration(null);
  }, [clipId]);

  useEffect(() => {
    const cached = useClipStore.getState().clips.find((c) => c.id === clipId);
    setTitleDraft(cached?.title?.trim() || `Clip #${clipId}`);
    setNotesDraft(cached?.notes ?? "");
    setTitleEdit(false);
  }, [clipId]);

  useEffect(() => {
    let cancelled = false;
    setRemoteClip(null);
    setFetchDone(false);
    setFetchError(null);

    void getClipById(clipId)
      .then((row) => {
        if (!cancelled) {
          setRemoteClip(row);
          setTitleDraft(row.title?.trim() || `Clip #${clipId}`);
          setNotesDraft(row.notes ?? "");
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setFetchError(e instanceof Error ? e.message : String(e));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setFetchDone(true);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [clipId, clipsRevision]);

  useEffect(() => {
    void refreshClipProducts();
  }, [clipId, clipsRevision, refreshClipProducts]);

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

  const patchLocalClip = useCallback(
    (patch: Partial<Clip>) => {
      setRemoteClip((c) => {
        const base = c ?? useClipStore.getState().clips.find((x) => x.id === clipId) ?? null;
        if (!base) {
          return c;
        }
        return { ...base, ...patch };
      });
    },
    [clipId],
  );

  const saveTitle = useCallback(async () => {
    if (!clip) {
      return;
    }
    const next = titleDraft.trim() || `Clip #${clip.id}`;
    await updateClipTitle(clip.id, next);
    patchLocalClip({ title: next });
    setTitleEdit(false);
    bumpClipsRevision();
    onMutated?.();
  }, [clip, titleDraft, bumpClipsRevision, onMutated, patchLocalClip]);

  const saveNotes = useCallback(async () => {
    if (!clip) {
      return;
    }
    await updateClipNotes(clip.id, notesDraft);
    patchLocalClip({ notes: notesDraft });
    bumpClipsRevision();
    onMutated?.();
  }, [clip, notesDraft, bumpClipsRevision, onMutated, patchLocalClip]);

  const onStatusChange = async (status: ClipStatus) => {
    if (!clip) {
      return;
    }
    await updateClipStatus(clip.id, status);
    patchLocalClip({ status });
    bumpClipsRevision();
    onMutated?.();
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
    onMutated?.();
    setActiveClipId(null);
  };

  const onTrimComplete = (newId: number) => {
    bumpClipsRevision();
    onMutated?.();
    setActiveClipId(newId);
  };

  if (!clip) {
    return (
      <div className="space-y-4">
        <Button type="button" variant="outline" size="sm" onClick={handleBack}>
          {backLabel}
        </Button>
        {!fetchDone ? (
          <p className="text-sm text-[var(--color-text-muted)]">Loading clip…</p>
        ) : (
          <p className="text-sm text-red-500" role="alert">
            {fetchError ?? "Không tải được clip."}
          </p>
        )}
      </div>
    );
  }

  const displayTitle = clip.title?.trim() || `Clip #${clip.id}`;
  const user = clip.account_username?.trim() ? `@${clip.account_username.replace(/^@/, "")}` : "—";

  return (
    <div className="space-y-4">
      <Button type="button" variant="outline" size="sm" onClick={handleBack}>
        {backLabel}
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

          <div className="space-y-2">
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs font-medium uppercase tracking-wide text-[var(--color-text-muted)]">
                Products
              </span>
              <Button type="button" variant="outline" size="sm" onClick={() => setProductPickerOpen(true)}>
                <Plus className="mr-1 h-3.5 w-3.5" />
                Add product
              </Button>
            </div>
            {clipProducts.length === 0 ? (
              <p className="text-sm text-[var(--color-text-muted)]">No products linked to this clip.</p>
            ) : (
              <div className="flex flex-wrap gap-2">
                {clipProducts.map((p) => (
                  <div
                    key={p.id}
                    className="inline-flex max-w-full items-center gap-1.5 rounded-full border border-[var(--color-border)] bg-background py-1 pl-1 pr-1 text-sm"
                  >
                    <ProductMediaThumb
                      imageUrl={p.image_url}
                      frameClassName="h-6 w-6 rounded-full text-[10px]"
                    />
                    <span className="max-w-[140px] truncate text-[var(--color-text)]">{p.name}</span>
                    <button
                      type="button"
                      className="rounded-full p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
                      aria-label={`Remove ${p.name}`}
                      onClick={() =>
                        void (async () => {
                          await untagClipProduct(clip.id, p.id);
                          void refreshClipProducts();
                        })()
                      }
                    >
                      <X className="h-3.5 w-3.5" />
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          <ProductPicker
            clipId={clip.id}
            open={productPickerOpen}
            onClose={() => setProductPickerOpen(false)}
            onUpdated={() => void refreshClipProducts()}
          />

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
