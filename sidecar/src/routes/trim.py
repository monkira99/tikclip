import asyncio
import re
from pathlib import Path

from fastapi import APIRouter, HTTPException

from config import settings
from core.time_hcm import today_ymd_hcm
from models.schemas import TrimClipRequest, TrimClipResponse

router = APIRouter()

_CLIP_FILE = re.compile(r"^clip_(\d{3})(?:_trimmed)?\.mp4$", re.IGNORECASE)
_CLIP_THUMB = re.compile(r"^clip_(\d{3})(?:_trimmed)?\.jpg$", re.IGNORECASE)


def _next_trimmed_index(out_dir: Path) -> int:
    max_n = 0
    if not out_dir.is_dir():
        return 1
    for p in out_dir.iterdir():
        if not p.is_file():
            continue
        for pat in (_CLIP_FILE, _CLIP_THUMB):
            m = pat.match(p.name)
            if m:
                max_n = max(max_n, int(m.group(1)))
                break
    return max_n + 1


def _trim_sync(src: Path, dest: Path, start_sec: float, duration_sec: float) -> None:
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
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False, timeout=600)
    if proc.returncode != 0:
        err = (proc.stderr or proc.stdout or "").strip()
        raise RuntimeError(f"ffmpeg trim failed ({proc.returncode}): {err[:2000]}")


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


@router.post("/api/clips/trim", response_model=TrimClipResponse)
async def trim_clip(body: TrimClipRequest):
    src = Path(body.source_path).expanduser()
    if not src.is_file():
        raise HTTPException(status_code=400, detail="Source file not found")
    if body.end_sec <= body.start_sec:
        raise HTTPException(status_code=400, detail="end_sec must be greater than start_sec")

    duration = body.end_sec - body.start_sec

    parts = src.parts
    username = "unknown"
    for i, part in enumerate(parts):
        if part == "clips" and i + 1 < len(parts):
            username = parts[i + 1]
            break

    date_str = today_ymd_hcm()
    out_dir = settings.storage_path / "clips" / username / date_str
    out_dir.mkdir(parents=True, exist_ok=True)

    idx = _next_trimmed_index(out_dir)
    clip_path = out_dir / f"clip_{idx:03d}_trimmed.mp4"
    thumb_path = out_dir / f"clip_{idx:03d}_trimmed.jpg"

    await asyncio.to_thread(_trim_sync, src, clip_path, body.start_sec, duration)
    await asyncio.to_thread(_extract_thumbnail_sync, clip_path, thumb_path, duration)

    return TrimClipResponse(
        file_path=str(clip_path),
        thumbnail_path=str(thumb_path),
        duration_sec=duration,
    )
