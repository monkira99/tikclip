"""Extract still frames from a short clip video (ffmpeg) under app storage."""

from __future__ import annotations

import logging
import shutil
import subprocess
from pathlib import Path

logger = logging.getLogger(__name__)


def probe_duration_seconds(video_path: Path) -> float:
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
        msg = f"ffprobe failed ({proc.returncode}): {(proc.stderr or proc.stdout or '').strip()}"
        raise RuntimeError(msg)
    line = (proc.stdout or "").strip().splitlines()[-1] if proc.stdout else ""
    try:
        return float(line)
    except ValueError as exc:
        msg = f"Could not parse duration: {line!r}"
        raise RuntimeError(msg) from exc


def extract_frames_evenly(video_path: Path, count: int, work_dir: Path) -> list[Path]:
    """Write ``count`` JPEG frames at evenly spaced timestamps into ``work_dir``."""
    work_dir.mkdir(parents=True, exist_ok=True)
    if count < 1:
        return []

    duration = probe_duration_seconds(video_path)
    if duration <= 0:
        return []

    out_paths: list[Path] = []
    for i in range(count):
        t = duration * (i + 1) / (count + 1)
        dest = work_dir / f"frame_{i:02d}.jpg"
        cmd = [
            "ffmpeg",
            "-y",
            "-ss",
            str(t),
            "-i",
            str(video_path),
            "-vframes",
            "1",
            "-q:v",
            "3",
            str(dest),
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True, check=False, timeout=120)
        if proc.returncode != 0:
            logger.warning(
                "ffmpeg frame extract failed at t=%s: %s",
                t,
                (proc.stderr or proc.stdout or "")[:500],
            )
            continue
        if dest.is_file():
            out_paths.append(dest)
    return out_paths


def cleanup_work_dir(work_dir: Path) -> None:
    if work_dir.is_dir():
        shutil.rmtree(work_dir, ignore_errors=True)
