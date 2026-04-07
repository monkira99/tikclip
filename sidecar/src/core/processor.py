"""Post-recording scene detection, clip extraction, and thumbnails (ffmpeg)."""

from __future__ import annotations

import asyncio
import logging
import re
from dataclasses import dataclass, field
from pathlib import Path

from config import settings
from core.time_hcm import today_ymd_hcm
from ws.manager import ws_manager

logger = logging.getLogger(__name__)

_CLIP_MP4 = re.compile(r"^clip_(\d{3})\.mp4$", re.IGNORECASE)
_CLIP_JPG = re.compile(r"^clip_(\d{3})\.jpg$", re.IGNORECASE)


def next_clip_file_index(out_dir: Path) -> int:
    """Next unused 1-based clip_NNN index for this folder (no overwrite across runs)."""
    max_n = 0
    if not out_dir.is_dir():
        return 1
    for p in out_dir.iterdir():
        if not p.is_file():
            continue
        for pat in (_CLIP_MP4, _CLIP_JPG):
            m = pat.match(p.name)
            if m:
                max_n = max(max_n, int(m.group(1)))
                break
    return max_n + 1


@dataclass
class ClipInfo:
    index: int
    path: Path
    thumbnail_path: Path
    start_sec: float
    end_sec: float
    duration_sec: float


@dataclass
class VideoProcessor:
    recording_id: str
    username: str
    source_path: Path
    account_id: int
    clip_min_duration: int
    clip_max_duration: int
    scene_threshold: float
    date_str: str = field(default_factory=today_ymd_hcm)
    status: str = "pending"
    progress_percent: float = 0.0
    clips: list[ClipInfo] = field(default_factory=list)
    error_message: str | None = None

    @staticmethod
    def build_clip_path(storage_root: Path, username: str, date_str: str, index: int) -> Path:
        return storage_root / "clips" / username / date_str / f"clip_{index:03d}.mp4"

    async def process(self) -> None:
        self.status = "processing"
        self.progress_percent = 0.0
        self.clips.clear()
        self.error_message = None

        if not self.source_path.is_file():
            self.status = "error"
            self.error_message = f"Source file not found: {self.source_path}"
            await self._broadcast_progress(0.0, 0, 0)
            return

        try:
            total_duration = await asyncio.to_thread(_probe_duration_seconds, self.source_path)
            scenes = await self._detect_scenes()
            segments = [(s, e) for s, e in scenes]
            grouped = self._group_scenes(segments, total_duration)
            n = len(grouped)
            if n == 0:
                self.status = "completed"
                self.progress_percent = 100.0
                await self._broadcast_progress(100.0, 0, 0)
                return

            out_dir = settings.storage_path / "clips" / self.username / self.date_str
            out_dir.mkdir(parents=True, exist_ok=True)
            start_idx = next_clip_file_index(out_dir)

            for i, (start_sec, end_sec) in enumerate(grouped):
                duration = max(0.0, end_sec - start_sec)
                file_index = start_idx + i
                clip_path = self.build_clip_path(
                    settings.storage_path, self.username, self.date_str, file_index
                )
                thumb_path = clip_path.with_suffix(".jpg")

                await asyncio.to_thread(
                    _extract_clip_sync,
                    self.source_path,
                    clip_path,
                    start_sec,
                    duration,
                )
                await asyncio.to_thread(
                    _extract_thumbnail_sync,
                    clip_path,
                    thumb_path,
                    duration,
                )

                info = ClipInfo(
                    index=file_index,
                    path=clip_path,
                    thumbnail_path=thumb_path,
                    start_sec=start_sec,
                    end_sec=end_sec,
                    duration_sec=duration,
                )
                self.clips.append(info)

                pct = 100.0 * (i + 1) / n
                self.progress_percent = pct
                await self._broadcast_progress(pct, i + 1, n)
                await ws_manager.broadcast(
                    "clip_ready",
                    {
                        "recording_id": self.recording_id,
                        "account_id": self.account_id,
                        "username": self.username,
                        "clip_index": info.index,
                        "path": str(info.path),
                        "thumbnail_path": str(info.thumbnail_path),
                        "start_sec": info.start_sec,
                        "end_sec": info.end_sec,
                        "duration_sec": info.duration_sec,
                    },
                )

            self.status = "completed"
            self.progress_percent = 100.0
        except Exception as e:
            logger.exception("Video processing failed for %s", self.recording_id)
            self.status = "error"
            self.error_message = str(e)
            await self._broadcast_progress(
                self.progress_percent, len(self.clips), max(1, len(self.clips))
            )

    async def _broadcast_progress(
        self, progress_percent: float, current_clip: int, total_clips: int
    ) -> None:
        payload: dict = {
            "recording_id": self.recording_id,
            "account_id": self.account_id,
            "username": self.username,
            "progress_percent": progress_percent,
            "current_clip_index": current_clip,
            "total_clips": total_clips,
            "status": self.status,
        }
        if self.error_message:
            payload["error_message"] = self.error_message
        await ws_manager.broadcast("processing_progress", payload)

    async def _detect_scenes(self) -> list[tuple[float, float]]:
        return await asyncio.to_thread(self._detect_scenes_sync)

    def _detect_scenes_sync(self) -> list[tuple[float, float]]:
        try:
            from scenedetect import ContentDetector, detect
        except ImportError as e:
            raise ImportError(
                "PySceneDetect is required for scene detection. Install the package "
                "with OpenCV support, e.g. `pip install 'scenedetect[opencv]>=0.6'` "
                "(or `opencv-python-headless` plus `scenedetect`)."
            ) from e

        scene_list = detect(
            str(self.source_path),
            ContentDetector(threshold=self.scene_threshold),
            show_progress=False,
            start_in_scene=True,
        )
        return [(start.get_seconds(), end.get_seconds()) for start, end in scene_list]

    def _group_scenes(
        self,
        segments: list[tuple[float, float]],
        total_duration: float,
    ) -> list[tuple[float, float]]:
        if total_duration <= 0:
            return []

        if not segments:
            return self._split_long_segment(0.0, total_duration)

        merged: list[tuple[float, float]] = []
        i = 0
        n = len(segments)
        while i < n:
            start = segments[i][0]
            end = segments[i][1]
            i += 1
            while i < n and end - start < self.clip_min_duration:
                end = segments[i][1]
                i += 1
            while i < n and segments[i][1] - start <= self.clip_max_duration:
                end = segments[i][1]
                i += 1
            merged.extend(self._split_long_segment(start, end))

        return merged

    def _split_long_segment(self, start: float, end: float) -> list[tuple[float, float]]:
        max_d = float(self.clip_max_duration)
        if end <= start:
            return []
        out: list[tuple[float, float]] = []
        t = start
        while end - t > max_d:
            out.append((t, t + max_d))
            t += max_d
        remainder = end - t
        if remainder >= self.clip_min_duration or not out:
            if remainder > 0:
                out.append((t, end))
        elif out:
            ls, _ = out[-1]
            out[-1] = (ls, end)
        else:
            out.append((t, end))
        return out


