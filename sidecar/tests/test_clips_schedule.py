"""Tests for try_schedule_video_processing."""

import asyncio
import tempfile
from pathlib import Path

import pytest

from routes.clips import _active_lock, _active_processors, try_schedule_video_processing


@pytest.mark.asyncio
async def test_try_schedule_missing_file():
    err = await try_schedule_video_processing("r-missing", "u", "/nonexistent/clip.mp4", 1)
    assert err == "file_not_found"


@pytest.mark.asyncio
async def test_try_schedule_starts_background_task(monkeypatch):
    with tempfile.NamedTemporaryFile(suffix=".mp4", delete=False) as f:
        path = f.name

    done = asyncio.Event()

    async def fake_process(self):
        done.set()

    monkeypatch.setattr("core.processor.VideoProcessor.process", fake_process)

    try:
        err = await try_schedule_video_processing("r-ok", "user", path, 42)
        assert err is None
        await asyncio.wait_for(done.wait(), timeout=5.0)
    finally:
        Path(path).unlink(missing_ok=True)
        async with _active_lock:
            _active_processors.pop("r-ok", None)
