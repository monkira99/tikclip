"""Match a new clip to a catalog product via frame embeddings + zvec (Gemini image embed)."""

from __future__ import annotations

import logging
from collections import Counter
from pathlib import Path
from uuid import uuid4

import httpx
from pydantic import BaseModel

from config import settings
from embeddings.product_vector import resolve_storage_media_path, search_by_media_path
from embeddings.video_frames import cleanup_work_dir, extract_frames_evenly

logger = logging.getLogger(__name__)


class ClipSuggestProductResult(BaseModel):
    product_id: int | None = None
    product_name: str | None = None
    best_score: float | None = None
    frames_used: int = 0
    skipped_reason: str | None = None


def _enabled() -> tuple[bool, str | None]:
    if not settings.auto_tag_clip_product_enabled:
        return False, "auto_tag_clip_product_enabled is off"
    if not settings.product_vector_enabled:
        return False, "product_vector_enabled is off"
    if not settings.gemini_api_key:
        return False, "Gemini API key is not configured"
    return True, None


async def suggest_product_for_clip(
    *,
    video_path: str,
    thumbnail_path: str | None,
    http: httpx.AsyncClient,
) -> ClipSuggestProductResult:
    ok, reason = _enabled()
    if not ok:
        logger.debug("suggest_product_for_clip skip: %s", reason)
        return ClipSuggestProductResult(skipped_reason=reason)

    try:
        video = resolve_storage_media_path(video_path)
    except (OSError, ValueError) as exc:
        logger.debug("suggest_product_for_clip skip resolve video: %s", exc)
        return ClipSuggestProductResult(skipped_reason=str(exc))

    if not video.is_file():
        logger.debug("suggest_product_for_clip skip: clip video not found %s", video)
        return ClipSuggestProductResult(skipped_reason="clip video file not found")

    n = settings.auto_tag_clip_frame_count
    n = max(1, min(12, n))
    logger.debug(
        "suggest_product_for_clip start video=%s n_frames=%s has_thumb_path=%s",
        str(video)[:120],
        n,
        bool(thumbnail_path and thumbnail_path.strip()),
    )

    frame_paths: list[Path] = []
    work_dir: Path | None = None
    thumb_included = False
    try:
        if thumbnail_path:
            try:
                thumb = resolve_storage_media_path(thumbnail_path)
                if thumb.is_file():
                    frame_paths.append(thumb)
                    thumb_included = True
            except (OSError, ValueError):
                pass

        work_dir = settings.storage_path / "tmp" / "clip_frames" / str(uuid4())
        extracted = extract_frames_evenly(video, n, work_dir)
        frame_paths.extend(extracted)
        if not frame_paths:
            logger.debug("suggest_product_for_clip skip: no frames (thumb+extract empty)")
            return ClipSuggestProductResult(skipped_reason="could not extract any frames")

        logger.debug(
            "suggest_product_for_clip frames total=%s thumb_included=%s extracted=%s",
            len(frame_paths),
            thumb_included,
            len(extracted),
        )

        top1: list[tuple[int, float, str | None]] = []
        for fp in frame_paths:
            try:
                hits = await search_by_media_path(
                    media_path=str(fp),
                    kind="image",
                    top_k=1,
                    http=http,
                )
            except (OSError, ValueError, FileNotFoundError) as exc:
                logger.debug("frame search skip %s: %s", fp, exc)
                continue
            if hits:
                h = hits[0]
                top1.append((h.product_id, h.score, h.product_name))

        if not top1:
            logger.debug(
                "suggest_product_for_clip skip: no hits after searching %s frames",
                len(frame_paths),
            )
            return ClipSuggestProductResult(
                frames_used=len(frame_paths),
                skipped_reason="no vector hits for extracted frames",
            )

        counts = Counter(pid for pid, _, _ in top1)
        winner_pid, win_count = counts.most_common(1)[0]
        half = (len(top1) + 1) // 2
        if win_count < half:
            best = min(top1, key=lambda t: t[1])
            winner_pid, win_score, win_name = best[0], best[1], best[2]
            pick = "min_distance_tiebreak"
        else:
            scores = [s for pid, s, _ in top1 if pid == winner_pid]
            win_score = min(scores)
            win_name = next((nm for pid, _, nm in top1 if pid == winner_pid), None)
            pick = "majority_vote"

        logger.debug(
            "suggest_product_for_clip votes=%s pick=%s winner_pid=%s win_score=%s",
            dict(counts),
            pick,
            winner_pid,
            win_score,
        )

        max_dist = settings.auto_tag_clip_max_score
        if win_score > max_dist:
            logger.debug(
                "suggest_product_for_clip skip: score %s > max_dist %s",
                win_score,
                max_dist,
            )
            return ClipSuggestProductResult(
                frames_used=len(frame_paths),
                skipped_reason=(
                    f"best match distance {win_score:.4f} above threshold {max_dist:.4f}"
                ),
            )

        logger.debug(
            "suggest_product_for_clip match product_id=%s name=%r score=%s",
            winner_pid,
            (win_name[:80] if win_name else None),
            win_score,
        )
        return ClipSuggestProductResult(
            product_id=winner_pid,
            product_name=win_name,
            best_score=win_score,
            frames_used=len(frame_paths),
        )
    finally:
        if work_dir is not None:
            cleanup_work_dir(work_dir)
