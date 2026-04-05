"""FFmpeg-based live stream recording worker."""

from __future__ import annotations

import asyncio
import os
import time
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path


@dataclass
class RecordingWorker:
    recording_id: str
    stream_url: str
    output_dir: Path
    username: str
    max_duration_seconds: int
    status: str = "pending"
    duration_seconds: int = 0
    file_size_bytes: int = 0
    file_path: str | None = None
    error_message: str | None = None
    _process: asyncio.subprocess.Process | None = field(default=None, repr=False)
    _monitor_task: asyncio.Task | None = field(default=None, repr=False)
    _stop_requested: bool = field(default=False, repr=False)

    def _output_file_path(self) -> Path:
        now = datetime.now()
        date_part = now.strftime("%Y-%m-%d")
        time_part = now.strftime("%H%M%S")
        out_dir = self.output_dir / self.username / date_part
        out_dir.mkdir(parents=True, exist_ok=True)
        return out_dir / f"{time_part}.flv"

    def _build_ffmpeg_command(self) -> list[str]:
        if not self.file_path:
            raise RuntimeError("file_path must be set before building ffmpeg command")
        return [
            "ffmpeg",
            "-y",
            "-i",
            self.stream_url,
            "-c",
            "copy",
            "-t",
            str(self.max_duration_seconds),
            self.file_path,
        ]

    async def _monitor_loop(self, start_time: float) -> None:
        while self._process is not None and self._process.returncode is None:
            await asyncio.sleep(5)
            if self.file_path and os.path.exists(self.file_path):
                try:
                    self.file_size_bytes = os.path.getsize(self.file_path)
                except OSError:
                    pass
            self.duration_seconds = int(time.monotonic() - start_time)

    async def start(self) -> None:
        self.file_path = str(self._output_file_path())
        cmd = self._build_ffmpeg_command()
        self.status = "recording"
        self._stop_requested = False
        process = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.DEVNULL,
            stderr=asyncio.subprocess.PIPE,
        )
        self._process = process
        start_time = time.monotonic()
        self._monitor_task = asyncio.create_task(self._monitor_loop(start_time))
        return_code = 0
        stderr_data = b""
        try:
            return_code = await process.wait()
        finally:
            if self._monitor_task:
                self._monitor_task.cancel()
                try:
                    await self._monitor_task
                except asyncio.CancelledError:
                    pass
                self._monitor_task = None
            if process.stderr:
                try:
                    stderr_data = await process.stderr.read()
                except Exception:
                    pass
            if self.file_path and os.path.exists(self.file_path):
                try:
                    self.file_size_bytes = os.path.getsize(self.file_path)
                except OSError:
                    pass
            self.duration_seconds = int(time.monotonic() - start_time)
            self._process = None

        if self._stop_requested:
            self.status = "stopped"
        elif return_code != 0:
            self.status = "error"
            text = stderr_data.decode(errors="replace").strip() if stderr_data else ""
            self.error_message = (
                text[:2000] if text else f"ffmpeg exited with code {return_code}"
            )
        else:
            self.status = "completed"

    async def stop(self) -> None:
        proc = self._process
        if proc is None or proc.returncode is not None:
            return
        self._stop_requested = True
        proc.terminate()
        deadline = time.monotonic() + 5.0
        while proc.returncode is None and time.monotonic() < deadline:
            await asyncio.sleep(0.05)
        if proc.returncode is None:
            proc.kill()
        while proc.returncode is None:
            await asyncio.sleep(0.05)

    def to_dict(self) -> dict:
        return {
            "recording_id": self.recording_id,
            "username": self.username,
            "status": self.status,
            "duration_seconds": self.duration_seconds,
            "file_size_bytes": self.file_size_bytes,
            "file_path": self.file_path,
            "error_message": self.error_message,
        }
