"""Clip processing, caption generation, and related HTTP endpoints.

Handlers enqueue work and return deterministic responses; they do not own
cross-node flow orchestration (handled by the desktop engine + DB sync).
"""

import asyncio
import logging
from pathlib import Path

import httpx
from fastapi import APIRouter, HTTPException

from config import settings
from core.captioner import generate_caption
from core.model_manager import ModelManager
from core.processor import VideoProcessor
from embeddings.clip_product_suggest import suggest_product_for_clip
from models.schemas import (
    ClipOutput,
    ClipSuggestProductRequest,
    ClipSuggestProductResponse,
    GenerateCaptionRequest,
    GenerateCaptionResponse,
    ModelStatusResponse,
    ProcessingStatusResponse,
    ProcessVideoAcceptedResponse,
    ProcessVideoRequest,
    SpeechSegmentOutput,
)
from ws.manager import ws_manager

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
                transcript_text=c.transcript_text,
            )
            for c in p.clips
        ],
        error_message=p.error_message,
        speech_segments=[
            SpeechSegmentOutput(
                start_sec=s.start_sec,
                end_sec=s.end_sec,
                text=s.text,
                confidence=s.confidence,
            )
            for s in p.speech_segments
        ],
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


@router.get(
    "/api/speech-segments/{recording_id}",
    response_model=list[SpeechSegmentOutput],
)
async def list_speech_segments_http(recording_id: str):
    """Speech segments for a recording while the processor is still in memory."""
    async with _active_lock:
        processor = _active_processors.get(recording_id)
    if processor is None:
        raise HTTPException(status_code=404, detail="Unknown recording_id")
    return [
        SpeechSegmentOutput(
            start_sec=s.start_sec,
            end_sec=s.end_sec,
            text=s.text,
            confidence=s.confidence,
        )
        for s in processor.speech_segments
    ]


@router.get("/api/models/status", response_model=ModelStatusResponse)
async def models_status():
    raw = ModelManager.get().status()
    return ModelStatusResponse(
        vad_ready=bool(raw.get("vad_ready")),
        stt_ready=bool(raw.get("stt_ready")),
        stt_quantize=str(raw.get("stt_quantize", "unknown")),
        vad_model_path=raw.get("vad_model_path"),
        stt_model_dir=raw.get("stt_model_dir"),
        stt_loaded=bool(raw.get("stt_loaded")),
    )


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
    tr_raw = body.transcript_text
    tr_s = tr_raw.strip() if tr_raw else ""
    logger.debug(
        "suggest-product start video=%s thumb=%s transcript_len=%s",
        video[:120],
        (thumb_s[:120] if thumb_s else ""),
        len(tr_s),
    )
    async with httpx.AsyncClient() as client:
        result = await suggest_product_for_clip(
            video_path=video,
            thumbnail_path=(thumb_s if thumb_s else None),
            transcript_text=(tr_s if tr_s else None),
            http=client,
        )
    logger.debug(
        "suggest-product done matched=%s product_id=%s score=%s frames=%s text_used=%s skip=%r",
        result.matched,
        result.product_id,
        result.best_score,
        result.frames_used,
        result.text_search_used,
        result.skipped_reason,
    )
    return result


@router.post(
    "/api/captions/generate",
    response_model=GenerateCaptionResponse,
)
async def generate_caption_route(body: GenerateCaptionRequest):
    username = body.username.strip()
    if not username:
        raise HTTPException(status_code=400, detail="username is required")
    transcript = (body.transcript_text or "").strip()
    title = (body.clip_title or "").strip()
    caption_text = generate_caption(
        username=username,
        transcript_text=transcript,
        clip_title=title,
    )
    payload = {
        "clip_id": body.clip_id,
        "caption_text": caption_text,
    }
    await ws_manager.broadcast("caption_ready", payload)
    return GenerateCaptionResponse(clip_id=body.clip_id, caption_text=caption_text)
