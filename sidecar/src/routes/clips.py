import asyncio
import logging
from pathlib import Path

import httpx
from fastapi import APIRouter, HTTPException

from config import settings
from core.processor import VideoProcessor
from embeddings.clip_product_suggest import suggest_product_for_clip
from models.schemas import (
    ClipOutput,
    ClipSuggestProductRequest,
    ClipSuggestProductResponse,
    ProcessingStatusResponse,
    ProcessVideoAcceptedResponse,
    ProcessVideoRequest,
)

logger = logging.getLogger(__name__)
router = APIRouter()

_active_processors: dict[str, VideoProcessor] = {}
_active_lock = asyncio.Lock()


async def try_schedule_video_processing(
    recording_id: str,
    username: str,
    file_path: str,
    account_id: int,
    *,
    clip_min_duration: int | None = None,
    clip_max_duration: int | None = None,
    scene_threshold: float | None = None,
) -> str | None:
    """Enqueue clip processing in the background.

    Returns ``None`` if a task was started. Otherwise an error code:
    ``file_not_found``, ``already_processing``.
    """
    path = Path(file_path).expanduser()
    if not path.is_file():
        return "file_not_found"

    cmin = clip_min_duration if clip_min_duration is not None else settings.clip_min_duration
    cmax = clip_max_duration if clip_max_duration is not None else settings.clip_max_duration
    sthr = scene_threshold if scene_threshold is not None else settings.scene_threshold

    async with _active_lock:
        existing = _active_processors.get(recording_id)
        if existing is not None and existing.status in ("pending", "processing"):
            return "already_processing"
        processor = VideoProcessor(
            recording_id=recording_id,
            username=username,
            source_path=path,
            account_id=account_id,
            clip_min_duration=cmin,
            clip_max_duration=cmax,
            scene_threshold=sthr,
        )
        _active_processors[recording_id] = processor

    async def _run() -> None:
        await processor.process()

    asyncio.create_task(_run())
    return None


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
    err = await try_schedule_video_processing(
        recording_id=body.recording_id,
        username=body.username,
        file_path=body.file_path,
        account_id=body.account_id,
        clip_min_duration=body.clip_min_duration,
        clip_max_duration=body.clip_max_duration,
        scene_threshold=body.scene_threshold,
    )
    if err == "file_not_found":
        raise HTTPException(status_code=400, detail="file not found")
    if err == "already_processing":
        raise HTTPException(
            status_code=409,
            detail="Processing already in progress for this recording_id",
        )
    return ProcessVideoAcceptedResponse(recording_id=body.recording_id)


@router.get("/api/processing/status/{recording_id}", response_model=ProcessingStatusResponse)
async def processing_status(recording_id: str):
    async with _active_lock:
        processor = _active_processors.get(recording_id)
    if processor is None:
        raise HTTPException(status_code=404, detail="Unknown recording_id")
    return _to_status_response(processor)


@router.post(
    "/api/clips/suggest-product",
    response_model=ClipSuggestProductResponse,
)
async def suggest_product_for_clip_route(body: ClipSuggestProductRequest):
    video = body.video_path.strip()
    if not video:
        raise HTTPException(status_code=400, detail="video_path is required")
    thumb_raw = body.thumbnail_path
    thumb_s = thumb_raw.strip() if thumb_raw else ""
    logger.debug(
        "suggest-product start video=%s thumb=%s",
        video[:120],
        (thumb_s[:120] if thumb_s else ""),
    )
    async with httpx.AsyncClient() as client:
        result = await suggest_product_for_clip(
            video_path=video,
            thumbnail_path=(thumb_s if thumb_s else None),
            http=client,
        )
    logger.debug(
        "suggest-product done matched=%s product_id=%s score=%s frames=%s skip=%r",
        result.matched,
        result.product_id,
        result.best_score,
        result.frames_used,
        result.skipped_reason,
    )
    return result
