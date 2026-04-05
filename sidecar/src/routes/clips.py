import asyncio
from pathlib import Path

from fastapi import APIRouter, HTTPException

from ..core.processor import VideoProcessor
from ..models.schemas import (
    ClipOutput,
    ProcessVideoAcceptedResponse,
    ProcessVideoRequest,
    ProcessingStatusResponse,
)

router = APIRouter()

_active_processors: dict[str, VideoProcessor] = {}
_active_lock = asyncio.Lock()


def _to_status_response(p: VideoProcessor) -> ProcessingStatusResponse:
    return ProcessingStatusResponse(
        recording_id=p.recording_id,
        account_id=p.account_id,
        username=p.username,
        status=p.status,
        progress_percent=p.progress_percent,
        clips=[
            ClipOutput(
                index=c.index,
                path=str(c.path),
                thumbnail_path=str(c.thumbnail_path),
                start_sec=c.start_sec,
                end_sec=c.end_sec,
                duration_sec=c.duration_sec,
            )
            for c in p.clips
        ],
        error_message=p.error_message,
    )


@router.post("/api/video/process", response_model=ProcessVideoAcceptedResponse)
async def process_video(body: ProcessVideoRequest):
    recording_id = body.recording_id
    path = Path(body.file_path).expanduser()

    async with _active_lock:
        existing = _active_processors.get(recording_id)
        if existing is not None and existing.status in ("pending", "processing"):
            raise HTTPException(
                status_code=409,
                detail="Processing already in progress for this recording_id",
            )
        processor = VideoProcessor(
            recording_id=recording_id,
            username=body.username,
            source_path=path,
            account_id=body.account_id,
            clip_min_duration=body.clip_min_duration,
            clip_max_duration=body.clip_max_duration,
            scene_threshold=body.scene_threshold,
        )
        _active_processors[recording_id] = processor

    async def _run() -> None:
        await processor.process()

    asyncio.create_task(_run())
    return ProcessVideoAcceptedResponse(recording_id=recording_id)


@router.get("/api/processing/status/{recording_id}", response_model=ProcessingStatusResponse)
async def processing_status(recording_id: str):
    async with _active_lock:
        processor = _active_processors.get(recording_id)
    if processor is None:
        raise HTTPException(status_code=404, detail="Unknown recording_id")
    return _to_status_response(processor)
