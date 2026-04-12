import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { getClipById } from "@/lib/api";
import { useClipStore } from "@/stores/clip-store";
import type { Clip } from "@/types";

export function ClipDetail({ clipId }: { clipId: number }) {
  const setActiveClipId = useClipStore((s) => s.setActiveClipId);
  const [clip, setClip] = useState<Clip | null>(null);

  useEffect(() => {
    let cancelled = false;
    void getClipById(clipId)
      .then((row) => {
        if (!cancelled) setClip(row);
      })
      .catch(() => {
        if (!cancelled) setClip(null);
      });
    return () => {
      cancelled = true;
    };
  }, [clipId]);

  const title = clip?.title?.trim() || `Clip #${clipId}`;

  return (
    <div className="space-y-4">
      <Button type="button" variant="outline" size="sm" onClick={() => setActiveClipId(null)}>
        Back
      </Button>
      <div className="space-y-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
        <h2 className="text-lg font-semibold text-[var(--color-text)]">{title}</h2>
        <p className="font-mono text-sm text-[var(--color-text-muted)]">id: {clipId}</p>
        <p className="text-sm text-[var(--color-text-muted)]">Full player in Task 10</p>
      </div>
    </div>
  );
}
