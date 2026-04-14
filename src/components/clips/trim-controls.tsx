import { useCallback, useEffect, useMemo, useState } from "react";
import type { RefObject } from "react";
import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { insertTrimmedClip, trimClip } from "@/lib/api";
import type { Clip } from "@/types";
import type { VideoPlayerHandle } from "@/components/clips/video-player";

function clamp(n: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, n));
}

function formatClock(sec: number): string {
  if (!Number.isFinite(sec) || sec < 0) {
    sec = 0;
  }
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  const frac = sec % 1;
  const fracStr = frac > 0.001 ? `.${frac.toFixed(1).slice(2)}` : "";
  return `${m}:${s.toString().padStart(2, "0")}${fracStr}`;
}

function parseTimeInput(raw: string): number | null {
  const t = raw.trim();
  if (!t) {
    return null;
  }
  const idx = t.indexOf(":");
  if (idx === -1) {
    const n = Number(t);
    return Number.isFinite(n) ? n : null;
  }
  const mm = Number(t.slice(0, idx));
  const secPart = t.slice(idx + 1);
  const s = Number(secPart);
  if (!Number.isFinite(mm) || !Number.isFinite(s)) {
    return null;
  }
  return mm * 60 + s;
}

type TrimControlsProps = {
  clip: Clip;
  playerRef: RefObject<VideoPlayerHandle | null>;
  durationSec: number;
  onTrimComplete: (newClipId: number) => void;
};

export function TrimControls({ clip, playerRef, durationSec, onTrimComplete }: TrimControlsProps) {
  const maxDur = useMemo(() => Math.max(durationSec, 0.001), [durationSec]);
  const [startSec, setStartSec] = useState(0);
  const [endSec, setEndSec] = useState(maxDur);
  const [startText, setStartText] = useState("0:00");
  const [endText, setEndText] = useState(formatClock(maxDur));
  const [trimming, setTrimming] = useState(false);

  useEffect(() => {
    setEndSec(maxDur);
    setStartSec(0);
    setStartText("0:00");
    setEndText(formatClock(maxDur));
  }, [clip.id, maxDur]);

  const syncTextFromSecs = useCallback((s: number, e: number) => {
    setStartText(formatClock(s));
    setEndText(formatClock(e));
  }, []);

  const preview = () => {
    const s = clamp(startSec, 0, maxDur);
    const e = clamp(endSec, 0, maxDur);
    if (!(e > s)) {
      return;
    }
    playerRef.current?.playRange(s, e);
  };

  const runTrim = async () => {
    const s = clamp(startSec, 0, maxDur);
    const e = clamp(endSec, 0, maxDur);
    if (!(e > s)) {
      return;
    }
    setTrimming(true);
    try {
      const out = await trimClip({
        source_path: clip.file_path,
        start_sec: s,
        end_sec: e,
        account_id: clip.account_id,
        recording_id: clip.recording_id,
      });
      const newId = await insertTrimmedClip({
        recording_id: clip.recording_id,
        account_id: clip.account_id,
        file_path: out.file_path,
        thumbnail_path: out.thumbnail_path,
        duration_sec: out.duration_sec,
        start_sec: s,
        end_sec: e,
      });
      onTrimComplete(newId);
    } finally {
      setTrimming(false);
    }
  };

  return (
    <div className="space-y-3 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
      <h3 className="text-sm font-medium text-[var(--color-text)]">Trim</h3>
      <div className="space-y-2">
        <label className="flex flex-col gap-1 text-xs text-[var(--color-text-muted)]">
          Start
          <input
            type="range"
            min={0}
            max={maxDur}
            step={0.1}
            value={clamp(startSec, 0, maxDur)}
            onChange={(ev) => {
              const v = Number(ev.target.value);
              const next = clamp(v, 0, endSec - 0.05);
              setStartSec(next);
              syncTextFromSecs(next, endSec);
            }}
            className="w-full accent-primary"
          />
        </label>
        <label className="flex flex-col gap-1 text-xs text-[var(--color-text-muted)]">
          End
          <input
            type="range"
            min={0}
            max={maxDur}
            step={0.1}
            value={clamp(endSec, 0, maxDur)}
            onChange={(ev) => {
              const v = Number(ev.target.value);
              const next = clamp(v, startSec + 0.05, maxDur);
              setEndSec(next);
              syncTextFromSecs(startSec, next);
            }}
            className="w-full accent-primary"
          />
        </label>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <label className="flex flex-col gap-1 text-xs text-[var(--color-text-muted)]">
          Start (MM:SS)
          <input
            value={startText}
            onChange={(e) => setStartText(e.target.value)}
            onBlur={() => {
              const p = parseTimeInput(startText);
              if (p == null) {
                syncTextFromSecs(startSec, endSec);
                return;
              }
              const next = clamp(p, 0, endSec - 0.05);
              setStartSec(next);
              syncTextFromSecs(next, endSec);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                (e.target as HTMLInputElement).blur();
              }
            }}
            className="rounded-md border border-[var(--color-border)] bg-background px-2 py-1 font-mono text-sm text-[var(--color-text)]"
          />
        </label>
        <label className="flex flex-col gap-1 text-xs text-[var(--color-text-muted)]">
          End (MM:SS)
          <input
            value={endText}
            onChange={(e) => setEndText(e.target.value)}
            onBlur={() => {
              const p = parseTimeInput(endText);
              if (p == null) {
                syncTextFromSecs(startSec, endSec);
                return;
              }
              const next = clamp(p, startSec + 0.05, maxDur);
              setEndSec(next);
              syncTextFromSecs(startSec, next);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                (e.target as HTMLInputElement).blur();
              }
            }}
            className="rounded-md border border-[var(--color-border)] bg-background px-2 py-1 font-mono text-sm text-[var(--color-text)]"
          />
        </label>
      </div>
      <div className="flex flex-wrap gap-2">
        <Button type="button" variant="secondary" size="sm" onClick={preview}>
          Preview
        </Button>
        <Button type="button" size="sm" disabled={trimming || !(endSec > startSec)} onClick={() => void runTrim()}>
          {trimming ? (
            <>
              <Loader2 className="mr-1 h-4 w-4 animate-spin" />
              Trimming…
            </>
          ) : (
            "Create trimmed clip"
          )}
        </Button>
      </div>
    </div>
  );
}
