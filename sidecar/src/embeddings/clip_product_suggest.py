"""Match a new clip to a catalog product via frame embeddings + zvec (Gemini image embed)."""

from __future__ import annotations

import logging
from collections import Counter
from pathlib import Path
from uuid import uuid4

import httpx

from config import settings
from embeddings.product_vector import resolve_storage_media_path, search_by_media_path
from embeddings.video_frames import cleanup_work_dir, extract_frames_evenly
from models.schemas import (
    ClipSuggestFrameRow,
    ClipSuggestProductResponse,
    ClipSuggestVoteRow,
)

logger = logging.getLogger(__name__)


def _config_fields() -> dict:
    return {
        "config_target_extracted_frames": max(1, min(12, settings.auto_tag_clip_frame_count)),
        "config_max_score_threshold": float(settings.auto_tag_clip_max_score),
    }


def _storage_relative(path: Path) -> str:
    root = settings.storage_path.resolve()
    try:
        return str(path.resolve().relative_to(root))
    except ValueError:
        return path.name


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
) -> ClipSuggestProductResponse:
    base = _config_fields()
    ok, reason = _enabled()
    if not ok:
        logger.debug("suggest_product_for_clip skip: %s", reason)
        return ClipSuggestProductResponse(skipped_reason=reason, **base)

    try:
        video = resolve_storage_media_path(video_path)
    except (OSError, ValueError) as exc:
        logger.debug("suggest_product_for_clip skip resolve video: %s", exc)
        return ClipSuggestProductResponse(skipped_reason=str(exc), **base)

    video_rel = _storage_relative(video)
    if not video.is_file():
        logger.debug("suggest_product_for_clip skip: clip video not found %s", video)
        return ClipSuggestProductResponse(
            skipped_reason="clip video file not found",
            video_relative_path=video_rel,
            **base,
        )

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
            return ClipSuggestProductResponse(
                skipped_reason="could not extract any frames",
                video_relative_path=video_rel,
                thumbnail_used=thumb_included,
                extracted_frame_count=0,
                **base,
            )

        logger.debug(
            "suggest_product_for_clip frames total=%s thumb_included=%s extracted=%s",
            len(frame_paths),
            thumb_included,
            len(extracted),
        )

        frame_rows: list[ClipSuggestFrameRow] = []
        top1: list[tuple[int, float, str | None]] = []
        frames_searched = 0

        for i, fp in enumerate(frame_paths):
            is_thumb = thumb_included and i == 0
            src: str = "thumbnail" if is_thumb else "extracted"
            rel = _storage_relative(fp)
            try:
                hits = await search_by_media_path(
                    media_path=str(fp),
                    kind="image",
                    top_k=1,
                    http=http,
                )
            except (OSError, ValueError, FileNotFoundError) as exc:
                logger.debug("frame search skip %s: %s", fp, exc)
                frame_rows.append(
                    ClipSuggestFrameRow(
                        index=i,
                        source=src,
                        media_relative_path=rel,
                        outcome="error",
                        error=str(exc),
                    ),
                )
                continue

            frames_searched += 1
            if not hits:
                frame_rows.append(
                    ClipSuggestFrameRow(
                        index=i,
                        source=src,
                        media_relative_path=rel,
                        outcome="no_hit",
                    ),
                )
                continue

            h = hits[0]
            top1.append((h.product_id, h.score, h.product_name))
            frame_rows.append(
                ClipSuggestFrameRow(
                    index=i,
                    source=src,
                    media_relative_path=rel,
                    outcome="hit",
                    top_product_id=h.product_id,
                    top_score=h.score,
                    top_product_name=h.product_name,
                ),
            )

        if not top1:
            logger.debug(
                "suggest_product_for_clip skip: no hits after searching %s frames",
                len(frame_paths),
            )
            return ClipSuggestProductResponse(
                frames_used=len(frame_paths),
                frames_searched=frames_searched,
                skipped_reason="no vector hits for extracted frames",
                video_relative_path=video_rel,
                thumbnail_used=thumb_included,
                extracted_frame_count=len(extracted),
                frame_rows=frame_rows,
                **base,
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

        votes_by_product = [
            ClipSuggestVoteRow(product_id=pid, vote_count=cnt) for pid, cnt in counts.most_common()
        ]

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
            return ClipSuggestProductResponse(
                frames_used=len(frame_paths),
                frames_searched=frames_searched,
                skipped_reason=(
                    f"best match distance {win_score:.4f} above threshold {max_dist:.4f}"
                ),
                video_relative_path=video_rel,
                thumbnail_used=thumb_included,
                extracted_frame_count=len(extracted),
                pick_method=pick,
                votes_by_product=votes_by_product,
                candidate_product_id=winner_pid,
                candidate_product_name=win_name,
                candidate_score=win_score,
                frame_rows=frame_rows,
                **base,
            )

        logger.debug(
            "suggest_product_for_clip match product_id=%s name=%r score=%s",
            winner_pid,
            (win_name[:80] if win_name else None),
            win_score,
        )
        return ClipSuggestProductResponse(
            matched=True,
            product_id=winner_pid,
            product_name=win_name,
            best_score=win_score,
            frames_used=len(frame_paths),
            frames_searched=frames_searched,
            video_relative_path=video_rel,
            thumbnail_used=thumb_included,
            extracted_frame_count=len(extracted),
            pick_method=pick,
            votes_by_product=votes_by_product,
            frame_rows=frame_rows,
            **base,
        )
    finally:
        if work_dir is not None:
            cleanup_work_dir(work_dir)