def _probe_duration_seconds(video_path: Path) -> float:
    import subprocess

    cmd = [
        "ffprobe",
        "-v",
        "error",
        "-show_entries",
        "format=duration",
        "-of",
        "default=noprint_wrappers=1:nokey=1",
        str(video_path),
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False, timeout=120)
    if proc.returncode != 0:
        raise RuntimeError(
            f"ffprobe failed ({proc.returncode}): {(proc.stderr or proc.stdout or '').strip()}"
        )
    line = (proc.stdout or "").strip().splitlines()[-1] if proc.stdout else ""
    try:
        return float(line)
    except ValueError as e:
        raise RuntimeError(f"Could not parse duration from ffprobe output: {line!r}") from e


def _extract_clip_sync(src: Path, dest: Path, start_sec: float, duration_sec: float) -> None:
    import subprocess

    dest.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        "ffmpeg",
        "-y",
        "-ss",
        str(start_sec),
        "-i",
        str(src),
        "-t",
        str(duration_sec),
        "-c",
        "copy",
        "-avoid_negative_ts",
        "make_zero",
        str(dest),
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False, timeout=3600)
    if proc.returncode != 0:
        err = (proc.stderr or proc.stdout or "").strip()
        raise RuntimeError(f"ffmpeg clip extract failed ({proc.returncode}): {err[:2000]}")


def _extract_thumbnail_sync(video_path: Path, dest_jpg: Path, clip_duration_sec: float) -> None:
    import subprocess

    offset = min(1.0, max(0.0, clip_duration_sec / 2))
    cmd = [
        "ffmpeg",
        "-y",
        "-ss",
        str(offset),
        "-i",
        str(video_path),
        "-vframes",
        "1",
        "-q:v",
        "2",
        str(dest_jpg),
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False, timeout=300)
    if proc.returncode != 0:
        err = (proc.stderr or proc.stdout or "").strip()
        raise RuntimeError(f"ffmpeg thumbnail failed ({proc.returncode}): {err[:2000]}")
