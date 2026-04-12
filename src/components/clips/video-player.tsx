import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
} from "react";
import { Maximize2, Pause, Play, Volume2, VolumeX } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export type VideoPlayerHandle = {
  seek: (t: number) => void;
  playRange: (start: number, end: number) => void;
};

function formatClock(sec: number): string {
  if (!Number.isFinite(sec) || sec < 0) {
    sec = 0;
  }
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

type VideoPlayerProps = {
  src: string | null;
  className?: string;
  onTimeUpdate?: (t: number) => void;
  onDurationChange?: (durationSec: number) => void;
};

export const VideoPlayer = forwardRef<VideoPlayerHandle, VideoPlayerProps>(function VideoPlayer(
  { src, className, onTimeUpdate, onDurationChange },
  ref,
) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const onTimeUpdateRef = useRef(onTimeUpdate);
  const onDurationChangeRef = useRef(onDurationChange);
  const playRangeEndRef = useRef<number | null>(null);

  const [playing, setPlaying] = useState(false);
  const [duration, setDuration] = useState(0);
  const [currentTime, setCurrentTime] = useState(0);
  const [volume, setVolume] = useState(1);
  const [muted, setMuted] = useState(false);
  const [playbackRate, setPlaybackRate] = useState(1);

  useEffect(() => {
    onTimeUpdateRef.current = onTimeUpdate;
  }, [onTimeUpdate]);

  useEffect(() => {
    onDurationChangeRef.current = onDurationChange;
  }, [onDurationChange]);

  useEffect(() => {
    setDuration(0);
    setCurrentTime(0);
    setPlaying(false);
    playRangeEndRef.current = null;
  }, [src]);

  useEffect(() => {
    const v = videoRef.current;
    if (!v) {
      return;
    }
    const onMeta = () => {
      const d = v.duration;
      if (Number.isFinite(d) && d > 0) {
        setDuration(d);
        onDurationChangeRef.current?.(d);
      }
    };
    const onPlay = () => setPlaying(true);
    const onPause = () => setPlaying(false);
    const onTime = () => {
      const t = v.currentTime;
      setCurrentTime(t);
      onTimeUpdateRef.current?.(t);
      const end = playRangeEndRef.current;
      if (end != null && t >= end - 0.02) {
        v.pause();
        playRangeEndRef.current = null;
      }
    };

    v.addEventListener("loadedmetadata", onMeta);
    v.addEventListener("play", onPlay);
    v.addEventListener("pause", onPause);
    v.addEventListener("timeupdate", onTime);
    return () => {
      v.removeEventListener("loadedmetadata", onMeta);
      v.removeEventListener("play", onPlay);
      v.removeEventListener("pause", onPause);
      v.removeEventListener("timeupdate", onTime);
    };
  }, [src]);

  useEffect(() => {
    const v = videoRef.current;
    if (!v) {
      return;
    }
    v.volume = volume;
    v.muted = muted;
    v.playbackRate = playbackRate;
  }, [volume, muted, playbackRate]);

  const togglePlay = useCallback(() => {
    const v = videoRef.current;
    if (!v) {
      return;
    }
    if (v.paused) {
      playRangeEndRef.current = null;
      void v.play();
    } else {
      v.pause();
    }
  }, []);

  const seekTo = useCallback((t: number) => {
    const v = videoRef.current;
    if (!v) {
      return;
    }
    playRangeEndRef.current = null;
    const next = Math.max(0, Math.min(t, Number.isFinite(v.duration) ? v.duration : t));
    v.currentTime = next;
    setCurrentTime(next);
  }, []);

  const playRange = useCallback((start: number, end: number) => {
    const v = videoRef.current;
    if (!v) {
      return;
    }
    if (!(end > start)) {
      return;
    }
    playRangeEndRef.current = end;
    const maxT = Number.isFinite(v.duration) ? v.duration : end;
    v.currentTime = Math.max(0, Math.min(start, maxT));
    void v.play();
  }, []);

  useImperativeHandle(
    ref,
    () => ({
      seek: seekTo,
      playRange,
    }),
    [seekTo, playRange],
  );

  const toggleMute = () => {
    const v = videoRef.current;
    if (!v) {
      return;
    }
    if (v.muted || v.volume === 0) {
      setMuted(false);
      setVolume((vol) => (vol > 0 ? vol : 0.5));
    } else {
      setMuted(true);
    }
  };

  const enterFullscreen = () => {
    const v = videoRef.current;
    if (!v) {
      return;
    }
    void v.requestFullscreen?.();
  };

  const safeDuration = Number.isFinite(duration) && duration > 0 ? duration : 0;

  if (!src) {
    return (
      <div
        className={cn(
          "flex aspect-video w-full items-center justify-center rounded-lg border border-[var(--color-border)] bg-black/40 text-sm text-[var(--color-text-muted)]",
          className,
        )}
      >
        No video source
      </div>
    );
  }

  return (
    <div className={cn("flex flex-col gap-2", className)}>
      <video
        ref={videoRef}
        src={src}
        className="aspect-video w-full rounded-lg bg-black object-contain"
        playsInline
        preload="metadata"
        onClick={togglePlay}
      />
      <div className="flex flex-wrap items-center gap-2 rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2 py-2 text-sm text-[var(--color-text)]">
        <Button type="button" variant="outline" size="icon" className="h-8 w-8 shrink-0" onClick={togglePlay}>
          {playing ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}
        </Button>
        <span className="shrink-0 font-mono text-xs tabular-nums text-[var(--color-text-muted)]">
          {formatClock(currentTime)} / {formatClock(safeDuration)}
        </span>
        <input
          type="range"
          min={0}
          max={safeDuration > 0 ? safeDuration : 1}
          step={0.05}
          value={Math.min(currentTime, safeDuration > 0 ? safeDuration : 0)}
          disabled={safeDuration <= 0}
          onChange={(e) => seekTo(Number(e.target.value))}
          className="h-1.5 min-w-[120px] flex-1 cursor-pointer accent-primary disabled:opacity-40"
        />
        <div className="flex items-center gap-1">
          <Button type="button" variant="ghost" size="icon" className="h-8 w-8 shrink-0" onClick={toggleMute}>
            {muted || volume === 0 ? <VolumeX className="h-4 w-4" /> : <Volume2 className="h-4 w-4" />}
          </Button>
          <input
            type="range"
            min={0}
            max={1}
            step={0.05}
            value={muted ? 0 : volume}
            onChange={(e) => {
              const v = Number(e.target.value);
              setVolume(v);
              setMuted(v === 0);
            }}
            className="h-1.5 w-20 cursor-pointer accent-primary"
          />
        </div>
        <select
          value={playbackRate}
          onChange={(e) => setPlaybackRate(Number(e.target.value))}
          className="h-8 rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2 text-xs"
        >
          <option value={0.5}>0.5×</option>
          <option value={1}>1×</option>
          <option value={1.5}>1.5×</option>
          <option value={2}>2×</option>
        </select>
        <Button type="button" variant="outline" size="icon" className="h-8 w-8 shrink-0" onClick={enterFullscreen}>
          <Maximize2 className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
});
